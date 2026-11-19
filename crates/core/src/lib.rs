//! ダイス・デルヴァーズのルールエンジン。

#![forbid(unsafe_code)]

pub mod ai;
mod board;
#[cfg(test)]
mod tests;

pub use board::{SHAPES, placement_cells};

/// 盤面の列数。
pub const W: usize = 13;
/// 盤面の行数。
pub const H: usize = 11;
/// ウォーデンの攻撃力。
pub const WARDEN_ATK: u8 = 10;

pub type Pos = (usize, usize);
/// 席とレベルで決まる、対局中に再利用されないユニット ID。
pub type UnitId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    P1,
    P2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faction {
    Ember,
    Frost,
}

const DEFAULT_FACTIONS: [Faction; 2] = [Faction::Ember, Faction::Frost];

impl Player {
    pub const fn index(self) -> usize {
        self as usize
    }

    /// 通常対局での表示名。計測用の陣営設定には Game::player_name を使う。
    pub const fn name(self) -> &'static str {
        self.name_with(DEFAULT_FACTIONS)
    }

    const fn name_with(self, factions: [Faction; 2]) -> &'static str {
        match (self, factions[self.index()]) {
            (Self::P1, Faction::Ember) => "P1 Ember",
            (Self::P1, Faction::Frost) => "P1 Frost",
            (Self::P2, Faction::Ember) => "P2 Ember",
            (Self::P2, Faction::Frost) => "P2 Frost",
        }
    }

    pub const fn opponent(self) -> Self {
        match self {
            Self::P1 => Self::P2,
            Self::P2 => Self::P1,
        }
    }

    pub const fn domain(self) -> Pos {
        match self {
            Self::P1 => (0, 5),
            Self::P2 => (W - 1, 5),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Empty,
    Domain(Player),
    Tile(Player),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rune {
    Move,
    Attack,
    Guard,
    Magic,
    Trap,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunePool {
    pub movement: u8,
    pub attack: u8,
    pub guard: u8,
    pub magic: u8,
    pub trap: u8,
}

impl RunePool {
    pub fn get(self, rune: Rune) -> u8 {
        let mut pool = self;
        *pool.amount_mut(rune)
    }

    fn amount_mut(&mut self, rune: Rune) -> &mut u8 {
        match rune {
            Rune::Move => &mut self.movement,
            Rune::Attack => &mut self.attack,
            Rune::Guard => &mut self.guard,
            Rune::Magic => &mut self.magic,
            Rune::Trap => &mut self.trap,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Summon(u8),
    Rune(Rune, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DieRoll {
    pub level: u8,
    pub face: Face,
}

/// レベル 1〜4 の面を順に収録。
pub const DICE_FACES: [[Face; 6]; 4] = {
    use Face::{Rune as Run, Summon};
    use Rune::{Attack, Guard, Magic, Move, Trap};
    [
        [
            Summon(1),
            Summon(1),
            Summon(1),
            Summon(1),
            Run(Move, 1),
            Run(Guard, 2),
        ],
        [
            Summon(2),
            Summon(2),
            Summon(2),
            Run(Move, 2),
            Run(Attack, 2),
            Run(Magic, 1),
        ],
        [
            Summon(3),
            Summon(3),
            Run(Move, 1),
            Run(Move, 1),
            Run(Attack, 1),
            Run(Trap, 2),
        ],
        [
            Summon(4),
            Run(Move, 2),
            Run(Attack, 1),
            Run(Guard, 1),
            Run(Magic, 2),
            Run(Trap, 1),
        ],
    ]
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ability {
    Swift,
    Tunnel,
    Wings,
    Flying,
    TwinStrike,
    Ward,
}

/// 陣営・レベル順のモンスター定義 (名前、レベル、HP、ATK、DEF、能力)。
pub const MONSTERS: [Monster; 6] = [
    ("Bramble Squire", 1, 20, 10, 10, Ability::Swift),
    ("Cinder Hound", 2, 30, 20, 10, Ability::Tunnel),
    ("Ash Colossus", 3, 40, 30, 20, Ability::Wings),
    ("Rime Sprite", 1, 10, 10, 10, Ability::Flying),
    ("Glacier Duelist", 2, 30, 20, 10, Ability::TwinStrike),
    ("Frost Leviathan", 3, 40, 30, 20, Ability::Ward),
];

pub type Monster = (&'static str, u8, u8, u8, u8, Ability);

/// 通常対局のユニット ID と定義。陣営指定のある対局には Game::monster を使う。
pub fn monster(player: Player, level: u8) -> Option<(UnitId, &'static Monster)> {
    roster_monster(DEFAULT_FACTIONS, player, level)
}

fn roster_monster(
    factions: [Faction; 2],
    player: Player,
    level: u8,
) -> Option<(UnitId, &'static Monster)> {
    if !(1..=3).contains(&level) {
        return None;
    }
    let id = player.index() * 3 + usize::from(level - 1);
    Some((
        id,
        &MONSTERS[factions[player.index()] as usize * 3 + usize::from(level - 1)],
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    pub id: UnitId,
    pub owner: Player,
    pub name: &'static str,
    pub level: u8,
    pub hp: u8,
    pub max_hp: u8,
    pub atk: u8,
    pub def: u8,
    pub ability: Ability,
    pub pos: Pos,
    pub flying: bool,
    pub moved: bool,
    pub attacked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combatant {
    Unit(UnitId),
    Warden(Player),
}

impl Combatant {
    /// 通常対局での表示名。陣営指定のある対局には Game::combatant_name を使う。
    pub fn name(self) -> String {
        self.name_with(DEFAULT_FACTIONS)
    }

    fn name_with(self, factions: [Faction; 2]) -> String {
        match self {
            Self::Unit(id) => MONSTERS[factions[id / 3] as usize * 3 + id % 3]
                .0
                .to_owned(),
            Self::Warden(player) => format!("{} のウォーデン", player.name_with(factions)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefenseChoice {
    None,
    Guard,
    Ward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Win(Player),
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    LpZero,
    Stalemate,
    TurnLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Roll,
    Place {
        level: u8,
    },
    Action,
    /// hits_left は現在の一撃を含む残り攻撃回数。
    AwaitDefense {
        attacker: Combatant,
        target: UnitId,
        hits_left: u8,
    },
    Finished(Outcome),
}

/// SHAPES の添字、時計回りの四分の一回転数、反転、外接矩形の左上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub shape: u8,
    pub rot: u8,
    pub flip: bool,
    pub origin: Pos,
}

impl Placement {
    /// 配置候補の 6 マスと召喚マスを返す。盤面の占有状態は参照しない。
    pub fn cells(self) -> Option<([Pos; 6], Pos)> {
        placement_cells(self.shape, self.rot, self.flip, self.origin)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Roll {
        dice: [u8; 3],
    },
    Place {
        shape: u8,
        rot: u8,
        flip: bool,
        origin: Pos,
    },
    Move {
        unit: UnitId,
        to: Pos,
    },
    Attack {
        attacker: Combatant,
        target: Combatant,
    },
    TwinStrike {
        attacker: UnitId,
        target: Combatant,
    },
    Wings {
        unit: UnitId,
    },
    Defend(DefenseChoice),
    EndTurn,
}

impl From<Placement> for Action {
    fn from(placement: Placement) -> Self {
        Self::Place {
            shape: placement.shape,
            rot: placement.rot,
            flip: placement.flip,
            origin: placement.origin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
    WrongPhase,
    InvalidDice,
    InvalidPlacement,
    InvalidMove,
    InvalidAttack,
    InvalidAbility,
    InvalidDefense,
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::WrongPhase => "現在のフェーズでは実行できません",
            Self::InvalidDice => "ダイスのレベルは 1〜4 です",
            Self::InvalidPlacement => "この場所に配置できません",
            Self::InvalidMove => "このマスには移動できません",
            Self::InvalidAttack => "この組み合わせでは攻撃できません",
            Self::InvalidAbility => "この能力は使用できません",
            Self::InvalidDefense => "この防御は使用できません",
        })
    }
}

impl std::error::Error for RuleError {}

#[derive(Debug, Clone)]
pub struct Game {
    factions: [Faction; 2],
    rng: fastrand::Rng,
    board: [[Cell; W]; H],
    units: Vec<Unit>,
    runes: [RunePool; 2],
    lp: [u8; 2],
    summoned: [u8; 2],
    warden_attacked: bool,
    turn_player: Player,
    turn: u16,
    phase: Phase,
    last_roll: Option<[DieRoll; 3]>,
    log: Vec<String>,
    logging: bool,
    end_reason: Option<EndReason>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self::with_factions(seed, DEFAULT_FACTIONS)
    }

    pub fn with_factions(seed: u64, factions: [Faction; 2]) -> Self {
        let mut board = [[Cell::Empty; W]; H];
        for player in [Player::P1, Player::P2] {
            let (x, y) = player.domain();
            board[y][x] = Cell::Domain(player);
        }
        Self {
            factions,
            rng: fastrand::Rng::with_seed(seed),
            board,
            units: Vec::new(),
            runes: [RunePool::default(); 2],
            lp: [3; 2],
            summoned: [0; 2],
            warden_attacked: false,
            turn_player: Player::P1,
            turn: 1,
            phase: Phase::Roll,
            last_roll: None,
            log: Vec::new(),
            logging: true,
            end_reason: None,
        }
    }

    pub fn monster(&self, player: Player, level: u8) -> Option<(UnitId, &'static Monster)> {
        roster_monster(self.factions, player, level)
    }

    pub fn player_name(&self, player: Player) -> &'static str {
        player.name_with(self.factions)
    }

    pub fn combatant_name(&self, combatant: Combatant) -> String {
        combatant.name_with(self.factions)
    }

    /// 行優先（board[y][x]）の盤面。ユニットの位置は units で参照する。
    pub fn board(&self) -> &[[Cell; W]; H] {
        &self.board
    }

    pub fn cell(&self, (x, y): Pos) -> Option<Cell> {
        self.board.get(y)?.get(x).copied()
    }

    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    pub fn unit(&self, id: UnitId) -> Option<&Unit> {
        self.units.iter().find(|unit| unit.id == id)
    }

    pub fn unit_at(&self, pos: Pos) -> Option<&Unit> {
        self.units.iter().find(|unit| unit.pos == pos)
    }

    pub fn runes(&self, player: Player) -> RunePool {
        self.runes[player.index()]
    }

    pub fn lp(&self, player: Player) -> u8 {
        self.lp[player.index()]
    }

    /// 撃破されたモンスターも召喚済みとして数える。
    pub fn has_summoned(&self, player: Player, level: u8) -> bool {
        (1..=3).contains(&level) && self.summoned[player.index()] & (1 << (level - 1)) != 0
    }

    pub fn turn_player(&self) -> Player {
        self.turn_player
    }

    /// 現在の手番のウォーデンが攻撃済みかを返す。
    pub fn warden_attacked(&self) -> bool {
        self.warden_attacked
    }

    /// P1 の最初の手番を 1 とする手番番号。
    pub fn turn(&self) -> u16 {
        self.turn
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn outcome(&self) -> Option<Outcome> {
        match self.phase {
            Phase::Finished(outcome) => Some(outcome),
            _ => None,
        }
    }

    pub fn end_reason(&self) -> Option<EndReason> {
        self.end_reason
    }

    /// 次の操作を行う側。終局では None。
    pub fn to_act(&self) -> Option<Player> {
        match self.phase {
            Phase::Finished(_) => None,
            Phase::AwaitDefense { .. } => Some(self.turn_player.opponent()),
            _ => Some(self.turn_player),
        }
    }

    pub fn last_roll(&self) -> Option<&[DieRoll; 3]> {
        self.last_roll.as_ref()
    }

    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// 以降のログ生成を切り替える。
    pub fn set_logging(&mut self, enabled: bool) {
        self.logging = enabled;
    }

    /// 生じる結果が同じでも、引数の異なる手は別々に返す。
    pub fn legal_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        match self.phase {
            Phase::Roll => {
                for a in 1..=4 {
                    for b in 1..=4 {
                        for c in 1..=4 {
                            actions.push(Action::Roll { dice: [a, b, c] });
                        }
                    }
                }
            }
            Phase::Place { .. } => {
                actions.extend(self.legal_placements().into_iter().map(Action::from))
            }
            Phase::Action => {
                let add_attacks = |actions: &mut Vec<_>, attacker| {
                    actions.extend(
                        self.attack_targets(attacker)
                            .into_iter()
                            .map(|target| Action::Attack { attacker, target }),
                    );
                };
                add_attacks(&mut actions, Combatant::Warden(self.turn_player));
                for unit in self
                    .units
                    .iter()
                    .filter(|u| u.owner == self.turn_player)
                    .map(|u| u.id)
                {
                    add_attacks(&mut actions, Combatant::Unit(unit));
                    actions.extend(self.twin_strike_targets(unit).into_iter().map(|target| {
                        Action::TwinStrike {
                            attacker: unit,
                            target,
                        }
                    }));
                    actions.extend(
                        self.reachable(unit)
                            .into_iter()
                            .map(|(to, _)| Action::Move { unit, to }),
                    );
                    if self.can_use_wings(unit) {
                        actions.push(Action::Wings { unit });
                    }
                }
                actions.push(Action::EndTurn);
            }
            Phase::AwaitDefense { .. } => {
                actions.extend(self.defense_choices().into_iter().map(Action::Defend))
            }
            Phase::Finished(_) => {}
        }
        actions
    }

    /// 合法手を適用する。Err の場合は乱数状態も含めて変更しない。
    pub fn apply(&mut self, action: Action) -> Result<(), RuleError> {
        match (self.phase, action) {
            (Phase::Roll, Action::Roll { dice }) => self.roll(dice),
            (
                Phase::Place { level },
                Action::Place {
                    shape,
                    rot,
                    flip,
                    origin,
                },
            ) => self.place(
                level,
                Placement {
                    shape,
                    rot,
                    flip,
                    origin,
                },
            ),
            (Phase::Action, Action::Move { unit, to }) => self.move_unit(unit, to),
            (Phase::Action, Action::Attack { attacker, target }) => {
                self.attack(attacker, target, false)
            }
            (Phase::Action, Action::TwinStrike { attacker, target }) => {
                self.attack(Combatant::Unit(attacker), target, true)
            }
            (Phase::Action, Action::Wings { unit }) => self.wings(unit),
            (
                Phase::AwaitDefense {
                    attacker,
                    target,
                    hits_left,
                },
                Action::Defend(choice),
            ) => self.defend(attacker, target, hits_left, choice),
            (Phase::Action, Action::EndTurn) => {
                self.end_turn();
                Ok(())
            }
            _ => Err(RuleError::WrongPhase),
        }
    }

    fn roll(&mut self, dice: [u8; 3]) -> Result<(), RuleError> {
        if !dice.iter().all(|level| (1..=4).contains(level)) {
            return Err(RuleError::InvalidDice);
        }
        let rolls = dice.map(|level| DieRoll {
            level,
            face: DICE_FACES[usize::from(level - 1)][self.rng.usize(0..6)],
        });
        let player = self.turn_player;
        let pool = &mut self.runes[player.index()];
        let mut summons = [0; 4];
        for roll in rolls {
            match roll.face {
                Face::Summon(level) => summons[usize::from(level - 1)] += 1,
                Face::Rune(rune, count) => {
                    let amount = pool.amount_mut(rune);
                    *amount = (*amount + count).min(10);
                }
            }
        }
        self.last_roll = Some(rolls);
        if self.logging {
            self.log.push(format!(
                "{} がダイスを振った",
                player.name_with(self.factions)
            ));
        }

        self.phase = (1..=3)
            .find(|&level| {
                summons[usize::from(level - 1)] >= 2 && !self.has_summoned(player, level)
            })
            .filter(|_| self.placements(player).next().is_some())
            .map_or(Phase::Action, |level| Phase::Place { level });
        Ok(())
    }

    fn summon(&mut self, level: u8, pos: Pos) {
        let owner = self.turn_player;
        let (id, &(name, level, hp, atk, def, ability)) = self.monster(owner, level).unwrap();
        self.summoned[owner.index()] |= 1 << (level - 1);
        self.units.push(Unit {
            id,
            owner,
            name,
            level,
            hp,
            max_hp: hp,
            atk,
            def,
            ability,
            pos,
            flying: ability == Ability::Flying,
            moved: false,
            attacked: false,
        });
        if self.logging {
            self.log.push(format!(
                "{} が {name} を召喚",
                owner.name_with(self.factions)
            ));
        }
    }

    fn move_unit(&mut self, id: UnitId, to @ (x, y): Pos) -> Result<(), RuleError> {
        let (_, cost) = self
            .reachable(id)
            .into_iter()
            .find(|&(pos, _)| pos == to)
            .ok_or(RuleError::InvalidMove)?;
        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.pos = to;
        unit.moved = true;
        self.runes[self.turn_player.index()].movement -= cost;
        if self.logging {
            self.log.push(format!("{} が ({x}, {y}) へ移動", unit.name));
        }
        Ok(())
    }

    pub fn can_use_wings(&self, id: UnitId) -> bool {
        self.phase == Phase::Action
            && self.runes(self.turn_player).magic > 0
            && self.unit(id).is_some_and(|unit| {
                unit.owner == self.turn_player && unit.ability == Ability::Wings && !unit.flying
            })
    }

    fn wings(&mut self, id: UnitId) -> Result<(), RuleError> {
        if !self.can_use_wings(id) {
            return Err(RuleError::InvalidAbility);
        }
        let unit = self.units.iter_mut().find(|unit| unit.id == id).unwrap();
        unit.flying = true;
        self.runes[self.turn_player.index()].magic -= 1;
        if self.logging {
            self.log.push(format!("{} が飛行を得た", unit.name));
        }
        Ok(())
    }

    /// このフェーズで通常攻撃できる対象。
    pub fn attack_targets(&self, attacker: Combatant) -> Vec<Combatant> {
        let player = self.turn_player;
        if self.phase != Phase::Action || self.runes(player).attack == 0 {
            return Vec::new();
        }
        let (pos, can_hit_flying) = match attacker {
            Combatant::Unit(id) => match self.unit(id) {
                Some(unit) if unit.owner == player && !unit.attacked => (unit.pos, unit.flying),
                _ => return Vec::new(),
            },
            Combatant::Warden(owner) if owner == player && !self.warden_attacked => {
                (owner.domain(), true)
            }
            _ => return Vec::new(),
        };
        let enemy = player.opponent();
        let mut targets: Vec<_> = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == enemy
                    && (!unit.flying || can_hit_flying)
                    && board::neighbors(pos).any(|p| p == unit.pos)
            })
            .map(|unit| Combatant::Unit(unit.id))
            .collect();
        if board::neighbors(pos).any(|p| p == enemy.domain()) {
            targets.push(Combatant::Warden(enemy));
        }
        targets
    }

    pub fn twin_strike_targets(&self, attacker: UnitId) -> Vec<Combatant> {
        if self.runes(self.turn_player).attack >= 2
            && self
                .unit(attacker)
                .is_some_and(|unit| unit.ability == Ability::TwinStrike)
        {
            // 能力はウォーデンに作用しない (SPEC §4)
            self.attack_targets(Combatant::Unit(attacker))
                .into_iter()
                .filter(|target| matches!(target, Combatant::Unit(_)))
                .collect()
        } else {
            Vec::new()
        }
    }

    fn attack(
        &mut self,
        attacker: Combatant,
        target: Combatant,
        twin: bool,
    ) -> Result<(), RuleError> {
        let targets = if twin {
            match attacker {
                Combatant::Unit(id) => self.twin_strike_targets(id),
                Combatant::Warden(_) => return Err(RuleError::InvalidAttack),
            }
        } else {
            self.attack_targets(attacker)
        };
        if !targets.contains(&target) {
            return Err(RuleError::InvalidAttack);
        }
        let hits = if twin { 2 } else { 1 };
        self.runes[self.turn_player.index()].attack -= hits;
        match attacker {
            Combatant::Unit(id) => {
                self.units
                    .iter_mut()
                    .find(|unit| unit.id == id)
                    .unwrap()
                    .attacked = true;
            }
            Combatant::Warden(_) => self.warden_attacked = true,
        }
        if self.logging {
            self.log.push(format!(
                "{} が {} を攻撃",
                attacker.name_with(self.factions),
                target.name_with(self.factions)
            ));
        }
        match target {
            Combatant::Warden(player) => {
                let lp = &mut self.lp[player.index()];
                *lp = lp.saturating_sub(hits);
                if self.logging {
                    self.log.push(format!(
                        "{} のウォーデンが被弾 (LP {lp})",
                        player.name_with(self.factions)
                    ));
                }
                if *lp == 0 {
                    self.finish(Outcome::Win(self.turn_player), EndReason::LpZero);
                }
            }
            Combatant::Unit(target) => {
                self.phase = Phase::AwaitDefense {
                    attacker,
                    target,
                    hits_left: hits,
                };
            }
        }
        Ok(())
    }

    pub fn defense_choices(&self) -> Vec<DefenseChoice> {
        let Phase::AwaitDefense { target, .. } = self.phase else {
            return Vec::new();
        };
        let unit = self.unit(target).unwrap();
        let pool = self.runes(unit.owner);
        let mut choices = vec![DefenseChoice::None];
        if pool.guard > 0 {
            choices.push(DefenseChoice::Guard);
        }
        if unit.ability == Ability::Ward && pool.trap > 0 {
            choices.push(DefenseChoice::Ward);
        }
        choices
    }

    /// 選択可能な防御手を適用した場合の現在の一撃のダメージ。
    pub fn pending_damage(&self, choice: DefenseChoice) -> Option<u8> {
        let Phase::AwaitDefense {
            attacker, target, ..
        } = self.phase
        else {
            return None;
        };
        if !self.defense_choices().contains(&choice) {
            return None;
        }
        let atk = match attacker {
            Combatant::Unit(id) => self.unit(id).unwrap().atk,
            Combatant::Warden(_) => WARDEN_ATK,
        };
        let def = self.unit(target).unwrap().def;
        Some(match choice {
            DefenseChoice::None => atk,
            DefenseChoice::Guard => atk.saturating_sub(def),
            DefenseChoice::Ward => 0,
        })
    }

    fn defend(
        &mut self,
        attacker: Combatant,
        target: UnitId,
        hits_left: u8,
        choice: DefenseChoice,
    ) -> Result<(), RuleError> {
        let damage = self
            .pending_damage(choice)
            .ok_or(RuleError::InvalidDefense)?;
        let unit = self
            .units
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap();
        let pool = &mut self.runes[unit.owner.index()];
        let defense = match choice {
            DefenseChoice::None => "防御なし",
            DefenseChoice::Guard => {
                pool.guard -= 1;
                "守護"
            }
            DefenseChoice::Ward => {
                pool.trap -= 1;
                "Ward"
            }
        };
        unit.hp = unit.hp.saturating_sub(damage);
        if self.logging {
            self.log
                .push(format!("{} は {defense} で {damage} ダメージ", unit.name));
        }
        if unit.hp == 0 {
            if self.logging {
                self.log.push(format!("{} を撃破", unit.name));
            }
            self.units.retain(|unit| unit.id != target);
            self.phase = Phase::Action;
            self.check_stalemate();
        } else if hits_left == 1 {
            self.phase = Phase::Action;
        } else {
            self.phase = Phase::AwaitDefense {
                attacker,
                target,
                hits_left: hits_left - 1,
            };
        }
        Ok(())
    }

    fn end_turn(&mut self) {
        if self.turn == 300 {
            self.finish_by_score(EndReason::TurnLimit);
        } else {
            self.check_stalemate();
            if self.outcome().is_some() {
                return;
            }
            self.turn += 1;
            self.turn_player = self.turn_player.opponent();
            self.phase = Phase::Roll;
            self.warden_attacked = false;
            for unit in &mut self.units {
                unit.moved = false;
                unit.attacked = false;
            }
        }
    }

    fn check_stalemate(&mut self) {
        if self.units.is_empty()
            && [Player::P1, Player::P2].into_iter().all(|player| {
                (1..=3).all(|level| self.has_summoned(player, level))
                    || self.placements(player).next().is_none()
            })
        {
            self.finish_by_score(EndReason::Stalemate);
        }
    }

    fn score(&self, player: Player) -> (u8, u32, u8) {
        let level_total = (1..=3)
            .filter(|&level| self.has_summoned(player, level))
            .sum();
        (
            self.lp(player),
            self.summoned[player.index()].count_ones(),
            level_total,
        )
    }

    fn finish_by_score(&mut self, reason: EndReason) {
        use std::cmp::Ordering;
        self.finish(
            match self.score(Player::P1).cmp(&self.score(Player::P2)) {
                Ordering::Greater => Outcome::Win(Player::P1),
                Ordering::Less => Outcome::Win(Player::P2),
                Ordering::Equal => Outcome::Draw,
            },
            reason,
        );
    }

    fn finish(&mut self, outcome: Outcome, reason: EndReason) {
        self.end_reason = Some(reason);
        self.phase = Phase::Finished(outcome);
        if self.logging {
            self.log.push(match outcome {
                Outcome::Win(player) => {
                    format!("対局終了：{} の勝ち", player.name_with(self.factions))
                }
                Outcome::Draw => "対局終了：引き分け".to_owned(),
            });
        }
    }
}
