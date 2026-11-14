import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, createApi, durationSeconds, formatDuration } from './api';
import { makeStatus, makeTrack } from '../test/fixtures';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('duration helpers', () => {
  it('parses mm:ss and hh:mm:ss', () => {
    expect(durationSeconds('3:05')).toBe(185);
    expect(durationSeconds('1:02:03')).toBe(3723);
    expect(durationSeconds('bad')).toBe(0);
    expect(durationSeconds('')).toBe(0);
  });

  it('formats durations', () => {
    expect(formatDuration(0)).toBe('0:00');
    expect(formatDuration(65)).toBe('1:05');
    expect(formatDuration(3723)).toBe('1:02:03');
  });
});

describe('api client', () => {
  it('sends JSON bodies and returns typed data', async () => {
    const fetchMock = vi.fn(async () =>
      new Response(JSON.stringify(makeStatus()), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );
    vi.stubGlobal('fetch', fetchMock);
    const api = createApi();
    const status = await api.setVolume(75);

    expect(status.playback.volume).toBe(50);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/volume',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ volume: 75 }),
      }),
    );
  });

  it('encodes search queries and playlist ids', async () => {
    const fetchMock = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      new Response(JSON.stringify({ tracks: [], playlists: [] }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );
    vi.stubGlobal('fetch', fetchMock);
    const api = createApi();

    await api.search('lo fi & rain');
    expect(fetchMock.mock.calls[0][0]).toBe('/api/search?q=lo%20fi%20%26%20rain');

    await api.playlist('VL/1');
    expect(fetchMock.mock.calls[1][0]).toBe('/api/playlists/VL%2F1');
  });

  it('maps error envelopes to ApiError', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () =>
        new Response(
          JSON.stringify({
            error: { code: 'upstream_error', message: 'cookies expired' },
          }),
          { status: 502, headers: { 'content-type': 'application/json' } },
        ),
      ),
    );
    const api = createApi();

    await expect(api.playlists()).rejects.toMatchObject({
      name: 'ApiError',
      code: 'upstream_error',
      status: 502,
      message: 'cookies expired',
    });
  });

  it('reports network failures with a stable code', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new TypeError('connection refused');
      }),
    );
    const api = createApi();

    await expect(api.status()).rejects.toMatchObject({
      name: 'ApiError',
      code: 'network_error',
    });
    await expect(api.status()).rejects.toBeInstanceOf(ApiError);
  });

  it('accepts 202 responses without a body', async () => {
    const fetchMock = vi.fn(async () => new Response(null, { status: 202 }));
    vi.stubGlobal('fetch', fetchMock);
    const api = createApi();

    await expect(api.retryDownload('v1')).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/downloads/retry',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ video_id: 'v1' }),
      }),
    );
  });

  it('sends a full track object when queueing', async () => {
    const fetchMock = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
      new Response(JSON.stringify({ items: [], current: null, repeat: 'off', shuffle: false }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    );
    vi.stubGlobal('fetch', fetchMock);
    const track = makeTrack('v9');
    const api = createApi();

    await api.queueAdd(track);
    expect(fetchMock.mock.calls[0][1]).toMatchObject({
      method: 'POST',
      body: JSON.stringify({ track }),
    });
  });
});
