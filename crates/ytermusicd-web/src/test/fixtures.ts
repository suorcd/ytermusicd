import { vi } from 'vitest';
import type { DownloadStatus } from '@api/DownloadStatus';
import type { Playlist } from '@api/Playlist';
import type { PlaylistDetail } from '@api/PlaylistDetail';
import type { Queue } from '@api/Queue';
import type { SearchResults } from '@api/SearchResults';
import type { Status } from '@api/Status';
import type { Track } from '@api/Track';
import type { YtermusicdApi } from '../lib/api';
import type { EventSourceLike } from '../lib/sse';

export function makeTrack(id: string, overrides: Partial<Track> = {}): Track {
  return {
    video_id: id,
    title: `Track ${id}`,
    artist: 'Artist',
    album: 'Album',
    duration: '3:00',
    ...overrides,
  };
}

export function makePlaylist(id: string, overrides: Partial<Playlist> = {}): Playlist {
  return {
    id,
    name: `Playlist ${id}`,
    subtitle: 'YouTube Music',
    ...overrides,
  };
}

export function makePlaylistDetail(
  playlist: Playlist,
  tracks: Track[],
): PlaylistDetail {
  return { playlist, tracks };
}

export function makeQueue(
  tracks: Track[] = [],
  overrides: Partial<Queue> = {},
): Queue {
  return {
    items: tracks.map((track) => ({ track, download: { state: 'not_downloaded' } })),
    current: tracks.length > 0 ? 0 : null,
    repeat: 'off',
    shuffle: false,
    ...overrides,
  };
}

export function makeStatus(overrides: Partial<Status> = {}): Status {
  return {
    name: 'ytermusicd',
    version: '0.1.0',
    status: 'ok',
    uptime_seconds: 12,
    auth: { state: 'ok', source: '/tmp/headers.txt', message: null },
    playback: {
      state: 'stopped',
      current: null,
      position_secs: 0,
      volume: 50,
      repeat: 'off',
      shuffle: false,
    },
    queue_length: 0,
    downloads: { queued: 0, active: 0, completed: 0, retrying: 0, failed: 0 },
    ...overrides,
  };
}

export function makeSearchResults(
  tracks: Track[] = [],
  playlists: Playlist[] = [],
): SearchResults {
  return { tracks, playlists };
}

export function makeApi(
  overrides: Partial<YtermusicdApi> = {},
  initialQueue: Queue = makeQueue(),
  initialStatus: Status = makeStatus(),
): YtermusicdApi {
  let status = initialStatus;
  let queue = initialQueue;
  const playlists = [makePlaylist('p1'), makePlaylist('p2')];
  const tracks = [makeTrack('v1'), makeTrack('v2')];

  const api: YtermusicdApi = {
    status: vi.fn(async () => status),
    playlists: vi.fn(async () => playlists),
    playlist: vi.fn(async (id: string) =>
      makePlaylistDetail(
        playlists.find((playlist) => playlist.id === id) ?? makePlaylist(id),
        tracks,
      ),
    ),
    search: vi.fn(async () => makeSearchResults(tracks, playlists.slice(0, 1))),
    queue: vi.fn(async () => queue),
    playPlaylist: vi.fn(async () => {
      status = makeStatus({
        playback: {
          ...status.playback,
          state: 'playing',
          current: tracks[0],
        },
      });
      queue = makeQueue(tracks);
      return status;
    }),
    playTrack: vi.fn(async (track: Track) => {
      status = makeStatus({
        playback: { ...status.playback, state: 'playing', current: track },
      });
      return status;
    }),
    queueAdd: vi.fn(async (track: Track) => {
      queue = {
        ...queue,
        items: [...queue.items, { track, download: { state: 'not_downloaded' } }],
      };
      return queue;
    }),
    queueRemove: vi.fn(async (index: number) => {
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
      queue = { ...queue, items, current };
      return queue;
    }),
    queueReorder: vi.fn(async (from: number, to: number) => {
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
      queue = { ...queue, items, current };
      return queue;
    }),
    queuePlayNext: vi.fn(async () => queue),
    queueClear: vi.fn(async () => {
      queue = { ...queue, items: [], current: null };
      return queue;
    }),
    pause: vi.fn(async () => {
      status = makeStatus({
        playback: { ...status.playback, state: 'paused' },
      });
      return status;
    }),
    resume: vi.fn(async () => {
      status = makeStatus({
        playback: { ...status.playback, state: 'playing' },
      });
      return status;
    }),
    next: vi.fn(async () => status),
    previous: vi.fn(async () => status),
    setVolume: vi.fn(async (volume: number) => {
      status = makeStatus({
        playback: { ...status.playback, volume },
      });
      return status;
    }),
    seek: vi.fn(async (position_secs: number) => {
      status = makeStatus({
        playback: { ...status.playback, position_secs },
      });
      return status;
    }),
    setShuffle: vi.fn(async (shuffle: boolean) => {
      status = makeStatus({
        playback: { ...status.playback, shuffle },
      });
      return status;
    }),
    setRepeat: vi.fn(async (repeat) => {
      status = makeStatus({
        playback: { ...status.playback, repeat },
      });
      return status;
    }),
    retryDownload: vi.fn(async () => undefined),
    ...overrides,
  };

  return api;
}

export class FakeEventSource implements EventSourceLike {
  onopen: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;

  readonly url: string;
  closed = false;
  private readonly listeners = new Map<string, Array<(event: MessageEvent) => void>>();

  constructor(url: string) {
    this.url = url;
  }

  addEventListener(type: string, listener: (event: MessageEvent) => void): void {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  close(): void {
    this.closed = true;
  }

  open(): void {
    this.onopen?.(new Event('open'));
  }

  fail(): void {
    this.onerror?.(new Event('error'));
  }

  emit(event: unknown): void {
    this.onmessage?.(
      new MessageEvent('message', { data: JSON.stringify({ id: 1, event }) }),
    );
  }

  emitNamed(type: string, event: unknown): void {
    const message = new MessageEvent(type, {
      data: JSON.stringify({ id: 1, event }),
    });
    for (const listener of this.listeners.get(type) ?? []) listener(message);
  }
}

export const downloadStatus = (state: DownloadStatus['state']): DownloadStatus => {
  switch (state) {
    case 'downloading':
      return { state, progress: 40 };
    case 'retrying':
      return { state, attempt: 1 };
    case 'failed':
      return { state, message: '403' };
    default:
      return { state } as DownloadStatus;
  }
};
