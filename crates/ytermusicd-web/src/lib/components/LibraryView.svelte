<script lang="ts">
  import type { AppStore } from '../store.svelte';
  import TrackTable from './TrackTable.svelte';

  let { store }: { store: AppStore } = $props();

  const playlistView = $derived(store.view.kind === 'playlist' ? store.view : null);
  const tracks = $derived(playlistView ? (store.playlist?.tracks ?? []) : []);
  const name = $derived(store.playlist?.playlist.name ?? playlistView?.name ?? 'Playlist');
</script>

{#if store.view.kind === 'library'}
  <section class="library" aria-label="Library">
    <header class="hero">
      <div>
        <p class="eyebrow">YouTube Music</p>
        <h1>Library</h1>
        <p class="muted">
          {store.filteredPlaylists.length} playlists
          {#if store.filter}
            matching “{store.filter}”
          {/if}
        </p>
      </div>
      <button class="refresh" onclick={() => store.loadPlaylists()}>Reload</button>
    </header>

    {#if store.playlistsLoading && store.playlists.length === 0}
      <div class="cards">
        {#each [1, 2, 3, 4, 5, 6] as item (item)}
          <div class="skeleton card-skeleton"></div>
        {/each}
      </div>
    {:else if store.filteredPlaylists.length === 0}
      <div class="empty-state">
        <p>No playlists found.</p>
        {#if store.filter}
          <button onclick={() => store.setFilter('')}>Clear filter</button>
        {/if}
      </div>
    {:else}
      <div class="cards">
        {#each store.filteredPlaylists as playlist (playlist.id)}
          <article class="card">
            <div class="art" aria-hidden="true">
              <span>{playlist.name.slice(0, 1).toUpperCase()}</span>
            </div>
            <div class="info">
              <h2 title={playlist.name}>{playlist.name}</h2>
              <p class="muted" title={playlist.subtitle}>{playlist.subtitle}</p>
            </div>
            <div class="actions">
              <button onclick={() => store.openPlaylist(playlist.id, playlist.name)}>
                Open
              </button>
              <button class="accent" onclick={() => store.playPlaylist(playlist.id)}>
                Play
              </button>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
{:else if playlistView}
  <section class="playlist-view" aria-label={`Playlist ${name}`}>
    <header class="playlist-header">
      <div class="art large" aria-hidden="true">{name.slice(0, 1).toUpperCase()}</div>
      <div class="heading">
        <p class="eyebrow">Playlist</p>
        <h1>{name}</h1>
        <p class="muted">
          {store.playlist?.playlist.subtitle ?? ''}
          {#if store.playlist}
            · {store.playlist.tracks.length} tracks
          {/if}
        </p>
      </div>
      <div class="header-actions">
        <button
          class="accent"
          disabled={!store.playlist || store.playlist.tracks.length === 0}
          onclick={() => store.playPlaylist(playlistView.id)}
        >
          Play all
        </button>
        <button
          disabled={!store.selectedTrack}
          onclick={() => store.queueSelected()}
        >
          Queue selected
        </button>
      </div>
    </header>

    <div class="table-wrap">
      {#if store.playlistLoading}
        <div class="loading">
          {#each [1, 2, 3, 4, 5, 6, 7, 8] as item (item)}
            <div class="skeleton row-skeleton"></div>
          {/each}
        </div>
      {:else if !store.playlist}
        <div class="empty-state">
          <p>This playlist did not load.</p>
          {#if store.error}<p class="muted">{store.error}</p>{/if}
          <button onclick={() => store.openPlaylist(playlistView.id, playlistView.name)}>
            Try again
          </button>
        </div>
      {:else}
        <TrackTable
          {tracks}
          selection={store.selection}
          currentTrack={store.currentTrack}
          downloadStatus={(track) => store.downloadStatusFor(track)}
          onselect={(index) => store.select(index)}
          onplay={(track) => store.playTrack(track)}
          onqueue={(track) => store.queueTrack(track)}
          onretry={(videoId) => store.retryDownload(videoId)}
          emptyMessage="This playlist has no tracks."
        />
      {/if}
    </div>
  </section>
{/if}

<style>
  .library,
  .playlist-view {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  .hero,
  .playlist-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1.1rem 1.25rem;
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  .hero {
    justify-content: space-between;
    background:
      radial-gradient(circle at 0% 0%, color-mix(in srgb, var(--accent-3) 16%, transparent), transparent 60%);
  }

  .eyebrow {
    margin: 0 0 0.15rem;
    font-size: 0.68rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--accent-2);
  }

  h1 {
    margin: 0;
    font-size: 1.5rem;
    color: var(--fg-bright);
  }

  .hero .muted,
  .playlist-header .muted {
    margin: 0.2rem 0 0;
    font-size: 0.78rem;
  }

  .refresh {
    align-self: flex-start;
    font-size: 0.78rem;
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
    gap: 0.8rem;
    padding: 1rem 1.25rem;
    overflow-y: auto;
    min-height: 0;
  }

  .card {
    display: grid;
    grid-template-columns: 3.2rem minmax(0, 1fr);
    grid-template-rows: auto auto;
    gap: 0.35rem 0.7rem;
    align-items: center;
    padding: 0.75rem;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: color-mix(in srgb, var(--bg-raised) 80%, transparent);
  }

  .card:hover {
    border-color: color-mix(in srgb, var(--accent-2) 45%, var(--border));
  }

  .art {
    display: grid;
    place-items: center;
    width: 3.2rem;
    height: 3.2rem;
    border-radius: 8px;
    background:
      linear-gradient(135deg, color-mix(in srgb, var(--accent) 70%, #000), var(--accent-3));
    color: var(--bg);
    font-size: 1.4rem;
    font-weight: 800;
    grid-row: span 2;
  }

  .art.large {
    width: 4.6rem;
    height: 4.6rem;
    font-size: 2rem;
    flex: none;
  }

  .info {
    min-width: 0;
  }

  .info h2 {
    margin: 0;
    font-size: 0.88rem;
    color: var(--fg-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .info p {
    margin: 0.1rem 0 0;
    font-size: 0.7rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: 0.35rem;
    grid-column: 2;
  }

  .actions button,
  .header-actions button {
    padding: 0.18rem 0.55rem;
    font-size: 0.74rem;
  }

  .accent {
    border-color: var(--accent);
    color: var(--accent);
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  .heading h1 {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-actions {
    display: flex;
    gap: 0.4rem;
    flex: none;
  }

  .table-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .loading {
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .row-skeleton {
    height: 2.1rem;
  }

  .card-skeleton {
    height: 5rem;
  }

  @media (max-width: 700px) {
    .playlist-header {
      flex-wrap: wrap;
    }
  }
</style>
