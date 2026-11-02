<script lang="ts" module>
  export interface MenuItem {
    label: string;
    action: () => void;
    disabled?: boolean;
    title?: string;
  }
</script>

<script lang="ts">
  import { tick } from "svelte";

  let {
    x,
    y,
    items,
    onClose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onClose: () => void;
  } = $props();

  let menu = $state<HTMLDivElement>();
  let left = $state(0);
  let top = $state(0);

  // 画面の端からはみ出さないように位置を調整し、最初の項目にフォーカスする
  $effect(() => {
    const px = x;
    const py = y;
    tick().then(() => {
      if (!menu) return;
      const r = menu.getBoundingClientRect();
      left = Math.max(4, Math.min(px, window.innerWidth - r.width - 4));
      top = Math.max(4, Math.min(py, window.innerHeight - r.height - 4));
      menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    });
  });

  function run(item: MenuItem) {
    if (item.disabled) return;
    onClose();
    item.action();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const buttons = [...(menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    if (buttons.length === 0) return;
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "ArrowDown" ? (i + 1) % buttons.length : (i - 1 + buttons.length) % buttons.length;
    buttons[next].focus();
  }
</script>

<svelte:window onblur={onClose} onresize={onClose} />

<div
  class="backdrop"
  role="presentation"
  onpointerdown={onClose}
  oncontextmenu={(e) => {
    e.preventDefault();
    onClose();
  }}
></div>
<div class="menu" role="menu" tabindex="-1" bind:this={menu} style:left="{left || x}px" style:top="{top || y}px" onkeydown={onKeydown}>
  {#each items as item (item.label)}
    <button role="menuitem" disabled={item.disabled} title={item.title} onclick={() => run(item)}>{item.label}</button>
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  .menu {
    position: fixed;
    z-index: 41;
    min-width: 180px;
    padding: 4px;
    display: grid;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.16);
  }
  .menu button {
    border: none;
    background: none;
    text-align: left;
    padding: 6px 10px;
    font-size: 13px;
    border-radius: calc(var(--radius) - 2px);
  }
  .menu button:hover:not(:disabled),
  .menu button:focus-visible {
    background: var(--accent-soft);
    color: var(--accent);
    outline: none;
  }
</style>
