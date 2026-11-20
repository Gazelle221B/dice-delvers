use super::*;
use Combatant::{Unit as Creature, Warden};
use Player::{P1, P2};

fn basic(seed: u64) -> Game {
    Game::new(Arc::new(Ruleset::basic()), [0, 1], seed)
}

/// 席ごとのビット i をロスター i 番目の召喚済みフラグにする。
fn set_summoned(game: &mut Game, masks: [u8; 2]) {
    for player in [P1, P2] {
        let ids: Vec<_> = game.roster(player).map(|(id, _)| id).collect();
        for (bit, id) in ids.into_iter().enumerate() {
            game.summoned[id] = masks[player.index()] & (1 << bit) != 0;
        }
    }
}

fn seeded_roll(indices: [usize; 3]) -> Game {
    let seed = (0..)
        .find(|&seed| {
            let mut rng = fastrand::Rng::with_seed(seed);
            indices == std::array::from_fn(|_| rng.usize(0..6))
        })
        .unwrap();
    basic(seed)
}

fn assert_rejected(game: &mut Game, action: Action, error: RuleError) {
    let before = format!("{game:?}");
    let seed = game.rng.get_seed();
    assert_eq!(game.apply(action), Err(error));
    assert_eq!(format!("{game:?}"), before);
    assert_eq!(game.rng.get_seed(), seed);
}

fn corridor(player: Player) -> Game {
    let mut game = basic(0);
    game.phase = Phase::Action;
    game.turn_player = player;
    for x in 1..game.rules.width - 1 {
        game.board[5][x] = Cell::Tile(P1);
    }
    game
}

fn placement(origin: Pos) -> Placement {
    Placement {
        shape: 0,
        rot: 0,
        flip: false,
        origin,
    }
}

#[test]
fn roll_adds_runes_with_individual_caps() {
    let mut game = seeded_roll([5, 4, 5]);
    game.runes[0].guard = 9;
    assert_rejected(
        &mut game,
        Action::Roll {
            dice: vec![1, 2, 0],
        },
        RuleError::InvalidDice,
    );
    game.apply(Action::Roll {
        dice: vec![1, 2, 3],
    })
    .unwrap();
    assert_eq!(
        game.last_roll()
            .unwrap()
            .iter()
            .map(|roll| roll.face)
            .collect::<Vec<_>>(),
        [
            Face::Rune(Rune::Guard, 2),
            Face::Rune(Rune::Attack, 2),
            Face::Rune(Rune::Trap, 2)
        ]
    );
    assert_eq!(
        game.runes(P1),
        RunePool {
            attack: 2,
            guard: 10,
            trap: 2,
            ..RunePool::default()
        }
    );
    assert_eq!(game.runes(P2), RunePool::default());

    let mut game = seeded_roll([3, 5, 4]);
    game.runes[0].movement = 9;
    game.apply(Action::Roll {
        dice: vec![2, 2, 4],
    })
    .unwrap();
    assert_eq!(
        game.runes(P1),
        RunePool {
            movement: 10,
            magic: 3,
            ..RunePool::default()
        }
    );
}

#[test]
fn summon_is_mandatory_when_possible() {
    let mut game = seeded_roll([0, 0, 4]);
    game.apply(Action::Roll { dice: vec![1; 3] }).unwrap();
    assert_eq!(game.phase(), Phase::Place { level: 1 });
    assert_eq!(game.runes(P1).movement, 1);
    assert_rejected(&mut game, Action::EndTurn, RuleError::WrongPhase);
    let legal = game.legal_placements();
    assert!(!legal.is_empty());
    for candidate in legal {
        let (cells, spawn) = candidate.cells().unwrap();
        let mut copy = game.clone();
        copy.apply(candidate.into()).unwrap();
        assert_eq!(copy.phase(), Phase::Action);
        assert!(copy.is_summoned(0));
        assert_eq!(copy.units()[0].pos, spawn);
        assert!(
            cells
                .into_iter()
                .all(|pos| copy.cell(pos) == Some(Cell::Tile(P1)))
        );
    }
}

#[test]
fn summon_is_skipped_for_used_or_missing_monster_and_blocked_domain() {
    let mut used = seeded_roll([0; 3]);
    used.summon(1, (1, 5));
    used.units.clear();
    used.apply(Action::Roll { dice: vec![1; 3] }).unwrap();
    assert_eq!(used.phase(), Phase::Action);
    assert!(used.is_summoned(0));

    let mut blocked = seeded_roll([0; 3]);
    for (x, y) in blocked.neighbors(blocked.domain(P1)) {
        blocked.board[y][x] = Cell::Tile(P2);
    }
    blocked.apply(Action::Roll { dice: vec![1; 3] }).unwrap();
    assert_eq!(blocked.phase(), Phase::Action);

    for dice in [vec![4; 3], vec![1, 2, 3]] {
        let mut unavailable = seeded_roll([0; 3]);
        unavailable.apply(Action::Roll { dice }).unwrap();
        assert_eq!(unavailable.phase(), Phase::Action);
        assert!(unavailable.legal_placements().is_empty());
    }
}

#[test]
fn placement_requires_side_contact_and_empty_in_bounds_cells() {
    let mut game = seeded_roll([0; 3]);
    game.apply(Action::Roll { dice: vec![1; 3] }).unwrap();
    for origin in [
        (2, 4),
        (1, 2),
        (0, 4),
        (game.rules.width - 2, 4),
        (usize::MAX, 0),
    ] {
        assert!(!game.is_legal_placement(placement(origin)));
        assert_rejected(
            &mut game,
            placement(origin).into(),
            RuleError::InvalidPlacement,
        );
    }
    let first = placement((0, 2));
    assert!(game.is_legal_placement(first));
    game.apply(first.into()).unwrap();
    game.phase = Phase::Place { level: 2 };
    for invalid in [
        first,
        placement((0, 6)),
        Placement { rot: 4, ..first },
        Placement { shape: 11, ..first },
    ] {
        assert_rejected(&mut game, invalid.into(), RuleError::InvalidPlacement);
    }
    game.apply(placement((4, 2)).into()).unwrap();
    assert_eq!(game.units().len(), 2);
}

#[test]
fn tile_transforms_preserve_spawn_index() {
    assert_eq!(SHAPES.len(), 11);
    assert_eq!(placement_cells(0, 0, false, (2, 3)).unwrap().1, (2, 3));
    assert_eq!(placement_cells(0, 1, false, (2, 3)).unwrap().1, (4, 3));
    assert_eq!(placement_cells(0, 1, true, (2, 3)).unwrap().1, (4, 6));
    for shape in 0..11 {
        for rot in 0..4 {
            for flip in [false, true] {
                let (cells, spawn) = placement_cells(shape, rot, flip, (2, 3)).unwrap();
                assert_eq!(spawn, cells[0]);
                assert_eq!(
                    cells
                        .into_iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                    6
                );
                assert_eq!(cells.iter().map(|p| p.0).min(), Some(2));
                assert_eq!(cells.iter().map(|p| p.1).min(), Some(3));
            }
        }
    }
}

#[test]
fn movement_costs_depend_on_ability_and_charge_once() {
    for (player, level, costs) in [(P1, 2, [1, 2, 3]), (P1, 1, [1, 1, 2]), (P2, 1, [2, 4, 6])] {
        let mut game = corridor(player);
        game.runes[player.index()].movement = 6;
        game.summon(level, (1, 5));
        let id = game.units()[0].id;
        let reachable = game.reachable(id);
        for (i, cost) in costs.into_iter().enumerate() {
            assert!(reachable.contains(&((i + 2, 5), cost)));
        }
        let domain = game.domain(player);
        assert_rejected(
            &mut game,
            Action::Move {
                unit: id,
                to: domain,
            },
            RuleError::InvalidMove,
        );
        game.apply(Action::Move {
            unit: id,
            to: (3, 5),
        })
        .unwrap();
        assert_eq!(game.runes(player).movement, 6 - costs[1]);
        assert!(game.reachable(id).is_empty());
        assert_rejected(
            &mut game,
            Action::Move {
                unit: id,
                to: (4, 5),
            },
            RuleError::InvalidMove,
        );
        game.apply(Action::EndTurn).unwrap();
        assert!(!game.unit(id).unwrap().moved);
    }
}

#[test]
fn movement_can_detour_and_only_tunnel_can_pass_enemies() {
    let mut game = corridor(P1);
    game.runes[0].movement = 5;
    game.summon(1, (1, 5));
    game.summon(2, (2, 5));
    game.turn_player = P2;
    game.summon(2, (3, 5));
    game.turn_player = P1;

    let swift = game.reachable(0);
    assert!(!swift.iter().any(|&(pos, _)| pos == (4, 5)));
    let tunnel = game.reachable(1);
    assert!(tunnel.contains(&((4, 5), 2)));
    assert!(!tunnel.iter().any(|&(pos, _)| pos == (3, 5)));

    for x in 2..=4 {
        game.board[4][x] = Cell::Tile(P2);
    }
    assert!(game.reachable(0).contains(&((2, 4), 1)));
    assert!(game.reachable(0).contains(&((4, 5), 3)));
}

#[test]
fn flying_can_only_be_attacked_by_wardens_and_flying_units() {
    let mut game = corridor(P1);
    game.summon(3, (2, 5));
    game.turn_player = P2;
    game.summon(1, (1, 5));
    game.turn_player = P1;
    game.runes[0] = RunePool {
        attack: 2,
        magic: 1,
        movement: 4,
        ..RunePool::default()
    };

    let target = Creature(3);
    assert!(!game.warden_attacked());
    assert!(!game.attack_targets(Creature(2)).contains(&target));
    assert!(game.attack_targets(Warden(P1)).contains(&target));
    game.apply(Action::Wings { unit: 2 }).unwrap();
    assert!(game.unit(2).unwrap().flying);
    assert_eq!(game.runes(P1).magic, 0);
    assert!(game.reachable(2).contains(&((3, 5), 2)));
    assert!(game.attack_targets(Creature(2)).contains(&target));
    assert_rejected(
        &mut game,
        Action::Wings { unit: 2 },
        RuleError::InvalidAbility,
    );
    game.apply(Action::Attack {
        attacker: Warden(P1),
        target,
    })
    .unwrap();
    assert!(game.warden_attacked());
    assert_eq!(game.to_act(), Some(P2));
    assert_eq!(
        game.log().last().unwrap(),
        "P1 Ember のウォーデン が Rime Sprite を攻撃"
    );
    assert_eq!(game.pending_damage(DefenseChoice::None), Some(10));
    game.apply(Action::Defend(DefenseChoice::None)).unwrap();
    assert!(game.unit(3).is_none());
    assert!(game.is_summoned(3));
    assert!(!game.attack_targets(Warden(P1)).contains(&target));
    game.apply(Action::EndTurn).unwrap();
    assert!(!game.warden_attacked());
}

#[test]
fn guard_ward_and_no_defense_apply_different_damage() {
    let mut game = corridor(P1);
    game.summon(3, (6, 5));
    game.turn_player = P2;
    game.summon(3, (7, 5));
    game.turn_player = P1;
    game.runes[0].attack = 2;
    game.runes[1].guard = 1;
    game.runes[1].trap = 1;
    game.apply(Action::Attack {
        attacker: Creature(2),
        target: Creature(5),
    })
    .unwrap();

    assert_eq!(game.turn_player(), P1);
    assert_eq!(game.to_act(), Some(P2));
    assert_eq!(
        game.defense_choices(),
        [
            DefenseChoice::None,
            DefenseChoice::Guard,
            DefenseChoice::Ward
        ]
    );
    assert_rejected(&mut game, Action::EndTurn, RuleError::WrongPhase);
    for (choice, hp, guard, trap) in [
        (DefenseChoice::None, 10, 1, 1),
        (DefenseChoice::Guard, 30, 0, 1),
        (DefenseChoice::Ward, 40, 1, 0),
    ] {
        let mut copy = game.clone();
        copy.apply(Action::Defend(choice)).unwrap();
        assert_eq!(copy.unit(5).unwrap().hp, hp);
        assert_eq!(copy.runes(P2).guard, guard);
        assert_eq!(copy.runes(P2).trap, trap);
        assert_eq!(copy.runes(P1).attack, 1);
        assert_eq!(copy.to_act(), Some(P1));
        assert!(copy.attack_targets(Creature(2)).is_empty());
    }
}

#[test]
fn twin_strike_requires_independent_defense_for_each_hit() {
    let mut game = corridor(P2);
    game.summon(2, (6, 5));
    game.turn_player = P1;
    game.summon(2, (5, 5));
    game.turn_player = P2;
    game.runes[1].attack = 2;
    game.runes[0].guard = 1;
    game.runes[0].trap = 1;
    game.apply(Action::TwinStrike {
        attacker: 4,
        target: Creature(1),
    })
    .unwrap();
    assert_eq!(
        game.phase(),
        Phase::AwaitDefense {
            attacker: Creature(4),
            target: 1,
            hits_left: 2
        }
    );
    assert_rejected(
        &mut game,
        Action::Defend(DefenseChoice::Ward),
        RuleError::InvalidDefense,
    );
    game.apply(Action::Defend(DefenseChoice::Guard)).unwrap();
    assert_eq!(game.unit(1).unwrap().hp, 20);
    assert_eq!(game.to_act(), Some(P1));
    assert_eq!(
        game.phase(),
        Phase::AwaitDefense {
            attacker: Creature(4),
            target: 1,
            hits_left: 1
        }
    );
    assert_rejected(
        &mut game,
        Action::Defend(DefenseChoice::Guard),
        RuleError::InvalidDefense,
    );
    game.apply(Action::Defend(DefenseChoice::None)).unwrap();
    assert!(game.unit(1).is_none());
    assert_eq!(game.to_act(), Some(P2));
    assert_eq!(game.runes(P2).attack, 0);
}

#[test]
fn warden_hit_is_immediate_and_zero_lp_ends_game() {
    let mut game = corridor(P1);
    game.summon(1, (11, 5));
    game.runes[0].attack = 2;
    game.runes[1].guard = 1;
    game.lp[1] = 1;
    game.apply(Action::Attack {
        attacker: Creature(0),
        target: Warden(P2),
    })
    .unwrap();
    assert_eq!(game.lp(P2), 0);
    assert_eq!(game.end_reason(), Some(EndReason::LpZero));
    assert_eq!(game.runes(P2).guard, 1);
    assert_eq!(game.outcome(), Some(Outcome::Win(P1)));
    assert_eq!(game.to_act(), None);
    assert_rejected(&mut game, Action::EndTurn, RuleError::WrongPhase);

    let mut game = corridor(P2);
    game.summon(2, (1, 5));
    game.runes[1].attack = 2;
    // 能力はウォーデンに作用しない
    assert_rejected(
        &mut game,
        Action::TwinStrike {
            attacker: 4,
            target: Warden(P1),
        },
        RuleError::InvalidAttack,
    );
    assert_eq!(game.lp(P1), 3);
}

#[test]
fn twin_strike_ends_when_first_hit_destroys_target() {
    let mut game = corridor(P2);
    game.summon(2, (6, 5));
    game.turn_player = P1;
    game.summon(1, (5, 5)); // Bramble Squire HP20
    game.turn_player = P2;
    game.runes[1].attack = 2;
    let target = game.unit_at((5, 5)).unwrap().id;
    game.apply(Action::TwinStrike {
        attacker: 4,
        target: Creature(target),
    })
    .unwrap();
    game.apply(Action::Defend(DefenseChoice::None)).unwrap();
    assert!(game.unit(target).is_none());
    assert_eq!(game.phase(), Phase::Action);
    assert_eq!(game.to_act(), Some(P2));
}

#[test]
fn guard_clamps_damage_at_zero() {
    let mut game = corridor(P1);
    game.summon(1, (6, 5)); // Bramble Squire ATK10
    game.turn_player = P2;
    game.summon(3, (7, 5)); // Frost Leviathan DEF20
    game.turn_player = P1;
    game.runes[0].attack = 1;
    game.runes[1].guard = 1;
    let target = game.unit_at((7, 5)).unwrap().id;
    let attacker = game.unit_at((6, 5)).unwrap().id;
    game.apply(Action::Attack {
        attacker: Creature(attacker),
        target: Creature(target),
    })
    .unwrap();
    assert_eq!(game.pending_damage(DefenseChoice::Guard), Some(0));
    game.apply(Action::Defend(DefenseChoice::Guard)).unwrap();
    assert_eq!(game.unit(target).unwrap().hp, 40);
}

#[test]
fn last_defeat_can_trigger_stalemate() {
    let mut game = corridor(P1);
    game.turn_player = P2;
    game.summon(1, (1, 5));
    game.turn_player = P1;
    set_summoned(&mut game, [0b111; 2]);
    game.lp[1] = 2;
    game.runes[0].attack = 1;
    game.apply(Action::Attack {
        attacker: Warden(P1),
        target: Creature(3),
    })
    .unwrap();
    game.apply(Action::Defend(DefenseChoice::None)).unwrap();
    assert_eq!(game.outcome(), Some(Outcome::Win(P1)));

    let mut blocked = corridor(P1);
    for cell in blocked.board.iter_mut().flatten() {
        if *cell == Cell::Empty {
            *cell = Cell::Tile(P1);
        }
    }
    blocked.apply(Action::EndTurn).unwrap();
    assert_eq!(blocked.outcome(), Some(Outcome::Draw));
    assert_eq!(blocked.end_reason(), Some(EndReason::Stalemate));
}

#[test]
fn turn_limit_uses_lp_then_count_then_level_sum() {
    for (lp, summoned, outcome) in [
        ([2, 3], [0b111, 0], Outcome::Win(P2)),
        ([3, 3], [0b011, 0b100], Outcome::Win(P1)),
        ([3, 3], [0b001, 0b010], Outcome::Win(P2)),
        ([3, 3], [0b101, 0b101], Outcome::Draw),
    ] {
        let mut game = corridor(P2);
        game.summon(3, (11, 5));
        set_summoned(&mut game, summoned);
        game.lp = lp;
        game.turn = 300;
        game.apply(Action::EndTurn).unwrap();
        assert_eq!(game.turn(), 300);
        assert_eq!(game.end_reason(), Some(EndReason::TurnLimit));
        assert_eq!(game.outcome(), Some(outcome));
    }
}

#[test]
fn cpu_mirror_matches_only_make_legal_moves_and_finish() {
    for seed in 0..24 {
        let mut game = basic(seed);
        for _ in 0..300 * 16 {
            if game.to_act().is_none() {
                break;
            }
            let action = ai::choose(&game);
            assert_eq!(
                game.apply(action.clone()),
                Ok(()),
                "seed={seed}, turn={}, action={action:?}",
                game.turn()
            );
        }
        assert!(
            game.outcome().is_some(),
            "seed={seed}, turn={}",
            game.turn()
        );
        assert!(game.turn() <= 300);
    }
}

#[test]
fn mirror_rosters_preserve_seat_ids_and_use_faction_abilities() {
    let rules = Arc::new(Ruleset::basic());
    for faction in 0..rules.factions.len() {
        let mut game = Game::new(rules.clone(), [faction; 2], 1);
        for player in [P1, P2] {
            game.turn_player = player;
            for level in 1..=3 {
                game.summon(level, (usize::from(level), player.index()));
                let (id, &definition) = game.roster(player).nth(usize::from(level - 1)).unwrap();
                assert_eq!(id, player.index() * 3 + usize::from(level - 1));
                assert_eq!(game.monster(id), Some((player, &definition)));
                let unit = game.unit(id).unwrap();
                assert_eq!(
                    unit.ability,
                    rules.factions[faction].monsters[usize::from(level - 1)].ability
                );
                assert_eq!(unit.name, definition.name);
                assert_eq!(game.combatant_name(Creature(id)), unit.name);
            }
        }
        assert_eq!(game.units().len(), 6);
    }
}

#[test]
fn legal_actions_agree_with_apply() {
    fn check(game: &Game, candidates: Vec<Action>) {
        let mut actions = game.legal_actions();
        for action in candidates {
            if game.clone().apply(action.clone()).is_ok() {
                let idx = actions
                    .iter()
                    .position(|a| *a == action)
                    .expect("合法手の列挙漏れ");
                actions.swap_remove(idx);
            }
        }
        assert!(actions.is_empty(), "違法手または重複: {actions:?}");
    }
    let mut game = seeded_roll([0; 3]);
    let mut rolls = Vec::new();
    for a in 0..=5 {
        for b in 0..=5 {
            for c in 0..=5 {
                rolls.push(Action::Roll {
                    dice: vec![a, b, c],
                });
            }
        }
    }
    check(&game, rolls);
    game.apply(Action::Roll { dice: vec![1; 3] }).unwrap();
    let mut placements = Vec::new();
    for shape in 0..11 {
        for rot in 0..4 {
            for flip in [false, true] {
                for x in 0..game.rules.width {
                    for y in 0..game.rules.height {
                        placements.push(Action::Place {
                            shape,
                            rot,
                            flip,
                            origin: (x, y),
                        });
                    }
                }
            }
        }
    }
    check(&game, placements);

    let mut game = corridor(P1);
    game.set_logging(false);
    game.turn_player = P2;
    game.summon(2, (2, 5));
    game.summon(3, (4, 5));
    game.turn_player = P1;
    game.summon(3, (3, 5));
    game.summon(1, (1, 5));
    for player in [P1, P2] {
        game.turn_player = player;
        game.runes[player.index()] = RunePool {
            movement: 4,
            attack: 2,
            magic: 1,
            trap: 1,
            guard: 1,
        };
        let mut candidates = vec![Action::EndTurn];
        let combatants: Vec<_> = (0..6)
            .map(Creature)
            .chain([Warden(P1), Warden(P2)])
            .collect();
        for &attacker in &combatants {
            for &target in &combatants {
                candidates.push(Action::Attack { attacker, target });
                if let Creature(id) = attacker {
                    candidates.push(Action::TwinStrike {
                        attacker: id,
                        target,
                    });
                }
            }
        }
        for unit in 0..6 {
            candidates.push(Action::Wings { unit });
            for x in 0..game.rules.width {
                for y in 0..game.rules.height {
                    candidates.push(Action::Move { unit, to: (x, y) });
                }
            }
        }
        check(&game, candidates);
    }
    game.turn_player = P1;
    game.apply(Action::Attack {
        attacker: Creature(2),
        target: Creature(5),
    })
    .unwrap();
    check(
        &game,
        [
            DefenseChoice::None,
            DefenseChoice::Guard,
            DefenseChoice::Ward,
        ]
        .map(Action::Defend)
        .to_vec(),
    );
    game.finish(Outcome::Draw, EndReason::TurnLimit);
    assert!(game.legal_actions().is_empty());
    assert!(game.log().is_empty());
}

#[test]
fn domain_path_requires_edge_contact_along_tiles() {
    let mut game = corridor(P1);
    game.board[5][5] = Cell::Tile(P2);
    game.summon(1, (5, 5));
    assert!(game.domains_connected());
    game.board[5][6] = Cell::Empty;
    game.board[4][6] = Cell::Tile(P2);
    assert!(!game.domains_connected());
}

#[test]
fn greedy_falls_back_to_level_two_when_placement_is_blocked() {
    let mut game = basic(0);
    for (x, y) in game.neighbors(game.domain(P1)) {
        game.board[y][x] = Cell::Tile(P2);
    }
    assert!(!game.is_summoned(0));
    assert_eq!(ai::choose(&game), Action::Roll { dice: vec![2; 3] });
}

#[test]
fn logging_does_not_change_random_play() {
    let mut logged = basic(17);
    let mut quiet = logged.clone();
    quiet.set_logging(false);
    let mut rng = fastrand::Rng::with_seed(41);
    let mut other_rng = rng.clone();
    while logged.to_act().is_some() {
        let a = ai::choose_random(&logged, &mut rng);
        assert_eq!(ai::choose_random(&quiet, &mut other_rng), a);
        logged.apply(a.clone()).unwrap();
        quiet.apply(a).unwrap();
        assert_eq!(logged.last_roll(), quiet.last_roll());
    }
    assert!(!logged.log().is_empty());
    assert!(quiet.log().is_empty());
    assert_eq!(logged.outcome(), quiet.outcome());
    assert_eq!(logged.end_reason(), quiet.end_reason());
    assert_eq!(logged.board(), quiet.board());
    assert_eq!(logged.units(), quiet.units());
}

/// ゴールデンテスト用の対局生成。ルールエンジンの API 変更時はここだけを追従させる。
fn golden_game(rules: Ruleset, seed: u64, factions: [&str; 2]) -> Game {
    let seats = factions.map(|name| rules.faction_index(name).unwrap());
    Game::new(Arc::new(rules), seats, seed)
}

/// Rust のバージョンに依存しない FNV-1a 64bit。
fn fnv1a(hash: &mut u64, text: &str) {
    for byte in text.bytes().chain([0xff]) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
}

/// 固定シードで最後まで対局し、行動列・出目・ログ・終局状態のハッシュを既知値と比べる。
/// 既知値はルールセットのデータ化前のコードで計算した。挙動を意図して変えたとき以外は更新しない。
#[test]
fn golden_games_match_known_hashes() {
    let results = golden_hashes(Ruleset::basic);
    assert_basic_hashes(results);
}

fn golden_hashes(preset: fn() -> Ruleset) -> Vec<String> {
    let mut results = Vec::new();
    for factions in [
        ["Ember", "Frost"],
        ["Frost", "Ember"],
        ["Ember", "Ember"],
        ["Frost", "Frost"],
    ] {
        for random in [[false, false], [true, false], [false, true], [true, true]] {
            let mut hash = 0xcbf2_9ce4_8422_2325;
            for seed in 0..6 {
                let mut game = golden_game(preset(), seed, factions);
                let mut rng = fastrand::Rng::with_seed(!seed);
                while let Some(actor) = game.to_act() {
                    let action = if random[actor.index()] {
                        ai::choose_random(&game, &mut rng)
                    } else {
                        ai::choose(&game)
                    };
                    fnv1a(&mut hash, &format!("{action:?}"));
                    let rolled = matches!(action, Action::Roll { .. } | Action::RollPool { .. });
                    game.apply(action).unwrap();
                    if rolled {
                        fnv1a(&mut hash, &format!("{:?}", game.last_roll()));
                    }
                }
                fnv1a(
                    &mut hash,
                    &format!(
                        "{:?} {:?} {} {} {} {:?} {:?} {:?} {:?}",
                        game.outcome(),
                        game.end_reason(),
                        game.turn(),
                        game.lp(P1),
                        game.lp(P2),
                        game.runes(P1),
                        game.runes(P2),
                        game.units(),
                        game.board(),
                    ),
                );
                for line in game.log() {
                    fnv1a(&mut hash, line);
                }
            }
            results.push(format!("{factions:?} {random:?} {hash:016x}"));
        }
    }
    results
}

fn assert_basic_hashes(results: Vec<String>) {
    let expected = [
        "[\"Ember\", \"Frost\"] [false, false] fe19ff74c4cec7db",
        "[\"Ember\", \"Frost\"] [true, false] 29a14a958a1634dd",
        "[\"Ember\", \"Frost\"] [false, true] 65b8fb3412a2051e",
        "[\"Ember\", \"Frost\"] [true, true] 884f9da22c790fa8",
        "[\"Frost\", \"Ember\"] [false, false] 6590605e0fa6e030",
        "[\"Frost\", \"Ember\"] [true, false] 89f5ca99bec74173",
        "[\"Frost\", \"Ember\"] [false, true] e7afefe358f1baad",
        "[\"Frost\", \"Ember\"] [true, true] 167d9fa330bed5ed",
        "[\"Ember\", \"Ember\"] [false, false] 41d2680950e48792",
        "[\"Ember\", \"Ember\"] [true, false] 7355861076eff5a2",
        "[\"Ember\", \"Ember\"] [false, true] 214849077c298cfe",
        "[\"Ember\", \"Ember\"] [true, true] cd63300eae0f5c6b",
        "[\"Frost\", \"Frost\"] [false, false] 6b098e6a81fa7688",
        "[\"Frost\", \"Frost\"] [true, false] 3e24079361a9d7c9",
        "[\"Frost\", \"Frost\"] [false, true] 3b20f3f3836535a2",
        "[\"Frost\", \"Frost\"] [true, true] 05b02c216e85cab7",
    ];
    assert_eq!(results, expected, "\n{}", results.join("\n"));
}

#[test]
fn custom_ruleset_changes_board_dice_and_rosters() {
    let mut rules = Ruleset::basic();
    rules.width = 9;
    rules.height = 6;
    rules.lp = 2;
    rules.turn_limit = 40;
    rules.dice_per_roll = 2;
    rules.dice.truncate(3);
    rules.dice[2].copies = 1;
    rules.factions[0].monsters.truncate(2);
    let extra = rules.factions[1].monsters[0];
    rules.factions[1].monsters.push(Monster {
        name: "Second Sprite",
        ..extra
    });
    let rules = Arc::new(rules);

    let mut game = Game::new(rules.clone(), [0, 1], 3);
    assert_eq!((game.domain(P1), game.domain(P2)), ((0, 3), (8, 3)));
    assert_eq!(game.cell((8, 3)), Some(Cell::Domain(P2)));
    assert_eq!(game.lp(P2), 2);
    assert_eq!(
        game.roster(P1).map(|(id, _)| id).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        game.roster(P2).map(|(id, _)| id).collect::<Vec<_>>(),
        [2, 3, 4, 5]
    );
    assert_eq!(game.monster(5).unwrap().1.name, "Second Sprite");
    assert!(game.monster(6).is_none());

    // 2 個振り・Lv3 は在庫 1 個なので [3, 3] は不可
    let rolls = game.legal_actions();
    assert_eq!(rolls.len(), 8);
    assert!(!rolls.contains(&Action::Roll { dice: vec![3, 3] }));
    for dice in [vec![1, 1, 1], vec![3, 3], vec![4, 1]] {
        assert_rejected(&mut game, Action::Roll { dice }, RuleError::InvalidDice);
    }

    // 同レベルが 2 体いれば先頭から召喚する
    game.turn_player = P2;
    game.summon(1, (1, 0));
    assert_eq!(game.summonable(P2, 1), Some(5));
    game.summon(1, (2, 0));
    assert_eq!(game.summonable(P2, 1), None);
    assert!(game.is_summoned(2) && game.is_summoned(5));

    for seed in 0..8 {
        let mut game = Game::new(rules.clone(), [1, 0], seed);
        let mut rng = fastrand::Rng::with_seed(seed);
        while let Some(actor) = game.to_act() {
            let action = if actor == P1 {
                ai::choose(&game)
            } else {
                ai::choose_random(&game, &mut rng)
            };
            assert_eq!(game.apply(action.clone()), Ok(()), "seed={seed} {action:?}");
        }
        assert!(game.turn() <= 40);
    }
}

#[test]
fn pool_rolls_distinct_owned_dice_and_spends_only_the_chosen_hit() {
    let mut rules = Ruleset::pool();
    rules.dice[0].faces = [Face::Summon(1); 6];
    rules.dice[1].faces = [Face::Summon(2); 6];
    let mut game = Game::new(Arc::new(rules), [0, 0], 99);
    let actions = game.legal_actions();
    assert_eq!(actions.len(), 455);
    for action in actions {
        assert_eq!(game.clone().apply(action), Ok(()));
    }
    for ids in [
        vec![0, 0, 1],
        vec![2, 1, 0],
        vec![0, 1],
        vec![0, 1, 15],
        vec![0, 1, usize::MAX],
    ] {
        assert_rejected(
            &mut game,
            Action::RollPool { dice: ids },
            RuleError::InvalidDice,
        );
    }
    assert_rejected(
        &mut game,
        Action::Roll { dice: vec![1; 3] },
        RuleError::WrongPhase,
    );
    game.apply(Action::RollPool {
        dice: vec![1, 3, 5],
    })
    .unwrap();
    assert_eq!(
        game.legal_actions(),
        [
            Action::ChooseSummon { die: 1 },
            Action::ChooseSummon { die: 3 }
        ]
    );
    for die in [0, 5, 15] {
        assert_rejected(
            &mut game,
            Action::ChooseSummon { die },
            RuleError::InvalidDice,
        );
    }
    game.apply(Action::ChooseSummon { die: 3 }).unwrap();
    assert_eq!(game.unsummoned(P1).count(), 15);
    assert_rejected(
        &mut game,
        placement((usize::MAX, 0)).into(),
        RuleError::InvalidPlacement,
    );
    game.apply(game.legal_actions().remove(0)).unwrap();
    assert_eq!(game.units()[0].id, 3);
    assert_eq!(game.unsummoned(P1).count(), 14);
    assert_eq!(game.unsummoned(P2).count(), 15);
    game.phase = Phase::Roll;
    assert_rejected(
        &mut game,
        Action::RollPool {
            dice: vec![1, 3, 5],
        },
        RuleError::InvalidDice,
    );
}

#[test]
fn pool_short_rolls_and_stalemate_require_a_possible_pair() {
    let mut game = Game::new(Arc::new(Ruleset::pool()), [0, 1], 5);
    for ids in [vec![0, 5], vec![0], vec![]] {
        game.summoned.fill(true);
        for &id in &ids {
            game.summoned[id] = false;
        }
        game.phase = Phase::Roll;
        let action = Action::RollPool { dice: ids.clone() };
        assert_eq!(game.legal_actions(), std::slice::from_ref(&action));
        assert_eq!(ai::choose(&game), action);
        game.apply(action).unwrap();
        assert_eq!(game.last_roll().unwrap().len(), ids.len());
        game.apply(Action::EndTurn).unwrap();
        assert_eq!(game.end_reason(), Some(EndReason::Stalemate));
    }
    let mut game = Game::new(Arc::new(Ruleset::pool()), [1, 1], 5);
    game.summoned.fill(true);
    game.summoned[0..2].fill(false);
    game.board[4][0] = Cell::Tile(P1);
    game.phase = Phase::Action;
    game.apply(Action::EndTurn).unwrap();
    assert!(game.outcome().is_none());
}

#[test]
fn pool_blocked_placement_returns_all_dice() {
    let mut rules = Ruleset::pool();
    rules.dice[0].faces = [Face::Summon(1); 6];
    let mut game = Game::new(Arc::new(rules), [1, 1], 1);
    for (x, y) in game.neighbors(game.domain(P1)) {
        game.board[y][x] = Cell::Tile(P2);
    }
    game.apply(Action::RollPool {
        dice: vec![0, 1, 2],
    })
    .unwrap();
    assert_eq!(game.phase(), Phase::Action);
    assert_eq!(game.unsummoned(P1).count(), 15);
}

#[test]
fn pool_games_match_known_hashes() {
    let results = golden_hashes(Ruleset::pool);
    assert_eq!(
        results,
        [
            "[\"Ember\", \"Frost\"] [false, false] 01149a17f93aefb1",
            "[\"Ember\", \"Frost\"] [true, false] 6ab8dd7c6d95ecc7",
            "[\"Ember\", \"Frost\"] [false, true] afc4454d5cb07a2b",
            "[\"Ember\", \"Frost\"] [true, true] 0f41641741fb02ca",
            "[\"Frost\", \"Ember\"] [false, false] 411c7bd9263319d2",
            "[\"Frost\", \"Ember\"] [true, false] 1b95722788237b51",
            "[\"Frost\", \"Ember\"] [false, true] 86caca41dab1b94a",
            "[\"Frost\", \"Ember\"] [true, true] 63b49b8192d71f0a",
            "[\"Ember\", \"Ember\"] [false, false] 7e2cf33c58ca33bb",
            "[\"Ember\", \"Ember\"] [true, false] 7775f27f533f2427",
            "[\"Ember\", \"Ember\"] [false, true] 941dc94da832c79e",
            "[\"Ember\", \"Ember\"] [true, true] 18cfb35bc8379f12",
            "[\"Frost\", \"Frost\"] [false, false] 14c17b0882a0c6f6",
            "[\"Frost\", \"Frost\"] [true, false] 0df7684733369c12",
            "[\"Frost\", \"Frost\"] [false, true] 4b5363fcde448d9c",
            "[\"Frost\", \"Frost\"] [true, true] 33f4ae80a3824d02"
        ],
        "{}",
        results.join("\n")
    );
}

#[test]
fn pool_multiple_pairs_offer_one_summon_and_all_actions_apply() {
    let mut rules = Ruleset::pool();
    rules.dice_per_roll = 4;
    for (i, die) in rules.dice.iter_mut().enumerate() {
        die.faces = [Face::Summon((i + 1) as u8); 6];
    }
    let mut g = Game::new(Arc::new(rules), [0, 1], 0);
    g.apply(Action::RollPool {
        dice: vec![1, 2, 5, 6],
    })
    .unwrap();
    assert_eq!(
        g.legal_actions(),
        [1, 2, 5, 6].map(|die| Action::ChooseSummon { die })
    );
    for die in [1, 5] {
        let mut branch = g.clone();
        branch.apply(Action::ChooseSummon { die }).unwrap();
        for placement in branch.legal_actions() {
            let mut placed = branch.clone();
            placed.apply(placement).unwrap();
            assert_eq!(placed.units()[0].id, die);
            assert_eq!(placed.unsummoned(P1).count(), 14);
        }
    }
}

#[test]
fn pool_counts_tiles_then_survivors_after_lp() {
    let mut g = Game::new(Arc::new(Ruleset::pool()), [0, 1], 0);
    g.summoned[0] = true;
    g.summoned[14] = true;
    g.summoned[29] = true;
    assert!(g.score(P1) > g.score(P2));
    g.summoned[14] = false;
    assert_eq!(g.score(P1), g.score(P2));
    g.summon_dice.push(0);
    g.summon(1, (0, 0));
    assert!(g.score(P1) > g.score(P2));
    g.lp[0] -= 1;
    assert!(g.score(P1) < g.score(P2));
}

#[test]
fn pool_cpu_changes_dice_when_a_needed_rune_is_full() {
    let mut rules = Ruleset::pool();
    rules.dice[0].faces = [Face::Rune(Rune::Move, 1); 6];
    rules.dice[1].faces = [Face::Rune(Rune::Attack, 1); 6];
    rules.dice[2].faces = [Face::Rune(Rune::Guard, 1); 6];
    rules.dice[3].faces = rules.dice[2].faces;
    let cap = rules.rune_cap;
    let mut game = Game::new(Arc::new(rules), [0, 0], 1);
    for (movement, attack, expected) in [(cap, 0, 2), (0, cap, 1)] {
        game.runes[0].movement = movement;
        game.runes[0].attack = attack;
        let Action::RollPool { dice } = ai::choose(&game) else {
            panic!("プールロールが必要")
        };
        assert!(game.valid_pool_roll(&dice));
        assert!(
            dice.iter()
                .all(|&id| game.monster(id).unwrap().1.level == expected)
        );
    }
}

#[test]
fn pool_legal_actions_apply_throughout_a_game() {
    let mut game = Game::new(Arc::new(Ruleset::pool()), [1, 0], 5);
    game.set_logging(false);
    while game.to_act().is_some() {
        let actions = game.legal_actions();
        let choice = ai::choose(&game);
        assert!(actions.contains(&choice));
        for action in actions {
            assert_eq!(game.clone().apply(action), Ok(()));
        }
        game.apply(choice).unwrap();
    }
    assert!(game.legal_actions().is_empty());
}

#[test]
fn tactical_uses_only_public_position_and_policy_seed() {
    let mut game = Game::new(Arc::new(Ruleset::pool()), [0, 0], 1);
    let mut other = game.clone();
    other.rng = fastrand::Rng::with_seed(2);
    let dice_seed = game.rng.get_seed();
    let a = ai::tactical(&game, &mut fastrand::Rng::with_seed(3));
    assert_eq!(a, ai::tactical(&other, &mut fastrand::Rng::with_seed(3)));
    assert_eq!(game.rng.get_seed(), dice_seed);
    assert!(game.legal_actions().contains(&a));
    game.apply(a).unwrap();
}

#[test]
fn experimental_presets_change_one_setting() {
    for name in ["p2-move", "opening-two", "reroll", "lp5"] {
        let actual = Ruleset::experiment(name).unwrap();
        let mut expected = Ruleset::pool();
        expected.name = actual.name;
        match name {
            "p2-move" => expected.initial_runes[1].movement = 1,
            "opening-two" => expected.opening_pool_roll = Some(2),
            "reroll" => expected.reroll_one = true,
            "lp5" => expected.lp = 5,
            _ => unreachable!(),
        }
        assert_eq!(actual, expected);
        let g = Game::new(Arc::new(actual), [1, 1], 1);
        assert_eq!(g.runes(P1), RunePool::default());
        assert_eq!(g.runes(P2).movement, u8::from(name == "p2-move"));
        assert_eq!(g.lp(P1), if name == "lp5" { 5 } else { 3 });
    }
}

#[test]
fn opening_two_applies_to_first_turn_only() {
    let mut g = Game::new(
        Arc::new(Ruleset::experiment("opening-two").unwrap()),
        [0, 0],
        1,
    );
    assert_eq!(g.legal_actions().len(), 105);
    assert_rejected(
        &mut g,
        Action::RollPool {
            dice: vec![0, 1, 2],
        },
        RuleError::InvalidDice,
    );
    g.apply(Action::RollPool { dice: vec![0, 5] }).unwrap();
    g.apply(Action::EndTurn).unwrap();
    assert_eq!(g.pool_roll_size(), 3);
    g.apply(Action::RollPool {
        dice: vec![15, 20, 25],
    })
    .unwrap();
    g.apply(Action::EndTurn).unwrap();
    assert_eq!(g.pool_roll_size(), 3);
}

#[test]
fn reroll_commits_only_final_faces_and_never_spends_twice() {
    let mut rules = Ruleset::experiment("reroll").unwrap();
    rules.dice[0].faces = [Face::Rune(Rune::Move, 2); 6];
    rules.dice[0].faces[0] = Face::Summon(1);
    let seed = (0..)
        .find(|&seed| {
            let mut r = fastrand::Rng::with_seed(seed);
            r.usize(..6) == 0 && r.usize(..6) == 0 && r.usize(..6) != 0 && r.usize(..6) != 0
        })
        .unwrap();
    let mut g = Game::new(Arc::new(rules), [0, 0], seed);
    g.apply(Action::RollPool {
        dice: vec![1, 2, 3],
    })
    .unwrap();
    assert_eq!(g.phase(), Phase::AwaitReroll);
    assert_eq!(g.runes(P1).movement, 0);
    assert_eq!(
        g.legal_actions(),
        [
            Action::KeepRoll,
            Action::Reroll { index: 0 },
            Action::Reroll { index: 1 },
            Action::Reroll { index: 2 }
        ]
    );
    for action in g.legal_actions() {
        g.clone().apply(action).unwrap();
    }
    assert_rejected(&mut g, Action::Reroll { index: 3 }, RuleError::InvalidDice);
    assert_rejected(
        &mut g,
        Action::ChooseSummon { die: 1 },
        RuleError::WrongPhase,
    );
    let mut keep = g.clone();
    let before = keep.rng.get_seed();
    keep.apply(Action::KeepRoll).unwrap();
    assert_eq!(keep.rng.get_seed(), before);
    assert_eq!(keep.runes(P1).movement, 2);
    assert_eq!(
        keep.legal_actions(),
        [
            Action::ChooseSummon { die: 1 },
            Action::ChooseSummon { die: 2 }
        ]
    );
    g.apply(Action::Reroll { index: 0 }).unwrap();
    assert_eq!(g.phase(), Phase::Action);
    assert_eq!(g.runes(P1).movement, 4);
    assert_eq!(g.unsummoned(P1).count(), 15);
    assert_rejected(&mut g, Action::Reroll { index: 0 }, RuleError::WrongPhase);
}
