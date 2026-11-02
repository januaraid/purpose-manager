import { getCurrentWebview } from "@tauri-apps/api/webview";

/** 選べる表示倍率。 */
export const ZOOM_STEPS = [0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2] as const;

export function clampZoom(z: unknown): number {
  if (typeof z !== "number" || !Number.isFinite(z)) return 1;
  return Math.min(ZOOM_STEPS[ZOOM_STEPS.length - 1], Math.max(ZOOM_STEPS[0], z));
}

/** 一段階大きく（dir=1）または小さく（dir=-1）した倍率。 */
export function stepZoom(current: number, dir: 1 | -1): number {
  if (dir > 0) return ZOOM_STEPS.find((s) => s > current + 1e-6) ?? ZOOM_STEPS[ZOOM_STEPS.length - 1];
  return [...ZOOM_STEPS].reverse().find((s) => s < current - 1e-6) ?? ZOOM_STEPS[0];
}

/** ウィンドウ全体の表示倍率を変える（ブラウザの拡大縮小と同じ仕組みなので、配置の計算は崩れない）。 */
export async function applyZoom(z: number): Promise<void> {
  await getCurrentWebview().setZoom(z);
}
