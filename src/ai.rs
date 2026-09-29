use crate::engine::{ActionKind, Difficulty, KongKind, Observation};
use crate::rules::Tile;
use std::cmp::Reverse;
use std::collections::HashMap;

pub fn choose_action(
    observation: &Observation,
    legal: &[ActionKind],
    difficulty: Difficulty,
    seed: u64,
) -> ActionKind {
    if let Some(action) = legal
        .iter()
        .find(|action| matches!(action, ActionKind::Win))
    {
        return action.clone();
    }
    match difficulty {
        Difficulty::Weak => weak(legal, seed),
        Difficulty::Medium => medium(observation, legal),
        Difficulty::Strong => strong(observation, legal),
    }
}

fn weak(legal: &[ActionKind], seed: u64) -> ActionKind {
    let mut state = if seed == 0 { 1 } else { seed };
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    let index = (state % legal.len() as u64) as usize;
    legal[index].clone()
}

fn medium(observation: &Observation, legal: &[ActionKind]) -> ActionKind {
    legal
        .iter()
        .max_by_key(|action| {
            (
                Reverse(action_shanten(observation, action)),
                matches!(action, ActionKind::Pass),
                Reverse(action_order(action)),
            )
        })
        .cloned()
        .expect("policy receives at least one legal action")
}

fn strong(observation: &Observation, legal: &[ActionKind]) -> ActionKind {
    let mut discards: Vec<ActionKind> = legal
        .iter()
        .filter(|action| matches!(action, ActionKind::Discard { .. }))
        .cloned()
        .collect();
    if discards.is_empty() {
        return claim_choice(observation, legal);
    }
    discards.sort_by_key(|action| (action_shanten(observation, action), action_order(action)));
    discards.truncate(8);
    discards
        .into_iter()
        .max_by_key(|action| {
            (
                Reverse(action_shanten(observation, action)),
                visible_improvement_count(observation, action),
                discard_safety(observation, action),
                Reverse(action_order(action)),
            )
        })
        .expect("strong policy kept at least one discard")
}

fn claim_choice(observation: &Observation, legal: &[ActionKind]) -> ActionKind {
    legal
        .iter()
        .max_by_key(|action| {
            (
                Reverse(action_shanten(observation, action)),
                matches!(action, ActionKind::Pass),
                Reverse(action_order(action)),
            )
        })
        .cloned()
        .expect("policy receives at least one legal action")
}

fn action_shanten(observation: &Observation, action: &ActionKind) -> i32 {
    let fixed = observation.melds.len();
    match action {
        ActionKind::Discard { tile } => {
            standard_shanten(&remove_many(&observation.hand, *tile, 1), fixed)
        }
        ActionKind::Pong { tile } => {
            let hand = remove_many(&observation.hand, *tile, 2);
            claimed_readiness(&hand, fixed + 1)
        }
        ActionKind::Chow { tiles } => {
            let hand = remove_many(&remove_many(&observation.hand, tiles[0], 1), tiles[1], 1);
            claimed_readiness(&hand, fixed + 1)
        }
        ActionKind::Kong { tile, kind } => {
            let amount = match kind {
                KongKind::Discard => 3,
                KongKind::Concealed => 4,
                KongKind::Added => 1,
            };
            let fixed = fixed + usize::from(!matches!(kind, KongKind::Added));
            standard_shanten(&remove_many(&observation.hand, *tile, amount), fixed)
        }
        ActionKind::Draw | ActionKind::Win | ActionKind::Pass => {
            standard_shanten(&observation.hand, fixed)
        }
    }
}

fn claimed_readiness(hand: &[Tile], fixed: usize) -> i32 {
    if hand.is_empty() {
        return standard_shanten(hand, fixed);
    }
    unique_tiles(hand)
        .into_iter()
        .map(|tile| standard_shanten(&remove_many(hand, tile, 1), fixed))
        .min()
        .unwrap_or_else(|| standard_shanten(hand, fixed))
}

pub fn standard_shanten(hand: &[Tile], fixed_melds: usize) -> i32 {
    let mut counts = [0u8; 34];
    for &tile in hand {
        if let Some(index) = tile_index(tile) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    let mut memo = HashMap::new();
    search_shanten(&mut counts, fixed_melds.min(5) as u8, 0, 0, 0, &mut memo)
}

type ShantenKey = ([u8; 34], u8, u8, u8);

fn search_shanten(
    counts: &mut [u8; 34],
    fixed_melds: u8,
    melds: u8,
    pairs: u8,
    taatsu: u8,
    memo: &mut HashMap<ShantenKey, i32>,
) -> i32 {
    let key = (*counts, melds, pairs, taatsu);
    if let Some(&value) = memo.get(&key) {
        return value;
    }
    let Some(first) = counts.iter().position(|&count| count > 0) else {
        let meld_count = fixed_melds + melds;
        let usable_taatsu = taatsu.min(5u8.saturating_sub(meld_count));
        return 10 - 2 * meld_count as i32 - usable_taatsu as i32 - pairs as i32;
    };
    let mut best = 99;

    counts[first] -= 1;
    best = best.min(search_shanten(
        counts,
        fixed_melds,
        melds,
        pairs,
        taatsu,
        memo,
    ));
    counts[first] += 1;

    if fixed_melds + melds < 5 && counts[first] >= 3 {
        counts[first] -= 3;
        best = best.min(search_shanten(
            counts,
            fixed_melds,
            melds + 1,
            pairs,
            taatsu,
            memo,
        ));
        counts[first] += 3;
    }

    if fixed_melds + melds < 5
        && first < 27
        && first % 9 <= 6
        && counts[first + 1] > 0
        && counts[first + 2] > 0
    {
        counts[first] -= 1;
        counts[first + 1] -= 1;
        counts[first + 2] -= 1;
        best = best.min(search_shanten(
            counts,
            fixed_melds,
            melds + 1,
            pairs,
            taatsu,
            memo,
        ));
        counts[first] += 1;
        counts[first + 1] += 1;
        counts[first + 2] += 1;
    }

    if pairs == 0 && counts[first] >= 2 {
        counts[first] -= 2;
        best = best.min(search_shanten(counts, fixed_melds, melds, 1, taatsu, memo));
        counts[first] += 2;
    }

    if taatsu < 5 {
        if counts[first] >= 2 {
            counts[first] -= 2;
            best = best.min(search_shanten(
                counts,
                fixed_melds,
                melds,
                pairs,
                taatsu + 1,
                memo,
            ));
            counts[first] += 2;
        }
        if first < 27 && first % 9 <= 7 && counts[first + 1] > 0 {
            counts[first] -= 1;
            counts[first + 1] -= 1;
            best = best.min(search_shanten(
                counts,
                fixed_melds,
                melds,
                pairs,
                taatsu + 1,
                memo,
            ));
            counts[first] += 1;
            counts[first + 1] += 1;
        }
        if first < 27 && first % 9 <= 6 && counts[first + 2] > 0 {
            counts[first] -= 1;
            counts[first + 2] -= 1;
            best = best.min(search_shanten(
                counts,
                fixed_melds,
                melds,
                pairs,
                taatsu + 1,
                memo,
            ));
            counts[first] += 1;
            counts[first + 2] += 1;
        }
    }

    memo.insert(key, best);
    best
}

fn visible_improvement_count(observation: &Observation, action: &ActionKind) -> i32 {
    let ActionKind::Discard { tile } = action else {
        return 0;
    };
    let hand = remove_many(&observation.hand, *tile, 1);
    let before = standard_shanten(&hand, observation.melds.len());
    all_tile_codes()
        .filter(|candidate| {
            let mut next = hand.clone();
            next.push(*candidate);
            standard_shanten(&next, observation.melds.len()) < before
        })
        .map(|candidate| 4 - visible_count(observation, candidate))
        .filter(|remaining| *remaining > 0)
        .sum()
}

fn discard_safety(observation: &Observation, action: &ActionKind) -> i32 {
    let ActionKind::Discard { tile } = action else {
        return 0;
    };
    public_count(observation, *tile)
}

fn visible_count(observation: &Observation, tile: Tile) -> i32 {
    let own = observation
        .hand
        .iter()
        .filter(|&&candidate| candidate == tile)
        .count() as i32;
    (own + public_count(observation, tile)).min(4)
}

fn public_count(observation: &Observation, tile: Tile) -> i32 {
    let mut count = 0;
    for player in &observation.public.players {
        count += player
            .flowers
            .iter()
            .filter(|&&candidate| candidate == tile)
            .count() as i32;
        for meld in &player.melds {
            count += meld
                .tiles
                .iter()
                .filter(|&&candidate| candidate == tile)
                .count() as i32;
        }
        count += player
            .discards
            .iter()
            .filter(|discard| !discard.claimed && discard.tile == tile)
            .count() as i32;
    }
    count.min(4)
}

fn remove_many(hand: &[Tile], tile: Tile, amount: usize) -> Vec<Tile> {
    let mut result = hand.to_vec();
    for _ in 0..amount {
        if let Some(index) = result.iter().position(|candidate| *candidate == tile) {
            result.remove(index);
        }
    }
    result
}

fn unique_tiles(hand: &[Tile]) -> Vec<Tile> {
    let mut result = hand.to_vec();
    result.sort_unstable();
    result.dedup();
    result
}

fn tile_index(tile: Tile) -> Option<usize> {
    match tile {
        1..=9 => Some((tile - 1) as usize),
        11..=19 => Some((tile - 11 + 9) as usize),
        21..=29 => Some((tile - 21 + 18) as usize),
        31..=34 => Some((tile - 31 + 27) as usize),
        41..=43 => Some((tile - 41 + 31) as usize),
        _ => None,
    }
}

fn action_order(action: &ActionKind) -> u32 {
    match action {
        ActionKind::Draw => 0,
        ActionKind::Win => 1,
        ActionKind::Discard { tile } => 100 + *tile as u32,
        ActionKind::Kong { tile, .. } => 200 + *tile as u32,
        ActionKind::Pong { tile } => 300 + *tile as u32,
        ActionKind::Chow { tiles } => 400 + tiles[0] as u32,
        ActionKind::Pass => 1000,
    }
}

fn all_tile_codes() -> impl Iterator<Item = Tile> {
    [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 21, 22, 23, 24, 25, 26, 27,
        28, 29, 31, 32, 33, 34, 41, 42, 43,
    ]
    .into_iter()
}
