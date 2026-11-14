import type { DownloadStatus } from '@api/DownloadStatus';
import type { ErrorEnvelope } from '@api/ErrorEnvelope';
import type { PlaybackSnapshot } from '@api/PlaybackSnapshot';
import type { Playlist } from '@api/Playlist';
import type { PlaylistDetail } from '@api/PlaylistDetail';
import type { Queue } from '@api/Queue';
import type { RepeatMode } from '@api/RepeatMode';
import type { SearchResults } from '@api/SearchResults';
import type { Status } from '@api/Status';
import type { Track } from '@api/Track';

export class ApiError extends Error {
  readonly code: string;
  readonly status: number | null;

  constructor(message: string, code = 'network_error', status: number | null = null) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    this.status = status;
  }
}

export interface YtermusicdApi {
  status(): Promise<Status>;
  playlists(): Promise<Playlist[]>;
  playlist(id: string): Promise<PlaylistDetail>;
  search(query: string): Promise<SearchResults>;
  queue(): Promise<Queue>;
  playPlaylist(playlistId: string): Promise<Status>;
  playTrack(track: Track): Promise<Status>;
  queueAdd(track: Track): Promise<Queue>;
  queueRemove(index: number): Promise<Queue>;
  queueReorder(from: number, to: number): Promise<Queue>;
  queuePlayNext(index: number): Promise<Queue>;
  queueClear(): Promise<Queue>;
  pause(): Promise<Status>;
  resume(): Promise<Status>;
  next(): Promise<Status>;
  previous(): Promise<Status>;
  setVolume(volume: number): Promise<Status>;
  seek(positionSecs: number): Promise<Status>;
  setShuffle(enabled: boolean): Promise<Status>;
  setRepeat(repeat: RepeatMode): Promise<Status>;
  retryDownload(videoId: string): Promise<void>;
}

export function describeError(error: unknown): string {
  if (error instanceof ApiError) {
    switch (error.code) {
      case 'upstream_error':
        return `YouTube Music rejected the request: ${error.message}`;
      case 'bad_request':
        return `Invalid request: ${error.message}`;
      case 'not_found':
        return `Not found: ${error.message}`;
      case 'internal_error':
        return `Daemon error: ${error.message}`;
      case 'network_error':
        return 'Daemon is unreachable. Is ytermusicd running?';
      default:
        return error.message;
    }
  }
  if (error instanceof Error) return error.message;
  return String(error);
}

async function parseError(response: Response): Promise<ApiError> {
  let code = 'http_error';
  let message = `${response.status} ${response.statusText}`;
  try {
    const body = (await response.json()) as ErrorEnvelope;
    if (body?.error) {
      code = body.error.code;
      message = body.error.message;
    }
  } catch {
    // Keep the status line as the message.
  }
  return new ApiError(message, code, response.status);
}

export function createApi(baseUrl = ''): YtermusicdApi {
  async function request<T>(path: string, init?: RequestInit): Promise<T> {
    let response: Response;
    try {
      response = await fetch(`${baseUrl}${path}`, {
        ...init,
        headers: {
          'content-type': 'application/json',
          ...(init?.headers ?? {}),
        },
      });
    } catch (error) {
      throw new ApiError(
        error instanceof Error ? error.message : 'network failure',
        'network_error',
      );
    }
    if (!response.ok) throw await parseError(response);
    if (response.status === 202 || response.status === 204) {
      return undefined as T;
    }
    return (await response.json()) as T;
  }

  const post = <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: body === undefined ? undefined : JSON.stringify(body),
    });

  return {
    status: () => request<Status>('/api/status'),
    playlists: () => request<Playlist[]>('/api/playlists'),
    playlist: (id) => request<PlaylistDetail>(`/api/playlists/${encodeURIComponent(id)}`),
    search: (query) => request<SearchResults>(`/api/search?q=${encodeURIComponent(query)}`),
    queue: () => request<Queue>('/api/queue'),
    playPlaylist: (playlistId) => post<Status>('/api/play/playlist', { playlist_id: playlistId }),
    playTrack: (track) => post<Status>('/api/play/track', { track }),
    queueAdd: (track) => post<Queue>('/api/queue', { track }),
    queueRemove: (index) => post<Queue>('/api/queue/remove', { index }),
    queueReorder: (from, to) => post<Queue>('/api/queue/reorder', { from, to }),
    queuePlayNext: (index) => post<Queue>('/api/queue/play-next', { index }),
    queueClear: () => request<Queue>('/api/queue', { method: 'DELETE' }),
    pause: () => post<Status>('/api/pause'),
    resume: () => post<Status>('/api/resume'),
    next: () => post<Status>('/api/next'),
    previous: () => post<Status>('/api/previous'),
    setVolume: (volume) => post<Status>('/api/volume', { volume }),
    seek: (positionSecs) => post<Status>('/api/seek', { position_secs: positionSecs }),
    setShuffle: (enabled) => post<Status>('/api/shuffle', { enabled }),
    setRepeat: (repeat) => post<Status>('/api/repeat', { repeat }),
    retryDownload: (videoId) => post<void>('/api/downloads/retry', { video_id: videoId }),
  };
}

export function durationSeconds(duration: string): number {
  const parts = duration.split(':').map(Number);
  if (parts.some((part) => !Number.isFinite(part))) return 0;
  if (parts.length === 2) return parts[0] * 60 + parts[1];
  if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2];
  return 0;
}

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0:00';
  const total = Math.floor(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  }
  return `${minutes}:${String(secs).padStart(2, '0')}`;
}

export function downloadLabel(status: DownloadStatus): string {
  switch (status.state) {
    case 'downloading':
      return `${status.progress}%`;
    case 'not_downloaded':
      return 'not cached';
    case 'downloaded':
      return 'cached';
    case 'retrying':
      return `retry ${status.attempt}`;
    case 'failed':
      return 'failed';
  }
}

export type { PlaybackSnapshot, Playlist, PlaylistDetail, Queue, RepeatMode, SearchResults, Status, Track };
