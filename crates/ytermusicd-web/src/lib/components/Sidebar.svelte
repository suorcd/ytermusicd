<script lang="ts">
  import type { AppStore } from '../store.svelte';

  let { store }: { store: AppStore } = $props();

  const libraryActive = $derived(store.view.kind === 'library');
  const queueActive = $derived(store.view.kind === 'queue');

  function openLiked(): void {
    store.openLibrary();
    store.setFilter('liked');
  }
</script>

<aside class="sidebar" aria-label="Library navigation">
  <div class="brand">
    <span class="mark" aria-hidden="true">♪</span>
    <span class="name">ytermusicd</span>
    <span class="connection" class:online={store.connection === 'online'} title={store.connection}>
      ●
    </span>
  </div>

  <nav>
    <button
      class="nav-item"
      class:active={libraryActive}
      aria-current={libraryActive ? 'page' : undefined}
      onclick={() => store.openLibrary()}
    >
      <span aria-hidden="true">▤</span> Library
    </button>
    <button class="nav-item" onclick={openLiked}>
      <span aria-hidden="true">♥</span> Liked Music
    </button>
    <button
      class="nav-item"
      class:active={queueActive}
      aria-current={queueActive ? 'page' : undefined}
      onclick={() => store.openQueue()}
    >
      <span aria-hidden="true">☰</span> Queue
      <span class="count">{store.queue?.items.length ?? 0}</span>
    </button>
  </nav>

  <div class="playlists">
    <div class="section-title">
      <span>Playlists</span>
      <button
        class="refresh"
        aria-label="Reload playlists"
        title="Reload playlists"
        onclick={() => store.loadPlaylists()}
      >
        ↻
      </button>
    </div>
    <input
      type="search"
      value={store.filter}
      placeholder="Filter playlists"
      aria-label="Filter playlists"
      oninput={(event) => store.setFilter(event.currentTarget.value)}
    />

    <div class="list">
      {#if store.playlistsLoading && store.playlists.length === 0}
        {#each [1, 2, 3, 4] as item (item)}
          <div class="skeleton playlist-skeleton" style:width={`${70 + item * 5}%`}></div>
        {/each}
      {:else if store.filteredPlaylists.length === 0}
        <p class="muted empty">
          {store.filter ? 'No playlist matches the filter.' : 'No playlists loaded.'}
        </p>
      {:else}
        {#each store.filteredPlaylists as playlist (playlist.id)}
          {@const active = store.view.kind === 'playlist' && store.view.id === playlist.id}
          <button
            class="playlist"
            class:active
            aria-current={active ? 'page' : undefined}
            title={`${playlist.name} — ${playlist.subtitle}`}
            onclick={() => store.openPlaylist(playlist.id, playlist.name)}
          >
            <span class="playlist-name">{playlist.name}</span>
            <span class="playlist-sub">
              {playlist.subtitle}
              {#if active && store.playlist}
                · {store.playlist.tracks.length} tracks
              {/if}
            </span>
          </button>
        {/each}
      {/if}
    </div>
  </div>

  <div class="downloads" aria-label="Download status">
    <span>Downloads</span>
    <span class="muted">
      {store.status?.downloads.active ?? 0} active ·
      {store.status?.downloads.queued ?? 0} queued
      {#if (store.status?.downloads.failed ?? 0) > 0}
        · <span class="failed">{store.status?.downloads.failed} failed</span>
      {/if}
    </span>
  </div>
</aside>

<style>
  .sidebar {
    width: 16.5rem;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.85rem 1rem 0.7rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--fg-bright);
  }

  .mark {
    display: grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    border-radius: 6px;
    background: linear-gradient(135deg, var(--accent), var(--accent-3));
    color: var(--bg);
    font-size: 0.9rem;
  }

  .name {
    flex: 1;
  }

  .connection {
    color: var(--danger);
    font-size: 0.7rem;
  }

  .connection.online {
    color: var(--success);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.25rem 0.6rem 0.6rem;
    border-bottom: 1px solid var(--border);
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    border: none;
    background: transparent;
    text-align: left;
    padding: 0.42rem 0.55rem;
    border-radius: 6px;
    font-size: 0.84rem;
    color: var(--fg-muted);
  }

  .nav-item:hover {
    background: var(--selection);
    color: var(--fg-bright);
  }

  .nav-item.active {
    background: var(--selection);
    color: var(--accent-2);
    box-shadow: inset 2px 0 0 var(--accent-2);
  }

  .count {
    margin-left: auto;
    font-size: 0.72rem;
    color: var(--fg-muted);
  }

  .playlists {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 0.6rem;
    gap: 0.45rem;
  }

  .section-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--fg-muted);
    padding: 0 0.2rem;
  }

  .refresh {
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 0 0.2rem;
    font-size: 0.9rem;
  }

  .refresh:hover {
    color: var(--accent-2);
  }

  input[type='search'] {
    font-size: 0.78rem;
    padding: 0.35rem 0.55rem;
  }

  .list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-height: 0;
  }

  .playlist {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    border: none;
    background: transparent;
    text-align: left;
    padding: 0.35rem 0.5rem;
    border-radius: 6px;
    min-width: 0;
  }

  .playlist:hover {
    background: color-mix(in srgb, var(--selection) 70%, transparent);
  }

  .playlist.active {
    background: var(--selection);
  }

  .playlist-name {
    font-size: 0.82rem;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .playlist.active .playlist-name {
    color: var(--accent-2);
  }

  .playlist-sub {
    font-size: 0.68rem;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .playlist-skeleton {
    height: 1.6rem;
    margin: 0.2rem 0.5rem;
  }

  .empty {
    font-size: 0.76rem;
    padding: 0.4rem 0.5rem;
  }

  .downloads {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.55rem 0.8rem;
    border-top: 1px solid var(--border);
    font-size: 0.68rem;
    color: var(--fg);
  }

  .downloads .muted {
    font-size: 0.68rem;
  }

  .failed {
    color: var(--danger);
  }

  @media (max-width: 800px) {
    .sidebar {
      width: 12.5rem;
    }
  }
</style>
