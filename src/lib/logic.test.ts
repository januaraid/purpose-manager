import { describe, expect, it } from "vitest";
import { fromDraft, idProblem, isDirty, newDraft, toDraft } from "./draft";
import {
  ancestorsIfParent,
  buildTree,
  descendants,
  layoutTree,
  folderName,
  parentCandidates,
  referencingChildren,
  todayLocal,
  visibleChildren,
  worstSeverity,
} from "./tree";
import type { PurposeFields, PurposeNode, Snapshot } from "./types";

function fields(id: string, parent: string | null, status: "active" | "archived" = "active"): PurposeFields {
  return {
    id,
    parent,
    name: id.toUpperCase(),
    purpose: `${id} の目的`,
    rationale: null,
    principles: null,
    status,
    valid_from: null,
    valid_until: null,
    review_at: null,
  };
}

function node(id: string, parent: string | null, extra: Partial<PurposeNode> = {}): PurposeNode {
  return {
    id,
    file_name: `${id}.yaml`,
    path: `/ws/purposes/${id}.yaml`,
    hash: `h-${id}`,
    has_comments: false,
    fields: fields(id, parent),
    raw_name: id.toUpperCase(),
    raw_parent: parent,
    unknown_fields: [],
    resolved: true,
    ancestors: [],
    children: [],
    effective: "active",
    inactive_because: null,
    diagnostics: [],
    ...extra,
  };
}

// root ─┬─ a ── a1
//       └─ old(archived) ── old1
// orphan（親なし）、broken（読み込めない）
const snap: Snapshot = {
  root: "/ws",
  name: "WS",
  diagnostics: [],
  nodes: [
    node("root", null, { children: ["a", "old"] }),
    node("a", "root", { ancestors: ["root"], children: ["a1"] }),
    node("a1", "a", { ancestors: ["root", "a"] }),
    node("old", "root", { ancestors: ["root"], children: ["old1"], effective: "archived", fields: fields("old", "root", "archived") }),
    node("old1", "old", { ancestors: ["root", "old"], effective: "inherited_inactive", inactive_because: "old" }),
    node("orphan", "missing", { resolved: false, effective: "unknown" }),
    node("broken", null, { fields: null, raw_parent: "a1", effective: "unknown", resolved: false }),
  ],
};

describe("tree", () => {
  const model = buildTree(snap);

  it("groups nodes", () => {
    expect(model.roots).toEqual(["root"]);
    expect(model.unresolved).toEqual(["orphan"]);
    expect(model.broken).toEqual(["broken"]);
  });

  it("hides archived unless requested", () => {
    expect(visibleChildren(model, "root", false)).toEqual(["a"]);
    expect(visibleChildren(model, "root", true)).toEqual(["a", "old"]);
  });

  it("collects descendants", () => {
    expect(descendants(model, "root").sort()).toEqual(["a", "a1", "old", "old1"]);
  });

  it("excludes self and descendants from parent candidates", () => {
    const ids = parentCandidates(model, "a").map((n) => n.id);
    expect(ids).not.toContain("a");
    expect(ids).not.toContain("a1");
    expect(ids).not.toContain("orphan");
    expect(ids).not.toContain("broken");
    expect(ids).toContain("root");
    expect(parentCandidates(model, null).map((n) => n.id)).toContain("a1");
  });

  it("computes new ancestors for a parent change", () => {
    expect(ancestorsIfParent(model, "a1")).toEqual(["root", "a", "a1"]);
    expect(ancestorsIfParent(model, null)).toEqual([]);
  });

  it("finds references from broken files too", () => {
    expect(referencingChildren(snap, "a1")).toEqual(["broken"]);
  });

  it("ranks severity", () => {
    const d = (severity: "error" | "warning" | "info") => ({ severity, code: "x", message: "", file: null, field: null });
    expect(worstSeverity([])).toBeNull();
    expect(worstSeverity([d("info"), d("error"), d("warning")])).toBe("error");
  });

  it("formats helpers", () => {
    expect(todayLocal(new Date(2026, 0, 5))).toBe("2026-01-05");
    expect(folderName("C:\\Users\\me\\work-purposes")).toBe("work-purposes");
    expect(folderName("/home/me/ws/")).toBe("ws");
  });
});

describe("draft", () => {
  it("round-trips fields", () => {
    const f: PurposeFields = { ...fields("x", "root"), rationale: "背景", principles: ["一", "二"], valid_from: "2026-10-01" };
    expect(fromDraft(toDraft(f))).toEqual(f);
  });

  it("normalizes blanks to null", () => {
    const d = { ...newDraft(null), id: " x ", name: "X", purpose: "P", principles: "\n  a \n\n", rationale: "  " };
    const f = fromDraft(d);
    expect(f.id).toBe("x");
    expect(f.parent).toBeNull();
    expect(f.principles).toEqual(["a"]);
    expect(f.rationale).toBeNull();
  });

  it("detects changes", () => {
    const f = fields("x", null);
    const d = toDraft(f);
    expect(isDirty(d, f)).toBe(false);
    expect(isDirty({ ...d, principles: "\n" }, f)).toBe(false);
    expect(isDirty({ ...d, name: "Y" }, f)).toBe(true);
    expect(isDirty(d, null)).toBe(true);
  });

  it("checks id rules", () => {
    expect(idProblem("job-matching")).toBeNull();
    for (const bad of ["", "Abc", "a_b", "-a", "a--b", "con", "a".repeat(65)]) {
      expect(idProblem(bad)).not.toBeNull();
    }
  });
});

describe("layoutTree", () => {
  const model = buildTree(snap);

  it("centers parents over children", () => {
    const l = layoutTree(model, model.roots, true);
    const at = (id: string) => l.nodes.find((n) => n.id === id)!;
    expect(at("a1")).toMatchObject({ depth: 2, row: 0 });
    expect(at("old1")).toMatchObject({ depth: 2, row: 1 });
    expect(at("a").row).toBe(0);
    expect(at("root")).toMatchObject({ depth: 0, row: 0.5 });
    expect(l.depths).toBe(3);
    expect(l.rows).toBe(2);
    expect(l.edges).toContainEqual({ from: "root", to: "old" });
  });

  it("hides archived subtrees", () => {
    const l = layoutTree(model, model.roots, false);
    expect(l.nodes.map((n) => n.id).sort()).toEqual(["a", "a1", "root"]);
  });

  it("separates multiple roots", () => {
    const l = layoutTree(model, ["a", "old"], true);
    expect(l.nodes.find((n) => n.id === "old1")!.row).toBe(1.5);
  });
});
