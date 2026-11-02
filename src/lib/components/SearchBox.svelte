<script lang="ts">
  import { displayName, pathLabel, searchPurposes, type TreeModel } from "$lib/tree";

  let { model, onOpen }: { model: TreeModel; onOpen: (id: string) => void } = $props();

  let query = $state("");
  let open = $state(false);
  let active = $state(0);
  const hits = $derived(searchPurposes(model, query));
  $effect(() => {
    query;
    active = 0;
  });

  const fieldLabel = { name: "", id: "ID", purpose: "目的", principles: "判断原則", rationale: "背景" } as const;

  function choose(id: string) {
    onOpen(id);
    query = "";
    open = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" && hits.length > 0) {
      e.preventDefault();
      open = true;
      active = (active + 1) % hits.length;
    } else if (e.key === "ArrowUp" && hits.length > 0) {
      e.preventDefault();
      active = (active - 1 + hits.length) % hits.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const h = hits[active];
      if (h) choose(h.id);
    } else if (e.key === "Escape") {
      if (query !== "") query = "";
      open = false;
    }
  }
</script>

<div class="search">
  <input
    type="search"
    bind:value={query}
    placeholder="名前・ID・本文で探す"
    spellcheck="false"
    role="combobox"
    aria-expanded={open && query.trim() !== ""}
    aria-controls="search-results"
    aria-autocomplete="list"
    aria-activedescendant={open && hits[active] ? `search-hit-${active}` : undefined}
    oninput={() => (open = true)}
    onfocus={() => (open = true)}
    onblur={() => setTimeout(() => (open = false), 120)}
    onkeydown={onKeydown}
  />
  {#if open && query.trim() !== ""}
    <ul class="results" id="search-results" role="listbox">
      {#each hits as h, i (h.id)}
        {@const n = model.byId.get(h.id)}
        <li
          id="search-hit-{i}"
          role="option"
          aria-selected={i === active}
          class:active={i === active}
          class:inactive={n?.effective !== "active"}
          onpointerdown={(e) => {
            e.preventDefault();
            choose(h.id);
          }}
          onpointerenter={() => (active = i)}
        >
          <span class="name">{displayName(n, h.id)}</span>
          {#if n && n.ancestors.length > 0}
            <span class="where">{pathLabel(model, n).split(" › ").slice(0, -1).join(" › ")}</span>
          {/if}
          {#if h.snippet}
            <span class="snippet"><em>{fieldLabel[h.field]}</em>{h.snippet}</span>
          {:else if h.field === "id"}
            <span class="snippet mono">{h.id}</span>
          {/if}
        </li>
      {:else}
        <li class="none">見つかりません</li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .search {
    position: relative;
  }
  input {
    width: 100%;
    font-size: 13px;
  }
  .results {
    position: absolute;
    z-index: 30;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.16);
    max-height: 60vh;
    overflow: auto;
  }
  li {
    display: grid;
    gap: 1px;
    padding: 5px 8px;
    border-radius: calc(var(--radius) - 2px);
    cursor: pointer;
    min-width: 0;
  }
  li.active {
    background: var(--accent-soft);
  }
  li.none {
    color: var(--text-3);
    font-size: 12px;
    cursor: default;
  }
  .name {
    font-size: 13px;
    font-weight: 600;
  }
  li.active .name {
    color: var(--accent);
  }
  li.inactive .name {
    color: var(--text-3);
  }
  .where,
  .snippet {
    font-size: 11px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .snippet em {
    font-style: normal;
    font-weight: 600;
    margin-right: 4px;
  }
  .name,
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
