import type { CacheScope } from '@api/CacheScope';
import type { CacheSizes } from '@api/CacheSizes';
import type { DownloadStatus } from '@api/DownloadStatus';
import type { EventKind } from '@api/EventKind';
import type { Playlist } from '@api/Playlist';
import type { PlaylistDetail } from '@api/PlaylistDetail';
import type { ProviderId } from '@api/ProviderId';
import type { ProviderInfo } from '@api/ProviderInfo';
import type { Queue } from '@api/Queue';
import type { RepeatMode } from '@api/RepeatMode';
import type { RssFeed } from '@api/RssFeed';
import type { SearchResults } from '@api/SearchResults';
import type { Status } from '@api/Status';
import type { Track } from '@api/Track';
import { ApiError, createApi, describeError, durationSeconds, formatBytes, type YtermusicdApi } from './api';
import { EventStream, type ConnectionState, type EventSourceLike } from './sse';

export type View =
  | { kind: 'library' }
  | { kind: 'playlist'; id: string; name?: string }
  | { kind: 'queue' };

export type ToastSeverity = 'info' | 'success' | 'warning' | 'error';

export type PlayerLayout = 'compact' | 'terminal';

export type SearchMode = 'normal' | 'add-to-playlist';

/// Which pane the keyboard drives. `main` is the library/queue view,
/// `sidebar` is the playlist list in the sidebar. Tab toggles it, and
/// Esc returns focus to the main view.
export type FocusPane = 'sidebar' | 'main';

export const PLAYER_LAYOUT_KEY = 'ytermusicd.player-layout';
export const PROVIDER_KEY = 'ytermusicd.provider';

export function readPlayerLayout(): PlayerLayout {
  try {
    return globalThis.localStorage?.getItem(PLAYER_LAYOUT_KEY) === 'terminal'
      ? 'terminal'
      : 'compact';
  } catch {
    return 'compact';
  }
}

/// Read the stored provider. An unknown or missing value means the
/// default provider.
export function readProvider(providers: ProviderId[] = ['youtube']): ProviderId {
  try {
    const stored = globalThis.localStorage?.getItem(PROVIDER_KEY);
    if (stored && (providers as string[]).includes(stored)) return stored as ProviderId;
  } catch {
    // Ignore storage failures.
  }
  return 'youtube';
}

export interface Toast {
  id: number;
  severity: ToastSeverity;
  message: string;
  action?: { label: string; run: () => void };
}

export interface StoreOptions {
  createEventSource?: (url: string) => EventSourceLike;
  toastTtlMs?: number;
  searchDebounceMs?: number;
  setTimeoutFn?: typeof setTimeout;
  clearTimeoutFn?: typeof clearTimeout;
  setIntervalFn?: typeof setInterval;
  clearIntervalFn?: typeof clearInterval;
}

const DEFAULT_TOAST_TTL = 6000;
const DEFAULT_SEARCH_DEBOUNCE = 250;

/// How often the playback watchdog checks for stalled events.
const PLAYBACK_WATCH_INTERVAL_MS = 1000;
/// Silence after which a playing track re-reads its position from the daemon.
const PLAYBACK_STALE_REFRESH_MS = 2000;
/// Silence before the first rebuild of a stalled event stream.
const PLAYBACK_STALE_RETRY_BASE_MS = 5000;
/// Longest silence before a rebuild. The threshold doubles after each
/// silent rebuild, up to this cap.
const PLAYBACK_STALE_RETRY_MAX_MS = 60000;

export function reorderQueue(queue: Queue, from: number, to: number): Queue {
  const items = [...queue.items];
  if (from === to || from < 0 || to < 0 || from >= items.length || to >= items.length) {
    return queue;
  }
  const [moved] = items.splice(from, 1);
  items.splice(to, 0, moved);
  let current = queue.current;
  if (current !== null) {
    if (current === from) current = to;
    else if (from < current && current <= to) current -= 1;
    else if (to <= current && current < from) current += 1;
  }
  return { ...queue, items, current };
}

export function removeQueueItem(queue: Queue, index: number): Queue {
  const items = [...queue.items];
  if (index < 0 || index >= items.length) return queue;
  items.splice(index, 1);
  let current = queue.current;
  if (current !== null) {
    if (current === index) {
      current = items.length === 0 ? null : Math.min(index, items.length - 1);
    } else if (current > index) {
      current -= 1;
    }
  }
  return { ...queue, items, current };
}

export class AppStore {
  readonly api: YtermusicdApi;

  status = $state<Status | null>(null);
  queue = $state<Queue | null>(null);
  providers = $state<ProviderInfo[]>([]);
  activeProvider = $state<ProviderId>('youtube');
  cache = $state<CacheSizes | null>(null);
  cacheLoading = $state(false);
  playlists = $state<Playlist[]>([]);
  rssFeeds = $state<RssFeed[]>([]);
  playlist = $state<PlaylistDetail | null>(null);
  playlistsLoading = $state(false);
  playlistLoading = $state(false);
  error = $state<string | null>(null);
  connection = $state<ConnectionState>('connecting');
  view = $state<View>({ kind: 'library' });
  selection = $state(0);
  filter = $state('');
  searchOpen = $state(false);
  searchQuery = $state('');
  searchResults = $state<SearchResults | null>(null);
  searchLoading = $state(false);
  searchMode = $state<SearchMode>('normal');
  searchPlaylistId = $state<string | null>(null);
  helpOpen = $state(false);
  focus = $state<FocusPane>('main');
  sidebarSelection = $state(0);
  confirmClearQueue = $state(false);
  createPlaylistOpen = $state(false);
  createPlaylistName = $state('');
  renamePlaylistOpen = $state(false);
  renamePlaylistId = $state<string | null>(null);
  renamePlaylistName = $state('');
  pickerOpen = $state(false);
  pickerTrack = $state<Track | null>(null);
  pickerSelection = $state(0);
  toasts = $state<Toast[]>([]);
  volume = $state(50);
  playerLayout = $state<PlayerLayout>('compact');
  fullPlayerOpen = $state(false);

  private readonly events: EventStream;
  private readonly options: Required<Pick<StoreOptions, 'toastTtlMs' | 'searchDebounceMs'>> &
    StoreOptions;
  private readonly liveToasts = new Set<ReturnType<typeof setTimeout>>();
  private toastSeq = 0;
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  private statusRefreshTimer: ReturnType<typeof setTimeout> | null = null;
  private playbackWatchTimer: ReturnType<typeof setInterval> | null = null;
  private lastPlaybackEventMs: number | null = null;
  private playbackStaleRefreshed = false;
  private playbackStaleRetried = false;
  private playbackStaleRetryCount = 0;

  constructor(api: YtermusicdApi = createApi(), options: StoreOptions = {}) {
    this.api = api;
    this.options = {
      toastTtlMs: DEFAULT_TOAST_TTL,
      searchDebounceMs: DEFAULT_SEARCH_DEBOUNCE,
      ...options,
    };
    this.playerLayout = readPlayerLayout();
    this.activeProvider = readProvider();
    this.events = new EventStream({
      create: options.createEventSource,
      onEvent: (event) => this.handleEvent(event),
      onState: (state) => {
        this.connection = state;
        if (state === 'online' && this.error === 'Daemon is unreachable. Is ytermusicd running?') {
          this.error = null;
        }
      },
      // After a reconnect (suspend resume, watchdog, or manual retry),
      // re-read status, providers, and queue. The position matches the
      // daemon, and the auth banner clears after a daemon restart
      // with a fresh cookie. A new connection starts a new watch
      // period: the old silence no longer counts against it.
      onReconnect: () => {
        this.resetPlaybackWatch();
        void this.refreshStatus();
        void this.loadProviders();
        void this.refreshQueue();
      },
      onParseError: () => this.notify('warning', 'Received a malformed event from the daemon.'),
    });
  }

  async init(): Promise<void> {
    this.parseLocation();
    this.events.connect();
    await Promise.allSettled([
      this.loadProviders(),
      this.refreshStatus(),
      this.loadPlaylists(),
      this.loadCache(),
    ]);
    if (this.view.kind === 'playlist') {
      await this.openPlaylist(this.view.id, this.view.name);
    }
  }

  dispose(): void {
    this.events.close();
    this.clearSearchTimer();
    this.clearStatusRefreshTimer();
    this.clearPlaybackWatchTimer();
    for (const id of this.liveToasts) {
      const clear = this.options.clearTimeoutFn ?? clearTimeout;
      clear(id);
    }
    this.liveToasts.clear();
  }

  get currentTrack(): Track | null {
    return this.status?.playback.current ?? null;
  }

  get activeProviderInfo(): ProviderInfo | null {
    return this.providers.find((provider) => provider.id === this.activeProvider) ?? null;
  }

  get activeProviderName(): string {
    return this.activeProviderInfo?.name ?? 'YouTube Music';
  }

  get canCreatePlaylist(): boolean {
    return this.activeProviderInfo?.capabilities.playlist_create ?? true;
  }

  get isRssProvider(): boolean {
    return this.activeProvider === 'rss';
  }

  get canEditPlaylist(): boolean {
    return this.activeProviderInfo?.capabilities.playlist_edit ?? true;
  }

  get canSearch(): boolean {
    return this.activeProviderInfo?.capabilities.search ?? true;
  }

  get filteredPlaylists(): Playlist[] {
    const filter = this.filter.trim().toLowerCase();
    if (!filter) return this.playlists;
    return this.playlists.filter(
      (playlist) =>
        playlist.name.toLowerCase().includes(filter) ||
        playlist.subtitle.toLowerCase().includes(filter),
    );
  }

  get viewTracks(): Track[] {
    if (this.view.kind === 'playlist') return this.playlist?.tracks ?? [];
    if (this.view.kind === 'queue') return this.queue?.items.map((item) => item.track) ?? [];
    return [];
  }

  get selectedTrack(): Track | null {
    return this.viewTracks[this.selection] ?? null;
  }

  /// The playlist the sidebar selection points at, clamped to the
  /// filtered list so it stays valid when the filter changes.
  get sidebarSelectedPlaylist(): Playlist | null {
    const playlists = this.filteredPlaylists;
    if (playlists.length === 0) return null;
    return playlists[Math.max(0, Math.min(this.sidebarSelection, playlists.length - 1))];
  }

  get searchTargetName(): string | null {
    if (!this.searchPlaylistId) return null;
    return (
      this.playlists.find((playlist) => playlist.id === this.searchPlaylistId)?.name ??
      (this.playlist?.playlist.id === this.searchPlaylistId
        ? this.playlist.playlist.name
        : null)
    );
  }

  get selectionCount(): number {
    return this.viewTracks.length;
  }

  async refreshStatus(): Promise<void> {
    try {
      this.applyStatus(await this.api.status());
    } catch (error) {
      this.error = describeError(error, this.activeProviderName);
    }
  }

  async loadProviders(): Promise<void> {
    try {
      this.providers = await this.api.providers();
      const known = this.providers.map((provider) => provider.id);
      let stored: string | null = null;
      try {
        stored = globalThis.localStorage?.getItem(PROVIDER_KEY) ?? null;
      } catch {
        // Ignore storage failures.
      }
      if (stored && (known as string[]).includes(stored)) {
        this.activeProvider = stored as ProviderId;
      } else {
        this.activeProvider =
          this.providers.find((provider) => provider.default)?.id ?? 'youtube';
      }
    } catch (error) {
      this.error = describeError(error, this.activeProviderName);
    }
  }

  /// Switch the active provider and reload the library for it.
  async setProvider(provider: ProviderId): Promise<void> {
    if (provider === this.activeProvider) return;
    this.activeProvider = provider;
    try {
      globalThis.localStorage?.setItem(PROVIDER_KEY, provider);
    } catch {
      // Ignore storage failures.
    }
    this.searchResults = null;
    this.searchQuery = '';
    await this.loadPlaylists();
    if (this.view.kind === 'playlist') {
      await this.openPlaylist(this.view.id, this.view.name);
    }
  }

  async refreshQueue(): Promise<void> {
    try {
      this.queue = await this.api.queue();
    } catch (error) {
      this.report(error);
    }
  }

  async loadPlaylists(): Promise<void> {
    this.playlistsLoading = true;
    try {
      this.playlists = await this.api.playlists(this.activeProvider);
      if (this.activeProvider === 'rss') {
        this.rssFeeds = await this.api.rssFeeds();
      }
    } catch (error) {
      this.setLoadError(error);
      this.handleAuthError(error);
    } finally {
      this.playlistsLoading = false;
    }
  }

  async loadRssFeeds(): Promise<void> {
    try {
      this.rssFeeds = await this.api.rssFeeds();
    } catch (error) {
      this.report(error);
    }
  }

  async addRssFeed(url: string): Promise<boolean> {
    try {
      const feed = await this.api.addRssFeed(url);
      this.rssFeeds = [...this.rssFeeds, feed];
      await this.loadPlaylists();
      this.notify('success', `Added “${feed.title}”.`);
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async removeRssFeed(id: string): Promise<boolean> {
    try {
      await this.api.removeRssFeed(id);
      this.rssFeeds = this.rssFeeds.filter((feed) => feed.id !== id);
      if (this.view.kind === 'playlist' && this.view.id === id) {
        this.openLibrary();
      }
      await this.loadPlaylists();
      this.notify('success', 'Feed removed.');
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async refreshRssFeed(id: string): Promise<boolean> {
    try {
      const feed = await this.api.refreshRssFeed(id);
      this.rssFeeds = this.rssFeeds.map((known) => (known.id === id ? feed : known));
      await this.loadPlaylists();
      this.notify('success', `Refreshed “${feed.title}”.`);
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async refreshRssFeeds(): Promise<boolean> {
    try {
      this.rssFeeds = await this.api.refreshRssFeeds();
      await this.loadPlaylists();
      this.notify('success', 'Feeds refreshed.');
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async loadCache(): Promise<void> {
    this.cacheLoading = true;
    try {
      this.cache = await this.api.cache();
    } catch (error) {
      this.error = describeError(error, this.activeProviderName);
    } finally {
      this.cacheLoading = false;
    }
  }

  async cleanCache(scope: CacheScope): Promise<void> {
    try {
      const result = await this.api.cleanCache(scope);
      this.notify('success', `Cache cleaned: freed ${formatBytes(result.freed_bytes)}.`);
      await Promise.all([this.loadCache(), this.refreshStatus(), this.refreshQueue()]);
    } catch (error) {
      this.error = describeError(error, this.activeProviderName);
    }
  }

  openPlaylist(id: string, name?: string): Promise<void> {
    this.view = { kind: 'playlist', id, name };
    this.selection = 0;
    this.playlist = null;
    this.playlistLoading = true;
    this.syncLocation();
    return this.loadPlaylist(id);
  }

  private async loadPlaylist(id: string): Promise<void> {
    try {
      this.playlist = await this.api.playlist(id, this.activeProvider);
    } catch (error) {
      this.setLoadError(error);
      this.handleAuthError(error);
    } finally {
      this.playlistLoading = false;
    }
  }

  async createPlaylist(name: string): Promise<boolean> {
    try {
      const playlist = await this.api.createPlaylist({
        title: name,
        description: '',
        privacy: 'private',
      });
      await this.loadPlaylists();
      this.notify('success', `Created “${playlist.name}”.`);
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async renamePlaylist(id: string, name: string): Promise<boolean> {
    try {
      const current = this.playlists.find((playlist) => playlist.id === id);
      const updated = await this.api.updatePlaylist(id, { title: name });
      const subtitle = current?.subtitle ?? this.playlist?.playlist.subtitle ?? '';
      this.playlists = this.playlists.map((playlist) =>
        playlist.id === id ? { ...playlist, name: updated.name } : playlist,
      );
      if (this.playlist?.playlist.id === id) {
        this.playlist = {
          ...this.playlist,
          playlist: { ...this.playlist.playlist, name: updated.name, subtitle },
        };
      }
      if (this.view.kind === 'playlist' && this.view.id === id) {
        this.view = { ...this.view, name: updated.name };
      }
      this.notify('success', `Renamed to “${updated.name}”.`);
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async deletePlaylist(id: string): Promise<boolean> {
    try {
      await this.api.deletePlaylist(id);
      this.playlists = this.playlists.filter((playlist) => playlist.id !== id);
      if (this.view.kind === 'playlist' && this.view.id === id) {
        this.openLibrary();
      }
      this.notify('success', 'Playlist deleted.');
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async addPlaylistTracks(id: string, provider: ProviderId, trackIds: string[]): Promise<boolean> {
    try {
      const tracks = await this.api.addPlaylistTracks(id, provider, trackIds);
      if (this.playlist?.playlist.id === id) {
        this.playlist = { ...this.playlist, tracks };
      }
      this.notify(
        'success',
        trackIds.length === 1
          ? 'Track added to the playlist.'
          : `${trackIds.length} tracks added to the playlist.`,
      );
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async removePlaylistTrack(id: string, index: number): Promise<boolean> {
    try {
      const tracks = await this.api.removePlaylistTracks(id, [index]);
      if (this.playlist?.playlist.id === id) {
        this.playlist = { ...this.playlist, tracks };
        this.selection = Math.max(0, Math.min(this.selection, tracks.length - 1));
      }
      this.notify('success', 'Track removed from the playlist.');
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  async movePlaylistTrack(id: string, from: number, to: number): Promise<boolean> {
    try {
      const tracks = await this.api.movePlaylistTrack(id, from, to);
      if (this.playlist?.playlist.id === id) {
        this.playlist = { ...this.playlist, tracks };
        this.selection = to;
      }
      return true;
    } catch (error) {
      this.report(error);
      return false;
    }
  }

  openLibrary(): void {
    this.view = { kind: 'library' };
    this.selection = 0;
    this.playlist = null;
    this.syncLocation();
  }

  openQueue(): void {
    this.view = { kind: 'queue' };
    this.selection = 0;
    this.syncLocation();
  }

  setFilter(filter: string): void {
    this.filter = filter;
  }

  select(index: number): void {
    if (this.focus === 'sidebar') {
      this.selectSidebar(index);
      return;
    }
    const count = this.selectionCount;
    if (count === 0) {
      this.selection = 0;
      return;
    }
    this.selection = Math.max(0, Math.min(index, count - 1));
  }

  selectSidebar(index: number): void {
    const count = this.filteredPlaylists.length;
    if (count === 0) {
      this.sidebarSelection = 0;
      return;
    }
    this.sidebarSelection = Math.max(0, Math.min(index, count - 1));
  }

  toggleFocus(): void {
    this.focus = this.focus === 'sidebar' ? 'main' : 'sidebar';
  }

  moveSelection(delta: number): void {
    if (this.focus === 'sidebar') {
      this.selectSidebar(this.sidebarSelection + delta);
      return;
    }
    this.select(this.selection + delta);
  }

  selectFirst(): void {
    if (this.focus === 'sidebar') {
      this.selectSidebar(0);
      return;
    }
    this.select(0);
  }

  selectLast(): void {
    if (this.focus === 'sidebar') {
      this.selectSidebar(this.filteredPlaylists.length - 1);
      return;
    }
    this.select(this.selectionCount - 1);
  }

  halfPage(direction: 1 | -1, rows = 8): void {
    this.moveSelection(direction * rows);
  }

  async playSelected(): Promise<void> {
    if (this.focus === 'sidebar') {
      const playlist = this.sidebarSelectedPlaylist;
      if (playlist) await this.openPlaylist(playlist.id, playlist.name);
      return;
    }
    if (this.view.kind === 'queue') {
      await this.playIndex(this.selection);
      return;
    }
    const track = this.selectedTrack;
    if (track) await this.playTrack(track);
  }

  /// Play the playlist the keyboard points at: the sidebar-selected
  /// playlist when the sidebar has focus, otherwise the open playlist
  /// view. Anywhere else there is no selected playlist, so this is a
  /// no-op (the library cards have no keyboard selection).
  async playSelectedPlaylist(): Promise<void> {
    const id =
      this.focus === 'sidebar'
        ? this.sidebarSelectedPlaylist?.id
        : this.view.kind === 'playlist'
          ? this.view.id
          : undefined;
    if (id) await this.playPlaylist(id);
  }

  async playTrack(track: Track): Promise<void> {
    try {
      this.applyStatus(await this.api.playTrack(track));
      await this.refreshQueue();
    } catch (error) {
      this.report(error);
    }
  }

  async playPlaylist(id: string): Promise<void> {
    try {
      this.applyStatus(await this.api.playPlaylist(id, this.activeProvider));
      await this.refreshQueue();
      this.notify('success', 'Playing playlist.');
    } catch (error) {
      this.report(error);
    }
  }

  async playIndex(index: number): Promise<void> {
    if (!this.queue) return;
    const track = this.queue.items[index]?.track;
    if (track) {
      await this.playTrack(track);
      this.selection = index;
    }
  }

  async queueTrack(track: Track): Promise<void> {
    try {
      this.queue = await this.api.queueAdd(track);
      this.notify('info', `Queued “${track.title}”.`);
    } catch (error) {
      this.report(error);
    }
  }

  async queueSelected(): Promise<void> {
    const track = this.selectedTrack;
    if (track) await this.queueTrack(track);
  }

  async togglePlayback(): Promise<void> {
    const playback = this.status?.playback;
    if (!playback) return;
    if (playback.state === 'stopped') {
      const track = playback.current;
      if (track) await this.playTrack(track);
      return;
    }
    const wasPlaying = playback.state === 'playing';
    this.patchPlayback({ state: wasPlaying ? 'paused' : 'playing' });
    try {
      this.applyStatus(wasPlaying ? await this.api.pause() : await this.api.resume());
    } catch (error) {
      this.patchPlayback({ state: playback.state });
      this.report(error);
    }
  }

  async next(): Promise<void> {
    try {
      this.applyStatus(await this.api.next());
      await this.refreshQueue();
    } catch (error) {
      this.report(error);
    }
  }

  async previous(): Promise<void> {
    try {
      this.applyStatus(await this.api.previous());
      await this.refreshQueue();
    } catch (error) {
      this.report(error);
    }
  }

  async setVolume(volume: number): Promise<void> {
    const previous = this.volume;
    const clamped = Math.max(0, Math.min(100, Math.round(volume)));
    this.volume = clamped;
    this.patchPlayback({ volume: clamped });
    try {
      this.applyStatus(await this.api.setVolume(clamped));
    } catch (error) {
      this.volume = previous;
      this.patchPlayback({ volume: previous });
      this.report(error);
    }
  }

  async seek(positionSecs: number): Promise<void> {
    const previous = this.status?.playback.position_secs ?? 0;
    this.patchPlayback({ position_secs: positionSecs });
    try {
      this.applyStatus(await this.api.seek(positionSecs));
    } catch (error) {
      this.patchPlayback({ position_secs: previous });
      this.report(error);
    }
  }

  async toggleShuffle(): Promise<void> {
    const current = this.status?.playback.shuffle ?? false;
    this.patchPlayback({ shuffle: !current });
    try {
      this.applyStatus(await this.api.setShuffle(!current));
    } catch (error) {
      this.patchPlayback({ shuffle: current });
      this.report(error);
    }
  }

  async cycleRepeat(): Promise<void> {
    const current = this.status?.playback.repeat ?? 'off';
    const next: RepeatMode = current === 'off' ? 'all' : current === 'all' ? 'one' : 'off';
    this.patchPlayback({ repeat: next });
    try {
      this.applyStatus(await this.api.setRepeat(next));
    } catch (error) {
      this.patchPlayback({ repeat: current });
      this.report(error);
    }
  }

  async adjustVolume(delta: number): Promise<void> {
    await this.setVolume(this.volume + delta);
  }

  /// Seek relative to the current position, clamped to [0, duration]
  /// like the TUI. No-ops when nothing is playing. When the duration
  /// is unknown (unparseable), only the lower bound applies.
  async seekRelative(deltaSecs: number): Promise<void> {
    const track = this.currentTrack;
    if (!track) return;
    const position = this.status?.playback.position_secs ?? 0;
    const duration = durationSeconds(track.duration);
    let target = position + deltaSecs;
    if (duration > 0) target = Math.min(target, duration);
    await this.seek(Math.max(0, target));
  }

  /// Cycle to the next provider, like the TUI. With one provider there
  /// is nothing to switch to.
  async cycleProvider(): Promise<void> {
    if (this.providers.length < 2) {
      this.notify('info', `${this.activeProviderName} is the only provider.`);
      return;
    }
    const current = this.providers.findIndex(
      (provider) => provider.id === this.activeProvider,
    );
    const next = this.providers[(current + 1) % this.providers.length];
    await this.setProvider(next.id);
  }

  async reorderQueue(from: number, to: number): Promise<void> {
    if (!this.queue) return;
    const previous = this.queue;
    this.queue = reorderQueue(previous, from, to);
    try {
      this.queue = await this.api.queueReorder(from, to);
    } catch (error) {
      this.queue = previous;
      this.report(error);
    }
  }

  async removeQueueIndex(index: number): Promise<void> {
    if (!this.queue) return;
    const previous = this.queue;
    this.queue = removeQueueItem(previous, index);
    this.selection = Math.max(0, Math.min(this.selection, this.queue.items.length - 1));
    try {
      this.queue = await this.api.queueRemove(index);
    } catch (error) {
      this.queue = previous;
      this.report(error);
    }
  }

  async playNext(index: number): Promise<void> {
    if (!this.queue) return;
    const previous = this.queue;
    try {
      this.queue = await this.api.queuePlayNext(index);
    } catch (error) {
      this.queue = previous;
      this.report(error);
    }
  }

  async clearQueue(): Promise<void> {
    if (!this.queue) return;
    const previous = this.queue;
    this.queue = { ...previous, items: [], current: null };
    this.selection = 0;
    this.confirmClearQueue = false;
    try {
      this.queue = await this.api.queueClear();
    } catch (error) {
      this.queue = previous;
      this.report(error);
    }
  }

  /// Arm the clear-queue confirmation. Queue view only, like the TUI,
  /// and only when there is something to clear.
  requestClearQueue(): void {
    if (this.view.kind !== 'queue') return;
    if ((this.queue?.items.length ?? 0) === 0) return;
    this.confirmClearQueue = true;
  }

  cancelClearQueue(): void {
    this.confirmClearQueue = false;
  }

  /// Move the selected queue item, following the issue (queue only;
  /// the TUI also moves playlist tracks, but the web keeps J/K to the
  /// queue). The selection follows the moved item.
  async moveSelectedQueueItem(delta: number): Promise<void> {
    if (this.view.kind !== 'queue' || !this.queue) return;
    const to = this.selection + delta;
    if (to < 0 || to >= this.queue.items.length) return;
    await this.reorderQueue(this.selection, to);
    this.selection = to;
  }

  async retryDownload(track: Track): Promise<void> {
    try {
      await this.api.retryDownload(track);
      this.notify('success', 'Download queued for retry.');
    } catch (error) {
      this.report(error);
    }
  }

  /// Retry the selected download. Queue-only, like the TUI: the retry
  /// button only exists on queue items, and a selected library or
  /// playlist track may not be in the queue at all.
  async retrySelected(): Promise<void> {
    if (this.view.kind !== 'queue') return;
    const track = this.queue?.items[this.selection]?.track;
    if (track) await this.retryDownload(track);
  }

  openCreatePlaylist(): void {
    if (!this.canCreatePlaylist) return;
    // The create form lives in the library header.
    if (this.view.kind !== 'library') this.openLibrary();
    this.createPlaylistName = '';
    this.createPlaylistOpen = true;
  }

  closeCreatePlaylist(): void {
    this.createPlaylistOpen = false;
    this.createPlaylistName = '';
  }

  /// Open the rename form for the playlist the keyboard points at:
  /// the open playlist view, or the sidebar-selected playlist (which
  /// is opened first, since the form lives in the playlist header).
  openRenamePlaylist(): void {
    if (!this.canEditPlaylist) return;
    const target =
      this.view.kind === 'playlist' && this.playlist
        ? { id: this.view.id, name: this.playlist.playlist.name }
        : this.sidebarSelectedPlaylist
          ? { id: this.sidebarSelectedPlaylist.id, name: this.sidebarSelectedPlaylist.name }
          : null;
    if (!target) return;
    if (this.view.kind !== 'playlist' || this.view.id !== target.id) {
      void this.openPlaylist(target.id, target.name);
    }
    this.renamePlaylistId = target.id;
    this.renamePlaylistName = target.name;
    this.renamePlaylistOpen = true;
  }

  closeRenamePlaylist(): void {
    this.renamePlaylistOpen = false;
    this.renamePlaylistId = null;
    this.renamePlaylistName = '';
  }

  /// Open the playlist picker for the selected track, the web
  /// equivalent of the TUI PickPlaylist overlay. No-ops with no
  /// selected track (e.g. the library view) or with no playlists.
  openPlaylistPicker(): void {
    if (this.focus === 'sidebar') return;
    const track = this.selectedTrack;
    if (!track) return;
    if (this.playlists.length === 0) {
      this.notify('info', 'No playlists yet.');
      return;
    }
    this.pickerTrack = track;
    this.pickerSelection = 0;
    this.pickerOpen = true;
  }

  closePlaylistPicker(): void {
    this.pickerOpen = false;
    this.pickerTrack = null;
    this.pickerSelection = 0;
  }

  selectPicker(index: number): void {
    if (this.playlists.length === 0) {
      this.pickerSelection = 0;
      return;
    }
    this.pickerSelection = Math.max(0, Math.min(index, this.playlists.length - 1));
  }

  async confirmPicker(): Promise<boolean> {
    const track = this.pickerTrack;
    const playlist = this.playlists[this.pickerSelection];
    if (!track || !playlist) return false;
    this.closePlaylistPicker();
    return this.addPlaylistTracks(playlist.id, track.provider, [track.track_id]);
  }

  openSearch(): void {
    this.searchMode = 'normal';
    this.searchPlaylistId = null;
    this.searchOpen = true;
  }

  openSearchForPlaylist(playlistId: string): void {
    this.clearSearchTimer();
    this.searchMode = 'add-to-playlist';
    this.searchPlaylistId = playlistId;
    this.searchQuery = '';
    this.searchResults = null;
    this.searchLoading = false;
    this.searchOpen = true;
  }

  closeSearch(): void {
    this.searchOpen = false;
    this.searchMode = 'normal';
    this.searchPlaylistId = null;
  }

  async addSearchSelection(track: Track): Promise<void> {
    if (this.searchMode === 'add-to-playlist' && this.searchPlaylistId) {
      await this.addPlaylistTracks(this.searchPlaylistId, track.provider, [track.track_id]);
      return;
    }
    this.closeSearch();
    await this.playTrack(track);
  }

  setSearchQuery(query: string): void {
    this.searchQuery = query;
    this.clearSearchTimer();
    if (!query.trim()) {
      this.searchResults = null;
      this.searchLoading = false;
      return;
    }
    const timeout = this.options.setTimeoutFn ?? setTimeout;
    this.searchTimer = timeout(() => {
      void this.searchNow();
    }, this.options.searchDebounceMs);
  }

  async searchNow(): Promise<void> {
    const query = this.searchQuery.trim();
    if (!query) return;
    this.searchLoading = true;
    try {
      const results = await this.api.search(query, this.activeProvider);
      if (this.searchQuery.trim() === query) this.searchResults = results;
    } catch (error) {
      this.searchResults = null;
      this.report(error);
    } finally {
      this.searchLoading = false;
    }
  }

  toggleHelp(): void {
    this.helpOpen = !this.helpOpen;
  }

  openFullPlayer(): void {
    this.fullPlayerOpen = true;
  }

  closeFullPlayer(): void {
    this.fullPlayerOpen = false;
  }

  toggleFullPlayer(): void {
    this.fullPlayerOpen = !this.fullPlayerOpen;
  }

  setPlayerLayout(layout: PlayerLayout): void {
    this.playerLayout = layout;
    try {
      globalThis.localStorage?.setItem(PLAYER_LAYOUT_KEY, layout);
    } catch {
      // Ignore storage failures.
    }
  }

  togglePlayerLayout(): void {
    this.setPlayerLayout(this.playerLayout === 'compact' ? 'terminal' : 'compact');
  }

  closeOverlays(): void {
    if (this.fullPlayerOpen) {
      this.fullPlayerOpen = false;
      return;
    }
    if (this.searchOpen) {
      this.searchOpen = false;
      return;
    }
    if (this.helpOpen) {
      this.helpOpen = false;
      return;
    }
    if (this.pickerOpen) {
      this.closePlaylistPicker();
      return;
    }
    if (this.createPlaylistOpen) {
      this.closeCreatePlaylist();
      return;
    }
    if (this.renamePlaylistOpen) {
      this.closeRenamePlaylist();
      return;
    }
    if (this.confirmClearQueue) {
      this.confirmClearQueue = false;
      return;
    }
    // Nothing open: Esc returns keyboard focus to the main view,
    // like the TUI.
    if (this.focus === 'sidebar') this.focus = 'main';
  }

  retryConnection(): void {
    this.events.retry();
  }

  handleEvent(event: EventKind): void {
    switch (event.type) {
      case 'snapshot':
        this.status = event.status;
        this.queue = event.queue;
        this.volume = event.status.playback.volume;
        this.notePlaybackEvent();
        break;
      case 'playback':
        this.patchPlayback(event.playback);
        this.notePlaybackEvent();
        break;
      case 'queue':
        this.queue = event.queue;
        if (this.status) {
          this.status = { ...this.status, queue_length: event.queue.items.length };
        }
        break;
      case 'download':
        if (this.queue) {
          this.queue = {
            ...this.queue,
            items: this.queue.items.map((item) =>
              item.track.provider === event.provider && item.track.track_id === event.track_id
                ? { ...item, download: event.status }
                : item,
            ),
          };
        }
        this.scheduleStatusRefresh();
        break;
      case 'track_ended':
        this.scheduleStatusRefresh();
        break;
      case 'error':
        this.notify('error', event.error.message);
        break;
      case 'library_changed':
        void this.loadPlaylists();
        break;
    }
  }

  notify(
    severity: ToastSeverity,
    message: string,
    action?: Toast['action'],
  ): void {
    const toast: Toast = { id: ++this.toastSeq, severity, message, action };
    this.toasts = [...this.toasts, toast];
    const timeout = this.options.setTimeoutFn ?? setTimeout;
    const ttl = severity === 'error' ? this.options.toastTtlMs * 2 : this.options.toastTtlMs;
    const id = timeout(() => {
      this.liveToasts.delete(id);
      this.dismissToast(toast.id);
    }, ttl);
    this.liveToasts.add(id);
  }

  dismissToast(id: number): void {
    this.toasts = this.toasts.filter((toast) => toast.id !== id);
  }

  downloadStatusFor(track: Track): DownloadStatus {
    return (
      this.queue?.items.find(
        (item) => item.track.provider === track.provider && item.track.track_id === track.track_id,
      )?.download ?? {
        state: 'not_downloaded',
      }
    );
  }

  private applyStatus(status: Status): void {
    this.status = status;
    this.volume = status.playback.volume;
    this.updatePlaybackWatch();
  }

  private patchPlayback(patch: Partial<Status['playback']>): void {
    if (!this.status) return;
    this.status = {
      ...this.status,
      playback: { ...this.status.playback, ...patch },
    };
    if (patch.volume !== undefined) this.volume = patch.volume;
    this.updatePlaybackWatch();
  }

  private report(error: unknown): void {
    if (error instanceof ApiError && error.code === 'network_error') {
      this.connection = 'offline';
    }
    this.handleAuthError(error);
    // Keep the auth banner as the only message for an expired
    // session. Every load path reports through this method, so the
    // rule covers search and queue loads as well as library loads.
    if (error instanceof ApiError && error.code === 'auth_expired') return;
    this.notify('error', describeError(error, this.activeProviderName));
  }

  private handleAuthError(error: unknown): void {
    if (error instanceof ApiError && error.code === 'auth_expired') {
      // The daemon sets the provider state before it returns 401.
      // Re-read it so the banner shows the source and message.
      void this.loadProviders();
      void this.refreshStatus();
    }
  }

  /// Keep the auth banner as the only message for an expired session.
  private setLoadError(error: unknown): void {
    if (error instanceof ApiError && error.code === 'auth_expired') return;
    this.error = describeError(error, this.activeProviderName);
  }

  private scheduleStatusRefresh(): void {
    this.clearStatusRefreshTimer();
    const timeout = this.options.setTimeoutFn ?? setTimeout;
    this.statusRefreshTimer = timeout(() => {
      this.statusRefreshTimer = null;
      void this.refreshStatus();
    }, 300);
  }

  /// Record a playback event from the stream. Only stream events move
  /// this clock: a REST status refresh must not hide a dead stream.
  /// The daemon publishes a playback event every 500 ms while it plays,
  /// so 2 s without one means the stream stopped. A real event also
  /// proves the stream is healthy, so it resets the retry backoff.
  private notePlaybackEvent(): void {
    this.lastPlaybackEventMs = Date.now();
    this.playbackStaleRefreshed = false;
    this.playbackStaleRetried = false;
    this.playbackStaleRetryCount = 0;
    this.updatePlaybackWatch();
  }

  /// Start a new watch period after a reconnect. The old silence no
  /// longer counts against the new connection. A reconnect alone does
  /// not reset the retry backoff: only a playback event proves the
  /// new stream publishes, so a silent rebuild keeps the longer wait.
  private resetPlaybackWatch(): void {
    this.lastPlaybackEventMs = Date.now();
    this.playbackStaleRefreshed = false;
    this.playbackStaleRetried = false;
    this.updatePlaybackWatch();
  }

  /// Run the watchdog only while playback runs. Pause, stop, and
  /// dispose clear it. A fresh episode starts with the first retry
  /// threshold, so a pause also resets the retry backoff.
  private updatePlaybackWatch(): void {
    if (this.status?.playback.state !== 'playing') {
      this.clearPlaybackWatchTimer();
      this.lastPlaybackEventMs = null;
      this.playbackStaleRefreshed = false;
      this.playbackStaleRetried = false;
      this.playbackStaleRetryCount = 0;
      return;
    }
    if (this.lastPlaybackEventMs === null) this.lastPlaybackEventMs = Date.now();
    this.startPlaybackWatchTimer();
  }

  private startPlaybackWatchTimer(): void {
    if (this.playbackWatchTimer !== null) return;
    // Browser timer functions require the global receiver when called
    // through the options object.
    const interval = this.options.setIntervalFn ?? setInterval.bind(globalThis);
    this.playbackWatchTimer = interval(
      () => this.checkPlaybackStaleness(),
      PLAYBACK_WATCH_INTERVAL_MS,
    );
  }

  private clearPlaybackWatchTimer(): void {
    if (this.playbackWatchTimer !== null) {
      const clear = this.options.clearIntervalFn ?? clearInterval.bind(globalThis);
      clear(this.playbackWatchTimer);
      this.playbackWatchTimer = null;
    }
  }

  /// Silence before the next stream rebuild. The threshold doubles
  /// after each rebuild that brings no playback event, up to the cap.
  /// A silent stream is rebuilt less often instead of every 5 s.
  private playbackStaleRetryMs(): number {
    return Math.min(
      PLAYBACK_STALE_RETRY_BASE_MS * 2 ** this.playbackStaleRetryCount,
      PLAYBACK_STALE_RETRY_MAX_MS,
    );
  }

  /// Check a playing track for stalled playback events. At 2 s without
  /// an event, re-read the status: the daemon computes the live position
  /// from the audio clock, so the bar advances again. At 5 s without
  /// an event, rebuild the stream for the first time. Later rebuilds
  /// wait longer: the threshold doubles after each silent rebuild, up
  /// to 60 s. A playback event resets the threshold to 5 s. Each step
  /// runs once per watch period, until a new event or a reconnect
  /// starts a new period.
  private checkPlaybackStaleness(): void {
    const playback = this.status?.playback;
    if (!playback || playback.state !== 'playing' || this.lastPlaybackEventMs === null) {
      this.clearPlaybackWatchTimer();
      return;
    }
    const silentMs = Date.now() - this.lastPlaybackEventMs;
    if (silentMs >= this.playbackStaleRetryMs() && !this.playbackStaleRetried) {
      this.playbackStaleRetried = true;
      this.playbackStaleRetryCount += 1;
      this.events.retry();
      return;
    }
    if (silentMs >= PLAYBACK_STALE_REFRESH_MS && !this.playbackStaleRefreshed) {
      this.playbackStaleRefreshed = true;
      void this.refreshStatus();
    }
  }

  private clearStatusRefreshTimer(): void {
    if (this.statusRefreshTimer !== null) {
      const clear = this.options.clearTimeoutFn ?? clearTimeout;
      clear(this.statusRefreshTimer);
      this.statusRefreshTimer = null;
    }
  }

  private clearSearchTimer(): void {
    if (this.searchTimer !== null) {
      const clear = this.options.clearTimeoutFn ?? clearTimeout;
      clear(this.searchTimer);
      this.searchTimer = null;
    }
  }

  private parseLocation(): void {
    const hash = window.location.hash;
    if (hash.startsWith('#/queue')) {
      this.view = { kind: 'queue' };
    } else if (hash.startsWith('#/playlist/')) {
      const id = decodeURIComponent(hash.slice('#/playlist/'.length));
      if (id) this.view = { kind: 'playlist', id };
    } else {
      this.view = { kind: 'library' };
    }
  }

  private syncLocation(): void {
    const hash =
      this.view.kind === 'library'
        ? '#/library'
        : this.view.kind === 'queue'
          ? '#/queue'
          : `#/playlist/${encodeURIComponent(this.view.id)}`;
    if (window.location.hash !== hash) {
      window.history.replaceState(null, '', hash);
    }
  }
}
