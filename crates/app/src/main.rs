mod board;

use board::{BoardView, COLS, ROWS, cell, hl};
use eframe::egui;

struct DelversApp {
    view: BoardView,
    backend: String,
    hovered: Option<(usize, usize)>,
}

impl DelversApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        board::init(rs);
        let backend = format!("{:?}", rs.adapter.get_info().backend);
        log::info!("wgpu backend: {backend}");
        Self {
            view: dummy_view(),
            backend,
            hovered: None,
        }
    }
}

/// ダミー盤面 (ゲームロジック実装までの仮データ)
fn dummy_view() -> BoardView {
    let mut v = BoardView::default();
    let i = |x: usize, y: usize| y * COLS + x;
    for y in 4..7 {
        for x in 0..3 {
            v.cells[i(x, y)] = cell::DOMAIN_P1;
            v.cells[i(COLS - 1 - x, y)] = cell::DOMAIN_P2;
        }
    }
    for x in 3..6 {
        v.cells[i(x, 5)] = cell::TILE_P1;
        v.cells[i(COLS - 1 - x, 5)] = cell::TILE_P2;
    }
    v.highlight[i(6, 4)] = hl::MOVE;
    v.highlight[i(6, 6)] = hl::MOVE;
    v.highlight[i(7, 5)] = hl::ATTACK;
    for x in 4..7 {
        v.highlight[i(x, 2)] = hl::PREVIEW;
        v.highlight[i(x, 8)] = hl::INVALID;
    }
    v
}

impl eframe::App for DelversApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label(format!(
                "Dice Delvers — backend: {} / hover: {:?}",
                self.backend, self.hovered
            ));

            // 13:11 の正方セルになるよう領域を確保
            let avail = ui.available_size();
            let cell = (avail.x / COLS as f32).min(avail.y / ROWS as f32);
            let size = egui::vec2(cell * COLS as f32, cell * ROWS as f32);
            let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());

            self.view.time = ui.input(|i| i.time) as f32;
            self.hovered = resp.hover_pos().and_then(|p| board::cell_at(rect, p));
            let painter = ui.painter_at(rect);
            board::paint(&painter, rect, &self.view);

            // ユニット (ダミー) は egui painter で重ねる
            for (x, y, color) in [
                (5, 5, egui::Color32::LIGHT_BLUE),
                (7, 5, egui::Color32::LIGHT_RED),
            ] {
                let r = board::cell_rect(rect, x, y);
                painter.circle_filled(r.center(), r.width() * 0.3, color);
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "W",
                    egui::FontId::proportional(r.width() * 0.35),
                    egui::Color32::BLACK,
                );
            }
            if let Some((x, y)) = self.hovered {
                painter.rect_stroke(
                    board::cell_rect(rect, x, y),
                    0.0,
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
        });
        // ponytail: 脈動アニメのため常時再描画。ハイライトが無いときは止める等は必要になってから
        ui.ctx().request_repaint();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Dice Delvers",
        options,
        Box::new(|cc| Ok(Box::new(DelversApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    console_log::init_with_level(log::Level::Info).ok();
    // WebOptions 既定の wgpu 設定は WebGPU 優先・WebGL2 フォールバック (wgpu の webgl feature は egui-wgpu 既定で有効)
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("the_canvas_id"))
            .expect("canvas not found")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("not a canvas");
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(DelversApp::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
