import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PurposeFields, PurposeText, SaveOutcome, SaveRequest, Settings, Snapshot } from "./types";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  loadWorkspace: (root: string, today: string) => invoke<Snapshot>("load_workspace", { root, today }),
  initWorkspace: (root: string, name: string) => invoke<void>("init_workspace", { root, name }),
  savePurpose: (root: string, request: SaveRequest) => invoke<SaveOutcome>("save_purpose", { root, request }),
  readPurposeYaml: (root: string, id: string) => invoke<PurposeText>("read_purpose_yaml", { root, id }),
  savePurposeYaml: (root: string, id: string, text: string, expectedHash: string) =>
    invoke<SaveOutcome>("save_purpose_yaml", { root, id, text, expectedHash }),
  createPurposeYaml: (root: string, text: string) => invoke<SaveOutcome>("create_purpose_yaml", { root, text }),
  purposeYamlTemplate: (fields: PurposeFields) => invoke<string>("purpose_yaml_template", { fields }),
  deletePurpose: (root: string, id: string, expectedHash: string) =>
    invoke<void>("delete_purpose", { root, id, expectedHash }),
  claudeMdSnippet: (root: string, id: string) => invoke<string>("claude_md_snippet", { root, id }),
  concatYaml: (root: string, ids: string[], title: string) => invoke<string>("concat_yaml", { root, ids, title }),
  watchWorkspace: (root: string) => invoke<void>("watch_workspace", { root }),
  onWorkspaceChanged: (handler: (root: string) => void): Promise<UnlistenFn> =>
    listen<string>("workspace-changed", (e) => handler(e.payload)),
};

/** Tauriのコマンドが返したエラーを、表示用の文字列にする。 */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e && typeof e.message === "string") return e.message;
  return JSON.stringify(e);
}
