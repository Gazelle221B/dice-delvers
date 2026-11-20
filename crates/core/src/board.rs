use std::collections::VecDeque;

use crate::{Ability, Cell, Game, Phase, Placement, Player, Pos, RuleError, UnitId};

/// 立方体の展開図 11 種。各形の添字 0 が召喚マス。
pub const SHAPES: [[Pos; 6]; 11] = [
    [(0, 0), (0, 1), (1, 1), (2, 1), (3, 1), (0, 2)],
    [(0, 0), (0, 1), (1, 1), (2, 1), (3, 1), (1, 2)],
    [(0, 0), (0, 1), (1, 1), (2, 1), (3, 1), (2, 2)],
    [(0, 0), (0, 1), (1, 1), (2, 1), (3, 1), (3, 2)],
    [(0, 0), (0, 1), (1, 1), (2, 1), (1, 2), (1, 3)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (2, 2), (1, 3)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (2, 2), (2, 3)],
    [(0, 0), (0, 1), (1, 1), (1, 2), (1, 3), (2, 3)],
    [(0, 0), (0, 1), (0, 2), (1, 2), (1, 3), (1, 4)],
    [(1, 0), (0, 1), (1, 1), (2, 1), (1, 2), (1, 3)],
    [(1, 0), (0, 1), (1, 1), (1, 2), (2, 2), (1, 3)],
];

/// 左右反転、時計回りの回転、外接矩形の左上を origin に移す、の順に変換する。
/// 盤外の座標もプレビュー用に返す。形・回転が範囲外か、座標が桁あふれすると None。
pub fn placement_cells(shape: u8, rot: u8, flip: bool, origin: Pos) -> Option<([Pos; 6], Pos)> {
    if rot >= 4 {
        return None;
    }
    let offsets = SHAPES.get(usize::from(shape))?.map(|(x, y)| {
        let x = if flip { -(x as isize) } else { x as isize };
        let y = y as isize;
        match rot {
            0 => (x, y),
            1 => (-y, x),
            2 => (-x, -y),
            _ => (y, -x),
        }
    });
    let left = offsets.iter().map(|&(x, _)| x).min()?;
    let top = offsets.iter().map(|&(_, y)| y).min()?;
    let mut cells = [(0, 0); 6];
    for (cell, (x, y)) in cells.iter_mut().zip(offsets) {
        *cell = (
            origin.0.checked_add((x - left) as usize)?,
            origin.1.checked_add((y - top) as usize)?,
        );
    }
    Some((cells, cells[0]))
}

impl Game {
    /// 盤内の上下左右の隣接マスを左・右・上・下の順に返す。
    pub(crate) fn neighbors(&self, (x, y): Pos) -> impl Iterator<Item = Pos> + use<> {
        let (w, h) = (self.rules.width, self.rules.height);
        [
            x.checked_sub(1).map(|x| (x, y)),
            (x + 1 < w).then_some((x + 1, y)),
            y.checked_sub(1).map(|y| (x, y)),
            (y + 1 < h).then_some((x, y + 1)),
        ]
        .into_iter()
        .flatten()
    }

    /// 召喚配置フェーズで選べる候補を返す。
    pub fn legal_placements(&self) -> Vec<Placement> {
        if matches!(self.phase, Phase::Place { .. }) {
            self.placements(self.turn_player).collect()
        } else {
            Vec::new()
        }
    }

    pub fn is_legal_placement(&self, placement: Placement) -> bool {
        matches!(self.phase, Phase::Place { .. })
            && placement.cells().is_some_and(|(cells, _)| {
                self.allows_placement(&cells, self.placement_contact(self.turn_player))
            })
    }

    fn placement_contact(&self, player: Player) -> Cell {
        if self.roster(player).all(|(id, _)| !self.is_summoned(id)) {
            Cell::Domain(player)
        } else {
            Cell::Tile(player)
        }
    }

    fn allows_placement(&self, cells: &[Pos; 6], contact: Cell) -> bool {
        cells.iter().all(|&pos| self.cell(pos) == Some(Cell::Empty))
            && cells
                .iter()
                .any(|&pos| self.neighbors(pos).any(|p| self.cell(p) == Some(contact)))
    }

    pub(super) fn placements(&self, player: Player) -> impl Iterator<Item = Placement> + '_ {
        let contact = self.placement_contact(player);
        let (w, h) = (self.rules.width, self.rules.height);
        (0..SHAPES.len() as u8).flat_map(move |shape| {
            (0..4).flat_map(move |rot| {
                [false, true].into_iter().flat_map(move |flip| {
                    let (offsets, _) = placement_cells(shape, rot, flip, (0, 0)).unwrap();
                    let right = offsets.iter().map(|p| p.0).max().unwrap();
                    let bottom = offsets.iter().map(|p| p.1).max().unwrap();
                    (0..h.saturating_sub(bottom)).flat_map(move |y| {
                        (0..w.saturating_sub(right)).filter_map(move |x| {
                            let cells = offsets.map(|(dx, dy)| (x + dx, y + dy));
                            self.allows_placement(&cells, contact).then_some(Placement {
                                shape,
                                rot,
                                flip,
                                origin: (x, y),
                            })
                        })
                    })
                })
            })
        })
    }

    pub(super) fn place(&mut self, level: u8, placement: Placement) -> Result<(), RuleError> {
        let (cells, spawn) = placement.cells().ok_or(RuleError::InvalidPlacement)?;
        if !self.allows_placement(&cells, self.placement_contact(self.turn_player)) {
            return Err(RuleError::InvalidPlacement);
        }
        for (x, y) in cells {
            self.board[y][x] = Cell::Tile(self.turn_player);
        }
        self.summon(level, spawn);
        self.phase = Phase::Action;
        Ok(())
    }

    /// この手番で移動できる停止マスと、それぞれの最小移動ルーン消費。
    pub fn reachable(&self, id: UnitId) -> Vec<(Pos, u8)> {
        self.reachable_with_budget(id, self.runes(self.turn_player).movement)
    }

    /// 指定した移動ルーン数で到達できるマス。占有・能力・行動済みの制約は通常どおり。
    pub fn reachable_with_budget(&self, id: UnitId, budget: u8) -> Vec<(Pos, u8)> {
        let Some(unit) = self.unit(id) else {
            return Vec::new();
        };
        if self.phase != Phase::Action || unit.owner != self.turn_player || unit.moved {
            return Vec::new();
        }
        let budget = budget.min(self.rules.rune_cap);
        let (x, y) = unit.pos;
        let mut seen = vec![false; self.rules.width * self.rules.height];
        seen[y * self.rules.width + x] = true;
        let mut queue = VecDeque::from([(unit.pos, 0u8)]);
        let mut reachable = Vec::new();
        while let Some((pos, steps)) = queue.pop_front() {
            let next_steps = steps + 1;
            let cost = if unit.flying {
                next_steps * 2
            } else if unit.ability == Ability::Swift {
                next_steps.div_ceil(2)
            } else {
                next_steps
            };
            if cost > budget {
                continue;
            }
            for (x, y) in self.neighbors(pos) {
                let index = y * self.rules.width + x;
                if seen[index] || !matches!(self.board[y][x], Cell::Tile(_)) {
                    continue;
                }
                seen[index] = true;
                let occupant = self.unit_at((x, y));
                if occupant.is_some_and(|other| other.owner != unit.owner)
                    && unit.ability != Ability::Tunnel
                {
                    continue;
                }
                queue.push_back(((x, y), next_steps));
                if occupant.is_none() {
                    reachable.push(((x, y), cost));
                }
            }
        }
        reachable
    }
}

impl Game {
    /// 両ドメイン間に上下左右のタイル経路があるか。所有者と占有ユニットは問わない。
    pub fn domains_connected(&self) -> bool {
        let goal = self.domain(Player::P2);
        let mut stack = vec![self.domain(Player::P1)];
        let mut seen = vec![false; self.rules.width * self.rules.height];
        while let Some(pos) = stack.pop() {
            for (x, y) in self.neighbors(pos) {
                if (x, y) == goal {
                    return true;
                }
                let index = y * self.rules.width + x;
                if !seen[index] && matches!(self.board[y][x], Cell::Tile(_)) {
                    seen[index] = true;
                    stack.push((x, y));
                }
            }
        }
        false
    }
}
