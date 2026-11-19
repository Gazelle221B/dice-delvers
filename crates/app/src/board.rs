//! 盤面の WGSL 描画 (egui_wgpu のペイントコールバック)。
//! 構成は egui 公式 `custom3d_wgpu` 例に倣う: 初期化時に RenderState からパイプラインを作って
//! `callback_resources` に保存し、毎フレーム `prepare` で uniform を書いて `paint` で描く。

use eframe::egui;
use eframe::egui_wgpu::{self, wgpu};

pub const COLS: usize = 13;
pub const ROWS: usize = 11;
pub const CELLS: usize = COLS * ROWS;

/// `BoardView::cells` の値。
pub mod cell {
    pub const EMPTY: u8 = 0;
    pub const DOMAIN_P1: u8 = 1;
    pub const DOMAIN_P2: u8 = 2;
    pub const TILE_P1: u8 = 3;
    pub const TILE_P2: u8 = 4;
}

/// `BoardView::highlight` の値。
pub mod hl {
    pub const NONE: u8 = 0;
    pub const MOVE: u8 = 1;
    pub const ATTACK: u8 = 2;
    pub const PREVIEW: u8 = 3;
    pub const INVALID: u8 = 4;
}

/// シェーダへ渡す盤面データ。インデックスは `y * COLS + x`。
#[derive(Clone)]
pub struct BoardView {
    pub cells: [u8; CELLS],
    pub highlight: [u8; CELLS],
    /// アニメ用の秒数
    pub time: f32,
}

impl Default for BoardView {
    fn default() -> Self {
        Self {
            cells: [cell::EMPTY; CELLS],
            highlight: [hl::NONE; CELLS],
            time: 0.0,
        }
    }
}

/// uniform のサイズ: vec4<u32> × 36 (セル) + vec4<f32> (params)
const UNIFORM_WORDS: usize = 36 * 4 + 4;

impl BoardView {
    fn pack(&self) -> [u32; UNIFORM_WORDS] {
        let mut w = [0u32; UNIFORM_WORDS];
        for (i, (&c, &h)) in self.cells.iter().zip(&self.highlight).enumerate() {
            w[i] = u32::from(c) | (u32::from(h) << 8);
        }
        w[36 * 4] = self.time.to_bits();
        w
    }
}

/// 画面上の盤面矩形 `rect` 内の点 `pos` をセル座標 (x, y) に変換。盤外なら `None`。
pub fn cell_at(rect: egui::Rect, pos: egui::Pos2) -> Option<(usize, usize)> {
    if !rect.contains(pos) {
        return None;
    }
    let x = ((pos.x - rect.min.x) / rect.width() * COLS as f32) as usize;
    let y = ((pos.y - rect.min.y) / rect.height() * ROWS as f32) as usize;
    Some((x.min(COLS - 1), y.min(ROWS - 1)))
}

/// セル (x, y) の画面上の矩形 (ユニット等を egui painter で重ねる用)。
pub fn cell_rect(rect: egui::Rect, x: usize, y: usize) -> egui::Rect {
    let size = egui::vec2(rect.width() / COLS as f32, rect.height() / ROWS as f32);
    egui::Rect::from_min_size(
        rect.min + egui::vec2(x as f32 * size.x, y as f32 * size.y),
        size,
    )
}

struct BoardResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}

/// 起動時に一度呼ぶ。
pub fn init(rs: &egui_wgpu::RenderState) {
    let device = &rs.device;
    let shader = device.create_shader_module(wgpu::include_wgsl!("board.wgsl"));
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("board"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("board"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("board"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(rs.target_format.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("board"),
        size: (UNIFORM_WORDS * 4) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("board"),
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    rs.renderer
        .write()
        .callback_resources
        .insert(BoardResources {
            pipeline,
            bind_group,
            uniform,
        });
}

// ponytail: uniform buffer は 1 つだけなので 1 フレームに描ける盤面は 1 枚。複数必要になったら盤面ごとにバッファを持つ。
struct BoardCallback([u32; UNIFORM_WORDS]);

impl egui_wgpu::CallbackTrait for BoardCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        res: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let r: &BoardResources = res.get().expect("board::init not called");
        queue.write_buffer(&r.uniform, 0, bytemuck::cast_slice(&self.0));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        res: &egui_wgpu::CallbackResources,
    ) {
        let r: &BoardResources = res.get().expect("board::init not called");
        pass.set_pipeline(&r.pipeline);
        pass.set_bind_group(0, &r.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// `rect` に盤面を描く (viewport は egui_wgpu が rect に合わせる)。
pub fn paint(painter: &egui::Painter, rect: egui::Rect, view: &BoardView) {
    painter.add(egui_wgpu::Callback::new_paint_callback(
        rect,
        BoardCallback(view.pack()),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_at_maps_corners_and_outside() {
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(130.0, 110.0));
        assert_eq!(cell_at(r, egui::pos2(10.0, 20.0)), Some((0, 0)));
        assert_eq!(cell_at(r, egui::pos2(139.9, 129.9)), Some((12, 10)));
        assert_eq!(cell_at(r, egui::pos2(140.0, 130.0)), Some((12, 10))); // 右下端は内側扱い
        assert_eq!(cell_at(r, egui::pos2(25.0, 45.0)), Some((1, 2)));
        assert_eq!(cell_at(r, egui::pos2(9.0, 50.0)), None);
        assert_eq!(cell_rect(r, 1, 2).min, egui::pos2(20.0, 40.0));
    }

    #[test]
    fn pack_layout() {
        let mut v = BoardView {
            time: 1.5,
            ..Default::default()
        };
        v.cells[142] = cell::TILE_P2;
        v.highlight[142] = hl::INVALID;
        let w = v.pack();
        assert_eq!(w[142], 4 | (4 << 8));
        assert_eq!(f32::from_bits(w[144]), 1.5);
    }
}
