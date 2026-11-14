<script lang="ts">
  import { tick } from 'svelte';
  import type { AppStore } from '../store.svelte';
  import type { SearchEntry } from '../types';

  let { store }: { store: AppStore } = $props();

  let input: HTMLInputElement | undefined = $state();
  let dialog: HTMLElement | undefined = $state();
  let active = $state(0);

  function buildEntries(): SearchEntry[] {
    return [
      ...(store.searchResults?.tracks.map((track) => ({ kind: 'track' as const, track })) ?? []),
      ...(store.searchResults?.playlists.map((playlist) => ({ kind: 'playlist' as const, playlist })) ??
        []),
    ];
  }

  const entries = $derived(buildEntries());

  $effect(() => {
    if (store.searchOpen) {
      void tick().then(() => input?.focus());
    }
  });

  $effect(() => {
    void store.searchResults;
    active = 0;
  });

  function select(index: number): void {
    if (entries.length === 0) return;
    active = (index + entries.length) % entries.length;
    document.getElementById(`search-entry-${active}`)?.scrollIntoView?.({ block: 'nearest' });
  }

  function activate(entry: SearchEntry): void {
    if (entry.kind === 'track') {
      void store.playTrack(entry.track);
      store.closeSearch();
    } else {
      void store.openPlaylist(entry.playlist.id, entry.playlist.name);
      store.closeSearch();
    }
  }

  function queue(entry: SearchEntry): void {
    if (entry.kind === 'track') void store.queueTrack(entry.track);
  }

  function onInputKeydown(event: KeyboardEvent): void {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault();
        select(active + 1);
        break;
      case 'ArrowUp':
        event.preventDefault();
        select(active - 1);
        break;
      case 'Enter':
        event.preventDefault();
        if (entries[active]) activate(entries[active]);
        break;
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        store.closeSearch();
        break;
      case 'Tab':
        trapFocus(event);
        break;
    }
  }

  function trapFocus(event: KeyboardEvent): void {
    if (!dialog) return;
    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        'input, button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    );
    if (focusable.length === 0) return;
    event.preventDefault();
    const index = focusable.indexOf(document.activeElement as HTMLElement);
    const next = event.shiftKey
      ? focusable[(index - 1 + focusable.length) % focusable.length]
      : focusable[(index + 1) % focusable.length];
    next?.focus();
  }
</script>

{#if store.searchOpen}
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) store.closeSearch();
    }}
  >
    <div
      class="palette"
      role="dialog"
      aria-modal="true"
      aria-label="Search YouTube Music"
      tabindex="-1"
      bind:this={dialog}
    >
      <div class="search-row">
        <span aria-hidden="true">⌕</span>
        <input
          bind:this={input}
          type="search"
          value={store.searchQuery}
          placeholder="Search tracks and playlists…"
          aria-label="Search"
          aria-controls="search-results"
          aria-activedescendant={entries.length ? `search-entry-${active}` : undefined}
          oninput={(event) => store.setSearchQuery(event.currentTarget.value)}
          onkeydown={onInputKeydown}
        />
        <kbd>Esc</kbd>
      </div>

      <div id="search-results" class="results" role="listbox" aria-label="Search results">
        {#if store.searchLoading}
          {#each [1, 2, 3] as item (item)}
            <div class="skeleton result-skeleton"></div>
          {/each}
        {:else if store.searchQuery.trim() === ''}
          <p class="hint muted">Type to search. Use ↑ and ↓ to select. Press Enter to play.</p>
        {:else if !store.searchResults || entries.length === 0}
          <p class="hint muted">No results for “{store.searchQuery}”.</p>
        {:else}
          {#each entries as entry, index (entry.kind === 'track' ? entry.track.video_id : entry.playlist.id)}
            <div class="result" class:active={index === active}>
              <button
                id={`search-entry-${index}`}
                class="result-main"
                role="option"
                aria-selected={index === active}
                onmouseenter={() => (active = index)}
                onclick={() => activate(entry)}
              >
                <span class="kind" aria-hidden="true">
                  {entry.kind === 'track' ? '♪' : '▤'}
                </span>
                <span class="text">
                  <span class="title">
                    {entry.kind === 'track' ? entry.track.title : entry.playlist.name}
                  </span>
                  <span class="sub muted">
                    {entry.kind === 'track' ? entry.track.artist : entry.playlist.subtitle}
                  </span>
                </span>
                {#if entry.kind === 'track'}
                  <span class="duration">{entry.track.duration}</span>
                {/if}
              </button>
              {#if entry.kind === 'track'}
                <button
                  class="queue"
                  title="Add to queue"
                  aria-label={`Add ${entry.track.title} to the queue`}
                  onclick={() => queue(entry)}
                >
                  + queue
                </button>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 45;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 8vh 1rem 2rem;
    background: rgb(4 2 10 / 70%);
    backdrop-filter: blur(3px);
  }

  .palette {
    width: min(44rem, 100%);
    max-height: min(36rem, 82vh);
    display: flex;
    flex-direction: column;
    border: 1px solid var(--accent-2);
    border-radius: 10px;
    background: var(--bg-raised);
    box-shadow:
      0 24px 80px rgb(0 0 0 / 65%),
      0 0 24px color-mix(in srgb, var(--accent-2) 20%, transparent);
    overflow: hidden;
  }

  .search-row {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.7rem 0.85rem;
    border-bottom: 1px solid var(--border);
  }

  .search-row > span {
    color: var(--accent-2);
    font-size: 1.1rem;
  }

  .search-row input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 0.95rem;
    padding: 0.2rem;
  }

  .search-row input:focus-visible {
    outline: none;
  }

  kbd {
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 0.3rem;
    font-size: 0.7rem;
    color: var(--fg-muted);
  }

  .results {
    overflow-y: auto;
    min-height: 0;
    padding: 0.35rem;
  }

  .result {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    border-radius: 6px;
  }

  .result.active {
    background: var(--selection);
  }

  .result-main {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    border: none;
    background: transparent;
    text-align: left;
    padding: 0.5rem 0.6rem;
    min-width: 0;
  }

  .kind {
    color: var(--accent-3);
    width: 1rem;
    text-align: center;
    flex: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .title {
    color: var(--fg-bright);
    font-size: 0.86rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 0.72rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .duration {
    font-size: 0.72rem;
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    flex: none;
  }

  .queue {
    margin-right: 0.4rem;
    padding: 0.1rem 0.5rem;
    font-size: 0.7rem;
    border-color: var(--border);
    color: var(--fg-muted);
  }

  .queue:hover {
    border-color: var(--accent-2);
    color: var(--accent-2);
  }

  .hint {
    padding: 1rem;
    text-align: center;
    font-size: 0.82rem;
  }

  .result-skeleton {
    height: 2.5rem;
    margin: 0.3rem;
  }
</style>
