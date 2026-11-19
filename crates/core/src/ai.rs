//! その局面で実行できる行動を一手ずつ選ぶ CPU。

use crate::{Ability, Action, Combatant, DefenseChoice, Game, Phase, Player, Pos};

/// game.to_act() が CPU を返す局面で呼ぶ。終局後に呼ぶとパニックする。
pub fn choose(game: &Game) -> Action {
    let player = game.turn_player();
    let enemy_domain = game.domain(player.opponent());
    match game.phase() {
        Phase::Roll => {
            // 未召喚の最低レベルを揃えて狙う。配置できなければ攻撃ルーンの出る Lv2。
            let level = game
                .unsummoned(player)
                .map(|(_, monster)| monster.level)
                .min()
                .filter(|_| game.placements(player).next().is_some())
                .unwrap_or(2);
            let rules = game.rules();
            let per_roll = rules.dice_per_roll;
            let copies = rules.dice.get(usize::from(level - 1)).map(|die| die.copies);
            if copies.is_some_and(|copies| usize::from(copies) >= per_roll) {
                Action::Roll {
                    dice: vec![level; per_roll],
                }
            } else {
                // 在庫が足りなければ、狙うレベルを最も多く含む合法なロールの先頭。
                game.legal_actions()
                    .into_iter()
                    .min_by_key(|action| match action {
                        Action::Roll { dice } => dice.iter().filter(|&&lv| lv != level).count(),
                        _ => unreachable!(),
                    })
                    .expect("ロールの合法手がある")
            }
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
                && game.neighbors(unit.pos).any(|pos| {
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
                Combatant::Warden(_) => game.rules().warden_atk,
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

/// 終局前の全合法手から、呼び出し側の乱数で一様に一手選ぶ。
pub fn choose_random(game: &Game, rng: &mut fastrand::Rng) -> Action {
    let mut actions = game.legal_actions();
    actions.swap_remove(rng.usize(..actions.len()))
}
