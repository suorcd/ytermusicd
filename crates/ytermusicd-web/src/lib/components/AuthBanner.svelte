<script lang="ts">
  import type { AuthStatus } from '@api/AuthStatus';

  let { auth }: { auth: AuthStatus | null } = $props();

  const visible = $derived(auth !== null && auth.state !== 'ok');
</script>

{#if visible && auth}
  <div class="auth-banner" role="alert">
    <strong>
      {auth.state === 'missing'
        ? 'YouTube Music sign-in is missing.'
        : 'YouTube Music sign-in is invalid.'}
    </strong>
    <span>
      Create or refresh <code>headers.txt</code> with the <code>Cookie</code> and
      <code>User-Agent</code> headers. Then restart the daemon.
    </span>
    {#if auth.source}
      <span class="path">Look for: <code>{auth.source}</code></span>
    {/if}
    {#if auth.message}
      <span class="path">{auth.message}</span>
    {/if}
    <a href="https://github.com/suorcd/ytermusicd#authentication" target="_blank" rel="noreferrer">
      Open the sign-in instructions
    </a>
  </div>
{/if}

<style>
  .auth-banner {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.45rem 0.8rem;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--bg-raised));
    color: var(--fg);
    font-size: 0.82rem;
  }

  strong {
    color: var(--danger);
  }

  code {
    color: var(--accent-2);
  }

  .path {
    color: var(--fg-muted);
  }

  a {
    color: var(--accent-2);
  }
</style>
