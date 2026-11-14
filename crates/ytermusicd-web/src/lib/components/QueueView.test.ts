import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import QueueView from './QueueView.svelte';
import { AppStore } from '../store.svelte';
import { downloadStatus, makeApi, makeQueue, makeTrack } from '../../test/fixtures';

function makeStore() {
  const initial = makeQueue([makeTrack('v0'), makeTrack('v1'), makeTrack('v2')], {
    current: 0,
  });
  const api = makeApi({}, initial);
  const store = new AppStore(api);
  store.queue = initial;
  return { api, store };
}

describe('QueueView', () => {
  it('reorders with drag and drop', async () => {
    const { api, store } = makeStore();
    const { container } = render(QueueView, { props: { store } });
    const rows = container.querySelectorAll('.row');

    await fireEvent.dragStart(rows[2]);
    await fireEvent.dragOver(rows[0]);
    await fireEvent.drop(rows[0]);
    await tick();

    expect(api.queueReorder).toHaveBeenCalledWith(2, 0);
    expect(store.queue?.items.map((item) => item.track.video_id)).toEqual([
      'v2',
      'v0',
      'v1',
    ]);
  });

  it('removes an item and updates selection', async () => {
    const { api, store } = makeStore();
    store.selection = 1;
    render(QueueView, { props: { store } });

    await fireEvent.click(
      screen.getByRole('button', { name: 'Remove Track v1 from the queue' }),
    );
    await tick();
    expect(api.queueRemove).toHaveBeenCalledWith(1);
    expect(store.queue?.items.map((item) => item.track.video_id)).toEqual([
      'v0',
      'v2',
    ]);
  });

  it('confirms before clearing the queue', async () => {
    const { api, store } = makeStore();
    render(QueueView, { props: { store } });

    await fireEvent.click(screen.getByRole('button', { name: 'Clear queue' }));
    expect(screen.getByText('Clear the queue?')).toBeInTheDocument();
    expect(api.queueClear).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear' }));
    await tick();
    expect(api.queueClear).toHaveBeenCalledOnce();
    expect(store.queue?.items).toHaveLength(0);
  });

  it('plays next and retries failed downloads', async () => {
    const { api, store } = makeStore();
    const failed = store.queue!.items[1];
    store.queue = {
      ...store.queue!,
      items: store.queue!.items.map((item, index) =>
        index === 1 ? { ...item, download: downloadStatus('failed') } : item,
      ),
    };
    render(QueueView, { props: { store } });

    await fireEvent.click(
      screen.getByRole('button', { name: `Retry download for ${failed.track.title}` }),
    );
    expect(api.retryDownload).toHaveBeenCalledWith(failed.track.video_id);

    await fireEvent.click(
      screen.getByRole('button', { name: `Play ${failed.track.title} next` }),
    );
    expect(api.queuePlayNext).toHaveBeenCalledWith(1);
  });

  it('shows an empty state', () => {
    const store = new AppStore(makeApi());
    store.queue = makeQueue();
    render(QueueView, { props: { store } });
    expect(screen.getByText('The queue is empty.')).toBeInTheDocument();
  });
});
