import type { Diagnostic, PurposeNode, Severity, Snapshot } from "./types";

export interface TreeModel {
  byId: Map<string, PurposeNode>;
  /** ルートまで辿れるルートノード。 */
  roots: string[];
  /** 編集はできるが、祖先を辿れないノード（親の欠落・循環・親のファイルのエラー）。 */
  unresolved: string[];
  /** ファイル単位のエラーで読み込めなかったノード。 */
  broken: string[];
}

export function buildTree(snap: Snapshot): TreeModel {
  const byId = new Map(snap.nodes.map((n) => [n.id, n]));
  const roots: string[] = [];
  const unresolved: string[] = [];
  const broken: string[] = [];
  for (const n of snap.nodes) {
    if (!n.fields) broken.push(n.id);
    else if (!n.resolved) unresolved.push(n.id);
    else if (n.fields.parent === null) roots.push(n.id);
  }
  return { byId, roots, unresolved, broken };
}

/** アーカイブ済みを隠すとき、自身が archived のノードを除く（子孫は親ごと隠れる）。 */
export function isVisible(node: PurposeNode, showArchived: boolean): boolean {
  return showArchived || node.effective !== "archived";
}

export function visibleChildren(model: TreeModel, id: string, showArchived: boolean): string[] {
  const node = model.byId.get(id);
  if (!node) return [];
  return node.children.filter((c) => {
    const child = model.byId.get(c);
    return child !== undefined && isVisible(child, showArchived);
  });
}

/** 子孫のID（自身は含まない）。 */
export function descendants(model: TreeModel, id: string): string[] {
  const out: string[] = [];
  const stack = [...(model.byId.get(id)?.children ?? [])];
  const seen = new Set<string>();
  while (stack.length > 0) {
    const c = stack.pop()!;
    if (seen.has(c)) continue;
    seen.add(c);
    out.push(c);
    stack.push(...(model.byId.get(c)?.children ?? []));
  }
  return out;
}

/** ファイルに親として書かれている参照も含めた、子のID（削除できるかの判定に使う）。 */
export function referencingChildren(snap: Snapshot, id: string): string[] {
  return snap.nodes.filter((n) => n.id !== id && n.raw_parent === id).map((n) => n.id);
}

/**
 * 親の候補。自身と子孫は循環になるため除く。ファイルにエラーがあるノードも除く。
 * `id` が null のときは新規作成（全ノードが候補）。
 */
export function parentCandidates(model: TreeModel, id: string | null): PurposeNode[] {
  const excluded = new Set(id ? [id, ...descendants(model, id)] : []);
  return [...model.byId.values()]
    .filter((n) => n.fields !== null && n.resolved && !excluded.has(n.id))
    .sort((a, b) => pathLabel(model, a).localeCompare(pathLabel(model, b), "ja"));
}

/**
 * サイドパネルのツリーの字下げ（px）。深い階層ほど1段の幅を狭くし、
 * 階層を重ねても名前を表示する幅が残るようにする（さらに CSS で上限を設ける）。
 */
export function treeIndent(depth: number): number {
  let px = 8;
  for (let d = 1; d <= depth; d++) px += d <= 3 ? 16 : d <= 6 ? 10 : 6;
  return px;
}

/** 子目的の親にできるか（親の選択肢に出るものと同じ条件）。 */
export function canHaveChild(node: PurposeNode | undefined): boolean {
  return !!node && node.fields !== null && node.resolved;
}

/** ルートからの表示名の連なり（親の選択肢に使う）。 */
export function pathLabel(model: TreeModel, node: PurposeNode): string {
  return [...node.ancestors, node.id].map((a) => displayName(model.byId.get(a), a)).join(" › ");
}

/** 親を `newParent` にしたときの祖先（ルートから親まで）。 */
export function ancestorsIfParent(model: TreeModel, newParent: string | null): string[] {
  if (newParent === null) return [];
  const p = model.byId.get(newParent);
  if (!p) return [];
  return [...p.ancestors, p.id];
}

export function displayName(node: PurposeNode | undefined, fallback = ""): string {
  return node?.fields?.name ?? node?.raw_name ?? node?.id ?? fallback;
}

const rank: Record<Severity, number> = { error: 3, warning: 2, info: 1 };

export function worstSeverity(diags: Diagnostic[]): Severity | null {
  let worst: Severity | null = null;
  for (const d of diags) {
    if (worst === null || rank[d.severity] > rank[worst]) worst = d.severity;
  }
  return worst;
}

/** 端末のローカル日付（YYYY-MM-DD）。 */
export function todayLocal(now = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export interface LayoutNode {
  id: string;
  depth: number;
  /** 行の位置（葉が1ずつ進む。親は子の中央）。 */
  row: number;
}

export interface Layout {
  nodes: LayoutNode[];
  edges: { from: string; to: string }[];
  depths: number;
  rows: number;
}

/**
 * 全体図のレイアウト（左から右へ深くなる）。座標は「深さ」と「行」の単位で返す。
 * 葉を上から順に並べ、親は子の上下中央に置く。ルートどうしの間は半行あける。
 */
export function layoutTree(model: TreeModel, roots: string[], showArchived: boolean): Layout {
  const nodes: LayoutNode[] = [];
  const edges: { from: string; to: string }[] = [];
  let next = 0;
  let maxDepth = -1;
  const seen = new Set<string>();

  const place = (id: string, depth: number): number => {
    seen.add(id);
    maxDepth = Math.max(maxDepth, depth);
    const kids = visibleChildren(model, id, showArchived).filter((k) => !seen.has(k));
    let row: number;
    if (kids.length === 0) {
      row = next;
      next += 1;
    } else {
      const rows = kids.map((k) => {
        edges.push({ from: id, to: k });
        return place(k, depth + 1);
      });
      row = (rows[0] + rows[rows.length - 1]) / 2;
    }
    nodes.push({ id, depth, row });
    return row;
  };

  roots.forEach((r, i) => {
    if (i > 0) next += 0.5;
    place(r, 0);
  });
  return { nodes, edges, depths: maxDepth + 1, rows: next };
}

/**
 * まとめてコピーする目的の並び（親から子へ、ツリーの表示順）。
 * `id` を指定したときは、祖先 → その目的 → 子孫。指定しないときは `roots` から下すべて。
 * 子孫はツリーと同じく、アーカイブを表示しないときは非アクティブなものを除く。
 */
export function exportOrder(model: TreeModel, id: string | null, roots: string[], showArchived: boolean): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const walk = (x: string) => {
    if (seen.has(x)) return;
    seen.add(x);
    out.push(x);
    for (const k of visibleChildren(model, x, showArchived)) walk(k);
  };
  if (id) {
    const n = model.byId.get(id);
    if (!n) return [];
    for (const a of n.ancestors) {
      seen.add(a);
      out.push(a);
    }
    walk(id);
  } else {
    roots.forEach(walk);
  }
  return out.filter((x) => model.byId.get(x)?.fields);
}

export interface SearchHit {
  id: string;
  /** どこに一致したか。 */
  field: "name" | "id" | "purpose" | "principles" | "rationale";
  /** 本文に一致したときの前後の抜粋。 */
  snippet: string | null;
}

const norm = (s: string) => s.normalize("NFKC").toLowerCase();

function excerpt(text: string, q: string): string {
  const i = norm(text).indexOf(q);
  const start = Math.max(0, i - 12);
  const end = Math.min(text.length, i + q.length + 24);
  return (start > 0 ? "…" : "") + text.slice(start, end).replace(/\s+/g, " ") + (end < text.length ? "…" : "");
}

/**
 * 表示名・ID・目的・判断原則・背景から探す。空白区切りの語はすべて含むものに絞る（AND）。
 * 並び順は、表示名の前方一致 → 表示名 → ID → 本文 の順。
 */
export function searchPurposes(model: TreeModel, query: string, limit = 8): SearchHit[] {
  const terms = norm(query).split(/\s+/).filter(Boolean);
  if (terms.length === 0) return [];
  const scored: { hit: SearchHit; score: number; label: string }[] = [];
  for (const n of model.byId.values()) {
    const f = n.fields;
    const name = norm(displayName(n));
    const id = norm(n.id);
    const fields: [SearchHit["field"], string][] = [
      ["purpose", f?.purpose ?? ""],
      ["principles", (f?.principles ?? []).join(" / ")],
      ["rationale", f?.rationale ?? ""],
    ];
    const all = [name, id, ...fields.map(([, t]) => norm(t))].join("\u0000");
    if (!terms.every((t) => all.includes(t))) continue;
    const t0 = terms[0];
    let hit: SearchHit;
    let score: number;
    if (terms.every((t) => name.includes(t))) {
      hit = { id: n.id, field: "name", snippet: null };
      score = name.startsWith(t0) ? 0 : 1;
    } else if (terms.every((t) => id.includes(t) || name.includes(t))) {
      hit = { id: n.id, field: "id", snippet: null };
      score = 2;
    } else {
      const [field, text] = fields.find(([, t]) => norm(t).includes(t0)) ?? fields.find(([, t]) => terms.some((q) => norm(t).includes(q)))!;
      const q = terms.find((x) => norm(text).includes(x))!;
      hit = { id: n.id, field, snippet: excerpt(text, q) };
      score = 3;
    }
    if (n.effective !== "active") score += 0.5;
    scored.push({ hit, score, label: name });
  }
  scored.sort((a, b) => a.score - b.score || a.label.localeCompare(b.label, "ja"));
  return scored.slice(0, limit).map((s) => s.hit);
}
