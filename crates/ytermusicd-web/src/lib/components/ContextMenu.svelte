<script lang="ts">
  import type { MenuItem } from '../types';

  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  } = $props();

  let menu: HTMLDivElement | undefined = $state();

  $effect(() => {
    const close = (event: Event) => {
      if (menu && event.target instanceof Node && menu.contains(event.target)) return;
      onclose();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onclose();
    };
    window.addEventListener('pointerdown', close);
    window.addEventListener('keydown', key);
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('keydown', key);
    };
  });
</script>

<div
  class="menu"
  role="menu"
  bind:this={menu}
  style:left={`${x}px`}
  style:top={`${y}px`}
>
  {#each items as item}
    <button
      role="menuitem"
      disabled={item.disabled}
      onclick={() => {
        item.action();
        onclose();
      }}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 60;
    min-width: 10rem;
    display: flex;
    flex-direction: column;
    padding: 0.3rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-raised);
    box-shadow: 0 16px 48px rgb(0 0 0 / 55%);
  }

  button {
    border: none;
    background: transparent;
    text-align: left;
    padding: 0.45rem 0.6rem;
    border-radius: 5px;
    font-size: 0.84rem;
  }

  button:hover:not(:disabled) {
    background: var(--selection);
  }
</style>
