#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
compile_error!("delvers-sim はネイティブ専用です");

use std::{env, ops::Range, process::ExitCode, sync::Arc};

use delvers_core::{Action, EndReason, Face, Game, Outcome, Player, Ruleset, ai};

const USAGE: &str = "delvers-sim <開始..終了 または 開始+試合数> <greedy|random> <greedy|random> [ember-frost|frost-ember|ember-ember|frost-frost] [basic|pool]";

#[derive(Debug, Clone, Copy)]
enum Policy {
    Greedy,
    Random,
}

fn policy(s: &str) -> Result<Policy, String> {
    match s {
        "greedy" => Ok(Policy::Greedy),
        "random" => Ok(Policy::Random),
        _ => Err(format!("未知の方策: {s}")),
    }
}

fn seeds(s: &str) -> Result<Range<u64>, String> {
    let number = |n: &str| n.parse::<u64>().map_err(|_| format!("無効な整数: {n}"));
    let (start, end) = if let Some((a, b)) = s.split_once("..") {
        (number(a)?, number(b)?)
    } else if let Some((a, n)) = s.split_once('+') {
        let start = number(a)?;
        (
            start,
            start
                .checked_add(number(n)?)
                .ok_or("シード範囲の桁あふれ")?,
        )
    } else {
        return Err("シード範囲を指定してください".into());
    };
    if end <= start {
        return Err("試合数は正の整数です".into());
    }
    Ok(start..end)
}

#[derive(Default)]
struct Stats {
    wins: [u64; 2],
    draws: u64,
    reasons: [u64; 3],
    turns: Vec<u16>,
    disconnected: u64,
    inactive_turns: u64,
    summons: [u64; 2],
    histogram: [Vec<u64>; 2],
    remaining: [u64; 2],
    lv4_summons: u64,
    attacks_disconnected: u64,
    exhausted_disconnected: u64,
    lv4_pairs: u64,
    greedy_rolls: u64,
    greedy_roll_exceptions: u64,
}

impl Stats {
    fn play(&mut self, rules: &Arc<Ruleset>, seed: u64, policies: [Policy; 2], seats: [usize; 2]) {
        let mut game = Game::new(rules.clone(), seats, seed);
        game.set_logging(false);
        // 方策の抽選がダイスの乱数列を消費しないよう分ける。
        let mut rng = fastrand::Rng::with_seed(!seed);
        let mut active = false;
        let mut attacks = 0;
        let mut summons = [0; 2];
        while let Some(actor) = game.to_act() {
            let seat = actor.index();
            let action = match policies[seat] {
                Policy::Greedy => ai::choose(&game),
                Policy::Random => ai::choose_random(&game, &mut rng),
            };
            if let Action::Roll { dice } = &action
                && matches!(policies[seat], Policy::Greedy)
            {
                let expected = game
                    .unsummoned(actor)
                    .map(|(_, monster)| monster.level)
                    .min()
                    .unwrap_or(2);
                self.greedy_rolls += 1;
                self.greedy_roll_exceptions += u64::from(dice.iter().any(|&lv| lv != expected));
            }
            let placed_four = matches!(action, Action::Place { .. })
                && matches!(game.phase(), delvers_core::Phase::Place { level: 4 });
            game.apply(action.clone())
                .unwrap_or_else(|e| panic!("seed={seed} turn={} {action:?}: {e}", game.turn()));
            match action {
                Action::Place { .. } => {
                    summons[seat] += 1;
                    self.lv4_summons += u64::from(placed_four);
                    active = true;
                }
                Action::Attack { .. } | Action::TwinStrike { .. } => {
                    attacks += 1;
                    active = true;
                }
                Action::Move { .. } => active = true,
                Action::Roll { .. } | Action::RollPool { .. } => {
                    if game
                        .last_roll()
                        .unwrap()
                        .iter()
                        .filter(|r| r.face == Face::Summon(4))
                        .count()
                        >= 2
                    {
                        self.lv4_pairs += 1;
                    }
                }
                _ => {}
            }
            if action == Action::EndTurn || game.outcome().is_some() {
                self.inactive_turns += u64::from(!active);
                active = false;
            }
        }
        match game.outcome().unwrap() {
            Outcome::Win(p) => self.wins[p.index()] += 1,
            Outcome::Draw => self.draws += 1,
        }
        let reason = game.end_reason().unwrap();
        self.reasons[match reason {
            EndReason::LpZero => 0,
            EndReason::Stalemate => 1,
            EndReason::TurnLimit => 2,
        }] += 1;
        self.turns.push(game.turn());
        if !game.domains_connected() {
            self.disconnected += 1;
            self.attacks_disconnected += attacks;
            if [Player::P1, Player::P2]
                .iter()
                .all(|&p| game.unsummoned(p).next().is_none())
                && reason == EndReason::TurnLimit
                && !game.units().is_empty()
            {
                self.exhausted_disconnected += 1;
            }
        }
        for player in [Player::P1, Player::P2] {
            let seat = player.index();
            self.summons[seat] += summons[seat];
            let count = summons[seat] as usize;
            let hist = &mut self.histogram[seat];
            if hist.len() <= count {
                hist.resize(count + 1, 0);
            }
            hist[count] += 1;
            if rules.personal_pool {
                self.remaining[seat] += game.unsummoned(player).count() as u64;
            }
        }
    }

    fn report(&mut self, personal_pool: bool) {
        self.turns.sort_unstable();
        let n = self.turns.len();
        let total: u64 = self.turns.iter().copied().map(u64::from).sum();
        let median = (f64::from(self.turns[(n - 1) / 2]) + f64::from(self.turns[n / 2])) / 2.;
        let pct = |count| 100. * count as f64 / n as f64;
        println!(
            "試合数 {n}　先手勝 {} ({:.2}%)　後手勝 {} ({:.2}%)　引分 {} ({:.2}%)",
            self.wins[0],
            pct(self.wins[0]),
            self.wins[1],
            pct(self.wins[1]),
            self.draws,
            pct(self.draws)
        );
        println!(
            "手番 平均 {:.3}　中央値 {median}　最大 {}　合計 {total}",
            total as f64 / n as f64,
            self.turns[n - 1]
        );
        println!(
            "終局 LP0 {}　膠着 {}　上限 {}",
            self.reasons[0], self.reasons[1], self.reasons[2]
        );
        println!(
            "未連結 {} ({:.2}%)　非活動手番 {} ({:.2}%)",
            self.disconnected,
            pct(self.disconnected),
            self.inactive_turns,
            100. * self.inactive_turns as f64 / total as f64
        );
        println!(
            "平均召喚 先手 {:.4}　後手 {:.4}",
            self.summons[0] as f64 / n as f64,
            self.summons[1] as f64 / n as f64
        );
        if !personal_pool {
            println!(
                "未連結試合の攻撃宣言 {}　未連結かつ双方3体召喚・生存あり・上限 {}　召喚4が2面以上のロール {}　貪欲ロール例外 {}/{}",
                self.attacks_disconnected,
                self.exhausted_disconnected,
                self.lv4_pairs,
                self.greedy_roll_exceptions,
                self.greedy_rolls
            );
            return;
        }
        for (i, &wins) in self.wins.iter().enumerate() {
            let (lo, hi) = wilson(wins, n);
            println!(
                "P{} 勝率95%区間 [{:.2}, {:.2}]　召喚数別件数 (0体から) {:?}",
                i + 1,
                100. * lo,
                100. * hi,
                self.histogram[i]
            );
        }
        println!(
            "平均残りダイス 先手 {:.4}　後手 {:.4}",
            self.remaining[0] as f64 / n as f64,
            self.remaining[1] as f64 / n as f64
        );
        println!(
            "未連結試合の攻撃宣言 {}　未連結かつ双方ロスター全召喚・生存あり・上限 {}　召喚4が2面以上のロール {}　Lv4召喚 {}",
            self.attacks_disconnected,
            self.exhausted_disconnected,
            self.lv4_pairs,
            self.lv4_summons
        );
    }
}

/// 引き分けを含む全試合を分母とする二項比率の Wilson スコア区間。
fn wilson(wins: u64, games: usize) -> (f64, f64) {
    let n = games as f64;
    let p = wins as f64 / n;
    let z = 1.959963984540054;
    let z2 = z * z;
    let denom = 1. + z2 / n;
    let center = (p + z2 / (2. * n)) / denom;
    let half = z * ((p * (1. - p) + z2 / (4. * n)) / n).sqrt() / denom;
    ((center - half).max(0.), (center + half).min(1.))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    if !(3..=5).contains(&args.len()) {
        return Err(USAGE.into());
    }
    let range = seeds(&args[0])?;
    let policies = [policy(&args[1])?, policy(&args[2])?];
    let rules = Arc::new(match args.get(4).map(String::as_str).unwrap_or("basic") {
        "basic" => Ruleset::basic(),
        "pool" => Ruleset::pool(),
        x => return Err(format!("未知のルールセット: {x}")),
    });
    let pairing = args.get(3).map(String::as_str).unwrap_or("ember-frost");
    let seats = pairing
        .split_once('-')
        .and_then(|(a, b)| Some([rules.faction_index(a)?, rules.faction_index(b)?]))
        .ok_or_else(|| format!("未知の陣営設定: {pairing}"))?;
    let names = seats.map(|seat| rules.factions[seat].name);
    if rules.personal_pool {
        print!("pool　");
    }
    println!(
        "シード {range:?}　方策 {policies:?}　陣営 [{}]",
        names.join(", ")
    );
    let mut stats = Stats::default();
    for seed in range {
        stats.play(&rules, seed, policies, seats);
    }
    stats.report(rules.personal_pool);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_ranges() {
        assert_eq!(seeds("4..6"), Ok(4..6));
        assert_eq!(seeds("4+2"), Ok(4..6));
        for invalid in ["0", "1..1", "2..1", "0+0", "-1+2", "18446744073709551615+1"] {
            assert!(seeds(invalid).is_err());
        }
    }
}
