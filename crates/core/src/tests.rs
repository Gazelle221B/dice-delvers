use super::*;
use Combatant::{Unit as Creature, Warden};
use Player::{P1, P2};

fn seeded_roll(indices: [usize; 3]) -> Game {
    let seed = (0..)
        .find(|&seed| {
            let mut rng = fastrand::Rng::with_seed(seed);
            indices == std::array::from_fn(|_| rng.usize(0..6))
        })
        .unwrap();
    Game::new(seed)
}

fn assert_rejected(game: &mut Game, action: Action, error: RuleError) {
    let before = format!("{game:?}");
    let seed = game.rng.get_seed();
    assert_eq!(game.apply(action), Err(error));
    assert_eq!(format!("{game:?}"), before);
    assert_eq!(game.rng.get_seed(), seed);
}

fn corridor(player: Player) -> Game {
    let mut game = Game::new(0);
    game.phase = Phase::Action;
    game.turn_player = player;
    for x in 1..W - 1 {
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
        Action::Roll { dice: [1, 2, 0] },
        RuleError::InvalidDice,
    );
    game.apply(Action::Roll { dice: [1, 2, 3] }).unwrap();
    assert_eq!(
        game.last_roll().unwrap().map(|roll| roll.face),
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
    game.apply(Action::Roll { dice: [2, 2, 4] }).unwrap();
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
    game.apply(Action::Roll { dice: [1; 3] }).unwrap();
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
        assert!(copy.has_summoned(P1, 1));
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
    used.apply(Action::Roll { dice: [1; 3] }).unwrap();
    assert_eq!(used.phase(), Phase::Action);
    assert!(used.has_summoned(P1, 1));

    let mut blocked = seeded_roll([0; 3]);
    for (x, y) in board::neighbors(P1.domain()) {
        blocked.board[y][x] = Cell::Tile(P2);
    }
    blocked.apply(Action::Roll { dice: [1; 3] }).unwrap();
    assert_eq!(blocked.phase(), Phase::Action);

    for dice in [[4; 3], [1, 2, 3]] {
        let mut unavailable = seeded_roll([0; 3]);
        unavailable.apply(Action::Roll { dice }).unwrap();
        assert_eq!(unavailable.phase(), Phase::Action);
        assert!(unavailable.legal_placements().is_empty());
    }
}

#[test]
fn placement_requires_side_contact_and_empty_in_bounds_cells() {
    let mut game = seeded_roll([0; 3]);
    game.apply(Action::Roll { dice: [1; 3] }).unwrap();
    for origin in [(2, 4), (1, 2), (0, 4), (W - 2, 4), (usize::MAX, 0)] {
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
        assert_rejected(
            &mut game,
            Action::Move {
                unit: id,
                to: player.domain(),
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
    assert!(game.has_summoned(P2, 1));
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
    game.summoned = [0b111; 2];
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
        game.summoned = summoned;
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
        let mut game = Game::new(seed);
        for _ in 0..300 * 16 {
            if game.to_act().is_none() {
                break;
            }
            let action = ai::choose(&game);
            assert_eq!(
                game.apply(action),
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
    for faction in [Faction::Ember, Faction::Frost] {
        let mut game = Game::with_factions(1, [faction; 2]);
        for player in [P1, P2] {
            game.turn_player = player;
            for level in 1..=3 {
                game.summon(level, (usize::from(level), player.index()));
                let (id, definition) = game.monster(player, level).unwrap();
                assert_eq!(id, player.index() * 3 + usize::from(level - 1));
                let unit = game.unit(id).unwrap();
                assert_eq!(
                    unit.ability,
                    MONSTERS[faction as usize * 3 + usize::from(level - 1)].5
                );
                assert_eq!(unit.name, definition.0);
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
            if game.clone().apply(action).is_ok() {
                let idx = actions
                    .iter()
                    .position(|&a| a == action)
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
                rolls.push(Action::Roll { dice: [a, b, c] });
            }
        }
    }
    check(&game, rolls);
    game.apply(Action::Roll { dice: [1; 3] }).unwrap();
    let mut placements = Vec::new();
    for shape in 0..11 {
        for rot in 0..4 {
            for flip in [false, true] {
                for x in 0..W {
                    for y in 0..H {
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
            for x in 0..W {
                for y in 0..H {
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
