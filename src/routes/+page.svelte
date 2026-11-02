<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "$lib/api";
  import { applyZoom, clampZoom, stepZoom } from "$lib/zoom";
  import { buildTree, canHaveChild, descendants, displayName, folderName, todayLocal, worstSeverity } from "$lib/tree";
  import type { LoadFailure, Settings, Snapshot } from "$lib/types";
  import ContextMenu, { type MenuItem } from "$lib/components/ContextMenu.svelte";
  import DiagnosticList from "$lib/components/DiagnosticList.svelte";
  import PurposeEditor from "$lib/components/PurposeEditor.svelte";
  import PendingRow from "$lib/components/PendingRow.svelte";
  import SearchBox from "$lib/components/SearchBox.svelte";
  import TreeItem from "$lib/components/TreeItem.svelte";
  import TreeMinimap from "$lib/components/TreeMinimap.svelte";
  import TreeOverview from "$lib/components/TreeOverview.svelte";

  // 表示の好み（端末ローカル。読めなくても既定値で動く）
  const PREFS_KEY = "purpose-manager.layout";
  const SIDEBAR_MIN = 220;
  const SIDEBAR_MAX = 640;
  function loadPrefs(): { width: number; open: boolean; view: "detail" | "overview"; zoom: number } {
    const d = { width: 300, open: true, view: "detail" as const, zoom: 1 };
    try {
      const p = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
      return {
        width: typeof p.width === "number" ? Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, p.width)) : d.width,
        open: typeof p.open === "boolean" ? p.open : d.open,
        view: p.view === "overview" ? "overview" : d.view,
        zoom: clampZoom(p.zoom),
      };
    } catch {
      return d;
    }
  }
  const prefs = loadPrefs();
  let sidebarWidth = $state(prefs.width);
  let sidebarOpen = $state(prefs.open);
  let view = $state<"detail" | "overview">(prefs.view);
  $effect(() => {
    const v = { width: sidebarWidth, open: sidebarOpen, view, zoom };
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(v));
    } catch {
      /* 保存できなくても動作に影響しない */
    }
  });

  let resizing = $state<{ x: number; width: number } | null>(null);
  function startResize(e: PointerEvent) {
    resizing = { x: e.clientX, width: sidebarWidth };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function onResize(e: PointerEvent) {
    if (!resizing) return;
    sidebarWidth = Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, resizing.width + e.clientX - resizing.x));
  }
  function endResize() {
    resizing = null;
  }
  function resizeByKey(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") sidebarWidth = Math.max(SIDEBAR_MIN, sidebarWidth - 20);
    if (e.key === "ArrowRight") sidebarWidth = Math.min(SIDEBAR_MAX, sidebarWidth + 20);
  }
  // 表示倍率（高精細な画面で文字が小さく感じるとき用）
  let zoom = $state(prefs.zoom);
  $effect(() => {
    applyZoom(zoom).catch(() => {
      /* Tauri の外（ブラウザでの確認時）では何もしない */
    });
  });
  function setUiZoom(z: number) {
    zoom = clampZoom(z);
    showToast(`表示倍率 ${Math.round(zoom * 100)}%`);
  }

  function onGlobalKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === "b") {
      e.preventDefault();
      sidebarOpen = !sidebarOpen;
    } else if (mod && (e.key === "+" || e.key === "=" || e.key === ";")) {
      e.preventDefault();
      setUiZoom(stepZoom(zoom, 1));
    } else if (mod && (e.key === "-" || e.key === "_")) {
      e.preventDefault();
      setUiZoom(stepZoom(zoom, -1));
    } else if (mod && !e.shiftKey && !e.altKey && (e.key === "1" || e.key === "2")) {
      e.preventDefault();
      view = e.key === "1" ? "detail" : "overview";
    } else if (mod && e.key === "0") {
      e.preventDefault();
      setUiZoom(1);
    }
  }

  let settings = $state<Settings>({ workspaces: [], last: null });
  let root = $state<string | null>(null);
  let snap = $state<Snapshot | null>(null);
  let loadError = $state<string | null>(null);
  let selectedId = $state<string | null>(null);
  /** 新規作成中なら親のID（ルートは null）。作成中でなければ undefined。 */
  let creating = $state<string | null | undefined>(undefined);
  /** 作成中の目的の親と表示名（フォームの入力に追従する）。ツリーに仮の行を出す。 */
  let pendingDraft = $state<{ parent: string | null; name: string } | null>(null);
  const pending = $derived(creating !== undefined ? (pendingDraft ?? { parent: creating, name: "" }) : null);
  $effect(() => {
    if (creating === undefined) pendingDraft = null;
  });
  // 仮の行が見えるように、親とその祖先を展開する
  $effect(() => {
    const p = pending?.parent;
    if (!p || !model) return;
    const n = model.byId.get(p);
    for (const a of [...(n?.ancestors ?? []), p]) if (collapsed.has(a)) collapsed.delete(a);
  });
  let showArchived = $state(false);
  let showWsDiags = $state(false);
  let editorDirty = $state(false);
  let toast = $state<string | null>(null);
  const collapsed = new SvelteSet<string>();

  const model = $derived(snap ? buildTree(snap) : null);
  const selected = $derived(model && selectedId ? (model.byId.get(selectedId) ?? null) : null);
  /** ツリーをこの目的から下だけに絞る（null は全体）。 */
  let focusId = $state<string | null>(null);
  const focusNode = $derived(model && focusId ? (model.byId.get(focusId) ?? null) : null);
  const focusSet = $derived(model && focusNode ? new Set([focusNode.id, ...descendants(model, focusNode.id)]) : null);
  const visibleRoots = $derived(
    !model
      ? []
      : focusNode
        ? [focusNode.id]
        : model.roots.filter((id) => showArchived || model.byId.get(id)?.effective !== "archived"),
  );
  // 絞り込みの外の目的を開いたり、外に目的を作り始めたら、全体表示に戻す
  $effect(() => {
    if (!focusId) return;
    if (model && !model.byId.has(focusId)) {
      focusId = null;
      return;
    }
    if (!focusSet) return;
    const outside = (id: string | null) => id === null || !focusSet.has(id);
    if ((selectedId && creating === undefined && outside(selectedId)) || (pending && outside(pending.parent))) {
      focusId = null;
    }
  });
  function focusOn(id: string | null) {
    focusId = id;
    if (id) collapsed.delete(id);
  }
  const wsSeverity = $derived(snap ? worstSeverity(snap.diagnostics) : null);
  const errorCount = $derived(
    snap ? snap.nodes.filter((n) => n.diagnostics.some((d) => d.severity === "error")).length : 0,
  );
  const editorKey = $derived(creating !== undefined ? `new:${creating ?? ""}` : `node:${selectedId ?? ""}`);

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showToast(msg: string) {
    toast = msg;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2600);
  }

  async function confirmDiscard(): Promise<boolean> {
    if (!editorDirty) return true;
    return ask("保存していない変更があります。破棄しますか？", {
      title: "未保存の変更",
      kind: "warning",
      okLabel: "破棄する",
      cancelLabel: "編集を続ける",
    });
  }

  async function persistSettings(next: Settings) {
    settings = next;
    try {
      await api.saveSettings(next);
    } catch (e) {
      showToast(`設定を保存できませんでした: ${errorMessage(e)}`);
    }
  }

  async function reload() {
    if (!root) return;
    try {
      snap = await api.loadWorkspace(root, todayLocal());
      loadError = null;
      if (selectedId && !snap.nodes.some((n) => n.id === selectedId)) selectedId = null;
    } catch (e) {
      loadError = errorMessage(e);
    }
  }

  async function openWorkspace(path: string): Promise<boolean> {
    if (!(await confirmDiscard())) return false;
    let next: Snapshot;
    try {
      next = await api.loadWorkspace(path, todayLocal());
    } catch (e) {
      const err = e as LoadFailure;
      if (err?.kind === "not_a_workspace") {
        const ok = await ask(
          `「${folderName(path)}」には purposes フォルダがありません。\nこのフォルダをWorkspaceとして初期化しますか？\n（workspace.yaml と purposes/ を作成します。既存のファイルは変更しません）`,
          { title: "Workspaceの初期化", okLabel: "初期化する", cancelLabel: "やめる" },
        );
        if (!ok) return false;
        try {
          await api.initWorkspace(path, folderName(path));
          next = await api.loadWorkspace(path, todayLocal());
        } catch (e2) {
          showToast(`初期化できませんでした: ${errorMessage(e2)}`);
          return false;
        }
      } else {
        root = path;
        snap = null;
        loadError = errorMessage(e);
        return false;
      }
    }
    root = path;
    snap = next;
    loadError = null;
    selectedId = null;
    creating = undefined;
    editorDirty = false;
    collapsed.clear();
    focusId = null;
    const workspaces = settings.workspaces.includes(path) ? settings.workspaces : [...settings.workspaces, path];
    await persistSettings({ workspaces, last: path });
    try {
      await api.watchWorkspace(path);
    } catch (e) {
      showToast(`ファイルの監視を開始できませんでした（手動で再読み込みしてください）: ${errorMessage(e)}`);
    }
    return true;
  }

  async function addWorkspace() {
    const dir = await open({ directory: true, multiple: false, title: "Workspaceのフォルダを選択" });
    if (typeof dir === "string") await openWorkspace(dir);
  }

  async function removeWorkspace(path: string) {
    const workspaces = settings.workspaces.filter((w) => w !== path);
    await persistSettings({ workspaces, last: settings.last === path ? null : settings.last });
    if (root === path) {
      root = null;
      snap = null;
      loadError = null;
      selectedId = null;
    }
  }

  async function select(id: string) {
    if (id === selectedId && creating === undefined) return;
    if (!(await confirmDiscard())) return;
    creating = undefined;
    selectedId = id;
  }

  /** 全体図から開く：選択して詳細表示へ切り替える。 */
  async function openFromOverview(id: string) {
    if (id !== selectedId || creating !== undefined) {
      if (!(await confirmDiscard())) return;
      creating = undefined;
      selectedId = id;
    }
    view = "detail";
  }

  async function startCreate(parent: string | null) {
    if (!(await confirmDiscard())) return;
    pendingDraft = null;
    creating = parent;
    view = "detail";
  }

  // 右クリックメニュー（サイドパネルのツリーと全体図で共通）
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  function openMenu(id: string, x: number, y: number) {
    const n = model?.byId.get(id);
    if (!n || !root) return;
    const parent = n.fields?.parent ?? null;
    const parentOk = parent === null || canHaveChild(model?.byId.get(parent));
    const ws = root;
    menu = {
      x,
      y,
      items: [
        { label: "開く", action: () => openFromOverview(id) },
        focusId === id
          ? { label: "全体の表示に戻す", action: () => focusOn(null) }
          : { label: "この目的から下だけを表示", action: () => focusOn(id) },
        {
          label: "子目的を追加",
          action: () => startCreate(id),
          disabled: !canHaveChild(n),
          title: canHaveChild(n) ? undefined : "エラーのあるファイルや、祖先を辿れない目的には追加できません",
        },
        {
          label: parent === null ? "ルートの目的を追加" : "同じ親の目的を追加",
          action: () => startCreate(parent),
          disabled: !n.fields || !parentOk,
        },
        {
          label: "CLAUDE.md 用にコピー",
          disabled: !n.fields,
          action: async () => {
            try {
              await navigator.clipboard.writeText(await api.claudeMdSnippet(ws, id));
              showToast("CLAUDE.md 向けの指示をコピーしました");
            } catch (e) {
              showToast(`コピーできませんでした: ${errorMessage(e)}`);
            }
          },
        },
      ],
    };
  }

  async function onSaved(id: string) {
    creating = undefined;
    selectedId = id;
    await reload();
  }

  async function onDeleted() {
    selectedId = null;
    editorDirty = false;
    await reload();
  }

  /** 検索結果から開く。非アクティブな目的ならアーカイブを表示に切り替える。 */
  async function openFound(id: string) {
    const n = model?.byId.get(id);
    if (n && n.effective !== "active" && n.resolved) showArchived = true;
    await select(id);
    view = "detail";
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await api.onWorkspaceChanged((changed) => {
        if (changed === root) reload();
      });
      try {
        settings = await api.getSettings();
      } catch {
        settings = { workspaces: [], last: null };
      }
      if (settings.last) await openWorkspace(settings.last);
    })();
    return () => unlisten?.();
  });
</script>

<svelte:window onkeydown={onGlobalKey} />

<div class="app" style:grid-template-columns={sidebarOpen ? `${sidebarWidth}px minmax(0, 1fr)` : "minmax(0, 1fr)"} class:resizing={resizing !== null}>
  {#if sidebarOpen}
  <nav class="sidebar">
    <!-- フォーカスできる分割バー（WAI-ARIA window splitter） -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label="サイドパネルの幅"
      aria-valuenow={sidebarWidth}
      aria-valuemin={SIDEBAR_MIN}
      aria-valuemax={SIDEBAR_MAX}
      tabindex="0"
      onpointerdown={startResize}
      onpointermove={onResize}
      onpointerup={endResize}
      onpointercancel={endResize}
      onkeydown={resizeByKey}
      ondblclick={() => (sidebarWidth = 300)}
      title="ドラッグで幅を変更（ダブルクリックで元に戻す）"
    ></div>
    <div class="ws">
      <select
        value={root ?? ""}
        onchange={async (e) => {
          const el = e.currentTarget;
          const v = el.value;
          if (v && !(await openWorkspace(v))) el.value = root ?? "";
        }}
        aria-label="Workspace"
      >
        {#if !root}<option value="">Workspaceを選択</option>{/if}
        {#each settings.workspaces as w (w)}
          <option value={w} title={w}>{root === w && snap ? snap.name : folderName(w)}</option>
        {/each}
      </select>
      <button class="icon" onclick={addWorkspace} title="フォルダを選んでWorkspaceを追加" aria-label="Workspaceを追加">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M8 3v10M3 8h10" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
      </button>
      <button class="icon" onclick={() => (sidebarOpen = false)} title="サイドパネルを閉じる（Ctrl+B）" aria-label="サイドパネルを閉じる">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M10 3L5 8l5 5" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
      </button>
    </div>

    {#if snap && model}
      <div class="tools">
        <label class="toggle"><input type="checkbox" bind:checked={showArchived} /> アーカイブを表示</label>
        <div class="tool-buttons">
          <button class="ghost small" onclick={() => startCreate(null)} title="ルートの目的を追加">＋ ルート</button>
          <button class="icon ghost" onclick={reload} title="再読み込み（ファイルを読み込み直す）" aria-label="再読み込み">
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.5v3h-3" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </button>
        </div>
      </div>
      <SearchBox {model} onOpen={openFound} />

      {#if snap.diagnostics.length > 0 || errorCount > 0}
        <button class="ws-diag {wsSeverity ?? (errorCount > 0 ? 'error' : 'info')}" onclick={() => (showWsDiags = !showWsDiags)}>
          {#if errorCount > 0}エラーのあるファイル {errorCount} 件{/if}
          {#if snap.diagnostics.length > 0}{errorCount > 0 ? " ・ " : ""}Workspaceの通知 {snap.diagnostics.length} 件{/if}
        </button>
        {#if showWsDiags}
          <div class="ws-diag-list"><DiagnosticList items={snap.diagnostics} showFile /></div>
        {/if}
      {/if}

      {#if focusNode}
        <div class="focus-bar">
          <nav class="crumbs" aria-label="表示中の範囲">
            <button class="crumb" onclick={() => focusOn(null)} title="全体の表示に戻す">全体</button>
            {#each focusNode.ancestors as a (a)}
              <span class="sep" aria-hidden="true">›</span>
              <button class="crumb" onclick={() => focusOn(a)} title="ここから下を表示">{displayName(model.byId.get(a), a)}</button>
            {/each}
            <span class="sep" aria-hidden="true">›</span>
            <span class="crumb current" aria-current="location">{displayName(focusNode)}</span>
          </nav>
          <button class="icon ghost" onclick={() => focusOn(null)} title="全体の表示に戻す" aria-label="全体の表示に戻す">
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
          </button>
        </div>
      {/if}

      <div class="tree-scroll">
        {#if snap.nodes.length === 0}
          <div class="empty">
            <p>まだ目的がありません。</p>
            <button class="primary" onclick={() => startCreate(null)}>ルートの目的を作成</button>
          </div>
        {/if}
        <ul class="tree" role="tree">
          {#each visibleRoots as id (id)}
            <TreeItem {model} {id} {showArchived} {selectedId} {collapsed} {pending} onSelect={select} onAddChild={startCreate} onContextMenu={openMenu} />
          {/each}
          {#if pending && pending.parent === null}
            <PendingRow name={pending.name} depth={0} />
          {/if}
        </ul>

        {#if model.unresolved.length > 0 && !focusNode}
          <h3 class="group">未解決（祖先を辿れない）</h3>
          <ul class="tree" role="tree">
            {#each model.unresolved as id (id)}
              <TreeItem {model} {id} {showArchived} {selectedId} {collapsed} onSelect={select} onAddChild={startCreate} onContextMenu={openMenu} />
            {/each}
          </ul>
        {/if}

        {#if model.broken.length > 0 && !focusNode}
          <h3 class="group">読み込めないファイル</h3>
          <ul class="tree" role="tree">
            {#each model.broken as id (id)}
              <li class="broken" role="treeitem" aria-selected={selectedId === id}>
                <button class="ghost" class:selected={selectedId === id} onclick={() => select(id)}>
                  <span class="mono">{model.byId.get(id)?.file_name}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </nav>
  {/if}

  <main class:overview-mode={view === "overview" && !!snap}>
    {#if root && snap}
      <div class="mainbar">
        {#if !sidebarOpen}
          <button class="icon" onclick={() => (sidebarOpen = true)} title="サイドパネルを開く（Ctrl+B）" aria-label="サイドパネルを開く">
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M6 3l5 5-5 5" stroke="currentColor" stroke-width="1.8" fill="none" /></svg>
          </button>
          <span class="ws-title" title={root}>{snap.name}</span>
        {/if}
        <div class="seg" role="tablist" aria-label="表示">
          <button role="tab" aria-selected={view === "detail"} class:on={view === "detail"} onclick={() => (view = "detail")} title="詳細（Ctrl+1）">詳細</button>
          <button role="tab" aria-selected={view === "overview"} class:on={view === "overview"} onclick={() => (view = "overview")} title="全体図（Ctrl+2）">全体図</button>
        </div>
      </div>
    {/if}
    {#if !root}
      <div class="welcome">
        <h1>Purpose Manager</h1>
        <p>目的（Why）と判断原則の階層を、ローカルのYAMLファイルで管理します。</p>
        <p class="muted">Workspaceは <code>workspace.yaml</code> と <code>purposes/</code> フォルダを持つフォルダです。空のフォルダを選ぶと初期化できます。</p>
        <button class="primary" onclick={addWorkspace}>Workspaceのフォルダを選ぶ</button>
      </div>
    {:else if loadError}
      <div class="welcome">
        <h1>Workspaceを読み込めません</h1>
        <p class="err">{loadError}</p>
        <div class="row">
          <button onclick={reload}>再試行</button>
          <button class="danger" onclick={() => root && removeWorkspace(root)}>一覧から外す</button>
        </div>
      </div>
    {:else if snap && model}
      {#if view === "overview"}
        <div class="overview-wrap">
          <TreeOverview {model} roots={visibleRoots} {showArchived} {selectedId} onOpen={openFromOverview} onAddChild={startCreate} onContextMenu={openMenu} />
        </div>
      {/if}
      <!-- 全体図に切り替えても編集中の内容を失わないよう、詳細は隠すだけにする -->
      <div class="detail" hidden={view === "overview"}>
      {#key editorKey}
        {#if creating !== undefined || selected}
          <PurposeEditor
            root={snap.root}
            {snap}
            {model}
            node={creating !== undefined ? null : selected}
            createParent={creating ?? null}
            {showArchived}
            {map}
            {onSaved}
            onCancel={() => {
              creating = undefined;
              editorDirty = false;
            }}
            {onDeleted}
            onCreateChild={(p) => startCreate(p)}
            onDirtyChange={(d) => (editorDirty = d)}
            onDraftChange={(d) => (pendingDraft = d)}
            onToast={showToast}
            onReload={reload}
          />
        {:else if snap.nodes.length > 0}
          <div class="welcome">
            <h1>{snap.name}</h1>
            <p class="muted">左のツリーから目的を選んでください。</p>
            {#if model.roots.length === 0}
              <button onclick={() => startCreate(null)}>ルートの目的を作成</button>
            {/if}
          </div>
        {/if}
      {/key}
      </div>
    {/if}
  </main>

  {#snippet map()}
    {#if model}
      <TreeMinimap
        {model}
        roots={visibleRoots}
        {showArchived}
        currentId={selectedId}
        pendingParent={pending ? pending.parent : undefined}
        onOpen={select}
      />
    {/if}
  {/snippet}

  {#if menu}
    <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => (menu = null)} />
  {/if}

  {#if toast}
    <div class="toast" role="status">{toast}</div>
  {/if}
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    height: 100vh;
  }
  .app.resizing {
    cursor: col-resize;
    user-select: none;
  }
  .sidebar {
    position: relative;
    border-right: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    padding: 10px 10px 0;
    gap: 8px;
  }
  .resizer {
    position: absolute;
    top: 0;
    right: -4px;
    width: 8px;
    height: 100%;
    cursor: col-resize;
    z-index: 5;
  }
  .resizer:hover,
  .resizer:focus-visible,
  .app.resizing .resizer {
    background: linear-gradient(to right, transparent 3px, var(--accent) 3px, var(--accent) 5px, transparent 5px);
    outline: none;
  }
  .ws {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .ws select {
    font-weight: 600;
    font-size: 13px;
    padding: 3px 6px;
    min-width: 0;
    flex: 1;
  }
  button.icon {
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    display: grid;
    place-items: center;
    color: var(--text-2);
  }
  .mainbar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
  }
  .ws-title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .seg {
    margin-left: auto;
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    flex: none;
  }
  .seg button {
    border: none;
    border-radius: 0;
    padding: 3px 14px;
    font-size: 13px;
    background: var(--surface);
    color: var(--text-2);
  }
  .seg button + button {
    border-left: 1px solid var(--border);
  }
  .seg button.on {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  main.overview-mode {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding-bottom: 16px;
  }
  .overview-wrap {
    flex: 1;
    min-height: 0;
  }
  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 6px;
    align-items: center;
  }
  .focus-bar {
    display: flex;
    align-items: flex-start;
    gap: 4px;
    padding: 4px 4px 4px 8px;
    border-radius: var(--radius);
    background: var(--accent-soft);
  }
  .crumbs {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 2px;
    font-size: 12px;
    line-height: 1.6;
  }
  .crumb {
    border: none;
    background: none;
    padding: 0 2px;
    font-size: 12px;
    color: var(--accent);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button.crumb:hover {
    text-decoration: underline;
  }
  .crumb.current {
    font-weight: 600;
    color: var(--text);
  }
  .sep {
    color: var(--text-3);
  }
  .focus-bar button.icon {
    width: 22px;
    height: 22px;
  }
  .tool-buttons {
    display: flex;
    gap: 2px;
    align-items: center;
    flex: none;
    margin-left: auto;
  }
  .tool-buttons button {
    white-space: nowrap;
  }
  .tool-buttons .small {
    padding: 2px 6px;
  }
  .tool-buttons button.icon {
    width: 24px;
    height: 24px;
  }
  .toggle {
    display: flex;
    white-space: nowrap;
    gap: 6px;
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
  }
  .small {
    font-size: 12px;
    padding: 2px 8px;
  }
  .ws-diag {
    text-align: left;
    font-size: 12px;
    border: none;
  }
  .ws-diag.error {
    background: var(--error-soft);
    color: var(--error);
  }
  .ws-diag.warning {
    background: var(--warning-soft);
    color: var(--warning);
  }
  .ws-diag.info {
    background: var(--info-soft);
    color: var(--info);
  }
  .ws-diag-list {
    padding: 4px 2px;
  }
  .tree-scroll {
    overflow: auto;
    flex: 1;
    min-height: 0;
    padding-bottom: 16px;
    margin: 0 -4px;
  }
  .tree {
    margin: 0;
    padding: 0;
  }
  .group {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    margin: 16px 8px 4px;
  }
  .broken {
    list-style: none;
  }
  .broken button {
    width: 100%;
    text-align: left;
    color: var(--error);
    font-size: 13px;
  }
  .broken button.selected {
    background: var(--error-soft);
  }
  .empty {
    padding: 12px 8px;
  }
  main {
    overflow: auto;
    padding: 20px 28px 40px;
    min-width: 0;
  }
  .welcome {
    max-width: 560px;
    margin: 12vh auto 0;
  }
  .welcome h1 {
    font-size: 22px;
    margin: 0 0 8px;
  }
  .welcome .row {
    display: flex;
    gap: 8px;
  }
  .err {
    color: var(--error);
  }
  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--text);
    color: var(--bg);
    padding: 8px 16px;
    border-radius: var(--radius);
    font-size: 13px;
    z-index: 30;
  }
</style>
