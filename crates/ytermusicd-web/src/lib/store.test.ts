import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from './api';
import { AppStore, removeQueueItem, reorderQueue } from './store.svelte';
import {
  FakeEventSource,
  downloadStatus,
  makeApi,
  makePlaylist,
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
    expect(moved.items.map((item) => item.track.video_id)).toEqual(['v2', 'v0', 'v1']);
    expect(moved.current).toBe(1);

    const same = reorderQueue(input, 1, 1);
    expect(same).toBe(input);
  });

  it('removes items and clamps the current index', () => {
    const removed = removeQueueItem(queue(), 0);
    expect(removed.items.map((item) => item.track.video_id)).toEqual(['v1', 'v2']);
    expect(removed.current).toBe(0);

    const last = removeQueueItem(makeQueue([makeTrack('v0')]), 0);
    expect(last.current).toBeNull();
  });
});

describe('AppStore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.history.replaceState(null, '', '/');
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
    expect(store.playlists).toHaveLength(2);
    expect(sources[0].url).toBe('/api/events');

    sources[0].open();
    expect(store.connection).toBe('online');
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
      video_id: 'v1',
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

  it('rolls back an optimistic queue reorder on error', async () => {
    const { api, store } = setup();
    await store.init();
    const tracks = [makeTrack('v0'), makeTrack('v1')];
    store.queue = makeQueue(tracks, { current: 0 });
    api.queueReorder = vi.fn(async () => {
      throw new ApiError('bad index', 'bad_request', 400);
    });

    await store.reorderQueue(0, 1);
    expect(store.queue?.items.map((item) => item.track.video_id)).toEqual([
      'v0',
      'v1',
    ]);
    expect(store.toasts.at(-1)?.message).toContain('bad index');
    store.dispose();
  });

  it('debounces search input', async () => {
    const { api, store } = setup();
    await store.init();
    store.setSearchQuery('civilization');
    expect(api.search).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(250);
    expect(api.search).toHaveBeenCalledWith('civilization');
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
    expect(api.playlist).toHaveBeenCalledWith('p1');
    expect(store.view).toEqual({ kind: 'playlist', id: 'p1' });
    expect(store.playlist?.tracks).toHaveLength(2);
    store.dispose();
  });

  it('marks the auth banner for upstream auth failures', async () => {
    const { api, store } = setup();
    await store.init();
    api.search = vi.fn(async () => {
      throw new ApiError('401 unauthorized: cookies expired', 'upstream_error', 502);
    });
    store.searchQuery = 'test';
    await store.searchNow();

    expect(store.authExpired).toBe(true);
    expect(store.toasts.at(-1)?.message).toContain('YouTube Music rejected the request');
    store.dispose();
  });

  it('filters playlists and exposes the selected track', async () => {
    const { store } = setup();
    await store.init();
    store.setFilter('Playlist p2');
    expect(store.filteredPlaylists).toEqual([makePlaylist('p2')]);

    store.queue = makeQueue([makeTrack('v1')]);
    store.openQueue();
    expect(store.selectedTrack?.video_id).toBe('v1');
    store.dispose();
  });
});
