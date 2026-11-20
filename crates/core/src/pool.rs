use crate::{Action, Face, Game, Phase, Player, RuleError, UnitId};

pub(super) fn combinations(
    ids: &[UnitId],
    count: usize,
    selected: &mut Vec<UnitId>,
    out: &mut Vec<Action>,
) {
    if count == 0 {
        out.push(Action::RollPool {
            dice: selected.clone(),
        });
        return;
    }
    for i in 0..=ids.len() - count {
        selected.push(ids[i]);
        combinations(&ids[i + 1..], count - 1, selected, out);
        selected.pop();
    }
}

impl Game {
    pub fn pool_roll_size(&self) -> usize {
        let count = if self.turn == 1 && self.turn_player == Player::P1 {
            self.rules
                .opening_pool_roll
                .unwrap_or(self.rules.dice_per_roll)
        } else {
            self.rules.dice_per_roll
        };
        count.min(self.unsummoned(self.turn_player).count())
    }

    /// ダイスは順序を持たないので昇順の ID 列だけを受理する。
    pub fn valid_pool_roll(&self, dice: &[UnitId]) -> bool {
        self.rules.personal_pool
            && self.phase == Phase::Roll
            && dice.len() == self.pool_roll_size()
            && dice.windows(2).all(|w| w[0] < w[1])
            && dice.iter().all(|&id| {
                self.monster(id)
                    .is_some_and(|(owner, _)| owner == self.turn_player)
                    && !self.is_summoned(id)
            })
    }

    pub(super) fn roll_pool(&mut self, dice: Vec<UnitId>) -> Result<(), RuleError> {
        if !self.valid_pool_roll(&dice) {
            return Err(RuleError::InvalidDice);
        }
        let levels = dice
            .iter()
            .map(|&id| self.monster(id).unwrap().1.level)
            .collect();
        self.roll(levels, &dice);
        Ok(())
    }

    pub(super) fn choose_summon(&mut self, die: UnitId) -> Result<(), RuleError> {
        if !self.summon_dice.contains(&die) {
            return Err(RuleError::InvalidDice);
        }
        let level = self.monster(die).unwrap().1.level;
        self.summon_dice.clear();
        self.summon_dice.push(die);
        self.phase = Phase::Place { level };
        Ok(())
    }

    pub fn placing_monster(&self, level: u8) -> Option<UnitId> {
        if self.rules.personal_pool {
            self.summon_dice.first().copied()
        } else {
            self.summonable(self.turn_player, level)
        }
    }

    pub(super) fn can_roll_pair(&self, player: Player) -> bool {
        self.rules.dice_per_roll >= 2
            && self.unsummoned(player).any(|(id, m)| {
                self.rules.dice[usize::from(m.level - 1)]
                    .faces
                    .contains(&Face::Summon(m.level))
                    && self
                        .unsummoned(player)
                        .any(|(other, n)| other > id && n.level == m.level)
            })
    }
}
