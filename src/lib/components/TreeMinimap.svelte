<script lang="ts">
  import { displayName, layoutTree, type TreeModel } from "$lib/tree";

  let {
    model,
    roots,
    showArchived,
    currentId,
    pendingParent = undefined,
    onOpen,
  }: {
    model: TreeModel;
    roots: string[];
    showArchived: boolean;
    /** 編集中の目的。 */
    currentId: string | null;
    /** 新規作成中なら親のID（ルートは null）。作成中でなければ undefined。 */
    pendingParent?: string | null;
    onOpen: (id: string) => void;
  } = $props();

  const W = 124;
  const H = 26;
  const COL = 18;
  /** これより小さくすると文字が読めないので、収まらない分はスクロールにする。 */
  const MIN_SCALE = 0.8;
  const ROW = 6;
  const PAD = 6;

  const layout = $derived(layoutTree(model, roots, showArchived));
  const pos = $derived(
    new Map(layout.nodes.map((n) => [n.id, { x: PAD + n.depth * (W + COL), y: PAD + n.row * (H + ROW) }])),
  );
  const width = $derived(PAD * 2 + Math.max(1, layout.depths) * (W + COL) - COL);
  const height = $derived(PAD * 2 + Math.max(1, layout.rows) * (H + ROW) - ROW);

  // 編集中の目的（作成中なら親）とその祖先を強調する
  const focus = $derived(pendingParent !== undefined ? pendingParent : currentId);
  const path = $derived.by(() => {
    const n = focus ? model.byId.get(focus) : undefined;
    return new Set(n ? [...n.ancestors, n.id] : []);
  });

  function edge(from: string, to: string): string {
    const a = pos.get(from);
    const b = pos.get(to);
    if (!a || !b) return "";
    const x1 = a.x + W;
    const y1 = a.y + H / 2;
    const x2 = b.x;
    const y2 = b.y + H / 2;
    const mx = (x1 + x2) / 2;
    return `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`;
  }

  // 作成中の仮ノードの位置（親の右、子の並びの下）
  const pendingBox = $derived.by(() => {
    if (pendingParent === undefined) return null;
    if (pendingParent === null) return { x: PAD, y: height - PAD + ROW, from: null };
    const p = pos.get(pendingParent);
    if (!p) return null;
    const kids = layout.edges.filter((e) => e.from === pendingParent).map((e) => pos.get(e.to)!.y);
    const y = kids.length > 0 ? Math.max(...kids) + H + ROW : p.y;
    return { x: p.x + W + COL, y, from: pendingParent };
  });
  const vbW = $derived(Math.max(width, pendingBox ? pendingBox.x + W + PAD : 0));
  const vbH = $derived(Math.max(height, pendingBox ? pendingBox.y + H + PAD : 0));

  let box = $state<HTMLDivElement>();
  let boxWidth = $state(0);
  const scale = $derived(boxWidth > 0 ? Math.min(1, Math.max(MIN_SCALE, boxWidth / vbW)) : MIN_SCALE);

  // 編集中の目的（作成中なら仮ノード）を中央付近に表示する
  $effect(() => {
    const target = pendingBox ?? (focus ? pos.get(focus) : undefined);
    const el = box;
    const k = scale;
    if (!target || !el) return;
    requestAnimationFrame(() => {
      const x = (target.x + W / 2) * k;
      const y = (target.y + H / 2) * k;
      el.scrollTo({ left: x - el.clientWidth / 2, top: y - el.clientHeight / 2 });
    });
  });
</script>

{#if layout.nodes.length > 0}
  <section class="minimap" aria-label="ツリー上の位置">
    <h2>位置</h2>
    <div class="box" bind:this={box} bind:clientWidth={boxWidth}>
      <svg viewBox="0 0 {vbW} {vbH}" width={vbW * scale} height={vbH * scale} role="img" aria-label="目的ツリーの縮小図">
        {#each layout.edges as e (e.from + ">" + e.to)}
          <path d={edge(e.from, e.to)} class="edge" class:on={path.has(e.from) && path.has(e.to)} />
        {/each}
        {#if pendingBox && pendingBox.from}
          {@const p = pos.get(pendingBox.from)!}
          {@const mx = (p.x + W + pendingBox.x) / 2}
          <path
            d="M{p.x + W},{p.y + H / 2} C{mx},{p.y + H / 2} {mx},{pendingBox.y + H / 2} {pendingBox.x},{pendingBox.y + H / 2}"
            class="edge on pending"
          />
        {/if}
        {#each layout.nodes as n (n.id)}
          {@const node = model.byId.get(n.id)}
          {@const p = pos.get(n.id)}
          {#if node && p}
            <foreignObject x={p.x} y={p.y} width={W} height={H}>
              <button
                class="node"
                class:path={path.has(n.id)}
                class:current={focus === n.id && pendingParent === undefined}
                class:inactive={node.effective !== "active"}
                onclick={() => onOpen(n.id)}
                title={displayName(node)}
              >{displayName(node)}</button>
            </foreignObject>
          {/if}
        {/each}
        {#if pendingBox}
          <foreignObject x={pendingBox.x} y={pendingBox.y} width={W} height={H}>
            <div class="node pending">作成中</div>
          </foreignObject>
        {/if}
      </svg>
    </div>
  </section>
{/if}

<style>
  .minimap {
    min-width: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 10px 12px 12px;
  }
  h2 {
    font-size: 13px;
    margin: 0 0 6px;
    color: var(--text-2);
  }
  .box {
    max-height: 240px;
    overflow: auto;
    overscroll-behavior: contain;
  }
  svg {
    display: block;
  }
  .edge {
    fill: none;
    stroke: var(--border);
    stroke-width: 1.5;
  }
  .edge.on {
    stroke: var(--accent);
    stroke-width: 2;
  }
  .edge.pending {
    stroke-dasharray: 4 3;
  }
  .node {
    width: 100%;
    height: 100%;
    display: block;
    padding: 0 6px;
    font-size: 11px;
    line-height: 24px;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-2);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 4px;
    cursor: pointer;
  }
  .node:hover {
    border-color: var(--accent);
  }
  .node.inactive {
    color: var(--text-3);
    border-style: dashed;
  }
  .node.path {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--surface);
  }
  .node.current {
    color: var(--accent-text);
    background: var(--accent);
    font-weight: 600;
  }
  .node.pending {
    color: var(--accent);
    background: var(--accent-soft);
    border: 1px dashed var(--accent);
    cursor: default;
    font-weight: 600;
  }
</style>
