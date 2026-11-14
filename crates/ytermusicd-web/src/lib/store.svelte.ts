import type { DownloadStatus } from '@api/DownloadStatus';
import type { EventKind } from '@api/EventKind';
import type { Playlist } from '@api/Playlist';
import type { PlaylistDetail } from '@api/PlaylistDetail';
import type { Queue } from '@api/Queue';
import type { RepeatMode } from '@api/RepeatMode';
import type { SearchResults } from '@api/SearchResults';
import type { Status } from '@api/Status';
import type { Track } from '@api/Track';
import { ApiError, createApi, describeError, type YtermusicdApi } from './api';
import { EventStream, type ConnectionState, type EventSourceLike } from './sse';

export type View =
  | { kind: 'library' }
  | { kind: 'playlist'; id: string; name?: string }
  | { kind: 'queue' };

export type ToastSeverity = 'info' | 'success' | 'warning' | 'error';

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
}

const DEFAULT_TOAST_TTL = 6000;
const DEFAULT_SEARCH_DEBOUNCE = 250;

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

export function looksLikeAuthError(message: string): boolean {
  const text = message.toLowerCase();
  return (
    text.includes('401') ||
    text.includes('403') ||
    text.includes('cookie') ||
    text.includes('sign in') ||
    text.includes('auth')
  );
}

export class AppStore {
  readonly api: YtermusicdApi;

  status = $state<Status | null>(null);
  queue = $state<Queue | null>(null);
  playlists = $state<Playlist[]>([]);
  playlist = $state<PlaylistDetail | null>(null);
  playlistsLoading = $state(false);
  playlistLoading = $state(false);
  error = $state<string | null>(null);
  authExpired = $state(false);
  connection = $state<ConnectionState>('connecting');
  view = $state<View>({ kind: 'library' });
  selection = $state(0);
  filter = $state('');
  searchOpen = $state(false);
  searchQuery = $state('');
  searchResults = $state<SearchResults | null>(null);
  searchLoading = $state(false);
  helpOpen = $state(false);
  toasts = $state<Toast[]>([]);
  volume = $state(50);

  private readonly events: EventStream;
  private readonly options: Required<Pick<StoreOptions, 'toastTtlMs' | 'searchDebounceMs'>> &
    StoreOptions;
  private readonly liveToasts = new Set<ReturnType<typeof setTimeout>>();
  private toastSeq = 0;
  private searchTimer: ReturnType<typeof setTimeout> | null = null;
  private statusRefreshTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(api: YtermusicdApi = createApi(), options: StoreOptions = {}) {
    this.api = api;
    this.options = {
      toastTtlMs: DEFAULT_TOAST_TTL,
      searchDebounceMs: DEFAULT_SEARCH_DEBOUNCE,
      ...options,
    };
    this.events = new EventStream({
      create: options.createEventSource,
      onEvent: (event) => this.handleEvent(event),
      onState: (state) => {
        this.connection = state;
        if (state === 'online' && this.error === 'Daemon is unreachable. Is ytermusicd running?') {
          this.error = null;
        }
      },
      onParseError: () => this.notify('warning', 'Received a malformed event from the daemon.'),
    });
  }

  async init(): Promise<void> {
    this.parseLocation();
    this.events.connect();
    await Promise.allSettled([this.refreshStatus(), this.loadPlaylists()]);
    if (this.view.kind === 'playlist') {
      await this.openPlaylist(this.view.id, this.view.name);
    }
  }

  dispose(): void {
    this.events.close();
    this.clearSearchTimer();
    this.clearStatusRefreshTimer();
    for (const id of this.liveToasts) {
      const clear = this.options.clearTimeoutFn ?? clearTimeout;
      clear(id);
    }
    this.liveToasts.clear();
  }

  get currentTrack(): Track | null {
    return this.status?.playback.current ?? null;
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

  get selectionCount(): number {
    return this.viewTracks.length;
  }

  async refreshStatus(): Promise<void> {
    try {
      this.applyStatus(await this.api.status());
    } catch (error) {
      this.error = describeError(error);
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
      this.playlists = await this.api.playlists();
    } catch (error) {
      this.error = describeError(error);
    } finally {
      this.playlistsLoading = false;
    }
  }

  async openPlaylist(id: string, name?: string): Promise<void> {
    this.view = { kind: 'playlist', id, name };
    this.selection = 0;
    this.playlist = null;
    this.playlistLoading = true;
    this.syncLocation();
    try {
      this.playlist = await this.api.playlist(id);
    } catch (error) {
      this.error = describeError(error);
    } finally {
      this.playlistLoading = false;
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
    const count = this.selectionCount;
    if (count === 0) {
      this.selection = 0;
      return;
    }
    this.selection = Math.max(0, Math.min(index, count - 1));
  }

  moveSelection(delta: number): void {
    this.select(this.selection + delta);
  }

  selectFirst(): void {
    this.select(0);
  }

  selectLast(): void {
    this.select(this.selectionCount - 1);
  }

  halfPage(direction: 1 | -1, rows = 8): void {
    this.moveSelection(direction * rows);
  }

  async playSelected(): Promise<void> {
    if (this.view.kind === 'queue') {
      await this.playIndex(this.selection);
      return;
    }
    const track = this.selectedTrack;
    if (track) await this.playTrack(track);
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
      this.applyStatus(await this.api.playPlaylist(id));
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
    try {
      this.queue = await this.api.queueClear();
    } catch (error) {
      this.queue = previous;
      this.report(error);
    }
  }

  async retryDownload(videoId: string): Promise<void> {
    try {
      await this.api.retryDownload(videoId);
      this.notify('success', 'Download queued for retry.');
    } catch (error) {
      this.report(error);
    }
  }

  openSearch(): void {
    this.searchOpen = true;
  }

  closeSearch(): void {
    this.searchOpen = false;
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
      const results = await this.api.search(query);
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

  closeOverlays(): void {
    if (this.searchOpen) {
      this.searchOpen = false;
      return;
    }
    if (this.helpOpen) this.helpOpen = false;
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
        break;
      case 'playback':
        this.patchPlayback(event.playback);
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
              item.track.video_id === event.video_id
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
      this.queue?.items.find((item) => item.track.video_id === track.video_id)?.download ?? {
        state: 'not_downloaded',
      }
    );
  }

  private applyStatus(status: Status): void {
    this.status = status;
    this.volume = status.playback.volume;
  }

  private patchPlayback(patch: Partial<Status['playback']>): void {
    if (!this.status) return;
    this.status = {
      ...this.status,
      playback: { ...this.status.playback, ...patch },
    };
    if (patch.volume !== undefined) this.volume = patch.volume;
  }

  private report(error: unknown): void {
    if (error instanceof ApiError && error.code === 'network_error') {
      this.connection = 'offline';
    }
    if (
      error instanceof ApiError &&
      error.code === 'upstream_error' &&
      looksLikeAuthError(error.message)
    ) {
      this.authExpired = true;
    }
    this.notify('error', describeError(error));
  }

  private scheduleStatusRefresh(): void {
    this.clearStatusRefreshTimer();
    const timeout = this.options.setTimeoutFn ?? setTimeout;
    this.statusRefreshTimer = timeout(() => {
      this.statusRefreshTimer = null;
      void this.refreshStatus();
    }, 300);
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
