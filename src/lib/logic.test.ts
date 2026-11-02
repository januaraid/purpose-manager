import { describe, expect, it } from "vitest";
import { fromDraft, idProblem, isDirty, newDraft, randomSuffix, suggestId, toDraft } from "./draft";
import {
  ancestorsIfParent,
  buildTree,
  canHaveChild,
  descendants,
  treeIndent,
  layoutTree,
  folderName,
  parentCandidates,
  referencingChildren,
  searchPurposes,
  todayLocal,
  visibleChildren,
  worstSeverity,
} from "./tree";
import { clampZoom, stepZoom } from "./zoom";
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

  it("allows children only under loadable, resolved nodes", () => {
    expect(canHaveChild(model.byId.get("a1"))).toBe(true);
    expect(canHaveChild(model.byId.get("old"))).toBe(true);
    expect(canHaveChild(model.byId.get("orphan"))).toBe(false);
    expect(canHaveChild(model.byId.get("broken"))).toBe(false);
    expect(canHaveChild(undefined)).toBe(false);
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

describe("zoom", () => {
  it("steps through presets", () => {
    expect(stepZoom(1, 1)).toBe(1.1);
    expect(stepZoom(1, -1)).toBe(0.9);
    expect(stepZoom(1.2, 1)).toBe(1.25);
    expect(stepZoom(2, 1)).toBe(2);
    expect(stepZoom(0.8, -1)).toBe(0.8);
  });
  it("clamps stored values", () => {
    expect(clampZoom("x")).toBe(1);
    expect(clampZoom(9)).toBe(2);
    expect(clampZoom(0.1)).toBe(0.8);
  });
});

describe("searchPurposes", () => {
  const model = buildTree({
    ...snap,
    nodes: snap.nodes.map((n) =>
      n.id === "a1" && n.fields
        ? { ...n, fields: { ...n.fields, name: "検索体験", purpose: "求職者が希望に合う求人をすぐ見つけられる", principles: ["応募数より満足度"] } }
        : n,
    ),
  });
  const ids = (q: string) => searchPurposes(model, q).map((h) => h.id);

  it("matches names, ids and body text", () => {
    expect(ids("検索")).toEqual(["a1"]);
    expect(ids("ｏｌｄ")).toEqual(["old", "old1"]);
    const h = searchPurposes(model, "満足度")[0];
    expect(h).toMatchObject({ id: "a1", field: "principles" });
    expect(h.snippet).toContain("満足度");
  });

  it("requires all terms and ranks name hits first", () => {
    expect(ids("検索 希望")).toEqual(["a1"]);
    expect(ids("検索 存在しない")).toEqual([]);
    expect(ids("a")[0]).toBe("a");
    expect(ids("  ")).toEqual([]);
  });
});

describe("treeIndent", () => {
  it("narrows the step for deeper levels", () => {
    expect(treeIndent(0)).toBe(8);
    expect(treeIndent(3)).toBe(56);
    expect(treeIndent(6)).toBe(86);
    expect(treeIndent(10)).toBe(110);
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

describe("suggestId", () => {
  const none = new Set<string>();

  it("slugifies latin names", () => {
    expect(suggestId("Acme Jobs", null, none, "k3f9")).toBe("acme-jobs");
    expect(suggestId("  Ｓｅａｒｃｈ　ＵＸ 2.0!! ", "x", none, "k3f9")).toBe("search-ux-2-0");
  });

  it("falls back to parent + digits + suffix for Japanese names", () => {
    expect(suggestId("検索結果の関連度改善（2026年下期）", "search-experience", none, "k3f9")).toBe("search-experience-2026-k3f9");
    expect(suggestId("検索体験", "job-matching", none, "k3f9")).toBe("job-matching-k3f9");
    expect(suggestId("会社", null, none, "k3f9")).toBe("purpose-k3f9");
    expect(suggestId("", null, none, "k3f9")).toBe("purpose-k3f9");
  });

  it("avoids existing ids and reserved names", () => {
    expect(suggestId("Acme Jobs", null, new Set(["acme-jobs", "acme-jobs-2"]), "k3f9")).toBe("acme-jobs-3");
    expect(suggestId("CON", null, none, "k3f9")).toBe("con-k3f9");
  });

  it("stays within 64 chars and valid", () => {
    const long = suggestId("a".repeat(100), null, none, "k3f9");
    expect(long.length).toBeLessThanOrEqual(64);
    const jp = suggestId("目的", "p".repeat(64), none, "k3f9");
    expect(jp.length).toBeLessThanOrEqual(64);
    expect(idProblem(jp)).toBeNull();
    expect(idProblem(suggestId("a--b__c", null, none, "k3f9"))).toBeNull();
  });

  it("generates suffixes from the safe alphabet", () => {
    expect(randomSuffix(4, () => 0)).toBe("aaaa");
    expect(randomSuffix(6)).toMatch(/^[a-km-np-z2-9]{6}$/);
  });
});
