//! その局面で実行できる行動を一手ずつ選ぶ CPU。

use crate::{
    Ability, Action, Combatant, DefenseChoice, Game, Phase, Player, Pos, WARDEN_ATK,
    board::neighbors,
};

/// game.to_act() が CPU を返す局面で呼ぶ。終局後に呼ぶとパニックする。
pub fn choose(game: &Game) -> Action {
    let player = game.turn_player();
    let enemy_domain = player.opponent().domain();
    match game.phase() {
        Phase::Roll => {
            let level = (1..=3)
                .find(|&level| !game.has_summoned(player, level))
                .filter(|_| game.placements(player).next().is_some())
                .unwrap_or(2);
            Action::Roll { dice: [level; 3] }
        }
        Phase::Place { .. } => {
            let placement = game
                .legal_placements()
                .into_iter()
                .min_by_key(|placement| {
                    let (cells, spawn) = placement.cells().unwrap();
                    let front = cells
                        .into_iter()
                        .map(|pos| distance(pos, enemy_domain))
                        .min()
                        .unwrap();
                    (front, distance(spawn, enemy_domain))
                })
                .expect("召喚配置フェーズには合法配置がある");
            placement.into()
        }
        Phase::AwaitDefense { .. } => {
            let guard = game.pending_damage(DefenseChoice::Guard);
            let choice = if guard == Some(0) {
                DefenseChoice::Guard
            } else if game.pending_damage(DefenseChoice::Ward).is_some() {
                DefenseChoice::Ward
            } else if guard.is_some() {
                DefenseChoice::Guard
            } else {
                DefenseChoice::None
            };
            Action::Defend(choice)
        }
        Phase::Action => choose_unit_action(game, player, enemy_domain),
        Phase::Finished(_) => panic!("終局後に実行できる行動はない"),
    }
}

fn choose_unit_action(game: &Game, player: Player, enemy_domain: Pos) -> Action {
    let units = || game.units().iter().filter(|unit| unit.owner == player);
    let attackers = std::iter::once(Combatant::Warden(player))
        .chain(units().map(|unit| Combatant::Unit(unit.id)));
    let mut attacks = Vec::new();
    for attacker in attackers {
        for target in game.attack_targets(attacker) {
            let score = attack_score(game, attacker, target, 1);
            attacks.push((score, Action::Attack { attacker, target }));
        }
        if let Combatant::Unit(id) = attacker {
            for target in game.twin_strike_targets(id) {
                let score = attack_score(game, attacker, target, 2);
                attacks.push((
                    score,
                    Action::TwinStrike {
                        attacker: id,
                        target,
                    },
                ));
            }
        }
    }
    if let Some((_, action)) = attacks.into_iter().max_by_key(|&(score, _)| score) {
        return action;
    }

    if game.runes(player).attack > 0
        && let Some(unit) = units().find(|unit| {
            !unit.attacked
                && game.can_use_wings(unit.id)
                && neighbors(unit.pos).any(|pos| {
                    game.unit_at(pos)
                        .is_some_and(|target| target.owner != player && target.flying)
                })
        })
    {
        return Action::Wings { unit: unit.id };
    }

    let mut moves = Vec::new();
    for unit in units() {
        for (to, cost) in game.reachable(unit.id) {
            let before = distance(unit.pos, enemy_domain);
            let after = distance(to, enemy_domain);
            if after < before {
                moves.push(((after, cost), Action::Move { unit: unit.id, to }));
            }
        }
    }
    moves
        .into_iter()
        .min_by_key(|&(score, _)| score)
        .map_or(Action::EndTurn, |(_, action)| action)
}

fn attack_score(game: &Game, attacker: Combatant, target: Combatant, hits: u8) -> (u8, u8, u8) {
    let (rank, damage) = match target {
        Combatant::Warden(player) => {
            let lp = game.lp(player);
            (if hits >= lp { 4 } else { 3 }, hits.min(lp))
        }
        Combatant::Unit(id) => {
            let unit = game.unit(id).unwrap();
            let atk = match attacker {
                Combatant::Warden(_) => WARDEN_ATK,
                Combatant::Unit(id) => game.unit(id).unwrap().atk,
            };
            let pool = game.runes(unit.owner);
            let ward = if unit.ability == Ability::Ward {
                pool.trap.min(hits)
            } else {
                0
            };
            let guarded = pool.guard.min(hits - ward);
            let damage = guarded * atk.saturating_sub(unit.def) + (hits - ward - guarded) * atk;
            (if damage >= unit.hp { 2 } else { 1 }, damage.min(unit.hp))
        }
    };
    (rank, damage, 2 - hits)
}

fn distance(a: Pos, b: Pos) -> usize {
    a.0.abs_diff(b.0) + a.1.abs_diff(b.1)
}
