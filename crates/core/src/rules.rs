//! 対局ごとに差し替えられるルールの数値と定義 (ルールセット)。

use crate::{Ability, Face, Rune};

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
    /// 各モンスターは対局中に 1 回だけ召喚できる。同レベルが複数あれば先頭から使う。
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

    /// 名前 (大文字小文字は区別しない) から陣営の添字を引く。
    pub fn faction_index(&self, name: &str) -> Option<usize> {
        self.factions
            .iter()
            .position(|faction| faction.name.eq_ignore_ascii_case(name))
    }
}
