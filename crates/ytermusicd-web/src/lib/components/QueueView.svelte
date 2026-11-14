<script lang="ts">
  import type { AppStore } from '../store.svelte';
  import { downloadLabel, durationSeconds, formatDuration } from '../api';

  let { store }: { store: AppStore } = $props();

  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);
  let confirmClear = $state(false);

  function drop(index: number): void {
    if (dragFrom !== null) {
      void store.reorderQueue(dragFrom, index);
      dragFrom = null;
      dragOver = null;
    }
  }
</script>

<section class="queue" aria-label="Queue">
  <header>
    <div>
      <h2>Queue</h2>
      <span class="muted">
        {store.queue?.items.length ?? 0} items · {store.status?.downloads.queued ?? 0} queued · {store.status?.downloads.active ?? 0} downloading
      </span>
    </div>
    {#if (store.queue?.items.length ?? 0) > 0}
      {#if confirmClear}
        <div class="confirm">
          <span>Clear the queue?</span>
          <button
            class="danger"
            onclick={() => {
              void store.clearQueue();
              confirmClear = false;
            }}
          >
            Clear
          </button>
          <button onclick={() => (confirmClear = false)}>Cancel</button>
        </div>
      {:else}
        <button onclick={() => (confirmClear = true)}>Clear queue</button>
      {/if}
    {/if}
  </header>

  {#if !store.queue || store.queue.items.length === 0}
    <div class="empty-state">
      <p>The queue is empty.</p>
      <p class="muted">Open a playlist and press Enter, or press x on a search result.</p>
    </div>
  {:else}
    <div class="rows" role="listbox" aria-label="Queue items">
      {#each store.queue.items as item, index (index)}
        {@const current = store.queue?.current === index}
        {@const failed = item.download.state === 'failed'}
        <div
          class="row"
          class:current
          class:selected={index === store.selection}
          class:dragging={dragFrom === index}
          class:drag-over={dragOver === index && dragFrom !== index}
          role="option"
          aria-selected={index === store.selection}
          tabindex={index === store.selection ? 0 : -1}
          draggable="true"
          onclick={() => store.select(index)}
          ondblclick={() => store.playIndex(index)}
          onkeydown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') event.preventDefault();
          }}
          ondragstart={(event) => {
            dragFrom = index;
            event.dataTransfer?.setData('text/plain', String(index));
            if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move';
          }}
          ondragover={(event) => {
            event.preventDefault();
            dragOver = index;
          }}
          ondragleave={() => {
            if (dragOver === index) dragOver = null;
          }}
          ondrop={(event) => {
            event.preventDefault();
            drop(index);
          }}
          ondragend={() => {
            dragFrom = null;
            dragOver = null;
          }}
        >
          <span class="handle" aria-hidden="true">⠿</span>
          <span class="index">
            {#if current}
              <span class="playing" aria-label="Now playing">▶</span>
            {:else}
              {index + 1}
            {/if}
          </span>
          <span class="track">
            <span class="title" title={item.track.title}>{item.track.title}</span>
            <span class="artist" title={item.track.artist}>{item.track.artist}</span>
          </span>
          <span class="time">{formatDuration(durationSeconds(item.track.duration))}</span>
          {#if failed}
            <button
              class="badge failed"
              title="Retry download"
              aria-label={`Retry download for ${item.track.title}`}
              onclick={(event) => {
                event.stopPropagation();
                store.retryDownload(item.track.video_id);
              }}
            >
              {downloadLabel(item.download)} ↻
            </button>
          {:else}
            <span class="badge {item.download.state}">{downloadLabel(item.download)}</span>
          {/if}
          <span class="actions">
            {#if !current}
              <button
                class="small"
                title="Play next"
                aria-label={`Play ${item.track.title} next`}
                onclick={(event) => {
                  event.stopPropagation();
                  void store.playNext(index);
                }}
              >
                ↑ next
              </button>
            {/if}
            <button
              class="small danger"
              title="Remove from queue"
              aria-label={`Remove ${item.track.title} from the queue`}
              onclick={(event) => {
                event.stopPropagation();
                void store.removeQueueIndex(index);
              }}
            >
              ×
            </button>
          </span>
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .queue {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem 1.25rem 0.8rem;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0 0 0.2rem;
    font-size: 1.15rem;
    color: var(--fg-bright);
  }

  header .muted {
    font-size: 0.78rem;
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.8rem;
  }

  .danger {
    border-color: var(--danger);
    color: var(--danger);
  }

  .rows {
    overflow-y: auto;
    min-height: 0;
  }

  .row {
    display: grid;
    grid-template-columns: 1.2rem 2rem minmax(0, 1fr) 4rem 5.5rem auto;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 45%, transparent);
    font-size: 0.84rem;
  }

  .row:hover {
    background: color-mix(in srgb, var(--selection) 55%, transparent);
  }

  .row.current {
    background: var(--selection);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .row.selected:not(.current) {
    background: color-mix(in srgb, var(--selection) 70%, transparent);
    box-shadow: inset 3px 0 0 var(--accent-2);
  }

  .row.dragging {
    opacity: 0.45;
  }

  .row.drag-over {
    box-shadow: inset 0 2px 0 var(--accent-2);
  }

  .handle {
    color: var(--fg-muted);
    cursor: grab;
    font-size: 0.9rem;
  }

  .index {
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
  }

  .playing {
    color: var(--accent);
    font-size: 0.7rem;
  }

  .track {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    color: var(--fg-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .artist {
    color: var(--fg-muted);
    font-size: 0.75rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .time {
    color: var(--fg-muted);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .badge {
    font-size: 0.68rem;
    justify-self: start;
  }

  .actions {
    display: flex;
    gap: 0.3rem;
    justify-content: flex-end;
  }

  .small {
    padding: 0.1rem 0.45rem;
    font-size: 0.72rem;
  }

  @media (max-width: 760px) {
    .row {
      grid-template-columns: 1rem 1.6rem minmax(0, 1fr) 4.8rem auto;
    }

    .time {
      display: none;
    }
  }
</style>
