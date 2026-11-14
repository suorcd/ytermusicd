<script lang="ts">
  import type { Toast } from '../store.svelte';

  let {
    store,
  }: {
    store: { toasts: Toast[]; dismissToast: (id: number) => void };
  } = $props();
</script>

<div class="toasts" aria-live="polite" aria-atomic="false">
  {#each store.toasts as toast (toast.id)}
    <div
      class="toast {toast.severity}"
      role={toast.severity === 'error' ? 'alert' : 'status'}
    >
      <span class="message">{toast.message}</span>
      {#if toast.action}
        <button
          class="action"
          onclick={() => {
            toast.action?.run();
            store.dismissToast(toast.id);
          }}
        >
          {toast.action.label}
        </button>
      {/if}
      <button
        class="dismiss"
        aria-label="Dismiss notification"
        onclick={() => store.dismissToast(toast.id)}
      >
        ×
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 1rem;
    bottom: 6.5rem;
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: min(28rem, calc(100vw - 2rem));
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 0.7rem;
    border: 1px solid var(--border);
    border-left-width: 4px;
    border-radius: 8px;
    background: var(--bg-raised);
    box-shadow: 0 8px 28px rgb(0 0 0 / 45%);
    pointer-events: auto;
    font-size: 0.85rem;
  }

  .toast.info {
    border-left-color: var(--accent-2);
  }
  .toast.success {
    border-left-color: var(--success);
  }
  .toast.warning {
    border-left-color: var(--warning);
  }
  .toast.error {
    border-left-color: var(--danger);
  }

  .message {
    flex: 1;
    overflow-wrap: anywhere;
  }

  .action {
    padding: 0.2rem 0.5rem;
    font-size: 0.78rem;
    border-color: var(--accent-2);
    color: var(--accent-2);
  }

  .dismiss {
    padding: 0 0.35rem;
    border: none;
    background: transparent;
    color: var(--fg-muted);
    font-size: 1.1rem;
    line-height: 1;
  }

  .dismiss:hover {
    color: var(--danger);
  }
</style>
