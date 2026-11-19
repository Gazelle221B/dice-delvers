// 盤面 13×11 を 1 枚の全面三角形で描く。セルごとの状態は uniform の u32 (下位 8bit = セル状態, 次の 8bit = ハイライト)。
// WebGL2 互換のため storage buffer は使わず、u32 を vec4<u32> に 4 個ずつ詰めた uniform 配列にする。

struct Board {
    cells: array<vec4<u32>, 36>, // 13*11 = 143 個 (+1 パディング)
    params: vec4<f32>,           // x = time (秒)
};

@group(0) @binding(0) var<uniform> board: Board;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // 全面三角形。uv は左上 (0,0) → 右下 (1,1)
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: VsOut;
    o.pos = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    o.uv = uv;
    return o;
}

const COLS: u32 = 13u;
const ROWS: u32 = 11u;
const P1: vec3<f32> = vec3<f32>(0.25, 0.55, 0.95);
const P2: vec3<f32> = vec3<f32>(0.95, 0.40, 0.30);

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let g = in.uv * vec2<f32>(f32(COLS), f32(ROWS));
    let px = fwidth(g); // 微分は分岐前 (uniform control flow) で取る
    let c = min(vec2<u32>(g), vec2<u32>(COLS - 1u, ROWS - 1u));
    let idx = c.y * COLS + c.x;
    let v = board.cells[idx / 4u][idx % 4u];
    let state = v & 0xffu;
    let hl = (v >> 8u) & 0xffu;
    let f = fract(g);
    let d = max(abs(f.x - 0.5), abs(f.y - 0.5)); // セル中心からの距離 (正方形)
    let t = board.params.x;

    // 下地の市松
    var col = select(vec3<f32>(0.16, 0.15, 0.19), vec3<f32>(0.20, 0.19, 0.23), ((c.x + c.y) & 1u) == 0u);

    // セル状態: 1/2 = ドメイン, 3/4 = タイル
    let pc = select(P2, P1, state == 1u || state == 3u);
    if state == 1u || state == 2u {
        col = mix(col, pc, 0.22);
    } else if state == 3u || state == 4u {
        let body = 1.0 - smoothstep(0.40, 0.43, d);
        let bevel = smoothstep(0.30, 0.40, d) * body;
        col = mix(col, pc * 0.75, body);
        col = mix(col, pc, bevel * 0.6);
    }

    // ハイライト: 1 移動可 / 2 攻撃可 / 3 配置プレビュー / 4 不正配置
    if hl != 0u {
        var hc = vec3<f32>(0.35, 0.85, 1.0);
        if hl == 2u { hc = vec3<f32>(1.0, 0.25, 0.2); }
        if hl == 3u { hc = vec3<f32>(0.35, 1.0, 0.45); }
        if hl == 4u { hc = vec3<f32>(0.55, 0.55, 0.55); }
        let pulse = 0.5 + 0.5 * sin(t * 4.0);
        let ring = smoothstep(0.38, 0.47, d);
        col = mix(col, hc, 0.15 + 0.2 * pulse + ring * 0.45);
        if hl == 4u && fract((f.x + f.y) * 3.0) < 0.5 {
            col = col * 0.6; // 斜線
        }
    }

    // グリッド線 (約 1px)
    let e = min(f, 1.0 - f) / px;
    let line = 1.0 - smoothstep(0.5, 1.5, min(e.x, e.y));
    col = mix(col, vec3<f32>(0.05, 0.05, 0.07), line);

    return vec4<f32>(col, 1.0);
}
