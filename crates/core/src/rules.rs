//! 対局ごとに差し替えられるルールの数値と定義 (ルールセット)。

use crate::{Ability, Face, Rune, RunePool};

/// モンスター 1 体の定義。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monster {
    pub name: &'static str,
    pub level: u8,
    pub hp: u8,
    pub atk: u8,
    pub def: u8,
    pub ability: Ability,
}

/// 陣営 (ロスター)。席に割り当てると、その席が召喚できるモンスターの一覧になる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faction {
    pub name: &'static str,
    /// 各モンスターは対局中に 1 回だけ召喚できる。個別プールではこの一覧の各要素が 1 個のダイスを表す。
    pub monsters: Vec<Monster>,
}

/// ダイス 1 種類 (1 レベル分)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Die {
    pub faces: [Face; 6],
    /// ダイス置き場にある個数。1 回のロールで同レベルを選べる上限。
    pub copies: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ruleset {
    /// 各席の未召喚モンスターに対応する個別ダイスを使う。
    pub personal_pool: bool,
    pub initial_runes: [RunePool; 2],
    /// 個人プールの先手第1手番だけの上限。
    pub opening_pool_roll: Option<usize>,
    pub reroll_one: bool,
    pub name: &'static str,
    /// 盤面の列数。ドメインは (0, height / 2) と (width - 1, height / 2)。
    pub width: usize,
    /// 盤面の行数。
    pub height: usize,
    /// ウォーデンの初期 LP。
    pub lp: u8,
    /// ウォーデンの攻撃力。
    pub warden_atk: u8,
    /// ルーン 1 種類あたりの貯蓄上限。
    pub rune_cap: u8,
    /// この手番の終了操作で対局を打ち切る手番番号。
    pub turn_limit: u16,
    /// 1 手番に振るダイスの個数。
    pub dice_per_roll: usize,
    /// 添字 i がレベル i + 1 のダイス。
    pub dice: Vec<Die>,
    /// 席に割り当てられる陣営。同じ陣営を両席に割り当ててもよい (ミラー対戦)。
    pub factions: Vec<Faction>,
}

impl Ruleset {
    /// SPEC.md の現行ルール。
    pub fn basic() -> Self {
        use Ability::{Flying, Swift, Tunnel, TwinStrike, Ward, Wings};
        use Face::{Rune as Run, Summon};
        use Rune::{Attack, Guard, Magic, Move, Trap};
        let monster = |name, level, hp, atk, def, ability| Monster {
            name,
            level,
            hp,
            atk,
            def,
            ability,
        };
        let die = |faces| Die { faces, copies: 3 };
        Self {
            name: "basic",
            personal_pool: false,
            initial_runes: [RunePool::default(); 2],
            opening_pool_roll: None,
            reroll_one: false,
            width: 13,
            height: 11,
            lp: 3,
            warden_atk: 10,
            rune_cap: 10,
            turn_limit: 300,
            dice_per_roll: 3,
            dice: vec![
                die([
                    Summon(1),
                    Summon(1),
                    Summon(1),
                    Summon(1),
                    Run(Move, 1),
                    Run(Guard, 2),
                ]),
                die([
                    Summon(2),
                    Summon(2),
                    Summon(2),
                    Run(Move, 2),
                    Run(Attack, 2),
                    Run(Magic, 1),
                ]),
                die([
                    Summon(3),
                    Summon(3),
                    Run(Move, 1),
                    Run(Move, 1),
                    Run(Attack, 1),
                    Run(Trap, 2),
                ]),
                die([
                    Summon(4),
                    Run(Move, 2),
                    Run(Attack, 1),
                    Run(Guard, 1),
                    Run(Magic, 2),
                    Run(Trap, 1),
                ]),
            ],
            factions: vec![
                Faction {
                    name: "Ember",
                    monsters: vec![
                        monster("Bramble Squire", 1, 20, 10, 10, Swift),
                        monster("Cinder Hound", 2, 30, 20, 10, Tunnel),
                        monster("Ash Colossus", 3, 40, 30, 20, Wings),
                    ],
                },
                Faction {
                    name: "Frost",
                    monsters: vec![
                        monster("Rime Sprite", 1, 10, 10, 10, Flying),
                        monster("Glacier Duelist", 2, 30, 20, 10, TwinStrike),
                        monster("Frost Leviathan", 3, 40, 30, 20, Ward),
                    ],
                },
            ],
        }
    }

    /// 各席がモンスターと対になった 15 個のダイスを持つ。
    pub fn pool() -> Self {
        let mut rules = Self::basic();
        rules.name = "pool";
        rules.personal_pool = true;
        let names = [
            [
                "Sootstitch",
                "Charbrindle",
                "Wickpatter",
                "Ashnib",
                "Kilnwhisk",
                "Cinderwicker",
                "Coalspindle",
                "Scorchloom",
                "Umberlatch",
                "Flintspool",
                "Sinterdrum",
                "Pyreweft",
                "Hearthtread",
                "Calderabell",
                "Cruciblequill",
            ],
            [
                "Rimeskein",
                "Sleetpip",
                "Hailstitch",
                "Hoarfleck",
                "Brinepurl",
                "Glazewinder",
                "Rimebuckle",
                "Sleetshuttle",
                "Floefret",
                "Coldspindle",
                "Driftmantle",
                "Hailchime",
                "Glacierweft",
                "Tundrabell",
                "Floequill",
            ],
        ];
        let levels = [1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4];
        for (faction, names) in rules.factions.iter_mut().zip(names) {
            let templates = &faction.monsters;
            faction.monsters = names
                .into_iter()
                .zip(levels)
                .enumerate()
                .map(|(i, (name, level))| {
                    let template = templates[usize::from(level.min(3u8) - 1)];
                    Monster {
                        name,
                        level,
                        ability: templates[i % templates.len()].ability,
                        ..template
                    }
                })
                .collect();
        }
        rules
    }

    /// pool から一項目だけ変えた、既定ではない実験プリセット。
    pub fn experiment(name: &str) -> Option<Self> {
        let mut rules = Self::pool();
        rules.name = match name {
            "p2-move" => {
                rules.initial_runes[1].movement = 1;
                "p2-move"
            }
            "opening-two" => {
                rules.opening_pool_roll = Some(2);
                "opening-two"
            }
            "reroll" => {
                rules.reroll_one = true;
                "reroll"
            }
            "lp5" => {
                rules.lp = 5;
                "lp5"
            }
            _ => return None,
        };
        Some(rules)
    }

    /// 名前 (大文字小文字は区別しない) から陣営の添字を引く。
    pub fn faction_index(&self, name: &str) -> Option<usize> {
        self.factions
            .iter()
            .position(|faction| faction.name.eq_ignore_ascii_case(name))
    }
}
