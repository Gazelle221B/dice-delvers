mod board;
mod ui;

use eframe::egui;

struct DelversApp {
    screen: ui::Screen,
}

impl DelversApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rs = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        board::init(rs);
        log::info!("wgpu backend: {:?}", rs.adapter.get_info().backend);
        // 日本語表示用: サブセット済み Noto Sans JP (OFL) を既存フォントのフォールバックに追加
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "noto-sans-jp".into(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/NotoSansJP-Subset.ttf"
            ))),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("noto-sans-jp".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        Self {
            screen: ui::Screen::Title,
        }
    }
}

impl eframe::App for DelversApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::show(&mut self.screen, ui);
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
