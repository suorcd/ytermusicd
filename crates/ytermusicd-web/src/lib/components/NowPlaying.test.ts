import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import NowPlaying from './NowPlaying.svelte';
import { AppStore } from '../store.svelte';
import { downloadStatus, makeApi, makeQueue, makeStatus, makeTrack } from '../../test/fixtures';
import type { PlaybackSnapshot } from '@api/PlaybackSnapshot';

function makeStore(playback: Partial<PlaybackSnapshot> = {}) {
  const api = makeApi();
  const store = new AppStore(api);
  const track = makeTrack('v1');
  store.status = makeStatus({
    playback: {
      state: 'playing',
      current: track,
      position_secs: 30,
      volume: 50,
      repeat: 'off',
      shuffle: false,
      ...playback,
    },
  });
  store.queue = makeQueue([track]);
  return { api, store, track };
}

describe('NowPlaying', () => {
  it('renders the current track and controls playback', async () => {
    const { api, store } = makeStore();
    render(NowPlaying, { props: { store } });

    expect(screen.getByText('Track v1')).toBeInTheDocument();
    expect(screen.getByText(/Artist/)).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Pause' }));
    expect(api.pause).toHaveBeenCalledOnce();
    await tick();
    expect(store.status?.playback.state).toBe('paused');

    await fireEvent.click(screen.getByRole('button', { name: 'Next track' }));
    expect(api.next).toHaveBeenCalledOnce();
  });

  it('seeks and changes the volume', async () => {
    const { api, store } = makeStore();
    render(NowPlaying, { props: { store } });

    const seek = screen.getByLabelText('Seek');
    await fireEvent.input(seek, { target: { value: '60' } });
    await fireEvent.change(seek, { target: { value: '60' } });
    expect(api.seek).toHaveBeenCalledWith(60);

    const volume = screen.getByLabelText('Volume');
    await fireEvent.input(volume, { target: { value: '70' } });
    await fireEvent.change(volume, { target: { value: '70' } });
    expect(api.setVolume).toHaveBeenCalledWith(70);
  });

  it('offers a retry for failed downloads', async () => {
    const { api, store, track } = makeStore();
    store.queue = makeQueue([track], {
      items: [{ track, download: downloadStatus('failed') }],
    });
    render(NowPlaying, { props: { store } });

    const retry = screen.getByRole('button', { name: /Download failed/ });
    await fireEvent.click(retry);
    expect(api.retryDownload).toHaveBeenCalledWith('v1');
  });

  it('handles the empty state', () => {
    const store = new AppStore(makeApi());
    store.status = makeStatus();
    render(NowPlaying, { props: { store } });
    expect(screen.getByText('Nothing playing')).toBeInTheDocument();
    expect(screen.getByLabelText('Seek')).toBeDisabled();
  });

  it('shows shuffle and repeat state', async () => {
    const { api, store } = makeStore({ shuffle: true, repeat: 'all' });
    render(NowPlaying, { props: { store } });

    expect(screen.getByRole('button', { name: 'Shuffle' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await fireEvent.click(screen.getByRole('button', { name: /Repeat mode/ }));
    expect(api.setRepeat).toHaveBeenCalledWith('one');
  });
});
