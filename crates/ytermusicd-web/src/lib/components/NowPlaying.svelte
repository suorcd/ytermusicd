<script lang="ts">
  import type { AppStore } from '../store.svelte';
  import { downloadLabel, durationSeconds, formatDuration } from '../api';

  let { store }: { store: AppStore } = $props();

  let scrub = $state<number | null>(null);

  const playback = $derived(store.status?.playback ?? null);
  const track = $derived(playback?.current ?? null);
  const duration = $derived(track ? durationSeconds(track.duration) : 0);
  const position = $derived(scrub ?? playback?.position_secs ?? 0);
  const download = $derived(track ? store.downloadStatusFor(track) : null);
  const isPlaying = $derived(playback?.state === 'playing');

  function commitSeek(value: number): void {
    scrub = null;
    void store.seek(value);
  }
</script>

<footer class="now-playing" aria-label="Now playing">
  <div class="track">
    {#if track}
      <div class="title" title={track.title}>{track.title}</div>
      <div class="meta" title={`${track.artist} — ${track.album}`}>
        {track.artist}
        {#if track.album}<span class="muted"> · {track.album}</span>{/if}
      </div>
      {#if download && download.state === 'failed'}
        <button class="retry" onclick={() => store.retryDownload(track.video_id)}>
          Download failed — retry
        </button>
      {:else if download}
        <span class="badge {download.state}">{downloadLabel(download)}</span>
      {/if}
    {:else}
      <div class="title muted">Nothing playing</div>
      <div class="meta muted">Select a track and press Enter.</div>
    {/if}
  </div>

  <div class="center">
    <div class="controls">
      <button
        class="icon"
        class:active={playback?.shuffle}
        aria-label="Shuffle"
        aria-pressed={playback?.shuffle ?? false}
        onclick={() => store.toggleShuffle()}
      >
        ⤨
      </button>
      <button class="icon" aria-label="Previous track" onclick={() => store.previous()}>
        ⏮
      </button>
      <button
        class="icon primary"
        aria-label={isPlaying ? 'Pause' : 'Play'}
        onclick={() => store.togglePlayback()}
      >
        {isPlaying ? '⏸' : '▶'}
      </button>
      <button class="icon" aria-label="Next track" onclick={() => store.next()}>
        ⏭
      </button>
      <button
        class="icon repeat"
        class:active={playback && playback.repeat !== 'off'}
        aria-label={`Repeat mode: ${playback?.repeat ?? 'off'}`}
        onclick={() => store.cycleRepeat()}
      >
        ↻{#if playback?.repeat === 'one'}<sup>1</sup>{/if}
      </button>
    </div>
    <div class="progress">
      <span class="time">{formatDuration(position)}</span>
      <input
        type="range"
        min="0"
        max={Math.max(duration, 1)}
        step="1"
        value={position}
        disabled={!track || duration === 0}
        aria-label="Seek"
        oninput={(event) => {
          scrub = Number(event.currentTarget.value);
        }}
        onchange={(event) => commitSeek(Number(event.currentTarget.value))}
      />
      <span class="time">{formatDuration(duration)}</span>
    </div>
  </div>

  <div class="volume">
    <span aria-hidden="true">🔊</span>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={store.volume}
      aria-label="Volume"
      oninput={(event) => {
        store.volume = Number(event.currentTarget.value);
      }}
      onchange={(event) => store.setVolume(Number(event.currentTarget.value))}
    />
    <span class="percent">{store.volume}%</span>
  </div>
</footer>

<style>
  .now-playing {
    display: grid;
    grid-template-columns: minmax(12rem, 1fr) minmax(18rem, 1.6fr) minmax(9rem, 0.8fr);
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 1rem;
    border-top: 1px solid var(--border);
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--accent-3) 8%, transparent), transparent),
      var(--bg-raised);
    flex: none;
    min-height: 5.2rem;
  }

  .track {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    align-items: flex-start;
  }

  .title {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg-bright);
    font-weight: 600;
    font-size: 0.92rem;
  }

  .meta {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.76rem;
    color: var(--fg-muted);
  }

  .center {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
  }

  .icon {
    border: none;
    background: transparent;
    color: var(--fg-muted);
    font-size: 1rem;
    padding: 0.25rem 0.45rem;
    border-radius: 50%;
  }

  .icon:hover {
    color: var(--fg-bright);
  }

  .icon.primary {
    color: var(--bg);
    background: var(--accent-2);
    width: 2.2rem;
    height: 2.2rem;
    display: grid;
    place-items: center;
  }

  .icon.primary:hover {
    background: var(--fg-bright);
  }

  .icon.active {
    color: var(--accent);
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .progress input[type='range'] {
    flex: 1;
    height: 4px;
  }

  .time {
    font-size: 0.7rem;
    color: var(--fg-muted);
    font-variant-numeric: tabular-nums;
    min-width: 2.4rem;
    text-align: center;
  }

  .volume {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.45rem;
  }

  .volume input[type='range'] {
    width: 6rem;
    height: 4px;
  }

  .percent {
    font-size: 0.72rem;
    color: var(--fg-muted);
    min-width: 2.4rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .retry {
    padding: 0.08rem 0.45rem;
    font-size: 0.7rem;
    border-color: var(--danger);
    color: var(--danger);
    background: transparent;
  }

  @media (max-width: 860px) {
    .now-playing {
      grid-template-columns: 1fr;
      gap: 0.35rem;
      padding: 0.5rem 0.75rem;
    }

    .volume {
      justify-content: flex-start;
    }
  }
</style>
