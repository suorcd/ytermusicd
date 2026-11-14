import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import TrackTable from './TrackTable.svelte';
import { downloadStatus, makeTrack } from '../../test/fixtures';

function props(tracks = [makeTrack('v1'), makeTrack('v2')]) {
  return {
    tracks,
    selection: 0,
    currentTrack: tracks[0] ?? null,
    downloadStatus: () => downloadStatus('not_downloaded'),
    onselect: vi.fn(),
    onplay: vi.fn(),
    onqueue: vi.fn(),
    onretry: vi.fn(),
  };
}

describe('TrackTable', () => {
  it('renders the required columns and rows', () => {
    const { container } = render(TrackTable, { props: props() });

    expect(screen.getByText('Title')).toBeInTheDocument();
    expect(screen.getAllByText('Artist')).toHaveLength(3);
    expect(screen.getAllByText('Album')).toHaveLength(3);
    expect(screen.getByText('Time')).toBeInTheDocument();
    expect(screen.getByText('Status')).toBeInTheDocument();
    expect(container.querySelectorAll('.row')).toHaveLength(2);
    expect(container.querySelector('.row.playing')).not.toBeNull();
  });

  it('selects, plays and opens the context menu', async () => {
    const actions = props();
    const { container } = render(TrackTable, { props: actions });
    const rows = container.querySelectorAll('.row');

    await fireEvent.click(rows[1]);
    expect(actions.onselect).toHaveBeenCalledWith(1);

    await fireEvent.dblClick(rows[1]);
    expect(actions.onplay).toHaveBeenCalledWith(actions.tracks[1]);

    await fireEvent.contextMenu(rows[0]);
    expect(screen.getByRole('menu')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Add to queue' }));
    expect(actions.onqueue).toHaveBeenCalledWith(actions.tracks[0]);
  });

  it('disables retry unless the download failed', async () => {
    const actions = props();
    const { container } = render(TrackTable, { props: actions });
    await fireEvent.contextMenu(container.querySelectorAll('.row')[0]);
    expect(screen.getByRole('menuitem', { name: 'Retry download' })).toBeDisabled();
  });

  it('retries a failed download from the badge', async () => {
    const actions = props([makeTrack('v1')]);
    actions.downloadStatus = () => downloadStatus('failed');
    const { container } = render(TrackTable, { props: actions });

    const retry = screen.getByRole('button', { name: 'Retry download for Track v1' });
    expect(retry).toHaveTextContent('failed');
    await fireEvent.click(retry);
    expect(actions.onretry).toHaveBeenCalledWith('v1');
    expect(container.querySelector('.row')).not.toBeNull();
  });

  it('windows large playlists', () => {
    const tracks = Array.from({ length: 300 }, (_, index) => makeTrack(`v${index}`));
    const { container } = render(TrackTable, {
      props: { ...props(tracks), selection: 0 },
    });
    expect(container.querySelectorAll('.row').length).toBeLessThan(60);
  });
});
