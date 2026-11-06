/** いちばん近いスクロールする祖先。 */
function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const { overflowY } = getComputedStyle(p);
    if ((overflowY === "auto" || overflowY === "scroll") && p.scrollHeight > p.clientHeight) return p;
  }
  return document.scrollingElement as HTMLElement | null;
}

/**
 * textarea の高さを内容の行数に合わせる。`rows` の高さより小さくはしない。
 * 値を引数に渡すと（`use:autoHeight={value}`）、入力や破棄・再読み込みで値が変わるたびに合わせ直す。
 * 幅が変わって折り返しが変わったときも合わせ直す。
 * 非表示のまま作られた場合は大きさを測れないので、表示されたときに合わせる。
 */
export function autoHeight(el: HTMLTextAreaElement, _value?: unknown) {
  const fit = () => {
    if (el.clientWidth === 0) return;
    // 一度高さを戻して測るときに、画面がスクロールして跳ねないようにする
    const scroller = scrollParent(el);
    const top = scroller?.scrollTop ?? 0;
    el.style.height = "auto";
    const border = el.offsetHeight - el.clientHeight;
    el.style.height = `${el.scrollHeight + border}px`;
    if (scroller && scroller.scrollTop !== top) scroller.scrollTop = top;
  };
  // 高さは内容で決まるので、手で変えるつまみと縦のスクロールバーは出さない
  el.style.resize = "none";
  el.style.overflowY = "hidden";
  let lastWidth = -1;
  const observer = new ResizeObserver(() => {
    if (el.clientWidth === lastWidth) return;
    lastWidth = el.clientWidth;
    fit();
  });
  observer.observe(el);
  return {
    update: fit,
    destroy() {
      observer.disconnect();
    },
  };
}
