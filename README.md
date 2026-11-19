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

## CPU 対戦の計測

`cargo run --release -p delvers-sim -- 0..2000 greedy random ember-frost` はシード 0〜1999 を 1 回ずつ実行する。
`0+2000` でも同じ範囲になる。方策は先手、後手の順で各々 `greedy` または `random`。
陣営指定を省略すると通常の Ember 対 Frost。同陣営戦と逆順も選べる (`--help`)。
シードはダイス用、ビット反転したシードは両席が共有する方策抽選用。ランダムは現在の合法な Action 値ごとに等確率。
ロールは順序付き 64 通り、配置は形・向き・反転・原点ごとに数え、EndTurn の重みは 1。

- 勝率・引分率・未連結率の分母は対戦数。終局理由は件数。手番数は P1 の初手を 1 とし、終局した手番を含む。
- 未連結とは、両ドメインの間にタイルだけを上下左右に通る経路がないこと。所有者・ユニットは無視し、タイルは消えないので終局時に調べる。
- 非活動率は、強制召喚・移動・攻撃宣言 (ウォーデン・Twin Strike を含む) のいずれも行わなかった手番数を、全試合の総手番数で割る。防御は手番を増やさず、Wings 単独は活動に数えない。移動や攻撃を自ら見送った場合も含む。
- 召喚数は撃破済みを含む席ごとの 1 試合平均。貪欲ロール例外は「未召喚の最小レベル (全召喚済みなら 2) を 3 個」と異なるロールを数える。
