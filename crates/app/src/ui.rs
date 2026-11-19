//! ゲーム画面 (タイトル・盤面・操作パネル・サイドパネル)。
//! ルール判定は持たず、`delvers-core` のクエリ (`reachable` / `attack_targets` / `is_legal_placement` など) に任せる。

use delvers_core::{
    Ability, Action, Cell, Combatant, DICE_FACES, DefenseChoice, Face, Game, MONSTERS, Outcome,
    Phase, Placement, Player, Pos, Rune, WARDEN_ATK, ai, monster, placement_cells,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, vec2};

use crate::board::{self, BoardView, CELLS, COLS, ROWS, cell, hl};

/// CPU が 1 手指すごとの待ち時間 (秒)。
const CPU_DELAY: f64 = 0.4;
/// エラートーストの表示時間 (秒)。
const TOAST_SECS: f64 = 3.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    VsCpu,
    Local,
}

pub enum Screen {
    Title,
    Play(Box<Play>),
}

/// UI 側だけが持つ選択状態。
struct Sel {
    /// 選択中の自ユニット / ウォーデン
    unit: Option<Combatant>,
    /// ツインストライク待ち (次に敵をクリックすると TwinStrike)
    twin: bool,
    shape: u8,
    rot: u8,
    flip: bool,
    /// レベル 1〜4 のダイス選択数 (合計 3)
    dice: [u8; 4],
}

pub struct Play {
    game: Game,
    mode: Mode,
    sel: Sel,
    next_cpu: f64,
    toast: Option<(String, f64)>,
    hover: Option<Pos>,
}

/// 対局の初期シード (時刻由来)。
fn seed() -> u64 {
    #[cfg(target_arch = "wasm32")]
    return eframe::web_sys::js_sys::Date::now() as u64;
    #[cfg(not(target_arch = "wasm32"))]
    return std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
}

// ---- 表示用の定数・テキスト ----

fn p_color(p: Player) -> Color32 {
    match p {
        Player::P1 => Color32::from_rgb(240, 100, 80),
        Player::P2 => Color32::from_rgb(70, 140, 240),
    }
}

fn ability_text(a: Ability) -> &'static str {
    match a {
        Ability::Swift => "Swift: 移動ルーン1個で2マス進む",
        Ability::Tunnel => "Tunnel: ユニットをすり抜けて移動できる",
        Ability::Wings => "Wings: 魔法1個で以後ずっと飛行になる",
        Ability::Flying => "Flying: 移動コスト2倍。飛行かウォーデンにしか攻撃されない",
        Ability::TwinStrike => "Twin Strike: 攻撃2個で同じ対象に2回攻撃",
        Ability::Ward => "Ward: 罠1個で被攻撃を1回無効化",
    }
}

fn rune_name(r: Rune) -> &'static str {
    match r {
        Rune::Move => "移動",
        Rune::Attack => "攻撃",
        Rune::Guard => "守護",
        Rune::Magic => "魔法",
        Rune::Trap => "罠",
    }
}

fn face_text(f: Face) -> String {
    match f {
        Face::Summon(l) => format!("召喚{l}"),
        Face::Rune(r, 1) => rune_name(r).to_owned(),
        Face::Rune(r, n) => format!("{}×{n}", rune_name(r)),
    }
}

/// 「召喚1(4面) 移動 守護×2」形式のダイス面一覧。
fn faces_text(level: u8) -> String {
    let mut out: Vec<(String, u8)> = Vec::new();
    for &f in &DICE_FACES[usize::from(level - 1)] {
        let t = face_text(f);
        match out.iter_mut().find(|(s, _)| *s == t) {
            Some((_, n)) => *n += 1,
            None => out.push((t, 1)),
        }
    }
    let parts: Vec<_> = out
        .into_iter()
        .map(|(t, n)| if n > 1 { format!("{t}({n}面)") } else { t })
        .collect();
    parts.join(" ")
}

fn phase_text(p: Phase) -> &'static str {
    match p {
        Phase::Roll => "ロール",
        Phase::Place { .. } => "召喚 (タイル配置)",
        Phase::Action => "行動",
        Phase::AwaitDefense { .. } => "防御選択",
        Phase::Finished(_) => "終局",
    }
}

fn comb_pos(g: &Game, c: Combatant) -> Option<Pos> {
    match c {
        Combatant::Unit(id) => g.unit(id).map(|u| u.pos),
        Combatant::Warden(p) => Some(p.domain()),
    }
}

// ---- 画面 ----

pub fn show(screen: &mut Screen, ui: &mut egui::Ui) {
    let next = match screen {
        Screen::Title => title(ui),
        Screen::Play(p) => p.show(ui),
    };
    if let Some(n) = next {
        *screen = n;
    }
}

fn title(ui: &mut egui::Ui) -> Option<Screen> {
    let mut next = None;
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.2);
            ui.label(egui::RichText::new("Dice Delvers").size(48.0).strong());
            ui.add_space(8.0);
            ui.label(
                "ダイスでルーンを集め、モンスターと迷宮タイルで相手のウォーデンを3回攻撃しよう。",
            );
            ui.add_space(24.0);
            let mut mode = None;
            if ui
                .add_sized([260.0, 44.0], egui::Button::new("対 CPU (あなた = P1)"))
                .clicked()
            {
                mode = Some(Mode::VsCpu);
            }
            ui.add_space(8.0);
            if ui
                .add_sized([260.0, 44.0], egui::Button::new("ローカル 2 人"))
                .clicked()
            {
                mode = Some(Mode::Local);
            }
            next = mode.map(|m| Screen::Play(Box::new(Play::new(m))));
        });
    });
    next
}

impl Play {
    fn new(mode: Mode) -> Self {
        Self {
            game: Game::new(seed()),
            mode,
            sel: Sel {
                unit: None,
                twin: false,
                shape: 0,
                rot: 0,
                flip: false,
                dice: [1, 1, 1, 0],
            },
            next_cpu: 0.0,
            toast: None,
            hover: None,
        }
    }

    fn human(&self, p: Player) -> bool {
        self.mode == Mode::Local || p == Player::P1
    }

    fn human_turn(&self) -> bool {
        self.game.to_act().is_some_and(|p| self.human(p))
    }

    fn show(&mut self, ui: &mut egui::Ui) -> Option<Screen> {
        let now = ui.input(|i| i.time);
        self.cpu_tick(now);
        if self.toast.as_ref().is_some_and(|t| t.1 < now) {
            self.toast = None;
        }
        // R / F キー (配置フェーズのみ)
        if self.human_turn() && matches!(self.game.phase(), Phase::Place { .. }) {
            if ui.input(|i| i.key_pressed(egui::Key::R)) {
                self.sel.rot = (self.sel.rot + 1) % 4;
            }
            if ui.input(|i| i.key_pressed(egui::Key::F)) {
                self.sel.flip = !self.sel.flip;
            }
        }

        let mut next = None;
        egui::Panel::right("side")
            .default_size(340.0)
            .show(ui, |ui| {
                if self.side(ui) {
                    next = Some(Screen::Title);
                }
            });
        egui::Panel::bottom("ctrl")
            .exact_size(190.0)
            .show(ui, |ui| {
                self.controls(ui, now);
            });
        egui::CentralPanel::default().show(ui, |ui| self.board(ui, now));

        if let Some(outcome) = self.game.outcome() {
            let text = match outcome {
                Outcome::Win(p) => format!("{} の勝ち！", p.name()),
                Outcome::Draw => "引き分け".to_owned(),
            };
            egui::Window::new("対局終了")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .show(ui.ctx(), |ui| {
                    ui.label(egui::RichText::new(text).size(28.0).strong());
                    ui.horizontal(|ui| {
                        if ui.button("もう一度").clicked() {
                            next = Some(Screen::Play(Box::new(Play::new(self.mode))));
                        }
                        if ui.button("タイトルへ").clicked() {
                            next = Some(Screen::Title);
                        }
                    });
                });
        }
        // ponytail: 脈動アニメと CPU ディレイのため常時再描画。必要になったら止める。
        ui.ctx().request_repaint();
        next
    }

    fn toast(&mut self, msg: impl Into<String>, now: f64) {
        self.toast = Some((msg.into(), now + TOAST_SECS));
    }

    /// 操作を core に適用。Err はトーストに出す。
    fn act(&mut self, a: Action, now: f64) {
        match self.game.apply(a) {
            Ok(()) => {
                self.sel.twin = false;
                let gone = match self.sel.unit {
                    Some(Combatant::Unit(id)) => self.game.unit(id).is_none(),
                    _ => false,
                };
                if gone || matches!(a, Action::EndTurn | Action::Roll { .. }) {
                    self.sel.unit = None;
                }
            }
            Err(e) => self.toast(e.to_string(), now),
        }
    }

    fn cpu_tick(&mut self, now: f64) {
        let Some(p) = self.game.to_act() else { return };
        if self.human(p) {
            self.next_cpu = now + CPU_DELAY;
        } else if now >= self.next_cpu {
            let a = ai::choose(&self.game);
            self.act(a, now);
            self.next_cpu = now + CPU_DELAY;
        }
    }

    // ---- 盤面 ----

    /// ホバー位置の召喚マスを基準にしたタイル配置候補。(盤上マス座標 (負あり), 合法な Placement)
    fn placement_preview(&self, hover: Pos) -> ([(isize, isize); 6], Option<Placement>) {
        let s = &self.sel;
        let (offs, _) = placement_cells(s.shape, s.rot, s.flip, (0, 0)).expect("shape は 0..11");
        let o = (
            hover.0 as isize - offs[0].0 as isize,
            hover.1 as isize - offs[0].1 as isize,
        );
        let cells = offs.map(|(x, y)| (o.0 + x as isize, o.1 + y as isize));
        let placement = (o.0 >= 0 && o.1 >= 0)
            .then_some(Placement {
                shape: s.shape,
                rot: s.rot,
                flip: s.flip,
                origin: (o.0 as usize, o.1 as usize),
            })
            .filter(|&p| self.game.is_legal_placement(p));
        (cells, placement)
    }

    fn board(&mut self, ui: &mut egui::Ui, now: f64) {
        let avail = ui.available_size();
        let c = (avail.x / COLS as f32).min(avail.y / ROWS as f32).max(10.0);
        let size = vec2(c * COLS as f32, c * ROWS as f32);
        let rect = Rect::from_center_size(ui.available_rect_before_wrap().center(), size);
        let resp = ui.allocate_rect(rect, Sense::click());
        let hover = resp.hover_pos().and_then(|p| board::cell_at(rect, p));
        self.hover = hover;

        let g = &self.game;
        let mut view = BoardView {
            time: now as f32,
            ..Default::default()
        };
        for (y, row) in g.board().iter().enumerate() {
            for (x, &cl) in row.iter().enumerate() {
                view.cells[y * COLS + x] = match cl {
                    Cell::Empty => cell::EMPTY,
                    Cell::Domain(Player::P1) => cell::DOMAIN_P1,
                    Cell::Domain(Player::P2) => cell::DOMAIN_P2,
                    Cell::Tile(Player::P1) => cell::TILE_P1,
                    Cell::Tile(Player::P2) => cell::TILE_P2,
                };
            }
        }
        let human = self.human_turn();
        let mut placement = None;
        let mut spawn = None;
        match g.phase() {
            Phase::Place { .. } if human => {
                if let Some(h) = hover {
                    let (cells, p) = self.placement_preview(h);
                    placement = p;
                    spawn = Some(h);
                    let v = if p.is_some() {
                        hl::PREVIEW
                    } else {
                        hl::INVALID
                    };
                    for (x, y) in cells {
                        if (0..COLS as isize).contains(&x) && (0..ROWS as isize).contains(&y) {
                            view.highlight[y as usize * COLS + x as usize] = v;
                        }
                    }
                }
            }
            Phase::Action if human => {
                if let Some(sel) = self.sel.unit {
                    if let Combatant::Unit(id) = sel {
                        for (x, y) in g.reachable(id).into_iter().map(|r| r.0) {
                            view.highlight[y * COLS + x] = hl::MOVE;
                        }
                    }
                    let targets = match (self.sel.twin, sel) {
                        (true, Combatant::Unit(id)) => g.twin_strike_targets(id),
                        _ => g.attack_targets(sel),
                    };
                    for (x, y) in targets.into_iter().filter_map(|t| comb_pos(g, t)) {
                        view.highlight[y * COLS + x] = hl::ATTACK;
                    }
                }
            }
            Phase::AwaitDefense { target, .. } => {
                if let Some((x, y)) = comb_pos(g, Combatant::Unit(target)) {
                    view.highlight[y * COLS + x] = hl::ATTACK;
                }
            }
            _ => {}
        }
        debug_assert_eq!(view.highlight.len(), CELLS);

        let painter = ui.painter_at(rect);
        board::paint(&painter, rect, &view);
        self.draw_units(&painter, rect);
        if let Some((x, y)) = spawn {
            // 召喚マスを金の枠で強調
            painter.rect_stroke(
                board::cell_rect(rect, x, y).shrink(2.0),
                2.0,
                Stroke::new(3.0, Color32::GOLD),
                StrokeKind::Inside,
            );
        } else if let Some((x, y)) = hover {
            painter.rect_stroke(
                board::cell_rect(rect, x, y),
                0.0,
                Stroke::new(2.0, Color32::WHITE),
                StrokeKind::Inside,
            );
        }

        if resp.clicked()
            && human
            && let Some(h) = hover
        {
            match self.game.phase() {
                Phase::Place { .. } => match placement {
                    Some(p) => self.act(p.into(), now),
                    None => self.toast("この場所には配置できません", now),
                },
                Phase::Action => self.click_action(h, now),
                _ => {}
            }
        }
    }

    fn click_action(&mut self, pos: Pos, now: f64) {
        let g = &self.game;
        let mut action = None;
        if let Some(sel) = self.sel.unit {
            let twin_id = match (self.sel.twin, sel) {
                (true, Combatant::Unit(id)) => Some(id),
                _ => None,
            };
            let targets = match twin_id {
                Some(id) => g.twin_strike_targets(id),
                None => g.attack_targets(sel),
            };
            if let Some(&t) = targets.iter().find(|&&t| comb_pos(g, t) == Some(pos)) {
                action = Some(match twin_id {
                    Some(id) => Action::TwinStrike {
                        attacker: id,
                        target: t,
                    },
                    None => Action::Attack {
                        attacker: sel,
                        target: t,
                    },
                });
            } else if let Combatant::Unit(id) = sel
                && g.reachable(id).iter().any(|r| r.0 == pos)
            {
                action = Some(Action::Move { unit: id, to: pos });
            }
        }
        if let Some(a) = action {
            return self.act(a, now);
        }
        // 自ユニット / ウォーデンの選択切り替え
        let own = g.turn_player();
        let new = if let Some(u) = g.unit_at(pos).filter(|u| u.owner == own) {
            Some(Combatant::Unit(u.id))
        } else if pos == own.domain() {
            Some(Combatant::Warden(own))
        } else {
            None
        };
        self.sel.unit = if new == self.sel.unit { None } else { new };
        self.sel.twin = false;
    }

    fn draw_units(&self, painter: &egui::Painter, rect: Rect) {
        let g = &self.game;
        let selected = self.sel.unit;
        let ring = |painter: &egui::Painter, center: Pos2, r: f32| {
            painter.circle_stroke(center, r + 3.0, Stroke::new(3.0, Color32::YELLOW));
        };
        for p in [Player::P1, Player::P2] {
            // ウォーデン (ドメイン上のひし形 + LP)
            let (x, y) = p.domain();
            let r = board::cell_rect(rect, x, y);
            let ctr = r.center() + vec2(0.0, -r.height() * 0.08);
            let rad = r.width() * 0.34;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    ctr + vec2(0.0, -rad),
                    ctr + vec2(rad, 0.0),
                    ctr + vec2(0.0, rad),
                    ctr + vec2(-rad, 0.0),
                ],
                p_color(p),
                Stroke::new(2.0, Color32::BLACK),
            ));
            painter.text(
                ctr,
                Align2::CENTER_CENTER,
                "W",
                FontId::proportional(r.width() * 0.32),
                Color32::BLACK,
            );
            painter.text(
                r.center_bottom() + vec2(0.0, -1.0),
                Align2::CENTER_BOTTOM,
                format!("LP {}", g.lp(p)),
                FontId::proportional(r.width() * 0.22),
                Color32::WHITE,
            );
            if selected == Some(Combatant::Warden(p)) {
                ring(painter, ctr, rad);
            }
        }
        for u in g.units() {
            let r = board::cell_rect(rect, u.pos.0, u.pos.1);
            let ctr = r.center() + vec2(0.0, -r.height() * 0.06);
            let rad = r.width() * 0.32;
            if u.flying {
                // 飛行の印: 左右の翼
                for s in [-1.0_f32, 1.0] {
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            ctr + vec2(s * rad * 0.7, -rad * 0.2),
                            ctr + vec2(s * rad * 1.45, -rad * 0.9),
                            ctr + vec2(s * rad * 1.2, rad * 0.2),
                        ],
                        Color32::WHITE,
                        Stroke::new(1.0, Color32::BLACK),
                    ));
                }
            }
            painter.circle(ctr, rad, p_color(u.owner), Stroke::new(2.0, Color32::BLACK));
            painter.text(
                ctr,
                Align2::CENTER_CENTER,
                u.name.chars().next().unwrap_or('?'),
                FontId::proportional(r.width() * 0.4),
                Color32::BLACK,
            );
            // HP バー
            let bar = Rect::from_min_size(
                r.left_bottom() + vec2(r.width() * 0.1, -r.height() * 0.14),
                vec2(r.width() * 0.8, r.height() * 0.09),
            );
            let ratio = f32::from(u.hp) / f32::from(u.max_hp);
            painter.rect_filled(bar, 0.0, Color32::from_gray(40));
            let mut fill = bar;
            fill.set_width(bar.width() * ratio);
            let col = if ratio > 0.5 {
                Color32::from_rgb(80, 200, 90)
            } else if ratio > 0.25 {
                Color32::from_rgb(230, 190, 50)
            } else {
                Color32::from_rgb(220, 60, 60)
            };
            painter.rect_filled(fill, 0.0, col);
            // 行動済みの印: 移動済み = 青、攻撃済み = 赤
            let dot = r.width() * 0.07;
            if u.moved {
                painter.circle_filled(
                    r.left_top() + vec2(dot * 1.5, dot * 1.5),
                    dot,
                    Color32::LIGHT_BLUE,
                );
            }
            if u.attacked {
                painter.circle_filled(
                    r.right_top() + vec2(-dot * 1.5, dot * 1.5),
                    dot,
                    Color32::LIGHT_RED,
                );
            }
            if selected == Some(Combatant::Unit(u.id)) {
                ring(painter, ctr, rad);
            }
        }
    }

    // ---- サイドパネル ----

    /// 「タイトルへ」が押されたら true。
    fn side(&mut self, ui: &mut egui::Ui) -> bool {
        let mut back = false;
        ui.horizontal(|ui| {
            ui.heading("Dice Delvers");
            if ui.button("タイトルへ").clicked() {
                back = true;
            }
        });
        let g = &self.game;
        for p in [Player::P1, Player::P2] {
            let who = match (self.mode, p) {
                (Mode::VsCpu, Player::P1) => " (あなた)",
                (Mode::VsCpu, Player::P2) => " (CPU)",
                _ => "",
            };
            let active = g.to_act() == Some(p);
            ui.separator();
            ui.horizontal(|ui| {
                let t = egui::RichText::new(format!("{}{who}", p.name()))
                    .color(p_color(p))
                    .strong();
                ui.label(t);
                if active {
                    ui.label("◀ 操作中");
                }
                ui.label(format!("LP {}", g.lp(p)));
            });
            let r = g.runes(p);
            ui.label(format!(
                "ルーン  移動 {}  攻撃 {}  守護 {}  魔法 {}  罠 {}",
                r.movement, r.attack, r.guard, r.magic, r.trap
            ));
            for lv in 1..=3u8 {
                let (id, &(name, lv, hp, atk, def, ab)) = monster(p, lv).unwrap();
                let state = match g.unit(id) {
                    Some(u) => format!("場に出ている HP {}/{}", u.hp, u.max_hp),
                    None if g.has_summoned(p, lv) => "撃破".to_owned(),
                    None => "未召喚".to_owned(),
                };
                ui.label(format!("Lv{lv} {name} [{state}]  HP{hp} ATK{atk} DEF{def}"));
                ui.weak(ability_text(ab));
            }
        }
        ui.separator();
        ui.label("ログ");
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink(false)
            .show(ui, |ui| {
                for l in g.log() {
                    ui.label(l);
                }
            });
        back
    }

    // ---- 下部操作パネル ----

    fn controls(&mut self, ui: &mut egui::Ui, now: f64) {
        let g = &self.game;
        let phase = g.phase();
        ui.horizontal_wrapped(|ui| {
            if let Some(p) = g.to_act() {
                ui.label(
                    egui::RichText::new(format!("手番: {}", g.turn_player().name()))
                        .color(p_color(g.turn_player()))
                        .strong(),
                );
                ui.label(format!("第{}手番 / {}", g.turn(), phase_text(phase)));
                if !self.human(p) {
                    ui.label("— CPU が考え中…");
                }
            }
            if let Some(roll) = g.last_roll() {
                ui.label("出目:");
                for d in roll {
                    ui.label(
                        egui::RichText::new(format!(" Lv{} {} ", d.level, face_text(d.face)))
                            .background_color(Color32::from_gray(60)),
                    );
                }
            }
        });
        // トースト / ホバー中ユニットの情報
        if let Some((msg, _)) = &self.toast {
            ui.colored_label(Color32::LIGHT_RED, msg);
        } else if let Some(u) = self.hover.and_then(|h| g.unit_at(h)) {
            ui.label(format!(
                "{} Lv{} HP {}/{} ATK {} DEF {}{}",
                u.name,
                u.level,
                u.hp,
                u.max_hp,
                u.atk,
                u.def,
                if u.flying { " 飛行" } else { "" }
            ));
        } else {
            ui.label("");
        }
        ui.separator();
        if !self.human_turn() {
            return;
        }
        match phase {
            Phase::Roll => self.roll_ui(ui, now),
            Phase::Place { level } => self.place_ui(ui, level),
            Phase::Action => self.action_ui(ui, now),
            Phase::AwaitDefense {
                attacker,
                target,
                hits_left,
            } => self.defense_ui(ui, now, attacker, target, hits_left),
            Phase::Finished(_) => {}
        }
    }

    fn roll_ui(&mut self, ui: &mut egui::Ui, now: f64) {
        let total: u8 = self.sel.dice.iter().sum();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                for lv in 1..=4u8 {
                    ui.horizontal(|ui| {
                        let n = &mut self.sel.dice[usize::from(lv - 1)];
                        ui.label(format!("Lv{lv}"));
                        if ui.add_enabled(*n > 0, egui::Button::new("−")).clicked() {
                            *n -= 1;
                        }
                        ui.label(n.to_string());
                        if ui.add_enabled(total < 3, egui::Button::new("+")).clicked() {
                            *n += 1;
                        }
                        ui.weak(faces_text(lv));
                    });
                }
            });
            ui.vertical(|ui| {
                ui.label(format!("選択中 {total}/3 個 (同レベル複数可)"));
                if ui
                    .add_enabled(
                        total == 3,
                        egui::Button::new("ロール！").min_size(vec2(120.0, 36.0)),
                    )
                    .clicked()
                {
                    let mut dice = [0u8; 3];
                    let levels = (1..=4u8).flat_map(|lv| {
                        std::iter::repeat_n(lv, usize::from(self.sel.dice[usize::from(lv - 1)]))
                    });
                    for (d, lv) in dice.iter_mut().zip(levels) {
                        *d = lv;
                    }
                    self.act(Action::Roll { dice }, now);
                }
            });
        });
    }

    fn place_ui(&mut self, ui: &mut egui::Ui, level: u8) {
        let name = monster(self.game.turn_player(), level).unwrap().1.0;
        ui.label(format!(
            "Lv{level} {name} を召喚: 形を選び、盤面でタイルを置く場所をクリック (金枠 = 召喚マス)"
        ));
        ui.horizontal(|ui| {
            for shape in 0..11u8 {
                let s = &mut self.sel;
                if shape_button(ui, shape, s.rot, s.flip, s.shape == shape).clicked() {
                    s.shape = shape;
                }
            }
            ui.vertical(|ui| {
                if ui.button("回転 (R)").clicked() {
                    self.sel.rot = (self.sel.rot + 1) % 4;
                }
                if ui.button("反転 (F)").clicked() {
                    self.sel.flip = !self.sel.flip;
                }
            });
        });
    }

    fn action_ui(&mut self, ui: &mut egui::Ui, now: f64) {
        let g = &self.game;
        let mut act = None;
        let mut twin_toggle = false;
        ui.horizontal(|ui| {
            match self.sel.unit {
                Some(c @ Combatant::Warden(_)) => {
                    let status = if g.warden_attacked() {
                        " 攻撃済み"
                    } else {
                        ""
                    };
                    ui.label(format!("選択中: {} ATK {WARDEN_ATK}{status}", c.name()))
                }
                Some(c) => ui.label(format!("選択中: {}", c.name())),
                None => ui.label("自分のユニットかウォーデンをクリックして選択"),
            };
            if let Some(Combatant::Unit(id)) = self.sel.unit {
                if g.unit(id).is_some_and(|u| u.ability == Ability::TwinStrike) {
                    let can = !g.twin_strike_targets(id).is_empty();
                    let label = if self.sel.twin {
                        "ツインストライク解除"
                    } else {
                        "ツインストライク (攻撃×2)"
                    };
                    if ui
                        .add_enabled(can || self.sel.twin, egui::Button::new(label))
                        .clicked()
                    {
                        twin_toggle = true;
                    }
                }
                if g.can_use_wings(id) && ui.button("ウィングス (魔法×1)").clicked() {
                    act = Some(Action::Wings { unit: id });
                }
            }
            if ui.button("ターン終了").clicked() {
                act = Some(Action::EndTurn);
            }
        });
        ui.weak("水色 = 移動可能 / 赤 = 攻撃対象。クリックで実行。移動は1ユニット1回、攻撃は1ユニット1回。");
        if twin_toggle {
            self.sel.twin = !self.sel.twin;
        }
        if let Some(a) = act {
            self.act(a, now);
        }
    }

    fn defense_ui(
        &mut self,
        ui: &mut egui::Ui,
        now: f64,
        attacker: Combatant,
        target: usize,
        hits_left: u8,
    ) {
        let g = &self.game;
        ui.label(format!(
            "{} が {} を攻撃！ (残り {hits_left} 撃) 防御を選んでください:",
            attacker.name(),
            MONSTERS[target].0
        ));
        let mut act = None;
        ui.horizontal(|ui| {
            for c in g.defense_choices() {
                let name = match c {
                    DefenseChoice::None => "防御しない",
                    DefenseChoice::Guard => "守護 (守護ルーン1)",
                    DefenseChoice::Ward => "Ward (罠ルーン1)",
                };
                let dmg = g.pending_damage(c).unwrap_or(0);
                if ui.button(format!("{name} → 被ダメ {dmg}")).clicked() {
                    act = Some(Action::Defend(c));
                }
            }
        });
        if let Some(a) = act {
            self.act(a, now);
        }
    }
}

/// 形の縮小プレビューつきボタン (金色のマス = 召喚マス)。
fn shape_button(
    ui: &mut egui::Ui,
    shape: u8,
    rot: u8,
    flip: bool,
    selected: bool,
) -> egui::Response {
    const CS: f32 = 9.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(5.0 * CS + 8.0, 5.0 * CS + 8.0), Sense::click());
    let bg = if selected {
        Color32::from_rgb(60, 80, 120)
    } else if resp.hovered() {
        Color32::from_gray(60)
    } else {
        Color32::from_gray(35)
    };
    ui.painter().rect_filled(rect, 4.0, bg);
    if selected {
        ui.painter().rect_stroke(
            rect,
            4.0,
            Stroke::new(2.0, Color32::WHITE),
            StrokeKind::Inside,
        );
    }
    if let Some((cells, _)) = placement_cells(shape, rot, flip, (0, 0)) {
        let w = cells.iter().map(|c| c.0).max().unwrap_or(0) + 1;
        let h = cells.iter().map(|c| c.1).max().unwrap_or(0) + 1;
        let origin = rect.center() - vec2(w as f32, h as f32) * CS / 2.0;
        for (i, (x, y)) in cells.into_iter().enumerate() {
            let r = Rect::from_min_size(origin + vec2(x as f32, y as f32) * CS, vec2(CS, CS));
            let col = if i == 0 {
                Color32::GOLD
            } else {
                Color32::from_gray(200)
            };
            ui.painter().rect_filled(r.shrink(0.5), 1.0, col);
        }
    }
    resp
}
