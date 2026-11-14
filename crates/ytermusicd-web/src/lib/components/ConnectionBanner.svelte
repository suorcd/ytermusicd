<script lang="ts">
  import type { ConnectionState } from '../sse';

  let {
    connection,
    error,
    onretry,
  }: {
    connection: ConnectionState;
    error: string | null;
    onretry: () => void;
  } = $props();

  const label = $derived(
    connection === 'connecting'
      ? 'Connecting to the daemon…'
      : connection === 'reconnecting'
        ? 'Connection lost. Reconnecting…'
        : 'The daemon is offline.',
  );
</script>

{#if connection !== 'online'}
  <div class="banner" class:offline={connection === 'offline'} role="status">
    <span class="dot" aria-hidden="true"></span>
    <span class="label">{label}</span>
    {#if error && connection === 'offline'}
      <span class="detail">{error}</span>
    {/if}
    <button onclick={onretry}>Retry now</button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.45rem 1rem;
    border-bottom: 1px solid var(--warning);
    background: color-mix(in srgb, var(--warning) 12%, var(--bg-raised));
    color: var(--fg-bright);
    font-size: 0.82rem;
  }

  .banner.offline {
    border-bottom-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 14%, var(--bg-raised));
  }

  .dot {
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 50%;
    background: var(--warning);
    animation: pulse 1.2s infinite alternate;
  }

  .offline .dot {
    background: var(--danger);
    animation: none;
  }

  .label {
    font-weight: 600;
  }

  .detail {
    color: var(--fg-muted);
  }

  button {
    margin-left: auto;
    padding: 0.2rem 0.6rem;
    font-size: 0.78rem;
  }

  @keyframes pulse {
    from {
      opacity: 0.35;
    }
    to {
      opacity: 1;
    }
  }
</style>
