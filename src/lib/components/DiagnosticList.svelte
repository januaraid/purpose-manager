<script lang="ts">
  import type { Diagnostic } from "$lib/types";

  let { items, showFile = false }: { items: Diagnostic[]; showFile?: boolean } = $props();

  const label = { error: "エラー", warning: "警告", info: "情報" } as const;
  const order = { error: 0, warning: 1, info: 2 } as const;
  const sorted = $derived([...items].sort((a, b) => order[a.severity] - order[b.severity]));
</script>

{#if sorted.length > 0}
  <ul class="diags">
    {#each sorted as d, i (i)}
      <li class={d.severity}>
        <span class="badge {d.severity}">{label[d.severity]}</span>
        <span class="msg">
          {d.message}
          {#if showFile && d.file}<span class="muted mono"> — {d.file}</span>{/if}
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .diags {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    font-size: 13px;
  }
  .msg {
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
