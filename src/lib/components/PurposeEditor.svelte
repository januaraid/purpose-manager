<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, errorMessage } from "$lib/api";
  import { autoHeight } from "$lib/autoHeight";
  import { indentEdit } from "$lib/indent";
  import { fromDraft, idProblem, isDirty, newDraft, randomSuffix, suggestId, toDraft, type Draft } from "$lib/draft";
  import {
    ancestorsIfParent,
    descendants,
    displayName,
    parentCandidates,
    pathLabel,
    referencingChildren,
    type TreeModel,
  } from "$lib/tree";
  import type { Diagnostic, PurposeFields, PurposeNode, SaveError, Snapshot } from "$lib/types";
  import DiagnosticList from "./DiagnosticList.svelte";
  import PurposeGuide from "./PurposeGuide.svelte";

  let {
    root,
    snap,
    model,
    node,
    createParent = null,
    showArchived = false,
    map,
    onSaved,
    onCancel,
    onDeleted,
    onCreateChild,
    onDirtyChange,
    onDraftChange,
    onCopyYaml,
    onToast,
    onReload,
  }: {
    root: string;
    snap: Snapshot;
    model: TreeModel;
    /** 編集対象。`null` は新規作成。 */
    node: PurposeNode | null;
    createParent?: string | null;
    /** false のとき、親の選択肢から非アクティブな目的を外す（現在の親は残す）。 */
    showArchived?: boolean;
    /** 右の欄の上部に出す縮小図。 */
    map?: Snippet;
    onSaved: (id: string) => void;
    onCancel: () => void;
    onDeleted: () => void;
    onCreateChild: (parent: string) => void;
    onDirtyChange: (dirty: boolean) => void;
    /** 新規作成中の親と表示名（ツリーに仮の行を出すため）。 */
    onDraftChange?: (d: { parent: string | null; name: string }) => void;
    /** 祖先〜この目的〜子孫のYAMLをまとめてコピーする。 */
    onCopyYaml?: (id: string) => void;
    onToast: (msg: string) => void;
    onReload: () => void;
  } = $props();

  // 編集を始めた時点のファイル内容。保存時の競合検知に使う。
  let baseHash = $state<string | null>(untrack(() => node?.hash ?? null));
  let baseFields = $state<PurposeFields | null>(untrack(() => node?.fields ?? null));
  let draft = $state<Draft>(untrack(() => (node?.fields ? toDraft(node.fields) : newDraft(createParent))));
  let externalChange = $state(false);
  let saving = $state(false);
  let saveDiagnostics = $state<Diagnostic[]>([]);
  let saveMessage = $state<string | null>(null);
  let snippet = $state<string | null>(null);

  const isNew = $derived(node === null);

  // 新規作成時のID自動生成。利用者がIDを直接入力したら、以後は上書きしない。
  let idTouched = $state(false);
  const idSuffix = randomSuffix();
  $effect(() => {
    if (!isNew || idTouched) return;
    const parent = draft.parent === "" ? null : draft.parent;
    const name = draft.name;
    const existing = new Set(model.byId.keys());
    draft.id = name.trim() === "" ? "" : suggestId(name, parent, existing, idSuffix);
  });
  const editable = $derived(isNew || node?.fields != null);

  // 編集の方法：フォームか、YAMLの直接編集か（端末ローカルに覚える）。新規作成はフォームだけ。
  const MODE_KEY = "purpose-manager.editorMode";
  let mode = $state<"form" | "yaml">(
    untrack(() => (node && localStorage.getItem(MODE_KEY) === "yaml" ? "yaml" : "form")),
  );
  /** 編集中のYAML。 */
  let yamlText = $state("");
  /** 読み込んだ（または保存した）時点のYAML。`null` は読み込み中。 */
  let yamlBase = $state<string | null>(null);
  let yamlHash = $state<string | null>(null);
  const yamlDirty = $derived(mode === "yaml" && yamlBase !== null && yamlText !== yamlBase);
  const formDirty = $derived(editable && (isNew ? hasInput(draft) : isDirty(draft, baseFields)));
  const dirty = $derived(mode === "yaml" ? yamlDirty : formDirty);
  untrack(() => {
    if (mode === "yaml") loadYaml();
  });
  $effect(() => onDirtyChange(dirty));
  $effect(() => {
    if (isNew) onDraftChange?.({ parent: draft.parent === "" ? null : draft.parent, name: draft.name });
  });

  // 外部変更：自分の保存なら baseHash（YAMLの直接編集では yamlHash）と一致するので何もしない。
  $effect(() => {
    const current = node;
    if (!current) return;
    untrack(() => {
      if (mode === "yaml") {
        if (yamlBase === null || current.hash === yamlHash) return;
        if (!yamlDirty) loadYaml();
        else externalChange = true;
        return;
      }
      if (current.hash === baseHash) return;
      if (!isDirty(draft, baseFields) || !current.fields) {
        reset(current);
      } else {
        externalChange = true;
      }
    });
  });

  function hasInput(d: Draft) {
    return d.id !== "" || d.name !== "" || d.purpose !== "" || d.rationale !== "" || d.principles !== "";
  }

  function reset(n: PurposeNode) {
    baseHash = n.hash;
    baseFields = n.fields;
    if (n.fields) draft = toDraft(n.fields);
    externalChange = false;
    saveDiagnostics = [];
    saveMessage = null;
  }

  const originalParent = $derived(isNew ? createParent ?? "" : (baseFields?.parent ?? ""));
  const candidates = $derived(
    parentCandidates(model, isNew ? null : (node?.id ?? null)).filter(
      (c) => showArchived || c.effective === "active" || c.id === draft.parent || c.id === originalParent,
    ),
  );
  const parentKnown = $derived(draft.parent === "" || candidates.some((c) => c.id === draft.parent));
  const parentChanged = $derived(!isNew && draft.parent !== originalParent);
  const previewAncestors = $derived(ancestorsIfParent(model, draft.parent === "" ? null : draft.parent));
  const descendantIds = $derived(node ? descendants(model, node.id) : []);
  const archiving = $derived(!isNew && baseFields?.status === "active" && draft.status === "archived");
  const deleteBlockers = $derived(node ? referencingChildren(snap, node.id) : []);
  const idError = $derived(isNew ? idProblem(draft.id.trim()) : null);
  const idTaken = $derived(isNew && model.byId.has(draft.id.trim()));
  const periodError = $derived(
    draft.valid_from !== "" && draft.valid_until !== "" && draft.valid_from > draft.valid_until
      ? "開始日が終了日より後になっています"
      : null,
  );
  const requiredMissing = $derived(draft.name.trim() === "" || draft.purpose.trim() === "");
  const canSave = $derived(editable && formDirty && !saving && !idError && !idTaken && !periodError && !requiredMissing);

  const effectiveLabel = {
    active: "有効",
    archived: "アーカイブ",
    inherited_inactive: "親がアーカイブ",
    unknown: "判定不可",
  } as const;
  const effectiveClass = {
    active: "active",
    archived: "archived",
    inherited_inactive: "inherited",
    unknown: "warning",
  } as const;

  async function save() {
    if (!canSave) return;
    let allowCommentLoss = false;
    if (node?.has_comments) {
      allowCommentLoss = await ask(
        "このファイルにはYAMLコメントがあります。GUIで保存するとコメントは失われます。\n補足はコメントではなく「背景」に書くことをおすすめします。\n\n保存しますか？",
        { title: "コメントが失われます", kind: "warning", okLabel: "保存する", cancelLabel: "やめる" },
      );
      if (!allowCommentLoss) return;
    }
    saving = true;
    saveDiagnostics = [];
    saveMessage = null;
    const fields = fromDraft(draft);
    try {
      const out = await api.savePurpose(root, {
        fields,
        expected_hash: isNew ? null : baseHash,
        allow_comment_loss: allowCommentLoss,
      });
      baseHash = out.hash;
      baseFields = fields;
      draft = toDraft(fields);
      externalChange = false;
      onToast(isNew ? "目的を作成しました" : "保存しました");
      onSaved(out.id);
    } catch (e) {
      showSaveError(e);
    } finally {
      saving = false;
    }
  }

  function showSaveError(e: unknown) {
    const err = e as SaveError;
    switch (err?.kind) {
      case "validation":
        saveDiagnostics = err.diagnostics;
        saveMessage = mode === "yaml" ? "YAMLに問題があるため保存していません。" : "入力内容に問題があるため保存していません。";
        break;
      case "conflict":
        externalChange = true;
        saveMessage = "編集中にファイルが外部で変更されたため、保存していません。";
        break;
      case "already_exists":
        saveMessage = "同じIDの目的がすでにあります。別のIDにしてください。";
        break;
      case "not_editable":
        saveDiagnostics = err.diagnostics;
        saveMessage = "ファイルにエラーがあるため、GUIでは保存できません。";
        break;
      default:
        saveMessage = `保存できませんでした: ${errorMessage(e)}`;
    }
  }

  async function loadYaml() {
    if (!node) return;
    const id = node.id;
    try {
      const t = await api.readPurposeYaml(root, id);
      if (node?.id !== id || mode !== "yaml") return;
      yamlText = t.text;
      yamlBase = t.text;
      yamlHash = t.hash;
      externalChange = false;
      saveDiagnostics = [];
      saveMessage = null;
    } catch (e) {
      saveMessage = `YAMLを読み込めませんでした: ${errorMessage(e)}`;
    }
  }

  async function saveYaml() {
    if (!node || !yamlDirty || saving || yamlHash === null) return;
    saving = true;
    saveDiagnostics = [];
    saveMessage = null;
    const text = yamlText;
    try {
      const out = await api.savePurposeYaml(root, node.id, text, yamlHash);
      // 保存時に末尾の改行をそろえるので、画面の内容も合わせる（保存中に入力された分は残す）
      const saved = text.endsWith("\n") ? text : text + "\n";
      if (yamlText === text) yamlText = saved;
      yamlBase = saved;
      yamlHash = out.hash;
      externalChange = false;
      onToast("保存しました");
      onSaved(out.id);
    } catch (e) {
      showSaveError(e);
    } finally {
      saving = false;
    }
  }

  // Tab で字下げ、Shift+Tab で字上げ。キーボードだけで抜けられるよう、Esc の直後の Tab は通常どおり移動する。
  let tabMovesFocus = false;
  function onYamlKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      tabMovesFocus = true;
      return;
    }
    if (e.key !== "Tab") {
      if (e.key !== "Shift") tabMovesFocus = false;
      return;
    }
    if (tabMovesFocus || e.ctrlKey || e.metaKey || e.altKey || e.isComposing) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLTextAreaElement;
    const edit = indentEdit(el.value, el.selectionStart, el.selectionEnd, e.shiftKey);
    if (!edit) return;
    el.setSelectionRange(edit.from, edit.to);
    // insertText なら Cmd+Z で元に戻せる。使えない環境では直接置き換える。
    if (!document.execCommand("insertText", false, edit.insert)) {
      el.setRangeText(edit.insert, edit.from, edit.to, "end");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
    el.setSelectionRange(edit.selStart, edit.selEnd);
  }

  function setMode(m: "form" | "yaml") {
    if (m === mode || dirty || !node) return;
    mode = m;
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {
      // 覚えられなくても切り替えはできる
    }
    externalChange = false;
    saveDiagnostics = [];
    saveMessage = null;
    if (m === "yaml") {
      yamlBase = null;
      loadYaml();
    } else {
      reset(node);
    }
  }

  function discard() {
    if (mode === "yaml") {
      if (yamlBase !== null) yamlText = yamlBase;
      saveDiagnostics = [];
      saveMessage = null;
      return;
    }
    if (isNew) {
      onCancel();
      return;
    }
    if (node) reset(node);
  }

  function reloadFromDisk() {
    if (mode === "yaml") loadYaml();
    else if (node) reset(node);
    onReload();
  }

  async function remove() {
    if (!node || deleteBlockers.length > 0) return;
    const ok = await ask(
      `「${displayName(node)}」（${node.id}）のファイルを削除します。\n\nこの目的を参照していた過去のセッションは参照先を失います。通常は削除ではなくアーカイブをおすすめします。\n\n削除しますか？`,
      { title: "目的の削除", kind: "warning", okLabel: "削除する", cancelLabel: "やめる" },
    );
    if (!ok) return;
    try {
      await api.deletePurpose(root, node.id, baseHash ?? node.hash);
      onToast("削除しました");
      onDeleted();
    } catch (e) {
      const err = e as SaveError;
      if (err?.kind === "conflict") {
        saveMessage = "ファイルが外部で変更されたため、削除していません。再読み込みしてください。";
      } else if (err?.kind === "has_children") {
        saveMessage = `子目的があるため削除できません: ${err.children.join(", ")}`;
      } else {
        saveMessage = `削除できませんでした: ${errorMessage(e)}`;
      }
    }
  }

  async function copySnippet() {
    if (!node) return;
    const text = await api.claudeMdSnippet(root, node.id);
    try {
      await navigator.clipboard.writeText(text);
      onToast("CLAUDE.md 向けの指示をコピーしました");
    } catch {
      snippet = text;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === "s") {
      e.preventDefault();
      if (mode === "yaml") saveYaml();
      else save();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="editor">
  <section class="form">
    <header>
      <div class="title">
        <h1>{isNew ? "新しい目的" : displayName(node ?? undefined)}</h1>
        {#if node}
          <span class="badge {effectiveClass[node.effective]}">{effectiveLabel[node.effective]}</span>
        {/if}
        {#if dirty}<span class="badge info">未保存</span>{/if}
      </div>
      {#if node}
        <div class="actions">
          <button onclick={copySnippet} title="選択中の目的を参照する指示を、CLAUDE.md に貼り付ける形でコピーします">CLAUDE.md 用にコピー</button>
          <button
            onclick={() => onCopyYaml?.(node.id)}
            disabled={!node.fields}
            title="祖先・この目的・子孫のYAMLを、ファイルの内容のまま1つのテキストにしてコピーします（AIへの相談用）"
          >YAMLをまとめてコピー</button>
          <button onclick={() => onCreateChild(node.id)} disabled={!node.fields}>子目的を追加</button>
          <button
            class="danger"
            onclick={remove}
            disabled={deleteBlockers.length > 0}
            title={deleteBlockers.length > 0 ? `子目的があるため削除できません（${deleteBlockers.join(", ")}）` : "ファイルを削除します"}
          >削除</button>
        </div>
      {/if}
    </header>

    {#if node}
      <p class="path mono muted" title={node.path}>{node.path}</p>
      <div
        class="modes"
        role="group"
        aria-label="編集の方法"
        title={dirty ? "未保存の変更を保存するか破棄してから切り替えてください" : undefined}
      >
        <button class:on={mode === "form"} aria-pressed={mode === "form"} disabled={dirty && mode !== "form"} onclick={() => setMode("form")}>フォーム</button>
        <button class:on={mode === "yaml"} aria-pressed={mode === "yaml"} disabled={dirty && mode !== "yaml"} onclick={() => setMode("yaml")}>YAML</button>
      </div>
    {/if}

    {#if externalChange}
      <div class="banner warning" role="alert">
        <span>このファイルは外部で変更されました。編集中の内容はまだ保存されていません。</span>
        <button onclick={reloadFromDisk}>外部の変更を読み込む（編集内容を破棄）</button>
      </div>
    {/if}

    {#if saveMessage}
      <div class="banner error" role="alert">
        <span>{saveMessage}</span>
      </div>
    {/if}
    <DiagnosticList items={saveDiagnostics} />

    {#if mode === "yaml" && node}
      <div class="fields">
        <div class="field">
          <span class="lbl"><label for="yaml-text">YAML</label> <em class="mono">{node.file_name}</em></span>
          {#if yamlBase !== null}
            <textarea
              id="yaml-text"
              class="yaml"
              use:autoHeight={yamlText}
              bind:value={yamlText}
              onkeydown={onYamlKeydown}
              onblur={() => (tabMovesFocus = false)}
              rows="16"
              spellcheck="false"
              autocapitalize="off"
              autocomplete="off"
            ></textarea>
          {:else}
            <p class="muted">読み込み中…</p>
          {/if}
          <span class="hint">ファイルの内容をそのまま編集します。コメントや書式もそのまま保存されます。保存の前に、フォームと同じ検証をします。Tab で字下げ、Shift+Tab で字上げ（Esc のあとに Tab で次の項目へ移動）。</span>
        </div>
        <DiagnosticList items={node.diagnostics} />
      </div>

      <footer>
        <button class="primary" onclick={saveYaml} disabled={!yamlDirty || saving}>{saving ? "保存中…" : "保存"}</button>
        <button onclick={discard} disabled={!yamlDirty}>変更を破棄</button>
        <span class="muted kbd">Ctrl+S で保存</span>
      </footer>
    {:else if !editable && node}
      <div class="banner error">
        <span>このファイルにはエラーがあるため、フォームでは編集できません。YAMLを直接編集して直すか、外部エディタで修正すると自動で反映されます。</span>
        <button onclick={() => setMode("yaml")}>YAMLを直接編集する</button>
      </div>
      <DiagnosticList items={node.diagnostics} />
    {:else}
      <div class="fields">
        <label class="field">
          <span class="lbl">表示名 <em>必須</em></span>
          <!-- svelte-ignore a11y_autofocus -->
          <input type="text" bind:value={draft.name} autofocus={isNew} />
        </label>

        <label class="field">
          <span class="lbl">
            ID
            {#if isNew}
              {#if idTouched}
                <button type="button" class="link" onclick={() => (idTouched = false)}>自動生成に戻す</button>
              {:else}
                <em>表示名から自動生成</em>
              {/if}
            {/if}
          </span>
          {#if isNew}
            <input
              type="text"
              bind:value={draft.id}
              oninput={() => (idTouched = draft.id !== "")}
              placeholder="表示名を入力すると自動で入ります"
              spellcheck="false"
              class="mono"
            />
            <span class="hint" class:err={draft.id !== "" && (idError || idTaken)}>
              {#if draft.id !== "" && idError}{idError}
              {:else if idTaken}同じIDの目的がすでにあります
              {:else}小文字英数字とハイフン。直接編集もできます。作成後は変更できません（ファイル名 <code>{draft.id || "<ID>"}.yaml</code>）{/if}
            </span>
          {:else}
            <input type="text" value={draft.id} readonly class="mono" />
            <span class="hint">IDは作成後に変更できません。目的そのものが変わる場合は新しい目的を作り、こちらをアーカイブします。</span>
          {/if}
        </label>

        <!-- ヘルプのボタンを置くため、label 要素で囲まない（押したときに入力欄へ移らないように） -->
        <div class="field">
          <span class="lbl"><label for="purpose-text">目的</label> <em>必須</em> <PurposeGuide /></span>
          <textarea id="purpose-text" use:autoHeight={draft.purpose} bind:value={draft.purpose} rows="3" placeholder="この目的が実現したい状態。親の目的にどう貢献するか"></textarea>
        </div>

        <label class="field">
          <span class="lbl">判断原則</span>
          <textarea use:autoHeight={draft.principles} bind:value={draft.principles} rows="3" placeholder="1行に1つ"></textarea>
          <span class="hint">1行に1つ。子目的からも参照されます。</span>
        </label>

        <label class="field">
          <span class="lbl">背景</span>
          <textarea use:autoHeight={draft.rationale} bind:value={draft.rationale} rows="3" placeholder="任意。なぜこの目的を設定したか"></textarea>
          <span class="hint">子目的には引き継がれません。会議記録・作業履歴・認証情報は書かないでください。</span>
        </label>

        <div class="row2">
          <label class="field">
            <span class="lbl">親目的</span>
            <select bind:value={draft.parent}>
              <option value="">（なし：ルート）</option>
              {#if !parentKnown}
                <option value={draft.parent}>{draft.parent}（見つかりません）</option>
              {/if}
              {#each candidates as c (c.id)}
                <option value={c.id}>{pathLabel(model, c)}{c.effective !== "active" ? "（非アクティブ）" : ""}</option>
              {/each}
            </select>
          </label>

          <label class="field">
            <span class="lbl">状態</span>
            <select bind:value={draft.status}>
              <option value="active">active（有効）</option>
              <option value="archived">archived（アーカイブ）</option>
            </select>
          </label>
        </div>

        {#if parentChanged}
          <div class="banner info">
            <span>
              親を変更すると、継承元が右の「継承元」の内容に変わります。
              {#if descendantIds.length > 0}子孫 {descendantIds.length} 件も一緒に移動します。{/if}
            </span>
          </div>
        {/if}
        {#if archiving && descendantIds.length > 0}
          <div class="banner warning">
            <span>アーカイブすると、子孫 {descendantIds.length} 件が実効的に非アクティブになります（子孫のファイルは変更しません）。継続する子目的は、先に別の親へ付け替えてください。</span>
          </div>
        {/if}

        <fieldset class="dates">
          <legend>期間（任意）</legend>
          <label class="field">
            <span class="lbl">有効開始日</span>
            <input type="date" bind:value={draft.valid_from} />
          </label>
          <label class="field">
            <span class="lbl">有効終了日</span>
            <input type="date" bind:value={draft.valid_until} />
          </label>
          <label class="field">
            <span class="lbl">見直し予定日</span>
            <input type="date" bind:value={draft.review_at} />
          </label>
          <p class="hint" class:err={periodError}>
            {periodError ?? "未設定は「期間未定」です。日付を過ぎても状態は自動では変わりません。"}
          </p>
        </fieldset>

        {#if node && node.unknown_fields.length > 0}
          <p class="hint">未知のフィールド（保存しても保持されます）: <code>{node.unknown_fields.join(", ")}</code></p>
        {/if}

        {#if node}
          <DiagnosticList items={node.diagnostics} />
        {/if}
      </div>

      <footer>
        <button class="primary" onclick={save} disabled={!canSave}>{saving ? "保存中…" : isNew ? "作成" : "保存"}</button>
        <button onclick={discard} disabled={!isNew && !dirty}>{isNew ? "キャンセル" : "変更を破棄"}</button>
        <span class="muted kbd">Ctrl+S で保存</span>
      </footer>
    {/if}
  </section>

  <div class="side">
  {@render map?.()}
  <aside class="lineage">
    <h2>継承元{parentChanged || isNew ? "（保存後）" : ""}</h2>
    {#if editable}
      {#if previewAncestors.length === 0}
        <p class="muted">ルートの目的です。継承元はありません。</p>
      {:else}
        <ol>
          {#each previewAncestors as aid (aid)}
            {@const a = model.byId.get(aid)}
            {#if a?.fields}
              <li class:inactive={a.effective !== "active"}>
                <div class="a-name">
                  {a.fields.name}
                  <span class="muted mono">{a.id}</span>
                  {#if a.fields.status === "archived"}<span class="badge archived">アーカイブ</span>{/if}
                </div>
                <p class="a-purpose">{a.fields.purpose}</p>
                {#if a.fields.principles && a.fields.principles.length > 0}
                  <ul class="principles">
                    {#each a.fields.principles as p, i (i)}<li>{p}</li>{/each}
                  </ul>
                {/if}
              </li>
            {/if}
          {/each}
        </ol>
      {/if}
    {/if}
  </aside>
  </div>
</div>

{#if snippet}
  <div class="modal-backdrop" role="presentation" onclick={() => (snippet = null)}>
    <div class="modal" role="dialog" aria-label="CLAUDE.md 用の指示" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (snippet = null)}>
      <p>クリップボードにコピーできませんでした。以下を選択してコピーしてください。</p>
      <textarea readonly rows="14" value={snippet}></textarea>
      <button onclick={() => (snippet = null)}>閉じる</button>
    </div>
  </div>
{/if}

<style>
  .editor {
    display: grid;
    /* 画面幅いっぱいに使う。継承元の欄も幅に応じて広げる */
    grid-template-columns: minmax(0, 1fr) clamp(260px, 32%, 560px);
    gap: 24px;
    align-items: start;
  }
  @media (max-width: 1100px) {
    .editor {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  header {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
  }
  .title {
    display: flex;
    gap: 8px;
    align-items: center;
    min-width: 0;
  }
  h1 {
    font-size: 20px;
    margin: 0;
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .path {
    margin: 4px 0 12px;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .modes {
    display: inline-flex;
    margin-bottom: 4px;
  }
  .modes button {
    padding: 3px 14px;
    font-size: 12px;
    border-radius: 0;
  }
  .modes button:first-child {
    border-radius: var(--radius) 0 0 var(--radius);
  }
  .modes button:last-child {
    border-radius: 0 var(--radius) var(--radius) 0;
    margin-left: -1px;
  }
  .modes button.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
    position: relative;
  }
  textarea.yaml {
    font-family: var(--mono);
    font-size: 13px;
    line-height: 1.55;
    tab-size: 2;
    white-space: pre;
    overflow-x: auto;
  }
  .fields {
    display: grid;
    gap: 14px;
    margin-top: 12px;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .lbl {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .lbl em {
    font-style: normal;
    font-weight: 400;
    color: var(--text-3);
    margin-left: 4px;
  }
  button.link {
    border: none;
    background: none;
    padding: 0;
    margin-left: 6px;
    font-size: 12px;
    font-weight: 400;
    color: var(--accent);
    text-decoration: underline;
  }
  .hint {
    font-size: 12px;
    color: var(--text-3);
    margin: 0;
  }
  .hint.err {
    color: var(--error);
  }
  .row2 {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
    gap: 12px;
  }
  .dates {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 8px 12px 12px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 12px;
    margin: 0;
  }
  .dates legend {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    padding: 0 4px;
  }
  .dates .hint {
    grid-column: 1 / -1;
  }
  .banner {
    display: flex;
    gap: 10px;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    padding: 8px 12px;
    border-radius: var(--radius);
    font-size: 13px;
    margin: 8px 0;
  }
  .banner.warning {
    background: var(--warning-soft);
    color: var(--warning);
  }
  .banner.error {
    background: var(--error-soft);
    color: var(--error);
  }
  .banner.info {
    background: var(--info-soft);
    color: var(--info);
  }
  footer {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .kbd {
    font-size: 12px;
    margin-left: auto;
  }
  .side {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    gap: 12px;
    position: sticky;
    top: 0;
    max-height: calc(100vh - 80px);
    overflow: auto;
  }
  .lineage {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 12px 14px;
  }
  .lineage h2 {
    font-size: 13px;
    margin: 0 0 8px;
    color: var(--text-2);
  }
  .lineage ol {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0;
  }
  .lineage ol > li {
    position: relative;
    padding: 6px 0 10px 14px;
    border-left: 2px solid var(--border);
  }
  .lineage ol > li::before {
    content: "";
    position: absolute;
    left: -5px;
    top: 12px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }
  .lineage ol > li.inactive::before {
    background: var(--archived);
  }
  .a-name {
    font-weight: 600;
    display: flex;
    gap: 6px;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .a-name .mono {
    font-weight: 400;
    font-size: 11px;
  }
  .a-purpose {
    margin: 2px 0 4px;
    color: var(--text-2);
    font-size: 13px;
  }
  .principles {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
    color: var(--text-2);
  }
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    display: grid;
    place-items: center;
    z-index: 20;
  }
  .modal {
    background: var(--surface);
    border-radius: var(--radius);
    padding: 16px;
    width: min(640px, 90vw);
    display: grid;
    gap: 10px;
  }
  .modal textarea {
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
