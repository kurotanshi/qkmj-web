use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Tile = u8;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WinSource {
    Initial,
    NormalDraw,
    FlowerReplacement,
    KongReplacement,
    Discard,
}

impl WinSource {
    pub fn self_draw(self) -> bool {
        self != Self::Discard
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeldKind {
    Chow,
    Pong,
    DiscardKong,
    ConcealedKong,
    AddedKong,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Meld {
    pub kind: MeldKind,
    pub tiles: Vec<Tile>,
    pub called: Option<Tile>,
}

impl Meld {
    pub fn chow(tiles: [Tile; 3], called: Option<Tile>) -> Self {
        Self {
            kind: MeldKind::Chow,
            tiles: tiles.to_vec(),
            called,
        }
    }

    pub fn pong(tile: Tile, called: Option<Tile>) -> Self {
        Self {
            kind: MeldKind::Pong,
            tiles: vec![tile; 3],
            called,
        }
    }

    pub fn kong(kind: MeldKind, tile: Tile, called: Option<Tile>) -> Self {
        Self {
            kind,
            tiles: vec![tile; 4],
            called,
        }
    }

    pub fn is_chow(&self) -> bool {
        self.kind == MeldKind::Chow
    }

    pub fn is_concealed(&self) -> bool {
        self.kind == MeldKind::ConcealedKong
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Set {
    Chow([Tile; 3]),
    Triplet([Tile; 3]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decomposition {
    pub pair: Tile,
    pub sets: Vec<Set>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TaiBreakdown {
    pub id: u8,
    pub name: String,
    pub value: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ScoreOutcome {
    pub total_tai: i32,
    pub tai: Vec<TaiBreakdown>,
    pub pair: Tile,
    pub sets: Vec<Vec<Tile>>,
}

#[derive(Clone, Debug)]
pub struct ScoreInput {
    pub concealed: Vec<Tile>,
    pub winning_tile: Tile,
    pub winning_in_hand: bool,
    pub exposed: Vec<Meld>,
    pub flowers: Vec<Tile>,
    pub winner: u8,
    pub card_owner: u8,
    pub dealer: u8,
    pub round_wind: u8,
    pub door_wind: u8,
    pub consecutive_dealer: u32,
    pub wall_remaining: u16,
    pub first_round: bool,
    pub source: WinSource,
}

pub const TAI_NAMES: [&str; 53] = [
    "莊家",
    "門清",
    "自摸",
    "斷么九",
    "一杯口",
    "槓上開花",
    "海底摸月",
    "河底撈魚",
    "搶槓",
    "東風",
    "南風",
    "西風",
    "北風",
    "紅中",
    "白",
    "發",
    "花牌",
    "東風東",
    "南風南",
    "西風西",
    "北風北",
    "春夏秋冬",
    "梅蘭菊竹",
    "全求人",
    "平胡",
    "混全帶么",
    "三色同順",
    "一條龍",
    "二杯口",
    "三暗刻",
    "三杠子",
    "三色同刻",
    "門清自摸",
    "碰碰胡",
    "混一色",
    "純全帶么",
    "混老頭",
    "小三元",
    "四暗刻",
    "四杠子",
    "大三元",
    "小四喜",
    "清一色",
    "字一色",
    "七搶一",
    "五暗刻",
    "清老頭",
    "大四喜",
    "八仙過海",
    "天胡",
    "地胡",
    "人胡",
    "連莊",
];

pub const TAI_VALUES: [i32; 53] = [
    1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 4, 4, 4, 4, 4, 6, 6, 8, 8, 8, 8, 0, 8, 8, 16, 0, 16, 16, 16, 2,
];

pub fn is_flower(tile: Tile) -> bool {
    (51..=58).contains(&tile)
}

pub fn is_suited(tile: Tile) -> bool {
    (1..=9).contains(&tile) || (11..=19).contains(&tile) || (21..=29).contains(&tile)
}

pub fn is_honor(tile: Tile) -> bool {
    (31..=34).contains(&tile) || (41..=43).contains(&tile)
}

pub fn is_terminal(tile: Tile) -> bool {
    is_suited(tile) && (tile % 10 == 1 || tile % 10 == 9)
}

fn counts(tiles: &[Tile]) -> [u8; 59] {
    let mut result = [0; 59];
    for &tile in tiles {
        if (tile as usize) < result.len() {
            result[tile as usize] += 1;
        }
    }
    result
}

fn first_tile(counts: &[u8; 59]) -> Option<Tile> {
    (1..59)
        .find(|&tile| counts[tile] > 0)
        .map(|tile| tile as Tile)
}

fn enumerate(
    counts: &mut [u8; 59],
    groups_left: usize,
    pair: Option<Tile>,
    sets: &mut Vec<Set>,
    output: &mut Vec<Decomposition>,
) {
    let Some(lead) = first_tile(counts) else {
        if groups_left == 0 {
            if let Some(pair) = pair {
                output.push(Decomposition {
                    pair,
                    sets: sets.clone(),
                });
            }
        }
        return;
    };
    if groups_left > 0 {
        if counts[lead as usize] >= 3 {
            counts[lead as usize] -= 3;
            sets.push(Set::Triplet([lead; 3]));
            enumerate(counts, groups_left - 1, pair, sets, output);
            sets.pop();
            counts[lead as usize] += 3;
        }
        if is_suited(lead) && lead % 10 <= 7 {
            let b = lead + 1;
            let c = lead + 2;
            if counts[b as usize] > 0 && counts[c as usize] > 0 {
                counts[lead as usize] -= 1;
                counts[b as usize] -= 1;
                counts[c as usize] -= 1;
                sets.push(Set::Chow([lead, b, c]));
                enumerate(counts, groups_left - 1, pair, sets, output);
                sets.pop();
                counts[lead as usize] += 1;
                counts[b as usize] += 1;
                counts[c as usize] += 1;
            }
        }
    }
    if pair.is_none() && counts[lead as usize] >= 2 {
        counts[lead as usize] -= 2;
        enumerate(counts, groups_left, Some(lead), sets, output);
        counts[lead as usize] += 2;
    }
}

fn decompositions(tiles: &[Tile], groups: usize) -> Vec<Decomposition> {
    let mut counts = counts(tiles);
    let mut output = Vec::new();
    enumerate(&mut counts, groups, None, &mut Vec::new(), &mut output);
    output
}

fn group_tiles(set: &Set) -> Vec<Tile> {
    match set {
        Set::Chow(tiles) | Set::Triplet(tiles) => tiles.to_vec(),
    }
}

fn all_groups(decomposition: &Decomposition, exposed: &[Meld]) -> Vec<Vec<Tile>> {
    decomposition
        .sets
        .iter()
        .map(group_tiles)
        .chain(exposed.iter().map(|meld| meld.tiles.clone()))
        .collect()
}

fn all_tiles(input: &ScoreInput, decomposition: &Decomposition) -> Vec<Tile> {
    let mut result = vec![decomposition.pair; 2];
    for set in &decomposition.sets {
        result.extend(group_tiles(set));
    }
    for meld in &input.exposed {
        result.extend(meld.tiles.iter().copied());
    }
    result
}

fn count_tile(tiles: &[Tile], tile: Tile) -> usize {
    tiles.iter().filter(|&&candidate| candidate == tile).count()
}

fn has_suited(tiles: &[Tile]) -> bool {
    tiles.iter().any(|&tile| is_suited(tile))
}

fn has_honor(tiles: &[Tile]) -> bool {
    tiles.iter().any(|&tile| is_honor(tile))
}

fn group_has_outside(group: &[Tile]) -> bool {
    group
        .iter()
        .any(|&tile| is_terminal(tile) || is_honor(tile))
}

fn tile_has_outside(tile: Tile) -> bool {
    is_terminal(tile) || is_honor(tile)
}

fn legacy_closed(input: &ScoreInput) -> bool {
    input.concealed.len() == 16 || (input.winning_in_hand && input.concealed.len() == 17)
}

fn has_alternate_wait(input: &ScoreInput) -> bool {
    let rank = input.winning_tile % 10;
    if !is_suited(input.winning_tile) {
        return false;
    }
    let mut alternatives = Vec::with_capacity(2);
    if rank <= 6 {
        alternatives.push(input.winning_tile + 3);
    }
    if rank >= 4 {
        alternatives.push(input.winning_tile - 3);
    }
    let Some(groups) = 5usize.checked_sub(input.exposed.len()) else {
        return false;
    };
    alternatives.into_iter().any(|tile| {
        let mut concealed = input.concealed.clone();
        concealed.push(tile);
        !decompositions(&concealed, groups).is_empty()
    })
}

fn triplet_like(group: &[Tile]) -> bool {
    group.len() >= 3 && group[0] == group[1] && group[1] == group[2]
}

fn chow_base(group: &[Tile]) -> Option<Tile> {
    (group.len() == 3
        && is_suited(group[0])
        && group[1] == group[0] + 1
        && group[2] == group[0] + 2)
        .then_some(group[0])
}

fn all_chow(decomposition: &Decomposition, exposed: &[Meld]) -> bool {
    decomposition
        .sets
        .iter()
        .all(|set| matches!(set, Set::Chow(_)))
        && exposed.iter().all(|meld| meld.is_chow())
}

fn double_sequence_count(decomposition: &Decomposition, exposed: &[Meld]) -> usize {
    let mut counts = BTreeMap::<Tile, usize>::new();
    for set in &decomposition.sets {
        if let Set::Chow(tiles) = set {
            *counts.entry(tiles[0]).or_default() += 1;
        }
    }
    for meld in exposed {
        if let Some(base) = chow_base(&meld.tiles) {
            *counts.entry(base).or_default() += 1;
        }
    }
    counts.values().filter(|&&count| count >= 2).count()
}

fn has_three_color_sequences(decomposition: &Decomposition, exposed: &[Meld]) -> bool {
    let mut bases = BTreeMap::<u8, [bool; 3]>::new();
    let mut visit = |group: &[Tile]| {
        if let Some(base) = chow_base(group) {
            let rank = base % 10;
            let suit = (base / 10) as usize;
            if suit < 3 {
                bases.entry(rank).or_default()[suit] = true;
            }
        }
    };
    for set in &decomposition.sets {
        let group = group_tiles(set);
        visit(&group);
    }
    for meld in exposed {
        visit(&meld.tiles);
    }
    bases.values().any(|flags| flags.iter().all(|flag| *flag))
}

fn set_score(scores: &mut [i32; 53], id: usize, value: i32) {
    scores[id] = value;
}

fn score_one(input: &ScoreInput, decomposition: &Decomposition) -> [i32; 53] {
    let mut score = [0; 53];
    let tiles = all_tiles(input, decomposition);
    let groups = all_groups(decomposition, &input.exposed);
    let self_draw = input.winner == input.card_owner;
    let suited_tiles = has_suited(&tiles);
    let no_exposed = input.exposed.is_empty();

    if input.winner == input.dealer {
        set_score(&mut score, 0, 1);
    }
    if no_exposed && legacy_closed(input) {
        set_score(&mut score, 1, 1);
    }
    if self_draw {
        set_score(&mut score, 2, 1);
    }
    if tiles
        .iter()
        .all(|&tile| !is_terminal(tile) && !is_honor(tile))
    {
        set_score(&mut score, 3, 1);
    }

    let doubles = double_sequence_count(decomposition, &input.exposed);
    if no_exposed && doubles == 1 {
        set_score(&mut score, 4, 1);
    } else if no_exposed && doubles >= 2 {
        set_score(&mut score, 28, 2);
    }
    if input.source == WinSource::KongReplacement {
        set_score(&mut score, 5, 1);
    }
    if input.wall_remaining == 16 && self_draw {
        set_score(&mut score, 6, 1);
    }
    if input.wall_remaining == 16 && !self_draw {
        set_score(&mut score, 7, 1);
    }

    for (index, wind) in [(9, 31), (10, 32), (11, 33), (12, 34)] {
        if count_tile(&tiles, wind) >= 3 {
            let mut value = 0;
            if input.round_wind == wind - 30 {
                value += 1;
            }
            if input.door_wind == wind - 30 {
                value += 1;
            }
            if value == 2 {
                set_score(&mut score, index + 8, 2);
            } else {
                set_score(&mut score, index, value);
            }
        }
    }
    for (index, dragon) in [(13, 41), (14, 42), (15, 43)] {
        if count_tile(&tiles, dragon) >= 3 {
            set_score(&mut score, index, 1);
        }
    }

    let matching_flowers = input
        .flowers
        .iter()
        .filter(|&&flower| {
            let index = (flower - 51) as usize;
            index == (input.door_wind - 1) as usize || index == (input.door_wind + 3) as usize
        })
        .count() as i32;
    set_score(&mut score, 16, matching_flowers);
    if (0..4).all(|index| input.flowers.contains(&(51 + index))) {
        set_score(&mut score, 21, 2);
        score[16] -= 1;
    }
    if (4..8).all(|index| input.flowers.contains(&(51 + index))) {
        set_score(&mut score, 22, 2);
        score[16] -= 1;
    }

    if !self_draw && input.concealed.len() == 1 && input.exposed.len() == 5 {
        set_score(&mut score, 23, 2);
    }

    let ping_hu = input.flowers.is_empty()
        && input.source == WinSource::Discard
        && !is_honor(decomposition.pair)
        && decomposition.pair != input.winning_tile
        && all_chow(decomposition, &input.exposed)
        && input.exposed.iter().all(|meld| meld.is_chow());
    if ping_hu && has_alternate_wait(input) {
        set_score(&mut score, 24, 2);
    }

    let concealed_has_suited_group = decomposition
        .sets
        .iter()
        .map(group_tiles)
        .any(|group| has_suited(&group))
        || is_suited(decomposition.pair);
    if suited_tiles
        && concealed_has_suited_group
        && tile_has_outside(decomposition.pair)
        && groups.iter().all(|group| group_has_outside(group))
    {
        score[25] = if legacy_closed(input) { 2 } else { 1 };
    }
    if has_three_color_sequences(decomposition, &input.exposed) {
        score[26] = if legacy_closed(input) { 2 } else { 1 };
    }
    let one_to_nine = [(1, 4, 7), (11, 14, 17), (21, 24, 27)];
    let one_long_straight = one_to_nine.iter().any(|&(a, b, c)| {
        let mut bases = BTreeMap::new();
        for set in &decomposition.sets {
            if let Set::Chow(tiles) = set {
                bases.insert(tiles[0], true);
            }
        }
        for meld in &input.exposed {
            if let Some(base) = chow_base(&meld.tiles) {
                bases.insert(base, true);
            }
        }
        bases.contains_key(&a) && bases.contains_key(&b) && bases.contains_key(&c)
    });
    if one_long_straight {
        score[27] = if legacy_closed(input) { 2 } else { 1 };
    }

    let concealed_triplets = decomposition
        .sets
        .iter()
        .filter(|set| {
            matches!(
                set,
                Set::Triplet(tiles)
                    if !(!self_draw && tiles[0] == input.winning_tile)
            )
        })
        .count()
        + input
            .exposed
            .iter()
            .filter(|meld| meld.is_concealed())
            .count();
    if concealed_triplets == 3 {
        score[29] = 2;
    } else if concealed_triplets == 4 {
        score[38] = 6;
    } else if concealed_triplets == 5 {
        score[45] = 8;
    }

    let kong_count = input
        .exposed
        .iter()
        .filter(|meld| {
            matches!(
                meld.kind,
                MeldKind::DiscardKong | MeldKind::ConcealedKong | MeldKind::AddedKong
            )
        })
        .count();
    if kong_count == 3 {
        score[30] = 2;
    } else if kong_count == 4 {
        score[39] = 6;
    }

    let three_color_triplet = (1..=9).any(|rank| {
        (0..3).all(|suit| {
            let tile = (suit * 10 + rank) as Tile;
            input
                .exposed
                .iter()
                .any(|meld| triplet_like(&meld.tiles) && meld.tiles[0] == tile)
                || decomposition
                    .sets
                    .iter()
                    .any(|set| matches!(set, Set::Triplet(tiles) if tiles[0] == tile))
        })
    });
    if three_color_triplet {
        score[31] = 2;
    }

    if no_exposed && legacy_closed(input) && self_draw {
        score[32] = 3;
        score[1] = 0;
        score[2] = 0;
    }
    if all_triplets(decomposition, &input.exposed) {
        score[33] = 4;
        if concealed_triplets == 5 {
            score[33] = 0;
        }
    }

    let suits: Vec<u8> = tiles
        .iter()
        .filter(|&&tile| is_suited(tile))
        .map(|&tile| tile / 10)
        .collect();
    let mixed_one_suit = suited_tiles && suits.iter().all(|&suit| suit == suits[0]);
    if mixed_one_suit && has_honor(&tiles) {
        score[34] = 4;
    }

    let pure_outside = tiles.iter().all(|&tile| is_suited(tile))
        && tile_has_outside(decomposition.pair)
        && groups.iter().all(|group| group_has_outside(group));
    if pure_outside {
        score[35] = 4;
        score[25] = 0;
    }
    let mixed_terminals = suited_tiles
        && tiles
            .iter()
            .all(|&tile| is_terminal(tile) || is_honor(tile));
    if mixed_terminals {
        score[36] = 4;
        score[25] = 0;
        score[33] = 0;
    }

    if (41..=43).all(|dragon| count_tile(&tiles, dragon) >= 2) {
        score[37] = 4;
        score[13] = 0;
        score[14] = 0;
        score[15] = 0;
    }
    if (41..=43).all(|dragon| count_tile(&tiles, dragon) >= 3) {
        score[40] = 8;
        score[13] = 0;
        score[14] = 0;
        score[15] = 0;
        score[37] = 0;
    }
    if tiles.iter().all(|&tile| is_honor(tile)) {
        score[43] = 8;
        score[33] = 0;
    }
    let pure_suit =
        suited_tiles && !has_honor(&tiles) && suits.iter().all(|&suit| suit == suits[0]);
    if pure_suit {
        score[42] = 8;
        score[34] = 0;
    }

    if (31..=34).all(|wind| count_tile(&tiles, wind) >= 2) {
        score[41] = 8;
        for value in score.iter_mut().take(13).skip(9) {
            *value = 0;
        }
        for value in score.iter_mut().take(21).skip(17) {
            *value = 0;
        }
    }
    if tiles.iter().all(|&tile| is_terminal(tile)) {
        score[46] = 8;
        score[33] = 0;
        score[35] = 0;
        score[36] = 0;
    }
    if (31..=34).all(|wind| count_tile(&tiles, wind) >= 3) {
        score[47] = 16;
        for value in score.iter_mut().take(13).skip(9) {
            *value = 0;
        }
        for value in score.iter_mut().take(21).skip(17) {
            *value = 0;
        }
        score[41] = 0;
    }

    if input.first_round && input.winner == input.dealer {
        score[49] = 16;
    }
    if input.first_round && self_draw && input.winner != input.dealer {
        score[50] = 16;
    }
    if input.first_round && input.winner != input.card_owner && input.winner != input.dealer {
        score[51] = 16;
    }
    if input.consecutive_dealer > 0 && input.winner == input.dealer {
        score[52] = input.consecutive_dealer as i32 * 2;
    }
    score
}

fn all_triplets(decomposition: &Decomposition, exposed: &[Meld]) -> bool {
    decomposition
        .sets
        .iter()
        .all(|set| matches!(set, Set::Triplet(_)))
        && exposed.iter().all(|meld| triplet_like(&meld.tiles))
}

pub fn score_hand(input: &ScoreInput) -> Option<ScoreOutcome> {
    let mut concealed = input.concealed.clone();
    if !input.winning_in_hand {
        concealed.push(input.winning_tile);
    }
    if concealed.iter().any(|&tile| is_flower(tile)) {
        return None;
    }
    let groups = 5usize.checked_sub(input.exposed.len())?;
    let candidates = decompositions(&concealed, groups);
    let mut best: Option<(i32, Decomposition, [i32; 53])> = None;
    for decomposition in candidates {
        let tai = score_one(input, &decomposition);
        let total = tai.iter().sum();
        if best
            .as_ref()
            .is_none_or(|(best_total, _, _)| total > *best_total)
        {
            best = Some((total, decomposition, tai));
        }
    }
    let (total_tai, decomposition, tai) = best?;
    let entries = tai
        .iter()
        .enumerate()
        .filter(|(_, value)| **value != 0)
        .map(|(id, &value)| TaiBreakdown {
            id: id as u8,
            name: TAI_NAMES[id].to_string(),
            value,
        })
        .collect();
    Some(ScoreOutcome {
        total_tai,
        tai: entries,
        pair: decomposition.pair,
        sets: decomposition.sets.iter().map(group_tiles).collect(),
    })
}
