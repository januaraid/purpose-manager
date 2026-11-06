import { describe, expect, it } from "vitest";
import { indentEdit, type TextEdit } from "./indent";

/** 編集を当てた結果を、選択範囲を [ ] で示した文字列にする。 */
function apply(value: string, start: number, end: number, outdent: boolean): string {
  const e: TextEdit | null = indentEdit(value, start, end, outdent);
  const v = e ? value.slice(0, e.from) + e.insert + value.slice(e.to) : value;
  const [s, t] = e ? [e.selStart, e.selEnd] : [start, end];
  return v.slice(0, s) + "[" + v.slice(s, t) + "]" + v.slice(t);
}
/** "[" と "]" で選択範囲を示した文字列から、値と選択範囲を取り出す。 */
function parse(marked: string): [string, number, number] {
  const s = marked.indexOf("[");
  const t = marked.indexOf("]") - 1;
  return [marked.replace("[", "").replace("]", ""), s, t];
}
const tab = (m: string) => apply(...parse(m), false);
const shiftTab = (m: string) => apply(...parse(m), true);

describe("indentEdit", () => {
  it("カーソル位置に空白2つを入れる", () => {
    expect(tab("a:\n[]b")).toBe("a:\n  []b");
    expect(tab("ab[c]d")).toBe("ab  []d");
  });

  it("複数行を選ぶと各行を字下げし、選択を保つ", () => {
    expect(tab("x\n[a\nb]\ny")).toBe("x\n[  a\n  b]\ny");
    expect(tab("a[b\nc]d")).toBe("  a[b\n  c]d");
  });

  it("選択の終わりが行頭ちょうどなら、その行は字下げしない", () => {
    expect(tab("[a\nb\n]c")).toBe("[  a\n  b\n]c");
  });

  it("空行は字下げしない", () => {
    expect(tab("[a\n\nb]")).toBe("[  a\n\n  b]");
  });

  it("Shift+Tab で行頭の空白を最大2つ取る", () => {
    expect(shiftTab("    a[]b")).toBe("  a[]b");
    expect(shiftTab(" []ab")).toBe("[]ab");
    expect(shiftTab("[  a\n b\nc]")).toBe("[a\nb\nc]");
    expect(shiftTab("\t[]a")).toBe("[]a");
  });

  it("字上げできる空白がなければ何もしない", () => {
    expect(indentEdit("ab", 1, 1, true)).toBeNull();
  });

  it("カーソルが行頭の空白の中にあっても行頭より前に出ない", () => {
    expect(shiftTab("x\n [] a")).toBe("x\n[]a");
  });
});
