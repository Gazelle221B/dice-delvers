// Windows のリリースビルドでコンソールを出さない
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// crates/app の WASM フロントエンドを配信するだけのシェル。IPC コマンド・プラグインは無し。
fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
