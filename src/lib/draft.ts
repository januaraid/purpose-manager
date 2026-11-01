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
