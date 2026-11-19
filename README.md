# Dice Delvers

ダイスでルーンを集め、モンスターを召喚し、迷宮を伸ばして相手のウォーデンを倒す 2 人用ボードゲーム。
Rust + WASM + WebGPU (WGSL) + Tauri v2。

ルール: [SPEC.md](SPEC.md)

## 開発

前提: Rust (stable, `rustup target add wasm32-unknown-unknown`)、[Trunk](https://trunkrs.dev/) (`brew install trunk` 等)、Node.js。
初回のみ `npm install` で Tauri CLI をローカルに入れる。

| 目的 | コマンド |
| --- | --- |
| ネイティブ実行 (wgpu) | `cargo run -p delvers-app` |
| ブラウザ実行 (WebGPU 優先 / WebGL2 フォールバック) | `cd crates/app && trunk serve` → <http://localhost:8080> |
| デスクトップ (Tauri) 開発 | `npx tauri dev` (trunk serve を自動起動) |
| デスクトップ (Tauri) ビルド | `npx tauri build` (trunk build --release を自動実行) |
| チェック | `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` |

- `src-tauri` は `crates/app/dist` を配信するため、`cargo clippy --workspace` 等の前に一度 `trunk build` しておく必要がある。
- 盤面は `crates/app/src/board.wgsl` (自前 WGSL) を egui_wgpu のペイントコールバックで描画。画面左上に実際の wgpu backend を表示する。
