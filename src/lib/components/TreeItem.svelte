<script lang="ts">
  import type { SvelteSet } from "svelte/reactivity";
  import { canHaveChild, displayName, visibleChildren, worstSeverity, type TreeModel } from "$lib/tree";
  import PendingRow from "./PendingRow.svelte";
  import TreeItem from "./TreeItem.svelte";

  let {
    model,
    id,
    depth = 0,
    showArchived,
    selectedId,
    collapsed,
    onSelect,
    pending = null,
    onAddChild,
    onContextMenu,
  }: {
    model: TreeModel;
    id: string;
    depth?: number;
    showArchived: boolean;
    selectedId: string | null;
    collapsed: SvelteSet<string>;
    onSelect: (id: string) => void;
    /** 新規作成中の目的（ここに追加されることを示す仮の行を出す）。 */
    pending?: { parent: string | null; name: string } | null;
    onAddChild: (parent: string) => void;
    onContextMenu: (id: string, x: number, y: number) => void;
  } = $props();

  const node = $derived(model.byId.get(id));
  const kids = $derived(visibleChildren(model, id, showArchived));
  const isCollapsed = $derived(collapsed.has(id));
  const pendingHere = $derived(pending?.parent === id);
  const severity = $derived(node ? worstSeverity(node.diagnostics.filter((d) => d.severity !== "info")) : null);

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (collapsed.has(id)) collapsed.delete(id);
    else collapsed.add(id);
  }
</script>

{#if node}
  <li role="treeitem" aria-selected={selectedId === id} aria-expanded={kids.length > 0 ? !isCollapsed : undefined}>
    <div
      class="row"
      class:selected={selectedId === id}
      class:archived={node.effective === "archived"}
      class:inherited={node.effective === "inherited_inactive"}
      style:padding-left="{8 + depth * 16}px"
      oncontextmenu={(e) => {
        e.preventDefault();
        onContextMenu(id, e.clientX, e.clientY);
      }}
      role="presentation"
    >
      {#if kids.length > 0}
        <button class="chevron ghost" onclick={toggle} aria-label={isCollapsed ? "展開" : "折り畳む"}>
          <svg viewBox="0 0 16 16" width="12" height="12" class:open={!isCollapsed} aria-hidden="true">
            <path d="M6 3l5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.8" />
          </svg>
        </button>
      {:else}
        <span class="chevron-space"></span>
      {/if}
      <button class="label ghost" onclick={() => onSelect(id)} title={node.path}>
        <span class="name">{displayName(node)}</span>
        {#if node.effective === "archived"}<span class="badge archived">アーカイブ</span>{/if}
        {#if node.effective === "inherited_inactive"}<span class="badge inherited">親がアーカイブ</span>{/if}
        {#if severity}<span class="dot {severity}" title={severity === "error" ? "エラーあり" : "警告あり"}></span>{/if}
      </button>
      {#if canHaveChild(node)}
        <button
          class="add ghost"
          onclick={() => onAddChild(id)}
          title="「{displayName(node)}」の子目的を追加"
          aria-label="「{displayName(node)}」の子目的を追加"
        >
          <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
        </button>
      {/if}
    </div>
    {#if (kids.length > 0 && !isCollapsed) || pendingHere}
      <ul role="group">
        {#each isCollapsed ? [] : kids as child (child)}
          <TreeItem {model} id={child} depth={depth + 1} {showArchived} {selectedId} {collapsed} {onSelect} {pending} {onAddChild} {onContextMenu} />
        {/each}
        {#if pendingHere && pending}
          <PendingRow name={pending.name} depth={depth + 1} />
        {/if}
      </ul>
    {/if}
  </li>
{/if}

<style>
  li {
    list-style: none;
  }
  ul {
    margin: 0;
    padding: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius);
    padding-right: 6px;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .row.selected {
    background: var(--accent-soft);
  }
  .row.selected .name {
    color: var(--accent);
    font-weight: 600;
  }
  .row.archived .name,
  .row.inherited .name {
    color: var(--text-3);
  }
  .row.archived .name {
    text-decoration: line-through;
    text-decoration-color: var(--border);
  }
  .chevron {
    padding: 4px;
    display: grid;
    place-items: center;
    color: var(--text-3);
  }
  .chevron svg {
    transition: transform 0.12s;
  }
  .chevron svg.open {
    transform: rotate(90deg);
  }
  .chevron-space {
    width: 22px;
    flex: none;
  }
  .label {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 4px;
    text-align: left;
  }
  .label:hover:not(:disabled) {
    background: transparent;
  }
  .add {
    flex: none;
    padding: 3px;
    display: grid;
    place-items: center;
    color: var(--text-3);
    opacity: 0;
  }
  .row:hover .add,
  .row.selected .add,
  .add:focus-visible {
    opacity: 1;
  }
  .add:hover {
    color: var(--accent);
    background: var(--surface);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
  }
  .dot.error {
    background: var(--error);
  }
  .dot.warning {
    background: var(--warning);
  }
</style>
