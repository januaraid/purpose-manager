<script lang="ts">
  /** 「目的・目標・手段の違い」を示すヘルプ。？ボタンを押すとポップオーバーで開く。 */
  let { align = "left", label = null }: { align?: "left" | "right"; label?: string | null } = $props();

  let open = $state(false);
  let root = $state<HTMLSpanElement>();
  const panelId = `purpose-guide-${Math.random().toString(36).slice(2, 8)}`;

  function onWindowPointer(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
  function onWindowKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      open = false;
      root?.querySelector("button")?.focus();
    }
  }
</script>

<svelte:window onpointerdown={onWindowPointer} onkeydown={onWindowKey} />

<span class="guide" bind:this={root}>
  <button
    type="button"
    class="trigger"
    class:with-label={label !== null}
    aria-expanded={open}
    aria-controls={panelId}
    title="目的・目標・手段の違い"
    aria-label={label ?? "目的・目標・手段の違い"}
    onclick={(e) => {
      e.preventDefault();
      e.stopPropagation();
      open = !open;
    }}
  >
    <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
      <circle cx="8" cy="8" r="6.4" fill="none" stroke="currentColor" stroke-width="1.4" />
      <path d="M6.2 6.3a1.9 1.9 0 1 1 2.6 1.75c-.5.2-.8.6-.8 1.1v.35" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      <circle cx="8" cy="11.4" r="0.85" fill="currentColor" />
    </svg>
    {#if label}<span>{label}</span>{/if}
  </button>

  {#if open}
    <div class="panel {align}" id={panelId} role="dialog" aria-label="目的・目標・手段の違い">
      <dl>
        <dt>目的（Why）</dt>
        <dd>何のために取り組むのか。実現したい価値や状態を示し、判断の基準となる。</dd>
        <dt>目標（What）</dt>
        <dd>何を達成するのか。目的に向けた具体的な到達点を示し、達成を判定できる。</dd>
        <dt>手段（How）</dt>
        <dd>どうやって達成するのか。目標に到達するための方法や行動を示す。</dd>
      </dl>
      <p class="summary"><strong>まとめ</strong>目的は判断するために、目標は達成するために、手段は実行するためにある。</p>
    </div>
  {/if}
</span>

<style>
  .guide {
    position: relative;
    display: inline-flex;
    vertical-align: middle;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px;
    border: none;
    background: none;
    color: var(--text-3);
    font-size: 12px;
    font-weight: 400;
    line-height: 1;
    border-radius: 50%;
  }
  .trigger.with-label {
    border-radius: var(--radius);
    padding: 3px 6px;
  }
  .trigger:hover,
  .trigger[aria-expanded="true"] {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .panel {
    position: absolute;
    top: calc(100% + 6px);
    z-index: 35;
    width: min(340px, 80vw);
    padding: 12px 14px;
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.16);
    font-size: 13px;
    font-weight: 400;
    line-height: 1.6;
    text-align: left;
    white-space: normal;
  }
  .panel.left {
    left: 0;
  }
  .panel.right {
    right: 0;
  }
  dl {
    margin: 0;
    display: grid;
    gap: 2px;
  }
  dt {
    font-weight: 600;
    color: var(--accent);
  }
  dt:not(:first-child) {
    margin-top: 8px;
  }
  dd {
    margin: 0;
    color: var(--text-2);
  }
  .summary {
    margin: 10px 0 0;
    padding-top: 8px;
    border-top: 1px solid var(--border);
    color: var(--text-2);
  }
  .summary strong {
    display: block;
    color: var(--text);
  }
</style>
