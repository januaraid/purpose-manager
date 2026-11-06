//! Tauri のコマンド層。ドメイン処理は `purpose-core` に置き、ここはOSとのやり取りだけを担う。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use purpose_core::{PurposeFields, PurposeText, SaveError, SaveOutcome, SaveRequest, Snapshot};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

/// 端末ごとのローカル設定（仕様書 §3.1：フォルダ選択履歴はアプリ側に保持する）。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    workspaces: Vec<String>,
    #[serde(default)]
    last: Option<String>,
}

#[derive(Default)]
struct WatchState(Mutex<Option<Debouncer<notify_debouncer_mini::notify::RecommendedWatcher>>>);

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

#[tauri::command(async)]
fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let path = settings_path(&app)?;
    match std::fs::read(&path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(_) => Ok(Settings::default()),
    }
}

#[tauri::command(async)]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_path(&app)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    purpose_core::store::atomic_write(&path, &json).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LoadFailure {
    NotFound { message: String },
    NotAWorkspace { message: String },
    Io { message: String },
}

#[tauri::command(async)]
fn load_workspace(root: String, today: Option<String>) -> Result<Snapshot, LoadFailure> {
    use purpose_core::LoadError;
    purpose_core::load(Path::new(&root), today.as_deref()).map_err(|e| match e {
        LoadError::NotFound(_) => LoadFailure::NotFound { message: e.to_string() },
        LoadError::NotAWorkspace(_) => LoadFailure::NotAWorkspace { message: e.to_string() },
        LoadError::Io(_) => LoadFailure::Io { message: e.to_string() },
    })
}

#[tauri::command(async)]
fn init_workspace(root: String, name: String) -> Result<(), SaveError> {
    purpose_core::init_workspace(Path::new(&root), &name)
}

#[tauri::command(async)]
fn save_purpose(root: String, request: SaveRequest) -> Result<SaveOutcome, SaveError> {
    purpose_core::save_purpose(Path::new(&root), request)
}

#[tauri::command(async)]
fn read_purpose_yaml(root: String, id: String) -> Result<PurposeText, SaveError> {
    purpose_core::read_purpose_text(Path::new(&root), &id)
}

#[tauri::command(async)]
fn save_purpose_yaml(root: String, id: String, text: String, expected_hash: String) -> Result<SaveOutcome, SaveError> {
    purpose_core::save_purpose_text(Path::new(&root), &id, &text, &expected_hash)
}

#[tauri::command(async)]
fn create_purpose_yaml(root: String, text: String) -> Result<SaveOutcome, SaveError> {
    purpose_core::create_purpose_text(Path::new(&root), &text)
}

#[tauri::command(async)]
fn purpose_yaml_template(fields: PurposeFields) -> Result<String, SaveError> {
    purpose_core::purpose_text_template(fields)
}

#[tauri::command(async)]
fn delete_purpose(root: String, id: String, expected_hash: String) -> Result<(), SaveError> {
    purpose_core::delete_purpose(Path::new(&root), &id, &expected_hash)
}

#[tauri::command(async)]
fn claude_md_snippet(root: String, id: String) -> String {
    purpose_core::harness::claude_md_snippet(Path::new(&root), &id)
}

#[tauri::command(async)]
fn concat_yaml(root: String, ids: Vec<String>, title: String) -> Result<String, String> {
    purpose_core::harness::concat_yaml(Path::new(&root), &ids, &title)
}

/// Workspaceの外部変更を監視し、`workspace-changed` イベントで知らせる。
/// 自分の保存による変更もイベントになるが、GUI側でハッシュを比べて区別する。
#[tauri::command(async)]
fn watch_workspace(app: AppHandle, state: State<'_, WatchState>, root: String) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    let root_path = PathBuf::from(&root);
    let emit_root = root.clone();
    let handle = app.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(300), move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let relevant = events.iter().any(|e| {
            let name = e.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            !name.ends_with(".tmp")
        });
        if relevant {
            let _ = handle.emit("workspace-changed", emit_root.clone());
        }
    })
    .map_err(|e| e.to_string())?;
    let watcher = debouncer.watcher();
    watcher.watch(&root_path, RecursiveMode::NonRecursive).map_err(|e| e.to_string())?;
    let purposes = purpose_core::workspace::purposes_dir(&root_path);
    if purposes.is_dir() {
        watcher.watch(&purposes, RecursiveMode::NonRecursive).map_err(|e| e.to_string())?;
    }
    *guard = Some(debouncer);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(WatchState::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            load_workspace,
            init_workspace,
            save_purpose,
            read_purpose_yaml,
            save_purpose_yaml,
            create_purpose_yaml,
            purpose_yaml_template,
            delete_purpose,
            claude_md_snippet,
            concat_yaml,
            watch_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
