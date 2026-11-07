/** YAMLの字下げの幅。 */
export const INDENT = "  ";

/** テキストの [from, to) を insert に置き換え、選択範囲を [selStart, selEnd) にする編集。 */
export interface TextEdit {
  from: number;
  to: number;
  insert: string;
  selStart: number;
  selEnd: number;
}

/**
 * Tab（outdent=false）／Shift+Tab（outdent=true）を押したときの編集を求める。
 * - 複数行を選んでいるとき、または Shift+Tab のときは、選択範囲にかかる各行を字下げ／字上げする。
 * - それ以外は、カーソル位置（選択中なら選択範囲）を字下げの空白に置き換える。
 * 変わるところがなければ `null`。
 */
export function indentEdit(value: string, start: number, end: number, outdent: boolean): TextEdit | null {
  const multiline = value.slice(start, end).includes("\n");
  if (!outdent && !multiline) {
    const pos = start + INDENT.length;
    return { from: start, to: end, insert: INDENT, selStart: pos, selEnd: pos };
  }

  // 選択範囲にかかる行全体。選択の終わりが行頭ちょうどなら、その行は含めない。
  const from = value.lastIndexOf("\n", start - 1) + 1;
  const lastPos = end > start && value[end - 1] === "\n" ? end - 1 : end;
  const nl = value.indexOf("\n", lastPos);
  const to = nl === -1 ? value.length : nl;

  const lines = value.slice(from, to).split("\n");
  let startShift = 0;
  let total = 0;
  const changed = lines.map((line, i) => {
    let delta: number;
    let next: string;
    if (outdent) {
      const remove = line.startsWith("\t") ? 1 : line.length - line.replace(/^ {1,2}/, "").length;
      next = line.slice(remove);
      delta = -remove;
    } else {
      next = line === "" ? line : INDENT + line;
      delta = next.length - line.length;
    }
    if (i === 0) startShift = delta;
    total += delta;
    return next;
  });
  if (total === 0) return null;

  const insert = changed.join("\n");
  // 選択が行頭から始まるなら行頭のままにする（行全体を選んだ状態を保つ）。
  // 字上げで、選択の始まりが行頭より前に出ないようにする。
  const selStart = start === from && start !== end ? from : Math.max(from, start + startShift);
  const selEnd = start === end ? selStart : Math.max(selStart, end + total);
  return { from, to, insert, selStart, selEnd };
}
