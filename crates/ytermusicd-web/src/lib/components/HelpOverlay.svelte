<script lang="ts">
  let { open, onclose }: { open: boolean; onclose: () => void } = $props();
</script>

{#if open}
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="overlay"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) onclose();
    }}
  >
    <div
      class="help"
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard and mouse help"
      tabindex="-1"
    >
      <header>
        <h2>Keys and mouse</h2>
        <button aria-label="Close help" onclick={onclose}>×</button>
      </header>
      <div class="columns">
        <section>
          <h3>Move</h3>
          <dl>
            <dt><kbd>j</kbd> / <kbd>↓</kbd></dt><dd>Next row</dd>
            <dt><kbd>k</kbd> / <kbd>↑</kbd></dt><dd>Previous row</dd>
            <dt><kbd>g</kbd><kbd>g</kbd></dt><dd>First row</dd>
            <dt><kbd>G</kbd></dt><dd>Last row</dd>
            <dt><kbd>Ctrl</kbd>+<kbd>d</kbd></dt><dd>Half page down</dd>
            <dt><kbd>Ctrl</kbd>+<kbd>u</kbd></dt><dd>Half page up</dd>
          </dl>
        </section>
        <section>
          <h3>Actions</h3>
          <dl>
            <dt><kbd>Enter</kbd></dt><dd>Play the selected item</dd>
            <dt><kbd>Space</kbd></dt><dd>Play or pause</dd>
            <dt><kbd>x</kbd></dt><dd>Queue the selected track</dd>
            <dt><kbd>d</kbd><kbd>d</kbd></dt><dd>Remove the selected queue item</dd>
            <dt><kbd>/</kbd> or <kbd>Ctrl</kbd>+<kbd>k</kbd></dt><dd>Open search</dd>
            <dt><kbd>?</kbd></dt><dd>Open this help</dd>
            <dt><kbd>Esc</kbd></dt><dd>Close a panel</dd>
          </dl>
        </section>
        <section>
          <h3>Mouse</h3>
          <dl>
            <dt>Click</dt><dd>Select a row</dd>
            <dt>Double click</dt><dd>Play a track</dd>
            <dt>Right click</dt><dd>Open the track menu</dd>
            <dt>Drag</dt><dd>Move a queue item</dd>
          </dl>
        </section>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 1rem;
    background: rgb(4 2 10 / 78%);
    backdrop-filter: blur(3px);
  }

  .help {
    width: min(52rem, 100%);
    max-height: min(38rem, 90vh);
    overflow: auto;
    border: 1px solid var(--accent-3);
    border-radius: 12px;
    background: var(--bg-raised);
    box-shadow: 0 24px 80px rgb(0 0 0 / 65%);
    padding: 1rem 1.25rem 1.4rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    margin-bottom: 1rem;
  }

  h2 {
    margin: 0 0 0.6rem;
    font-size: 1.1rem;
    color: var(--fg-bright);
  }

  h3 {
    margin: 0 0 0.5rem;
    color: var(--accent-2);
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  header button {
    border: none;
    background: transparent;
    font-size: 1.3rem;
    color: var(--fg-muted);
  }

  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: 1.4rem;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.35rem 0.8rem;
    margin: 0;
    font-size: 0.84rem;
  }

  dt {
    color: var(--fg-bright);
  }

  dd {
    margin: 0;
    color: var(--fg-muted);
  }

  kbd {
    display: inline-block;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 0 0.3rem;
    background: var(--bg-sunken);
    font-size: 0.78rem;
  }
</style>
