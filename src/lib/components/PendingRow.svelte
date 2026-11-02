<script lang="ts">
  let { name, depth }: { name: string; depth: number } = $props();

  let el = $state<HTMLLIElement>();
  $effect(() => {
    el?.scrollIntoView({ block: "nearest" });
  });
</script>

<!-- 新規作成中の目的が、保存後にツリーのどこへ入るかを示す仮の行 -->
<li bind:this={el} class="pending" role="treeitem" aria-selected="true" aria-label="作成中の目的">
  <div class="row" style:margin-left="{8 + depth * 16 + 22}px">
    <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
    <span class="name" class:empty={name.trim() === ""}>{name.trim() === "" ? "新しい目的" : name}</span>
    <span class="tag">作成中</span>
  </div>
</li>

<style>
  li {
    list-style: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 2px 6px 2px 0;
    padding: 3px 8px;
    border: 1px dashed var(--accent);
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .name.empty {
    font-style: italic;
    font-weight: 400;
    opacity: 0.8;
  }
  .tag {
    flex: none;
    font-size: 10.5px;
    opacity: 0.85;
  }
</style>
