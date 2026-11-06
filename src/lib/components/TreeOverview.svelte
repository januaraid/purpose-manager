<script lang="ts">
  import { canHaveChild, displayName, layoutTree, worstSeverity, type TreeModel } from "$lib/tree";

  let {
    model,
    roots,
    showArchived,
    selectedId,
    onOpen,
    onAddChild,
    onContextMenu,
  }: {
    model: TreeModel;
    roots: string[];
    showArchived: boolean;
    selectedId: string | null;
    onOpen: (id: string) => void;
    onAddChild: (parent: string) => void;
    onContextMenu: (id: string, x: number, y: number) => void;
  } = $props();

  const NODE_W = 200;
  const NODE_H = 70;
  const COL_GAP = 56;
  const ROW_GAP = 14;
  const PAD = 24;

  const layout = $derived(layoutTree(model, roots, showArchived));
  const pos = $derived(
    new Map(
      layout.nodes.map((n) => [
        n.id,
        { x: PAD + n.depth * (NODE_W + COL_GAP), y: PAD + n.row * (NODE_H + ROW_GAP) },
      ]),
    ),
  );
  const width = $derived(PAD * 2 + Math.max(1, layout.depths) * (NODE_W + COL_GAP) - COL_GAP);
  const height = $derived(PAD * 2 + Math.max(1, layout.rows) * (NODE_H + ROW_GAP) - ROW_GAP);

  let zoom = $state(1);
  let scroller = $state<HTMLDivElement>();
  const clampZoom = (z: number) => Math.min(2, Math.max(0.25, Math.round(z * 100) / 100));

  function setZoom(z: number, cx?: number, cy?: number) {
    if (!scroller) {
      zoom = clampZoom(z);
      return;
    }
    const rect = scroller.getBoundingClientRect();
    const px = cx ?? rect.width / 2;
    const py = cy ?? rect.height / 2;
    const ux = (scroller.scrollLeft + px) / zoom;
    const uy = (scroller.scrollTop + py) / zoom;
    zoom = clampZoom(z);
    requestAnimationFrame(() => {
      if (!scroller) return;
      scroller.scrollLeft = ux * zoom - px;
      scroller.scrollTop = uy * zoom - py;
    });
  }

  function fit() {
    if (!scroller) return;
    const r = scroller.getBoundingClientRect();
    zoom = clampZoom(Math.min(1, r.width / width, r.height / height));
    scroller.scrollTo(0, 0);
  }

  function onWheel(e: WheelEvent) {
    if (!(e.ctrlKey || e.metaKey) || !scroller) return;
    e.preventDefault();
    const r = scroller.getBoundingClientRect();
    setZoom(zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1), e.clientX - r.left, e.clientY - r.top);
  }

  // 背景をドラッグして移動する
  let drag: { x: number; y: number; left: number; top: number; moved: boolean } | null = null;
  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !scroller || (e.target as Element).closest(".cell")) return;
    drag = { x: e.clientX, y: e.clientY, left: scroller.scrollLeft, top: scroller.scrollTop, moved: false };
    scroller.setPointerCapture(e.pointerId);
  }
  function onPointerMove(e: PointerEvent) {
    if (!drag || !scroller) return;
    scroller.scrollLeft = drag.left - (e.clientX - drag.x);
    scroller.scrollTop = drag.top - (e.clientY - drag.y);
  }
  function onPointerUp() {
    drag = null;
  }

  function edgePath(from: string, to: string): string {
    const a = pos.get(from);
    const b = pos.get(to);
    if (!a || !b) return "";
    const x1 = a.x + NODE_W;
    const y1 = a.y + NODE_H / 2;
    const x2 = b.x;
    const y2 = b.y + NODE_H / 2;
    const mx = (x1 + x2) / 2;
    return `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`;
  }

  // 選択中のノードが見えるようにする
  $effect(() => {
    const p = selectedId ? pos.get(selectedId) : undefined;
    if (!p || !scroller) return;
    const r = scroller.getBoundingClientRect();
    const x = p.x * zoom;
    const y = p.y * zoom;
    const inView =
      x >= scroller.scrollLeft && x + NODE_W * zoom <= scroller.scrollLeft + r.width &&
      y >= scroller.scrollTop && y + NODE_H * zoom <= scroller.scrollTop + r.height;
    if (!inView) scroller.scrollTo({ left: x - r.width / 3, top: y - r.height / 2 + (NODE_H * zoom) / 2 });
  });
</script>

<div class="overview">
  <div class="zoombar">
    <button onclick={() => setZoom(zoom / 1.2)} aria-label="縮小">−</button>
    <span class="pct mono">{Math.round(zoom * 100)}%</span>
    <button onclick={() => setZoom(zoom * 1.2)} aria-label="拡大">＋</button>
    <button onclick={fit}>全体を表示</button>
    <button onclick={() => setZoom(1)}>100%</button>
    <span class="muted hint">Ctrl+ホイールで拡大縮小・背景をドラッグで移動・クリックで詳細・右クリックでメニュー</span>
  </div>

  {#if layout.nodes.length === 0}
    <p class="muted">表示できる目的がありません。</p>
  {:else}
    <div
      class="canvas"
      bind:this={scroller}
      onwheel={onWheel}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
      role="presentation"
    >
      <!-- foreignObject は WebKit（macOS）で拡大縮小・配置が崩れるため、線だけをSVGで描き、
           ノードは通常のHTMLとして重ね、全体を CSS の transform で拡大縮小する -->
      <div class="stage" style:width="{width * zoom}px" style:height="{height * zoom}px">
      <div class="plane" style:width="{width}px" style:height="{height}px" style:transform="scale({zoom})" role="tree" aria-label="目的ツリーの全体図">
        <svg class="edges" width={width} height={height} viewBox="0 0 {width} {height}" aria-hidden="true">
          {#each layout.edges as e (e.from + ">" + e.to)}
            {@const child = model.byId.get(e.to)}
            <path d={edgePath(e.from, e.to)} class:inactive={child?.effective !== "active"} />
          {/each}
        </svg>
        {#each layout.nodes as n (n.id)}
          {@const node = model.byId.get(n.id)}
          {@const p = pos.get(n.id)}
          {#if node && p}
            {@const sev = worstSeverity(node.diagnostics.filter((d) => d.severity !== "info"))}
              <div
                class="cell"
                style:left="{p.x}px"
                style:top="{p.y}px"
                style:width="{NODE_W}px"
                style:height="{NODE_H}px"
                role="presentation"
                oncontextmenu={(e) => {
                  e.preventDefault();
                  onContextMenu(n.id, e.clientX, e.clientY);
                }}
              >
              <button
                class="node {node.effective}"
                class:selected={selectedId === n.id}
                onclick={() => onOpen(n.id)}
                title="{displayName(node)}{node.fields ? '\n' + node.fields.purpose : ''}"
                role="treeitem"
                aria-selected={selectedId === n.id}
              >
                <span class="name">{displayName(node)}</span>
                <span class="meta">
                  <span class="mono id">{node.id}</span>
                  {#if node.effective === "archived"}<span class="badge archived">アーカイブ</span>{/if}
                  {#if node.effective === "inherited_inactive"}<span class="badge inherited">親がアーカイブ</span>{/if}
                  {#if node.fields?.valid_until}<span class="badge">〜{node.fields.valid_until}</span>{/if}
                  {#if sev}<span class="dot {sev}"></span>{/if}
                </span>
              </button>
              {#if canHaveChild(node)}
                <button
                  class="add"
                  onclick={() => onAddChild(n.id)}
                  title="「{displayName(node)}」の子目的を追加"
                  aria-label="「{displayName(node)}」の子目的を追加"
                >
                  <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
                </button>
              {/if}
              </div>
          {/if}
        {/each}
      </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    gap: 8px;
  }
  .zoombar {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .zoombar button {
    padding: 2px 10px;
    font-size: 13px;
  }
  .pct {
    width: 44px;
    text-align: center;
    font-size: 12px;
  }
  .hint {
    font-size: 12px;
    margin-left: 8px;
  }
  .canvas {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background-color: var(--surface);
    background-image: radial-gradient(var(--border) 1px, transparent 1px);
    background-size: 18px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: grab;
  }
  .canvas:active {
    cursor: grabbing;
  }
  .stage {
    position: relative;
    overflow: hidden;
  }
  .plane {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: 0 0;
  }
  svg.edges {
    position: absolute;
    top: 0;
    left: 0;
    display: block;
    overflow: visible;
  }
  .edges path {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.5;
    opacity: 0.55;
  }
  .edges path.inactive {
    stroke: var(--archived);
    stroke-dasharray: 4 3;
  }
  .cell {
    position: absolute;
  }
  .add {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 22px;
    height: 22px;
    padding: 0;
    display: grid;
    place-items: center;
    color: var(--text-3);
    background: var(--surface);
    opacity: 0;
  }
  .cell:hover .add,
  .add:focus-visible {
    opacity: 1;
  }
  .add:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  .node {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    text-align: left;
    padding: 6px 28px 6px 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius);
    cursor: pointer;
    overflow: hidden;
  }
  .node:hover {
    background: var(--surface-2);
  }
  .node.selected {
    background: var(--accent-soft);
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .node.archived,
  .node.inherited_inactive {
    border-left-color: var(--archived);
    border-style: dashed;
    border-left-style: solid;
  }
  .node.archived .name,
  .node.inherited_inactive .name {
    color: var(--text-3);
  }
  .node.unknown {
    border-left-color: var(--warning);
  }
  .name {
    font-weight: 600;
    font-size: 13px;
    line-height: 1.3;
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow-wrap: anywhere;
  }
  .meta {
    display: flex;
    gap: 4px;
    align-items: center;
    min-width: 0;
    overflow: hidden;
  }
  .id {
    font-size: 10.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 0 1 auto;
  }
  .meta .badge {
    font-size: 10px;
    padding: 2px 4px;
    flex: none;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
    margin-left: auto;
  }
  .dot.error {
    background: var(--error);
  }
  .dot.warning {
    background: var(--warning);
  }
</style>
