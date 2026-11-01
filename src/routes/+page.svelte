<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "$lib/api";
  import { buildTree, displayName, folderName, todayLocal, worstSeverity } from "$lib/tree";
  import type { LoadFailure, Settings, Snapshot } from "$lib/types";
  import DiagnosticList from "$lib/components/DiagnosticList.svelte";
  import PurposeEditor from "$lib/components/PurposeEditor.svelte";
  import TreeItem from "$lib/components/TreeItem.svelte";

  let settings = $state<Settings>({ workspaces: [], last: null });
  let root = $state<string | null>(null);
  let snap = $state<Snapshot | null>(null);
  let loadError = $state<string | null>(null);
  let selectedId = $state<string | null>(null);
  /** 新規作成中なら親のID（ルートは null）。作成中でなければ undefined。 */
  let creating = $state<string | null | undefined>(undefined);
  let showArchived = $state(false);
  let showWsDiags = $state(false);
  let editorDirty = $state(false);
  let toast = $state<string | null>(null);
  let idQuery = $state("");
  const collapsed = new SvelteSet<string>();

  const model = $derived(snap ? buildTree(snap) : null);
  const selected = $derived(model && selectedId ? (model.byId.get(selectedId) ?? null) : null);
  const visibleRoots = $derived(
    model ? model.roots.filter((id) => showArchived || model.byId.get(id)?.effective !== "archived") : [],
  );
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

  async function startCreate(parent: string | null) {
    if (!(await confirmDiscard())) return;
    creating = parent;
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

  async function openById(e: SubmitEvent) {
    e.preventDefault();
    const id = idQuery.trim();
    if (!id) return;
    if (model?.byId.has(id)) {
      await select(id);
      idQuery = "";
    } else {
      showToast(`ID「${id}」の目的は見つかりません`);
    }
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

<div class="app">
  <nav class="sidebar">
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
      <button onclick={addWorkspace} title="フォルダを選んでWorkspaceを追加">追加</button>
    </div>
    {#if root}
      <p class="ws-path mono muted" title={root}>{root}</p>
    {/if}

    {#if snap && model}
      <div class="tools">
        <label class="toggle"><input type="checkbox" bind:checked={showArchived} /> アーカイブを表示</label>
        <button class="ghost small" onclick={reload} title="ファイルを読み込み直す">再読み込み</button>
      </div>
      <form class="find" onsubmit={openById}>
        <input type="search" bind:value={idQuery} placeholder="IDを指定して開く" spellcheck="false" />
      </form>

      {#if snap.diagnostics.length > 0 || errorCount > 0}
        <button class="ws-diag {wsSeverity ?? (errorCount > 0 ? 'error' : 'info')}" onclick={() => (showWsDiags = !showWsDiags)}>
          {#if errorCount > 0}エラーのあるファイル {errorCount} 件{/if}
          {#if snap.diagnostics.length > 0}{errorCount > 0 ? " ・ " : ""}Workspaceの通知 {snap.diagnostics.length} 件{/if}
        </button>
        {#if showWsDiags}
          <div class="ws-diag-list"><DiagnosticList items={snap.diagnostics} showFile /></div>
        {/if}
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
            <TreeItem {model} {id} {showArchived} {selectedId} {collapsed} onSelect={select} />
          {/each}
        </ul>

        {#if model.unresolved.length > 0}
          <h3 class="group">未解決（祖先を辿れない）</h3>
          <ul class="tree" role="tree">
            {#each model.unresolved as id (id)}
              <TreeItem {model} {id} {showArchived} {selectedId} {collapsed} onSelect={select} />
            {/each}
          </ul>
        {/if}

        {#if model.broken.length > 0}
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

  <main>
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
      {#key editorKey}
        {#if creating !== undefined || selected}
          <PurposeEditor
            root={snap.root}
            {snap}
            {model}
            node={creating !== undefined ? null : selected}
            createParent={creating ?? null}
            {onSaved}
            onCancel={() => {
              creating = undefined;
              editorDirty = false;
            }}
            {onDeleted}
            onCreateChild={(p) => startCreate(p)}
            onDirtyChange={(d) => (editorDirty = d)}
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
    {/if}
  </main>

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
  .sidebar {
    border-right: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 12px 10px 0;
    gap: 8px;
  }
  .ws {
    display: flex;
    gap: 6px;
  }
  .ws select {
    font-weight: 600;
  }
  .ws-path {
    margin: -4px 2px 0;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tools {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .toggle {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 13px;
    color: var(--text-2);
  }
  .small {
    font-size: 12px;
    padding: 2px 8px;
  }
  .find input {
    font-size: 13px;
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
