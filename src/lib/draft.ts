import type { PurposeFields, Status } from "./types";

/** 編集フォームの状態。空文字は「未設定」を表す。 */
export interface Draft {
  id: string;
  parent: string;
  name: string;
  purpose: string;
  rationale: string;
  /** 1行に1つ。 */
  principles: string;
  status: Status;
  valid_from: string;
  valid_until: string;
  review_at: string;
}

export function toDraft(f: PurposeFields): Draft {
  return {
    id: f.id,
    parent: f.parent ?? "",
    name: f.name,
    purpose: f.purpose,
    rationale: f.rationale ?? "",
    principles: (f.principles ?? []).join("\n"),
    status: f.status,
    valid_from: f.valid_from ?? "",
    valid_until: f.valid_until ?? "",
    review_at: f.review_at ?? "",
  };
}

export function newDraft(parent: string | null): Draft {
  return {
    id: "",
    parent: parent ?? "",
    name: "",
    purpose: "",
    rationale: "",
    principles: "",
    status: "active",
    valid_from: "",
    valid_until: "",
    review_at: "",
  };
}

const orNull = (s: string): string | null => (s.trim() === "" ? null : s);

export function fromDraft(d: Draft): PurposeFields {
  const principles = d.principles
    .split("\n")
    .map((p) => p.trim())
    .filter((p) => p !== "");
  return {
    id: d.id.trim(),
    parent: orNull(d.parent),
    name: d.name.trim(),
    purpose: d.purpose,
    rationale: orNull(d.rationale),
    principles: principles.length > 0 ? principles : null,
    status: d.status,
    valid_from: orNull(d.valid_from),
    valid_until: orNull(d.valid_until),
    review_at: orNull(d.review_at),
  };
}

/** 保存すると内容が変わるか（正規化後の値で比べる）。 */
export function isDirty(d: Draft, original: PurposeFields | null): boolean {
  if (original === null) return true;
  return JSON.stringify(fromDraft(d)) !== JSON.stringify(fromDraft(toDraft(original)));
}

/** IDの規則（仕様書 §4.1）。Rust側でも検証するが、入力中に知らせるために使う。 */
export function idProblem(id: string): string | null {
  const reserved = new Set(["con", "prn", "aux", "nul", ...[1, 2, 3, 4, 5, 6, 7, 8, 9].flatMap((n) => [`com${n}`, `lpt${n}`])]);
  if (id === "") return "IDを入力してください";
  if (id.length > 64) return "IDは64文字以内にしてください";
  if (/[A-Z]/.test(id)) return "大文字は使えません";
  if (!/^[a-z0-9-]+$/.test(id)) return "小文字英数字とハイフンだけが使えます";
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(id)) return "ハイフンは先頭・末尾・連続で使えません";
  if (reserved.has(id)) return "Windowsの予約名のため使えません";
  return null;
}

const ID_MAX = 64;

/** IDの末尾に付けるランダムな文字列（新規作成を始めたときに1回だけ作る）。 */
export function randomSuffix(len = 4, rand: () => number = Math.random): string {
  const chars = "abcdefghijkmnpqrstuvwxyz23456789"; // 紛らわしい l / o / 0 / 1 を除く
  let out = "";
  for (let i = 0; i < len; i++) out += chars[Math.floor(rand() * chars.length)];
  return out;
}

function slugify(s: string): string {
  return s
    .normalize("NFKC")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function clip(s: string, max: number): string {
  return s.slice(0, max).replace(/-+$/g, "");
}

/**
 * 表示名からIDの候補を作る。
 * - 英字を3文字以上含む表示名は、そのままスラッグにする（例: "Acme Jobs" → "acme-jobs"）。
 * - それ以外（日本語中心）は、親のID＋表示名中の英数字＋ランダムな文字列にする。
 * - 既存のIDと重なる場合は "-2" "-3" … を付ける。
 */
export function suggestId(name: string, parent: string | null, existing: Set<string>, suffix: string): string {
  const slug = slugify(name);
  const meaningful = (slug.match(/[a-z]/g) ?? []).length >= 3;
  let base: string;
  if (meaningful) {
    base = clip(slug, ID_MAX);
  } else {
    const tail = [clip(slug, 24), suffix].filter(Boolean).join("-");
    const head = parent ?? "purpose";
    base = `${clip(head, ID_MAX - tail.length - 1)}-${tail}`;
  }
  if (idProblem(base) !== null) base = `${clip(base, ID_MAX - suffix.length - 1)}-${suffix}`;
  if (!existing.has(base)) return base;
  for (let n = 2; ; n++) {
    const cand = `${clip(base, ID_MAX - String(n).length - 1)}-${n}`;
    if (!existing.has(cand)) return cand;
  }
}
