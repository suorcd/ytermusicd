import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from './api';
import { AppStore, PLAYER_LAYOUT_KEY, removeQueueItem, reorderQueue } from './store.svelte';
import {
  FakeEventSource,
  downloadStatus,
  makeApi,
  makePlaylist,
  makeProviderInfo,
  makeQueue,
  makeStatus,
  makeTrack,
} from '../test/fixtures';

function setup() {
  const api = makeApi();
  const sources: FakeEventSource[] = [];
  const store = new AppStore(api, {
    createEventSource: (url) => {
      const source = new FakeEventSource(url);
      sources.push(source);
      return source;
    },
    toastTtlMs: 1000,
    searchDebounceMs: 250,
  });
  return { api, store, sources };
}

describe('queue helpers', () => {
  const queue = () => makeQueue([makeTrack('v0'), makeTrack('v1'), makeTrack('v2')]);

  it('moves items and keeps the current index', () => {
    const input = queue();
    const moved = reorderQueue(input, 2, 0);
    expect(moved.items.map((item) => item.track.track_id)).toEqual(['v2', 'v0', 'v1']);
    expect(moved.current).toBe(1);

    const same = reorderQueue(input, 1, 1);
    expect(same).toBe(input);
  });

  it('removes items and clamps the current index', () => {
    const removed = removeQueueItem(queue(), 0);
    expect(removed.items.map((item) => item.track.track_id)).toEqual(['v1', 'v2']);
    expect(removed.current).toBe(0);

    const last = removeQueueItem(makeQueue([makeTrack('v0')]), 0);
    expect(last.current).toBeNull();
  });
});

describe('AppStore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.history.replaceState(null, '', '/');
    localStorage.clear();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('initializes state and connects the event stream', async () => {
    const { api, store, sources } = setup();
    await store.init();

    expect(api.status).toHaveBeenCalledOnce();
    expect(api.playlists).toHaveBeenCalledOnce();
    expect(api.providers).toHaveBeenCalledOnce();
    expect(store.providers).toHaveLength(1);
    expect(store.activeProvider).toBe('youtube');
    expect(store.activeProviderName).toBe('YouTube Music');
    expect(store.playlists).toHaveLength(2);
    expect(sources[0].url).toBe('/api/events');

    sources[0].open();
    expect(store.connection).toBe('online');
    store.dispose();
  });

  it('refreshes status and queue after the event stream reconnects', async () => {
    const { api, store, sources } = setup();
    await store.init();
    expect(api.status).toHaveBeenCalledOnce();

    sources[0].open();
    expect(store.connection).toBe('online');

    sources[0].fail();
    // Backoff is the default 1000 ms; the stream has no jitter override
    // here, so advancing past the maximum delay reconnects it.
    vi.advanceTimersByTime(1000);
    expect(sources).toHaveLength(2);
    sources[1].open();

    expect(api.status).toHaveBeenCalledTimes(2);
    expect(api.queue).toHaveBeenCalledTimes(1);
    store.dispose();
  });

  it('sends a fresh status on reconnect before new events apply', async () => {
    const { api, store, sources } = setup();
    await store.init();
    expect(api.status).toHaveBeenCalledOnce();

    sources[0].open();
    sources[0].fail();
    // Backoff is the default 1000 ms; advancing past the maximum
    // delay reconnects it.
    await vi.advanceTimersByTimeAsync(1000);
    expect(sources).toHaveLength(2);
    sources[1].open();

    // The reconnect refresh runs before any event on the new stream.
    expect(api.status).toHaveBeenCalledTimes(2);
    sources[1].emit({
      type: 'playback',
      playback: {
        state: 'playing',
        current: makeTrack('v1'),
        position_secs: 20,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });
    expect(store.status?.playback.position_secs).toBe(20);
    store.dispose();
  });

  it('refreshes status after 2 s without playback events and retries the stream after 5 s', async () => {
    const { api, store, sources } = setup();
    api.status = vi.fn(async () =>
      makeStatus({
        playback: {
          state: 'playing',
          current: makeTrack('v1'),
          position_secs: 12,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      }),
    );
    await store.init();
    expect(api.status).toHaveBeenCalledOnce();
    expect(store.status?.playback.position_secs).toBe(12);

    sources[0].open();
    sources[0].emit({
      type: 'playback',
      playback: {
        state: 'playing',
        current: makeTrack('v1'),
        position_secs: 12,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });

    // The daemon reports a later position when the status is re-read.
    api.status = vi.fn(async () =>
      makeStatus({
        playback: {
          state: 'playing',
          current: makeTrack('v1'),
          position_secs: 14,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      }),
    );

    await vi.advanceTimersByTimeAsync(1999);
    expect(api.status).not.toHaveBeenCalled();
    expect(sources).toHaveLength(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(api.status).toHaveBeenCalledOnce();
    await vi.waitFor(() => {
      expect(store.status?.playback.position_secs).toBe(14);
    });

    await vi.advanceTimersByTimeAsync(3000);
    expect(sources).toHaveLength(2);
    expect(sources[0].closed).toBe(true);
    expect(sources[1].closed).toBe(false);
    store.dispose();
  });

  it('backs off stream retries when rebuilt streams stay silent', async () => {
    const { api, store, sources } = setup();
    api.status = vi.fn(async () =>
      makeStatus({
        playback: {
          state: 'playing',
          current: makeTrack('v1'),
          position_secs: 12,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      }),
    );
    await store.init();

    sources[0].open();
    const playing = {
      type: 'playback',
      playback: {
        state: 'playing',
        current: makeTrack('v1'),
        position_secs: 12,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    };
    sources[0].emit(playing);

    // The rebuilt stream sends heartbeats but no playback events, so
    // the watchdog stays quiet and only the store retry rebuilds it.
    // Advance in 5 s steps and heartbeat the newest source each step.
    const heartbeatNewest = () => {
      const current = sources[sources.length - 1];
      if (!current.closed) current.emitNamed('heartbeat', {});
    };
    const advanceSilent = async (ms: number) => {
      for (let t = 0; t < ms; t += 5000) {
        await vi.advanceTimersByTimeAsync(5000);
        heartbeatNewest();
      }
    };

    // First retry still fires at 5 s.
    await advanceSilent(5000);
    expect(sources).toHaveLength(2);
    sources[1].open();

    // Second retry waits 10 s, not 5 s.
    await advanceSilent(5000);
    expect(sources).toHaveLength(2);
    await advanceSilent(5000);
    expect(sources).toHaveLength(3);
    sources[2].open();

    // Third retry waits 20 s.
    await advanceSilent(10000);
    expect(sources).toHaveLength(3);
    await advanceSilent(10000);
    expect(sources).toHaveLength(4);
    sources[3].open();

    // Fourth retry waits 40 s.
    await advanceSilent(20000);
    expect(sources).toHaveLength(4);
    await advanceSilent(20000);
    expect(sources).toHaveLength(5);
    sources[4].open();

    // The threshold caps at 60 s.
    await advanceSilent(30000);
    expect(sources).toHaveLength(5);
    await advanceSilent(30000);
    expect(sources).toHaveLength(6);
    store.dispose();
  });

  it('resets the retry backoff when a rebuilt stream publishes again', async () => {
    const { api, store, sources } = setup();
    api.status = vi.fn(async () =>
      makeStatus({
        playback: {
          state: 'playing',
          current: makeTrack('v1'),
          position_secs: 12,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      }),
    );
    await store.init();

    sources[0].open();
    const playing = {
      type: 'playback',
      playback: {
        state: 'playing',
        current: makeTrack('v1'),
        position_secs: 12,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    };
    sources[0].emit(playing);

    await vi.advanceTimersByTimeAsync(5000);
    expect(sources).toHaveLength(2);
    sources[1].open();

    // The rebuilt stream recovers: a playback event resets the
    // backoff, so the next silence retries at 5 s again, not 10 s.
    sources[1].emit({
      type: 'playback',
      playback: {
        state: 'playing',
        current: makeTrack('v1'),
        position_secs: 13,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });

    await vi.advanceTimersByTimeAsync(4999);
    expect(sources).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(sources).toHaveLength(3);
    store.dispose();
  });

  it('does not start the fallback timer when paused', async () => {
    const { api, store, sources } = setup();
    api.status = vi.fn(async () =>
      makeStatus({
        playback: {
          state: 'paused',
          current: makeTrack('v1'),
          position_secs: 10,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      }),
    );
    await store.init();

    sources[0].open();
    sources[0].emit({
      type: 'playback',
      playback: {
        state: 'paused',
        current: makeTrack('v1'),
        position_secs: 10,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });

    await vi.advanceTimersByTimeAsync(10000);
    expect(api.status).toHaveBeenCalledOnce();
    expect(sources).toHaveLength(1);
    store.dispose();
  });

  it('does not start the fallback timer when stopped', async () => {
    const { api, store, sources } = setup();
    await store.init();
    expect(store.status?.playback.state).toBe('stopped');

    await vi.advanceTimersByTimeAsync(10000);
    expect(api.status).toHaveBeenCalledOnce();
    expect(sources).toHaveLength(1);
    store.dispose();
  });

  it('clears the fallback timer on pause', async () => {
    const { api, store, sources } = setup();
    api.status = vi.fn(async () =>
      makeStatus({
        playback: {
          state: 'playing',
          current: makeTrack('v1'),
          position_secs: 12,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      }),
    );
    await store.init();

    sources[0].open();
    sources[0].emit({
      type: 'playback',
      playback: {
        state: 'playing',
        current: makeTrack('v1'),
        position_secs: 12,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });
    await vi.advanceTimersByTimeAsync(1000);
    expect(api.status).toHaveBeenCalledOnce();

    sources[0].emit({
      type: 'playback',
      playback: {
        state: 'paused',
        current: makeTrack('v1'),
        position_secs: 12,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });
    await vi.advanceTimersByTimeAsync(10000);
    expect(api.status).toHaveBeenCalledOnce();
    expect(sources).toHaveLength(1);
    store.dispose();
  });

  it('clears the expired banner after the event stream reconnects', async () => {
    const { api, store, sources } = setup();
    api.providers = vi.fn(async () => [
      makeProviderInfo({
        auth: {
          state: 'expired',
          source: '/tmp/headers.txt',
          message: 'The YouTube Music session expired.',
        },
      }),
    ]);
    await store.init();
    expect(store.activeProviderInfo?.auth.state).toBe('expired');

    // The daemon restarted with a fresh cookie: providers now report ok.
    api.providers = vi.fn(async () => [makeProviderInfo()]);
    sources[0].open();
    expect(store.connection).toBe('online');
    sources[0].fail();
    vi.advanceTimersByTime(1000);
    expect(sources).toHaveLength(2);
    sources[1].open();

    await vi.waitFor(() => {
      expect(store.activeProviderInfo?.auth.state).toBe('ok');
    });
    // The fresh mock serves only the reconnect fetch; init used the
    // expired mock that this test replaced.
    expect(api.providers).toHaveBeenCalledTimes(1);
    store.dispose();
  });

  it('loads providers and scopes the library to the default', async () => {
    const { api, store } = setup();
    await store.init();
    expect(api.providers).toHaveBeenCalledOnce();
    expect(store.providers).toHaveLength(1);
    expect(store.activeProvider).toBe('youtube');
    expect(api.playlists).toHaveBeenCalledWith('youtube');
    expect(store.canCreatePlaylist).toBe(true);
    // Switching to the active provider again does nothing.
    await store.setProvider('youtube');
    expect(api.playlists).toHaveBeenCalledTimes(1);
    store.dispose();
  });

  it('hides playlist creation when the provider lacks the capability', async () => {
    const { api, store } = setup();
    api.providers = vi.fn(async () => [
      makeProviderInfo({
        capabilities: {
          search: true,
          library: true,
          playlists: true,
          playlist_create: false,
          playlist_edit: false,
          downloads: true,
          covers: true,
        },
      }),
    ]);
    await store.init();
    expect(store.canCreatePlaylist).toBe(false);
    expect(store.canEditPlaylist).toBe(false);
    store.dispose();
  });

  it('loads RSS feeds when the RSS provider is active', async () => {
    const { api, store } = setup();
    api.providers = vi.fn(async () => [
      makeProviderInfo(),
      makeProviderInfo({
        id: 'rss',
        name: 'RSS',
        default: false,
        capabilities: {
          search: true,
          library: true,
          playlists: true,
          playlist_create: false,
          playlist_edit: false,
          downloads: true,
          covers: true,
        },
        auth: { state: 'ok', source: null, message: null },
      }),
    ]);
    await store.init();
    await store.setProvider('rss');
    expect(store.isRssProvider).toBe(true);
    expect(store.canCreatePlaylist).toBe(false);
    expect(api.rssFeeds).toHaveBeenCalled();
    expect(store.rssFeeds).toHaveLength(1);
    expect(store.rssFeeds[0].title).toBe('Test Tones');
    store.dispose();
  });

  it('adds, refreshes and removes RSS feeds', async () => {
    const { api, store } = setup();
    await store.init();
    await store.loadRssFeeds();
    expect(store.rssFeeds).toHaveLength(1);

    expect(await store.addRssFeed('https://example.com/other.xml')).toBe(true);
    expect(store.rssFeeds).toHaveLength(2);
    expect(api.addRssFeed).toHaveBeenCalledWith('https://example.com/other.xml');

    const refreshed = await store.refreshRssFeed('0123456789abcdef');
    expect(refreshed).toBe(true);
    expect(api.refreshRssFeed).toHaveBeenCalledWith('0123456789abcdef');

    expect(await store.refreshRssFeeds()).toBe(true);
    expect(api.refreshRssFeeds).toHaveBeenCalled();

    expect(await store.removeRssFeed('0123456789abcdef')).toBe(true);
    expect(store.rssFeeds).toHaveLength(1);
    expect(api.removeRssFeed).toHaveBeenCalledWith('0123456789abcdef');
    store.dispose();
  });

  it('toggles and persists the player layout', () => {
    const { store } = setup();
    expect(store.playerLayout).toBe('compact');

    store.togglePlayerLayout();
    expect(store.playerLayout).toBe('terminal');
    expect(localStorage.getItem(PLAYER_LAYOUT_KEY)).toBe('terminal');

    const { store: reloaded } = setup();
    expect(reloaded.playerLayout).toBe('terminal');
    reloaded.togglePlayerLayout();
    expect(reloaded.playerLayout).toBe('compact');
    expect(localStorage.getItem(PLAYER_LAYOUT_KEY)).toBe('compact');
    store.dispose();
    reloaded.dispose();
  });

  it('opens and closes the full player', () => {
    const { store } = setup();
    expect(store.fullPlayerOpen).toBe(false);

    store.openFullPlayer();
    expect(store.fullPlayerOpen).toBe(true);

    store.closeOverlays();
    expect(store.fullPlayerOpen).toBe(false);

    store.toggleFullPlayer();
    expect(store.fullPlayerOpen).toBe(true);
    store.closeFullPlayer();
    expect(store.fullPlayerOpen).toBe(false);
    store.dispose();
  });

  it('applies snapshot, playback, queue and download events', async () => {
    const { store, sources } = setup();
    await store.init();
    const source = sources[0];

    const tracks = [makeTrack('v1'), makeTrack('v2')];
    source.emit({
      type: 'snapshot',
      status: makeStatus({
        playback: {
          state: 'playing',
          current: tracks[0],
          position_secs: 3,
          volume: 70,
          repeat: 'all',
          shuffle: true,
        },
      }),
      queue: makeQueue(tracks),
    });
    expect(store.status?.playback.volume).toBe(70);
    expect(store.volume).toBe(70);
    expect(store.queue?.items).toHaveLength(2);

    source.emit({
      type: 'playback',
      playback: {
        state: 'playing',
        current: tracks[0],
        position_secs: 12,
        volume: 70,
        repeat: 'all',
        shuffle: true,
      },
    });
    expect(store.status?.playback.position_secs).toBe(12);

    source.emit({
      type: 'download',
      provider: 'youtube',
      track_id: 'v1',
      status: downloadStatus('failed'),
    });
    expect(store.queue?.items[0].download.state).toBe('failed');
    expect(store.downloadStatusFor(tracks[0]).state).toBe('failed');

    source.emit({ type: 'queue', queue: makeQueue(tracks, { current: 1 }) });
    expect(store.queue?.current).toBe(1);
    store.dispose();
  });

  it('adds error events as dismissible toasts', async () => {
    const { store } = setup();
    await store.init();
    store.handleEvent({
      type: 'error',
      error: { code: 'internal_error', message: 'audio device lost' },
    });
    expect(store.toasts).toHaveLength(1);
    expect(store.toasts[0].severity).toBe('error');
    expect(store.toasts[0].message).toBe('audio device lost');

    store.dismissToast(store.toasts[0].id);
    expect(store.toasts).toHaveLength(0);
    store.dispose();
  });

  it('auto-dismisses toasts', async () => {
    const { store } = setup();
    await store.init();
    store.notify('info', 'hello');
    expect(store.toasts).toHaveLength(1);
    vi.advanceTimersByTime(1001);
    expect(store.toasts).toHaveLength(0);
    store.dispose();
  });

  it('shows reconnecting and offline states, then recovers', async () => {
    const { store, sources } = setup();
    await store.init();
    sources[0].open();
    expect(store.connection).toBe('online');

    sources[0].fail();
    expect(store.connection).toBe('reconnecting');
    vi.advanceTimersByTime(5000);
    expect(store.connection).toBe('offline');

    sources.at(-1)!.open();
    expect(store.connection).toBe('online');
    store.dispose();
  });

  it('rolls back an optimistic volume change on error', async () => {
    const { api, store } = setup();
    await store.init();
    api.setVolume = vi.fn(async () => {
      throw new ApiError('volume failed', 'internal_error', 500);
    });

    await store.setVolume(75);
    expect(store.volume).toBe(50);
    expect(store.status?.playback.volume).toBe(50);
    expect(store.toasts.at(-1)?.message).toContain('volume failed');
    store.dispose();
  });

  it('rolls back a failed seek', async () => {
    const { api, store } = setup();
    await store.init();
    const track = makeTrack('v1');
    store.status = makeStatus({
      playback: {
        state: 'playing',
        current: track,
        position_secs: 10,
        volume: 50,
        repeat: 'off',
        shuffle: false,
      },
    });
    api.seek = vi.fn(async () => {
      throw new ApiError('seek failed', 'internal_error', 500);
    });

    await store.seek(60);

    expect(store.status?.playback.position_secs).toBe(10);
    expect(store.toasts.at(-1)?.message).toContain('seek failed');
    store.dispose();
  });

  it('rolls back an optimistic queue reorder on error', async () => {
    const { api, store } = setup();
    await store.init();
    const tracks = [makeTrack('v0'), makeTrack('v1')];
    store.queue = makeQueue(tracks, { current: 0 });
    api.queueReorder = vi.fn(async () => {
      throw new ApiError('bad index', 'bad_request', 400);
    });

    await store.reorderQueue(0, 1);
    expect(store.queue?.items.map((item) => item.track.track_id)).toEqual([
      'v0',
      'v1',
    ]);
    expect(store.toasts.at(-1)?.message).toContain('bad index');
    store.dispose();
  });

  it('creates, renames and deletes playlists', async () => {
    const { api, store } = setup();
    await store.init();

    expect(await store.createPlaylist('Mix')).toBe(true);
    expect(api.createPlaylist).toHaveBeenCalledWith({
      title: 'Mix',
      description: '',
      privacy: 'private',
    });
    expect(store.toasts.at(-1)?.message).toContain('Created');

    await store.openPlaylist('p1');
    expect(await store.renamePlaylist('p1', 'New name')).toBe(true);
    expect(api.updatePlaylist).toHaveBeenCalledWith('p1', { title: 'New name' });
    expect(store.playlist?.playlist.name).toBe('New name');
    expect(store.view).toMatchObject({ kind: 'playlist', id: 'p1', name: 'New name' });

    expect(await store.deletePlaylist('p1')).toBe(true);
    expect(api.deletePlaylist).toHaveBeenCalledWith('p1');
    expect(store.view).toEqual({ kind: 'library' });
    store.dispose();
  });

  it('adds, moves and removes playlist tracks', async () => {
    const { api, store } = setup();
    await store.init();
    await store.openPlaylist('p1');

    expect(await store.addPlaylistTracks('p1', 'youtube', ['v3'])).toBe(true);
    expect(api.addPlaylistTracks).toHaveBeenCalledWith('p1', 'youtube', ['v3']);
    expect(store.playlist?.tracks.map((track) => track.track_id)).toContain('v3');

    expect(await store.movePlaylistTrack('p1', 0, 1)).toBe(true);
    expect(api.movePlaylistTrack).toHaveBeenCalledWith('p1', 0, 1);
    expect(store.selection).toBe(1);

    expect(await store.removePlaylistTrack('p1', 0)).toBe(true);
    expect(api.removePlaylistTracks).toHaveBeenCalledWith('p1', [0]);
    expect(store.playlist?.tracks).toHaveLength(2);
    store.dispose();
  });

  it('adds search results to a playlist in add mode', async () => {
    const { api, store } = setup();
    await store.init();
    await store.openPlaylist('p1');
    store.openSearchForPlaylist('p1');

    expect(store.searchMode).toBe('add-to-playlist');
    expect(store.searchPlaylistId).toBe('p1');
    expect(store.searchTargetName).toBe('Playlist p1');

    await store.addSearchSelection(makeTrack('v9'));
    expect(api.addPlaylistTracks).toHaveBeenCalledWith('p1', 'youtube', ['v9']);
    expect(store.searchOpen).toBe(true);
    expect(api.playTrack).not.toHaveBeenCalled();

    store.openSearch();
    expect(store.searchMode).toBe('normal');
    await store.addSearchSelection(makeTrack('v8'));
    expect(api.playTrack).toHaveBeenCalledWith(makeTrack('v8'));
    expect(store.searchOpen).toBe(false);
    store.dispose();
  });

  it('debounces search input', async () => {
    const { api, store } = setup();
    await store.init();
    store.setSearchQuery('civilization');
    expect(api.search).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(250);
    expect(api.search).toHaveBeenCalledWith('civilization', 'youtube');
    expect(store.searchResults?.tracks).toHaveLength(2);
    store.dispose();
  });

  it('queues tracks and reports success', async () => {
    const { api, store } = setup();
    await store.init();
    const track = makeTrack('v9');
    await store.queueTrack(track);
    expect(api.queueAdd).toHaveBeenCalledWith(track);
    expect(store.toasts.at(-1)?.message).toContain('Queued');
    store.dispose();
  });

  it('opens a playlist from the location hash', async () => {
    window.history.replaceState(null, '', '/#/playlist/p1');
    const { api, store } = setup();
    await store.init();
    expect(api.playlist).toHaveBeenCalledWith('p1', 'youtube');
    expect(store.view).toEqual({ kind: 'playlist', id: 'p1' });
    expect(store.playlist?.tracks).toHaveLength(2);
    store.dispose();
  });

  it('reloads the auth state after an expired-session error', async () => {
    const { api, store } = setup();
    await store.init();
    vi.mocked(api.providers).mockClear();
    vi.mocked(api.status).mockClear();
    api.search = vi.fn(async () => {
      throw new ApiError('The YouTube Music session expired.', 'auth_expired', 401);
    });
    api.providers = vi.fn(async () => [
      makeProviderInfo({
        auth: {
          state: 'expired',
          source: '/tmp/headers.txt',
          message: 'The YouTube Music session expired.',
        },
      }),
    ]);
    api.status = vi.fn(async () =>
      makeStatus({
        auth: {
          state: 'expired',
          source: '/tmp/headers.txt',
          message: 'The YouTube Music session expired.',
        },
      }),
    );
    store.searchQuery = 'test';
    await store.searchNow();

    await vi.waitFor(() => {
      expect(store.activeProviderInfo?.auth.state).toBe('expired');
    });
    expect(api.providers).toHaveBeenCalled();
    expect(api.status).toHaveBeenCalled();
    // The auth banner is the only message: no error toast.
    expect(store.toasts).toHaveLength(0);
    store.dispose();
  });

  it('shows no toast when the queue load fails with an expired session', async () => {
    const { api, store } = setup();
    await store.init();
    vi.mocked(api.providers).mockClear();
    vi.mocked(api.status).mockClear();
    api.queue = vi.fn(async () => {
      throw new ApiError('The YouTube Music session expired.', 'auth_expired', 401);
    });
    api.providers = vi.fn(async () => [
      makeProviderInfo({
        auth: {
          state: 'expired',
          source: '/tmp/headers.txt',
          message: 'The YouTube Music session expired.',
        },
      }),
    ]);
    api.status = vi.fn(async () =>
      makeStatus({
        auth: {
          state: 'expired',
          source: '/tmp/headers.txt',
          message: 'The YouTube Music session expired.',
        },
      }),
    );
    await store.refreshQueue();

    await vi.waitFor(() => {
      expect(store.activeProviderInfo?.auth.state).toBe('expired');
    });
    expect(api.providers).toHaveBeenCalled();
    expect(api.status).toHaveBeenCalled();
    // The auth banner is the only message: no error toast.
    expect(store.toasts).toHaveLength(0);
    store.dispose();
  });

  it('shows the expired state when the first library load fails', async () => {
    const api = makeApi();
    api.playlists = vi.fn(async () => {
      throw new ApiError('The YouTube Music session expired.', 'auth_expired', 401);
    });
    api.providers = vi.fn(async () => [
      makeProviderInfo({
        auth: {
          state: 'expired',
          source: '/tmp/headers.txt',
          message: 'The YouTube Music session expired.',
        },
      }),
    ]);
    const sources: FakeEventSource[] = [];
    const store = new AppStore(api, {
      createEventSource: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
    });
    await store.init();

    expect(store.activeProviderInfo?.auth.state).toBe('expired');
    expect(store.error).toBeNull();
    store.dispose();
  });

  it('initializes with the browser EventSource by default', async () => {
    const sources: FakeEventSource[] = [];
    class BrowserSource extends FakeEventSource {
      constructor(url: string) {
        super(url);
        sources.push(this);
      }
    }
    vi.stubGlobal('EventSource', BrowserSource);
    const api = makeApi();
    const store = new AppStore(api);

    await store.init();

    expect(sources).toHaveLength(1);
    expect(store.status).not.toBeNull();
    expect(store.playlists).toHaveLength(2);
    store.dispose();
    vi.unstubAllGlobals();
  });

  it('filters playlists and exposes the selected track', async () => {
    const { store } = setup();
    await store.init();
    store.setFilter('Playlist p2');
    expect(store.filteredPlaylists).toEqual([makePlaylist('p2')]);

    store.queue = makeQueue([makeTrack('v1')]);
    store.openQueue();
    expect(store.selectedTrack?.track_id).toBe('v1');
    store.dispose();
  });

  it('toggles keyboard focus and navigates the sidebar', async () => {
    const { api, store } = setup();
    await store.init();
    expect(store.focus).toBe('main');

    store.toggleFocus();
    expect(store.focus).toBe('sidebar');
    expect(store.sidebarSelectedPlaylist?.id).toBe('p1');

    store.moveSelection(1);
    expect(store.sidebarSelection).toBe(1);
    expect(store.sidebarSelectedPlaylist?.id).toBe('p2');
    expect(store.selection).toBe(0);

    store.selectFirst();
    expect(store.sidebarSelection).toBe(0);
    store.selectLast();
    expect(store.sidebarSelection).toBe(1);

    await store.playSelected();
    expect(api.playlist).toHaveBeenCalledWith('p2', 'youtube');
    expect(store.view).toEqual({ kind: 'playlist', id: 'p2', name: 'Playlist p2' });

    // Esc with nothing open returns focus to the main view.
    store.closeOverlays();
    expect(store.focus).toBe('main');
    store.dispose();
  });

  it('cycles providers and reports a lone provider', async () => {
    const { api, store } = setup();
    await store.init();
    await store.cycleProvider();
    expect(store.activeProvider).toBe('youtube');
    expect(store.toasts.at(-1)?.message).toContain('only provider');

    api.providers = vi.fn(async () => [
      makeProviderInfo(),
      makeProviderInfo({ id: 'rss', name: 'RSS', default: false }),
    ]);
    await store.loadProviders();
    await store.cycleProvider();
    expect(store.activeProvider).toBe('rss');
    await store.cycleProvider();
    expect(store.activeProvider).toBe('youtube');
    store.dispose();
  });

  it('adjusts volume in steps and clamps', async () => {
    const { api, store } = setup();
    await store.init();
    expect(store.volume).toBe(50);

    await store.adjustVolume(5);
    expect(store.volume).toBe(55);
    expect(api.setVolume).toHaveBeenLastCalledWith(55);

    await store.adjustVolume(-5);
    expect(store.volume).toBe(50);

    await store.setVolume(98);
    await store.adjustVolume(5);
    expect(store.volume).toBe(100);
    store.dispose();
  });

  it('seeks relative to the position and clamps to the duration', async () => {
    const { api, store } = setup();
    await store.init();
    // The fake api.seek rebuilds status with no current track, so
    // re-establish playback before each relative seek.
    const playing = (position: number) =>
      makeStatus({
        playback: {
          state: 'playing',
          current: makeTrack('v1'),
          position_secs: position,
          volume: 50,
          repeat: 'off',
          shuffle: false,
        },
      });

    store.status = playing(10);
    await store.seekRelative(5);
    expect(api.seek).toHaveBeenLastCalledWith(15);

    store.status = playing(170);
    await store.seekRelative(500);
    expect(api.seek).toHaveBeenLastCalledWith(180);

    store.status = playing(10);
    await store.seekRelative(-500);
    expect(api.seek).toHaveBeenLastCalledWith(0);

    store.status = makeStatus();
    await store.seekRelative(5);
    expect(api.seek).toHaveBeenCalledTimes(3);
    store.dispose();
  });

  it('moves the selected queue item and follows it', async () => {
    // Pass the queue to makeApi so the fake queueReorder acts on the
    // same queue the store holds.
    const initial = makeQueue([makeTrack('v0'), makeTrack('v1'), makeTrack('v2')]);
    const api = makeApi({}, initial);
    const sources: FakeEventSource[] = [];
    const store = new AppStore(api, {
      createEventSource: (url) => {
        const source = new FakeEventSource(url);
        sources.push(source);
        return source;
      },
      toastTtlMs: 1000,
      searchDebounceMs: 250,
    });
    await store.init();
    store.queue = initial;
    store.openQueue();
    store.select(0);

    await store.moveSelectedQueueItem(1);
    expect(store.queue?.items.map((item) => item.track.track_id)).toEqual([
      'v1',
      'v0',
      'v2',
    ]);
    expect(store.selection).toBe(1);
    expect(api.queueReorder).toHaveBeenCalledWith(0, 1);

    // At the bottom edge there is nowhere to move.
    store.select(2);
    await store.moveSelectedQueueItem(1);
    expect(api.queueReorder).toHaveBeenCalledTimes(1);

    // Outside the queue view J/K do nothing.
    store.openLibrary();
    await store.moveSelectedQueueItem(-1);
    expect(api.queueReorder).toHaveBeenCalledTimes(1);
    store.dispose();
  });

  it('clears the queue only after confirmation', async () => {
    const { api, store } = setup();
    await store.init();
    store.queue = makeQueue([makeTrack('v1')]);

    store.requestClearQueue();
    expect(store.confirmClearQueue).toBe(false);

    store.openQueue();
    store.requestClearQueue();
    expect(store.confirmClearQueue).toBe(true);

    store.cancelClearQueue();
    expect(store.confirmClearQueue).toBe(false);

    store.requestClearQueue();
    await store.clearQueue();
    expect(store.confirmClearQueue).toBe(false);
    expect(store.queue?.items).toHaveLength(0);
    expect(api.queueClear).toHaveBeenCalledOnce();
    store.dispose();
  });

  it('opens and closes the create playlist form', async () => {
    const { store } = setup();
    await store.init();
    store.openQueue();

    store.openCreatePlaylist();
    expect(store.createPlaylistOpen).toBe(true);
    expect(store.view).toEqual({ kind: 'library' });

    store.closeCreatePlaylist();
    expect(store.createPlaylistOpen).toBe(false);
    expect(store.createPlaylistName).toBe('');
    store.dispose();
  });

  it('opens the rename form for the open or sidebar-selected playlist', async () => {
    const { store } = setup();
    await store.init();
    await store.openPlaylist('p1');

    store.openRenamePlaylist();
    expect(store.renamePlaylistOpen).toBe(true);
    expect(store.renamePlaylistId).toBe('p1');
    expect(store.renamePlaylistName).toBe('Playlist p1');

    store.closeRenamePlaylist();
    store.openLibrary();
    store.toggleFocus();
    store.selectSidebar(1);
    store.openRenamePlaylist();
    expect(store.renamePlaylistId).toBe('p2');
    expect(store.view).toEqual({ kind: 'playlist', id: 'p2', name: 'Playlist p2' });
    store.dispose();
  });

  it('plays the selected playlist from the view or the sidebar', async () => {
    const { api, store } = setup();
    await store.init();

    await store.playSelectedPlaylist();
    expect(api.playPlaylist).not.toHaveBeenCalled();

    await store.openPlaylist('p1');
    await store.playSelectedPlaylist();
    expect(api.playPlaylist).toHaveBeenCalledWith('p1', 'youtube');

    store.openLibrary();
    store.toggleFocus();
    store.selectSidebar(1);
    await store.playSelectedPlaylist();
    expect(api.playPlaylist).toHaveBeenLastCalledWith('p2', 'youtube');
    store.dispose();
  });

  it('retries only the selected queue download', async () => {
    const { api, store } = setup();
    await store.init();
    store.queue = makeQueue([makeTrack('v1'), makeTrack('v2')]);

    await store.retrySelected();
    expect(api.retryDownload).not.toHaveBeenCalled();

    store.openQueue();
    store.select(1);
    await store.retrySelected();
    expect(api.retryDownload).toHaveBeenCalledWith(makeTrack('v2'));
    store.dispose();
  });

  it('opens the playlist picker and adds the selected track', async () => {
    const { api, store } = setup();
    await store.init();

    store.openPlaylistPicker();
    expect(store.pickerOpen).toBe(false);

    await store.openPlaylist('p1');
    store.select(1);
    store.openPlaylistPicker();
    expect(store.pickerOpen).toBe(true);
    expect(store.pickerTrack?.track_id).toBe('v2');

    store.selectPicker(1);
    expect(await store.confirmPicker()).toBe(true);
    expect(store.pickerOpen).toBe(false);
    expect(api.addPlaylistTracks).toHaveBeenCalledWith('p2', 'youtube', ['v2']);
    store.dispose();
  });

  it('closes new overlays with Escape, innermost first', async () => {
    const { store } = setup();
    await store.init();
    await store.openPlaylist('p1');
    store.select(0);
    store.openPlaylistPicker();
    expect(store.pickerOpen).toBe(true);

    store.closeOverlays();
    expect(store.pickerOpen).toBe(false);

    store.openCreatePlaylist();
    store.closeOverlays();
    expect(store.createPlaylistOpen).toBe(false);

    store.openRenamePlaylist();
    store.closeOverlays();
    expect(store.renamePlaylistOpen).toBe(false);
    store.dispose();
  });
});
