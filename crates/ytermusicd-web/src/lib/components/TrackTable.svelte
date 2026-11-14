<script lang="ts">
  import type { DownloadStatus } from '@api/DownloadStatus';
  import type { Track } from '@api/Track';
  import { downloadLabel, formatDuration, durationSeconds } from '../api';
  import type { MenuItem } from '../types';
  import ContextMenu from './ContextMenu.svelte';

  let {
    tracks,
    selection,
    currentTrack,
    downloadStatus,
    onselect,
    onplay,
    onqueue,
    onretry,
    emptyMessage = 'No tracks here.',
  }: {
    tracks: Track[];
    selection: number;
    currentTrack: Track | null;
    downloadStatus: (track: Track) => DownloadStatus;
    onselect: (index: number) => void;
    onplay: (track: Track) => void;
    onqueue: (track: Track) => void;
    onretry: (videoId: string) => void;
    emptyMessage?: string;
  } = $props();

  const ROW_HEIGHT = 44;
  const OVERSCAN = 5;

  let scrollEl: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(480);
  let menu = $state<{ x: number; y: number; index: number; track: Track } | null>(null);

  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  const end = $derived(
    Math.min(tracks.length, Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN),
  );
  const visible = $derived(tracks.slice(start, end));

  $effect(() => {
    if (!scrollEl) return;
    viewportHeight = scrollEl.clientHeight || 480;
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => {
      viewportHeight = scrollEl?.clientHeight || 480;
    });
    observer.observe(scrollEl);
    return () => observer.disconnect();
  });

  $effect(() => {
    if (!scrollEl || selection < 0) return;
    const top = selection * ROW_HEIGHT;
    const bottom = top + ROW_HEIGHT;
    if (top < scrollEl.scrollTop) {
      scrollEl.scrollTop = top;
    } else if (bottom > scrollEl.scrollTop + viewportHeight) {
      scrollEl.scrollTop = bottom - viewportHeight;
    }
  });

  function buildMenu(): MenuItem[] {
    if (!menu) return [];
    return [
      { label: 'Play now', action: () => onplay(menu!.track) },
      { label: 'Add to queue', action: () => onqueue(menu!.track) },
      {
        label: 'Retry download',
        disabled: downloadStatus(menu.track).state !== 'failed',
        action: () => onretry(menu!.track.video_id),
      },
    ];
  }

  const menuItems = $derived(buildMenu());

  function openMenu(event: MouseEvent, index: number, track: Track): void {
    event.preventDefault();
    onselect(index);
    menu = { x: event.clientX, y: event.clientY, index, track };
  }

  function openMenuAtButton(event: MouseEvent, index: number, track: Track): void {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onselect(index);
    menu = { x: rect.left, y: rect.bottom + 4, index, track };
  }

  function badgeClass(status: DownloadStatus): string {
    return status.state;
  }
</script>

{#if tracks.length === 0}
  <div class="empty-state">{emptyMessage}</div>
{:else}
  <div class="table-header" role="row">
    <span class="index">#</span>
    <span>Title</span>
    <span>Artist</span>
    <span class="optional">Album</span>
    <span class="time">Time</span>
    <span class="status">Status</span>
    <span></span>
  </div>
  <div
    class="scroll"
    bind:this={scrollEl}
    onscroll={() => {
      scrollTop = scrollEl?.scrollTop ?? 0;
    }}
  >
    <div class="spacer" style:height={`${tracks.length * ROW_HEIGHT}px`}>
      {#each visible as track, offset (start + offset)}
        {@const index = start + offset}
        {@const download = downloadStatus(track)}
        {@const playing = currentTrack?.video_id === track.video_id}
        <div
          class="row"
          class:selected={index === selection}
          class:playing
          role="row"
          tabindex={index === selection ? 0 : -1}
          aria-selected={index === selection}
          style:top={`${index * ROW_HEIGHT}px`}
          style:height={`${ROW_HEIGHT}px`}
          onclick={() => onselect(index)}
          ondblclick={() => onplay(track)}
          oncontextmenu={(event) => openMenu(event, index, track)}
          onkeydown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') event.preventDefault();
          }}
        >
          <span class="index">
            {#if playing}<span class="play-marker" aria-label="Now playing">▶</span>{/if}
            {index + 1}
          </span>
          <span class="title" title={track.title}>{track.title}</span>
          <span class="artist" title={track.artist}>{track.artist}</span>
          <span class="optional album" title={track.album}>{track.album}</span>
          <span class="time">{formatDuration(durationSeconds(track.duration))}</span>
          <span class="status">
            {#if download.state === 'failed'}
              <button
                class="badge failed"
                title="Retry download"
                aria-label={`Retry download for ${track.title}`}
                onclick={(event) => {
                  event.stopPropagation();
                  onretry(track.video_id);
                }}
              >
                {downloadLabel(download)} ↻
              </button>
            {:else}
              <span class="badge {badgeClass(download)}">{downloadLabel(download)}</span>
            {/if}
          </span>
          <button
            class="more"
            aria-label={`Actions for ${track.title}`}
            title="Actions"
            onclick={(event) => openMenuAtButton(event, index, track)}
          >
            ⋯
          </button>
        </div>
      {/each}
    </div>
  </div>
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menuItems} onclose={() => (menu = null)} />
{/if}

<style>
  .table-header,
  .row {
    display: grid;
    grid-template-columns: 3rem minmax(0, 2.2fr) minmax(0, 1.4fr) minmax(0, 1.2fr) 4rem 5.5rem 2rem;
    align-items: center;
    gap: 0.6rem;
    padding: 0 0.75rem;
  }

  .table-header {
    height: 2rem;
    border-bottom: 1px solid var(--border);
    color: var(--fg-muted);
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    flex: none;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .spacer {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
    font-size: 0.84rem;
    cursor: default;
  }

  .row:hover {
    background: color-mix(in srgb, var(--selection) 55%, transparent);
  }

  .row.selected {
    background: var(--selection);
    box-shadow: inset 3px 0 0 var(--accent-2);
  }

  .row.playing .title,
  .row.playing .index {
    color: var(--accent);
  }

  .index,
  .time,
  .status {
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .title {
    color: var(--fg-bright);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .artist,
  .album {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg-muted);
  }

  .time {
    text-align: right;
  }

  .play-marker {
    color: var(--accent);
    margin-right: 0.2rem;
    font-size: 0.65rem;
  }

  .badge {
    font-size: 0.68rem;
    padding: 0.05rem 0.4rem;
  }

  button.badge {
    background: transparent;
  }

  .more {
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 0 0.2rem;
    opacity: 0;
  }

  .row:hover .more,
  .row.selected .more,
  .more:focus-visible {
    opacity: 1;
  }

  @media (max-width: 900px) {
    .table-header,
    .row {
      grid-template-columns: 2.5rem minmax(0, 2fr) minmax(0, 1.2fr) 3.5rem 4.8rem 2rem;
    }

    .optional {
      display: none;
    }
  }
</style>
