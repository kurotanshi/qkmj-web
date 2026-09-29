use crate::ai;
use crate::rules::{
    is_flower, score_hand, Meld, MeldKind, ScoreInput, TaiBreakdown, Tile, WinSource,
};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const PLAYER_COUNT: usize = 4;
pub const HUMAN_SEAT: u8 = 0;
pub const DEFAULT_MONEY: i64 = 20_000;
pub const DEFAULT_BASE: i64 = 500;
pub const DEFAULT_TAI: i64 = 200;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Weak,
    Medium,
    Strong,
}

impl Difficulty {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "weak" => Some(Self::Weak),
            "medium" => Some(Self::Medium),
            "strong" => Some(Self::Strong),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KongKind {
    Discard,
    Concealed,
    Added,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionKind {
    Draw,
    Discard { tile: Tile },
    Win,
    Pass,
    Chow { tiles: [Tile; 2] },
    Pong { tile: Tile },
    Kong { tile: Tile, kind: KongKind },
}

impl ActionKind {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Action {
    pub revision: u64,
    pub actor: u8,
    pub kind: ActionKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Setup,
    Opening,
    NeedDraw { seat: u8 },
    NeedDiscard { seat: u8 },
    Claim { discarder: u8, tile: Tile },
    Result,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Discard {
    pub tile: Tile,
    pub claimed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PublicPlayer {
    pub seat: u8,
    pub name: String,
    pub physical_wind: u8,
    pub door_wind: u8,
    pub score: i64,
    pub difficulty: Option<Difficulty>,
    pub concealed_count: usize,
    pub flowers: Vec<Tile>,
    pub melds: Vec<Meld>,
    pub discards: Vec<Discard>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WinningDecomposition {
    pub pair: Tile,
    pub sets: Vec<Vec<Tile>>,
    pub exposed: Vec<Meld>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RoundResult {
    pub winner: Option<u8>,
    pub source: Option<WinSource>,
    pub winning_tile: Option<Tile>,
    pub decomposition: Option<WinningDecomposition>,
    pub payment_source: Option<u8>,
    pub total_tai: i32,
    pub tai: Vec<TaiBreakdown>,
    pub base_value: i64,
    pub tai_value: i64,
    pub changes: [i64; PLAYER_COUNT],
    pub dealer_before: u8,
    pub consecutive_dealer_before: u32,
    pub dealer_continued: bool,
    pub dealer_surcharge_tai: i32,
    pub dealer_after: u8,
    pub round_wind_after: u8,
    pub consecutive_dealer_after: u32,
    pub revealed_hands: Vec<Vec<Tile>>,
    pub draw: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PublicState {
    pub api_version: u8,
    pub revision: u64,
    pub phase: Phase,
    pub status: String,
    pub dealer: u8,
    pub round_wind: u8,
    pub consecutive_dealer: u32,
    pub wall_remaining: u16,
    pub current_seat: Option<u8>,
    pub card_owner: Option<u8>,
    pub human_seat: u8,
    pub players: Vec<PublicPlayer>,
    pub result: Option<RoundResult>,
    pub reveal_hands: Option<Vec<Vec<Tile>>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PrivateState {
    pub seat: u8,
    pub hand: Vec<Tile>,
    pub legal_actions: Vec<Action>,
    pub needs_human: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ApiSnapshot {
    pub public: PublicState,
    pub private: PrivateState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    Draw {
        seat: u8,
        tile: Option<Tile>,
        source: Option<WinSource>,
    },
    Discard {
        seat: u8,
        tile: Tile,
    },
    Claim {
        seat: u8,
        kind: String,
        tile: Tile,
    },
    Pass {
        seat: u8,
    },
    Win {
        seat: u8,
        source: WinSource,
    },
    DrawResult,
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    InvalidSeat,
    InvalidPhase,
    StaleRevision,
    NotYourTurn,
    IllegalAction,
    HandLocked,
    CannotStart,
    NotResult,
    InvalidConfiguration,
    InvalidJson(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSeat => "無效座位",
            Self::InvalidPhase => "目前階段不接受這個動作",
            Self::StaleRevision => "畫面版本已過期，請重新選擇",
            Self::NotYourTurn => "還沒輪到這個座位",
            Self::IllegalAction => "不是合法動作",
            Self::HandLocked => "本局已開始，不能更改難度",
            Self::CannotStart => "目前不能開始",
            Self::NotResult => "尚未結算",
            Self::InvalidConfiguration => "無效設定",
            Self::InvalidJson(message) => message,
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EngineError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Player {
    name: String,
    hand: Vec<Tile>,
    flowers: Vec<Tile>,
    melds: Vec<Meld>,
    discards: Vec<Discard>,
    score: i64,
    door_wind: u8,
    first_round: bool,
    difficulty: Option<Difficulty>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub seat: u8,
    pub hand: Vec<Tile>,
    pub flowers: Vec<Tile>,
    pub melds: Vec<Meld>,
    pub public: PublicState,
    pub legal_actions: Vec<ActionKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    players: [Player; PLAYER_COUNT],
    wall: Vec<Tile>,
    wall_pos: usize,
    seed: u64,
    hand_number: u32,
    revision: u64,
    dealer: u8,
    round_wind: u8,
    consecutive_dealer: u32,
    phase: Phase,
    current_seat: Option<u8>,
    card_owner: Option<u8>,
    last_discard: Option<Tile>,
    last_draw: Option<Tile>,
    last_draw_source: Option<WinSource>,
    pending_replacement: bool,
    drawn_for_turn: bool,
    responses: [Option<ActionKind>; PLAYER_COUNT],
    result: Option<RoundResult>,
    status: String,
    policy_seed: u64,
    policy_counter: u64,
    hand_locked: bool,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        let players = std::array::from_fn(|seat| Player {
            name: if seat == 0 {
                "你".to_string()
            } else {
                format!("電腦{}", seat)
            },
            hand: Vec::new(),
            flowers: Vec::new(),
            melds: Vec::new(),
            discards: Vec::new(),
            score: DEFAULT_MONEY,
            door_wind: (seat + 1) as u8,
            first_round: true,
            difficulty: (seat != 0).then_some(Difficulty::Medium),
        });
        let mut game = Self {
            players,
            wall: Vec::new(),
            wall_pos: 0,
            seed: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
            hand_number: 0,
            revision: 0,
            dealer: 0,
            round_wind: 1,
            consecutive_dealer: 0,
            phase: Phase::Setup,
            current_seat: None,
            card_owner: None,
            last_discard: None,
            last_draw: None,
            last_draw_source: None,
            pending_replacement: false,
            drawn_for_turn: false,
            responses: std::array::from_fn(|_| None),
            result: None,
            status: "請選擇電腦難度後開始".to_string(),
            policy_seed: mix(seed ^ 0x6a09e667f3bcc909),
            policy_counter: 0,
            hand_locked: false,
        };
        game.begin_hand();
        game
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn wall_remaining(&self) -> u16 {
        self.wall.len().saturating_sub(self.wall_pos) as u16
    }

    pub fn scores(&self) -> [i64; PLAYER_COUNT] {
        std::array::from_fn(|seat| self.players[seat].score)
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn result(&self) -> Option<&RoundResult> {
        self.result.as_ref()
    }

    pub fn set_difficulty(&mut self, seat: u8, difficulty: Difficulty) -> Result<(), EngineError> {
        if self.hand_locked && !matches!(self.phase, Phase::Setup | Phase::Result) {
            return Err(EngineError::HandLocked);
        }
        let seat = self.player_index(seat)?;
        if seat == 0 {
            return Err(EngineError::InvalidConfiguration);
        }
        self.players[seat].difficulty = Some(difficulty);
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), EngineError> {
        if matches!(self.phase, Phase::Result) {
            return Err(EngineError::CannotStart);
        }
        self.hand_locked = true;
        self.status = "牌局開始".to_string();
        Ok(())
    }

    pub fn next_hand(&mut self) -> Result<(), EngineError> {
        if !matches!(self.phase, Phase::Result) {
            return Err(EngineError::NotResult);
        }
        self.hand_number = self.hand_number.saturating_add(1);
        self.revision = self.revision.saturating_add(1);
        self.begin_hand();
        self.hand_locked = true;
        Ok(())
    }

    pub fn legal_actions(&self, seat: u8) -> Vec<Action> {
        let Ok(seat) = self.player_index(seat) else {
            return Vec::new();
        };
        self.legal_action_kinds(seat as u8)
            .into_iter()
            .map(|kind| Action {
                revision: self.revision,
                actor: seat as u8,
                kind,
            })
            .collect()
    }

    pub fn legal_action_kinds(&self, seat: u8) -> Vec<ActionKind> {
        let Ok(seat) = self.player_index(seat) else {
            return Vec::new();
        };
        match self.phase {
            Phase::NeedDraw { seat: current } if current == seat as u8 => vec![ActionKind::Draw],
            Phase::NeedDiscard { seat: current } if current == seat as u8 => {
                self.discard_actions(seat as u8)
            }
            Phase::Claim { discarder, tile } if discarder != seat as u8 => {
                if self.responses[seat].is_some() {
                    return Vec::new();
                }
                let mut actions = Vec::new();
                if self.can_win(seat as u8, tile, WinSource::Discard) {
                    actions.push(ActionKind::Win);
                }
                if self.count_in_hand(seat, tile) >= 3 {
                    actions.push(ActionKind::Kong {
                        tile,
                        kind: KongKind::Discard,
                    });
                }
                if self.count_in_hand(seat, tile) >= 2 {
                    actions.push(ActionKind::Pong { tile });
                }
                if next_seat(discarder) == seat as u8 {
                    for tiles in self.chow_options(seat, tile) {
                        actions.push(ActionKind::Chow { tiles });
                    }
                }
                actions.push(ActionKind::Pass);
                actions
            }
            _ => Vec::new(),
        }
    }

    pub fn apply(&mut self, action: Action) -> Result<Vec<GameEvent>, EngineError> {
        self.validate_action(&action)?;
        self.hand_locked = true;
        let actor = self.player_index(action.actor)?;
        let kind = action.kind.clone();
        let mut events = Vec::new();
        match kind {
            ActionKind::Draw => {
                let source = if self.pending_replacement {
                    WinSource::KongReplacement
                } else {
                    WinSource::NormalDraw
                };
                self.pending_replacement = false;
                let drawn = self.draw_until_live(actor as u8, source);
                events.push(GameEvent::Draw {
                    seat: actor as u8,
                    tile: drawn,
                    source: self.last_draw_source,
                });
                if drawn.is_some() && !matches!(self.phase, Phase::Result) {
                    self.phase = Phase::NeedDiscard { seat: actor as u8 };
                    self.current_seat = Some(actor as u8);
                    self.status = format!("{} 請出牌", self.players[actor].name);
                }
            }
            ActionKind::Discard { tile } => {
                self.discard(actor as u8, tile)?;
                events.push(GameEvent::Discard {
                    seat: actor as u8,
                    tile,
                });
            }
            ActionKind::Win => match self.phase {
                Phase::NeedDiscard { .. } => {
                    let tile = self.last_draw.ok_or(EngineError::IllegalAction)?;
                    let source = self.last_draw_source.ok_or(EngineError::IllegalAction)?;
                    self.finish_win(actor as u8, tile, source)?;
                    events.push(GameEvent::Win {
                        seat: actor as u8,
                        source,
                    });
                }
                Phase::Claim { .. } => {
                    self.responses[actor] = Some(ActionKind::Win);
                    self.resolve_claims(&mut events)?;
                }
                _ => return Err(EngineError::InvalidPhase),
            },
            ActionKind::Kong { tile, kind } => {
                if matches!(self.phase, Phase::Claim { .. }) {
                    self.record_claim(actor as u8, ActionKind::Kong { tile, kind }, &mut events)?;
                } else {
                    self.apply_kong(actor as u8, tile, kind)?;
                    events.push(GameEvent::Claim {
                        seat: actor as u8,
                        kind: "kong".to_string(),
                        tile,
                    });
                }
            }
            ActionKind::Pong { tile } => {
                self.record_claim(actor as u8, ActionKind::Pong { tile }, &mut events)?;
            }
            ActionKind::Chow { tiles } => {
                self.record_claim(actor as u8, ActionKind::Chow { tiles }, &mut events)?;
            }
            ActionKind::Pass => {
                self.responses[actor] = Some(ActionKind::Pass);
                events.push(GameEvent::Pass { seat: actor as u8 });
                self.resolve_claims(&mut events)?;
            }
        }
        self.revision = self.revision.saturating_add(1);
        Ok(events)
    }

    pub fn apply_json(&mut self, json: &str) -> Result<Vec<GameEvent>, EngineError> {
        if json.len() > 4096 {
            return Err(EngineError::InvalidJson("動作資料過大".to_string()));
        }
        let action: Action = serde_json::from_str(json)
            .map_err(|error| EngineError::InvalidJson(error.to_string()))?;
        if action.actor != HUMAN_SEAT {
            return Err(EngineError::NotYourTurn);
        }
        self.apply(action)
    }

    pub fn bot_step(&mut self) -> Result<Vec<GameEvent>, EngineError> {
        self.auto_pass_human_if_unable()?;
        if self.human_needs_action() {
            return Ok(Vec::new());
        }
        let Some(seat) = self.bot_seat_to_move() else {
            return Ok(Vec::new());
        };
        let legal = self.legal_action_kinds(seat);
        if legal.is_empty() {
            return Err(EngineError::InvalidPhase);
        }
        let observation = self.observation(seat);
        let difficulty = self.players[seat as usize]
            .difficulty
            .unwrap_or(Difficulty::Medium);
        let seed = self
            .policy_seed
            .wrapping_add(self.policy_counter)
            .wrapping_add(self.revision);
        self.policy_counter = self.policy_counter.wrapping_add(1);
        let kind = ai::choose_action(&observation, &legal, difficulty, seed);
        self.apply(Action {
            revision: self.revision,
            actor: seat,
            kind,
        })
    }

    pub fn observation(&self, seat: u8) -> Observation {
        let index = self.player_index(seat).unwrap_or(0);
        Observation {
            seat,
            hand: self.players[index].hand.clone(),
            flowers: self.players[index].flowers.clone(),
            melds: self.players[index].melds.clone(),
            public: self.public_state(),
            legal_actions: self.legal_action_kinds(seat),
        }
    }

    pub fn public_state(&self) -> PublicState {
        let players = self
            .players
            .iter()
            .enumerate()
            .map(|(seat, player)| PublicPlayer {
                seat: seat as u8,
                name: player.name.clone(),
                physical_wind: seat as u8 + 1,
                door_wind: player.door_wind,
                score: player.score,
                difficulty: player.difficulty,
                concealed_count: player.hand.len(),
                flowers: player.flowers.clone(),
                melds: player.melds.clone(),
                discards: player.discards.clone(),
            })
            .collect();
        PublicState {
            api_version: 1,
            revision: self.revision,
            phase: self.phase.clone(),
            status: self.status.clone(),
            dealer: self.dealer,
            round_wind: self.round_wind,
            consecutive_dealer: self.consecutive_dealer,
            wall_remaining: self.wall_remaining(),
            current_seat: self.current_seat,
            card_owner: self.card_owner,
            human_seat: HUMAN_SEAT,
            players,
            result: self.result.clone(),
            reveal_hands: self
                .result
                .as_ref()
                .map(|result| result.revealed_hands.clone()),
        }
    }

    pub fn private_state(&self, seat: u8) -> Result<PrivateState, EngineError> {
        let seat_index = self.player_index(seat)?;
        Ok(PrivateState {
            seat,
            hand: self.players[seat_index].hand.clone(),
            legal_actions: self.legal_actions(seat),
            needs_human: seat == HUMAN_SEAT && self.human_needs_action(),
        })
    }

    pub fn snapshot(&self) -> ApiSnapshot {
        ApiSnapshot {
            public: self.public_state(),
            private: self.private_state(HUMAN_SEAT).expect("human seat is fixed"),
        }
    }

    pub fn tile_conservation(&self) -> bool {
        let expected = tile_counts(&full_deck());
        let mut actual = [0u8; 59];
        let mut add = |tile: Tile| -> bool {
            let index = tile as usize;
            if index >= actual.len() {
                return false;
            }
            actual[index] = actual[index].saturating_add(1);
            true
        };
        for &tile in &self.wall[self.wall_pos..] {
            if !add(tile) {
                return false;
            }
        }
        for player in &self.players {
            for &tile in player.hand.iter().chain(&player.flowers) {
                if !add(tile) {
                    return false;
                }
            }
            for meld in &player.melds {
                for &tile in &meld.tiles {
                    if !add(tile) {
                        return false;
                    }
                }
            }
            for discard in player.discards.iter().filter(|discard| !discard.claimed) {
                if !add(discard.tile) {
                    return false;
                }
            }
        }
        actual == expected
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn fixture(
        hands: [Vec<Tile>; PLAYER_COUNT],
        melds: [Vec<Meld>; PLAYER_COUNT],
        wall_remaining: Vec<Tile>,
        phase: Phase,
        dealer: u8,
        card_owner: Option<u8>,
        last_draw: Option<Tile>,
        last_draw_source: Option<WinSource>,
        drawn_for_turn: bool,
    ) -> Self {
        let mut players = std::array::from_fn(|seat| Player {
            name: if seat == 0 {
                "你".to_string()
            } else {
                format!("電腦{}", seat)
            },
            hand: hands[seat].clone(),
            flowers: Vec::new(),
            melds: melds[seat].clone(),
            discards: Vec::new(),
            score: DEFAULT_MONEY,
            door_wind: seat as u8 + 1,
            first_round: true,
            difficulty: (seat != 0).then_some(Difficulty::Medium),
        });
        for player in &mut players {
            player.hand.sort_unstable();
        }
        let wall_pos = 144usize.saturating_sub(wall_remaining.len());
        let mut wall = vec![0; wall_pos];
        wall.extend(wall_remaining);
        let mut game = Self {
            players,
            wall,
            wall_pos,
            seed: 1,
            hand_number: 0,
            revision: 0,
            dealer,
            round_wind: 1,
            consecutive_dealer: 0,
            phase: phase.clone(),
            current_seat: phase_seat(&phase),
            card_owner,
            last_discard: match &phase {
                Phase::Claim { tile, .. } => Some(*tile),
                _ => None,
            },
            last_draw,
            last_draw_source,
            pending_replacement: false,
            drawn_for_turn,
            responses: std::array::from_fn(|_| None),
            result: None,
            status: "測試牌局".to_string(),
            policy_seed: 7,
            policy_counter: 0,
            hand_locked: true,
        };
        if let Phase::Claim { discarder, tile } = &phase {
            game.players[*discarder as usize].discards.push(Discard {
                tile: *tile,
                claimed: false,
            });
        }
        game
    }

    fn validate_action(&self, action: &Action) -> Result<(), EngineError> {
        self.player_index(action.actor)?;
        if action.revision != self.revision {
            return Err(EngineError::StaleRevision);
        }
        if !self.legal_action_kinds(action.actor).contains(&action.kind) {
            return Err(match self.phase {
                Phase::NeedDraw { seat } | Phase::NeedDiscard { seat } if seat != action.actor => {
                    EngineError::NotYourTurn
                }
                Phase::Claim { discarder, .. } if discarder == action.actor => {
                    EngineError::NotYourTurn
                }
                _ => EngineError::IllegalAction,
            });
        }
        Ok(())
    }

    fn player_index(&self, seat: u8) -> Result<usize, EngineError> {
        (seat < PLAYER_COUNT as u8)
            .then_some(seat as usize)
            .ok_or(EngineError::InvalidSeat)
    }

    fn begin_hand(&mut self) {
        self.phase = Phase::Opening;
        self.current_seat = Some(self.dealer);
        self.card_owner = Some(self.dealer);
        self.last_discard = None;
        self.last_draw = None;
        self.last_draw_source = Some(WinSource::Initial);
        self.pending_replacement = false;
        self.drawn_for_turn = true;
        self.responses = std::array::from_fn(|_| None);
        self.result = None;
        self.hand_locked = false;
        self.policy_counter = 0;
        for (seat, player) in self.players.iter_mut().enumerate() {
            player.hand.clear();
            player.flowers.clear();
            player.melds.clear();
            player.discards.clear();
            player.first_round = true;
            player.door_wind = (seat + 1) as u8;
        }
        self.wall = full_deck();
        shuffle(
            &mut self.wall,
            mix(self.seed ^ self.hand_number as u64 ^ 0x243f6a8885a308d3),
        );
        let mut door_rng = mix(self.seed ^ self.hand_number as u64 ^ 0x13198a2e03707344);
        let door_start = (next_u64(&mut door_rng) % PLAYER_COUNT as u64) as usize;
        for offset in 0..PLAYER_COUNT {
            let seat = (door_start + offset) % PLAYER_COUNT;
            self.players[seat].door_wind = offset as u8 + 1;
        }
        self.wall_pos = 0;
        for seat in 0..PLAYER_COUNT {
            for _ in 0..16 {
                let tile = self.take_raw().expect("144-tile wall has opening deal");
                self.players[seat].hand.push(tile);
            }
        }
        for player in &mut self.players {
            player.hand.sort_unstable();
        }
        let dealer_tile = self.take_raw().expect("144-tile wall has dealer tile");
        self.players[self.dealer as usize].hand.push(dealer_tile);

        let dealer_extra = self.players[self.dealer as usize].hand.len() - 1;
        if !self.replace_flower_at(self.dealer as usize, dealer_extra) {
            return;
        }
        let opening_tile = self.players[self.dealer as usize].hand[dealer_extra];
        for offset in 0..PLAYER_COUNT {
            let seat = (self.dealer as usize + offset) % PLAYER_COUNT;
            for position in 0..16 {
                if !self.replace_flower_at(seat, position) {
                    return;
                }
            }
        }
        for player in &mut self.players {
            player.hand.sort_unstable();
        }
        self.phase = Phase::NeedDiscard { seat: self.dealer };
        self.current_seat = Some(self.dealer);
        self.card_owner = Some(self.dealer);
        self.last_draw = Some(opening_tile);
        self.last_draw_source = Some(WinSource::Initial);
        self.status = "莊家先打出一張牌".to_string();
    }

    fn take_raw(&mut self) -> Option<Tile> {
        let tile = self.wall.get(self.wall_pos).copied()?;
        self.wall_pos += 1;
        Some(tile)
    }

    fn can_draw(&self) -> bool {
        self.wall_remaining() > 16
    }

    fn replace_flower_at(&mut self, seat: usize, position: usize) -> bool {
        loop {
            let Some(&tile) = self.players[seat].hand.get(position) else {
                return true;
            };
            if !is_flower(tile) {
                return true;
            }
            self.players[seat].hand.remove(position);
            self.players[seat].flowers.push(tile);
            if !self.can_draw() {
                self.finish_draw();
                return false;
            }
            let replacement = self.take_raw().expect("reserve check protects replacement");
            self.players[seat].hand.insert(position, replacement);
        }
    }

    fn draw_until_live(&mut self, seat: u8, mut source: WinSource) -> Option<Tile> {
        let seat_index = seat as usize;
        loop {
            if !self.can_draw() {
                self.finish_draw();
                return None;
            }
            let tile = self.take_raw()?;
            if is_flower(tile) {
                self.players[seat_index].flowers.push(tile);
                if source != WinSource::KongReplacement {
                    source = WinSource::FlowerReplacement;
                }
                continue;
            }
            self.players[seat_index].hand.push(tile);
            self.players[seat_index].hand.sort_unstable();
            self.card_owner = Some(seat);
            self.last_draw = Some(tile);
            self.last_draw_source = Some(source);
            self.drawn_for_turn = true;
            return Some(tile);
        }
    }

    fn discard_actions(&self, seat: u8) -> Vec<ActionKind> {
        let mut actions = Vec::new();
        if self.drawn_for_turn {
            if let (Some(tile), Some(source)) = (self.last_draw, self.last_draw_source) {
                if self.can_win(seat, tile, source) {
                    actions.push(ActionKind::Win);
                }
            }
            for tile in unique_tiles(&self.players[seat as usize].hand) {
                if self.count_in_hand(seat as usize, tile) >= 4 {
                    actions.push(ActionKind::Kong {
                        tile,
                        kind: KongKind::Concealed,
                    });
                }
                if self.players[seat as usize]
                    .melds
                    .iter()
                    .any(|meld| meld.kind == MeldKind::Pong && meld.tiles.first() == Some(&tile))
                {
                    actions.push(ActionKind::Kong {
                        tile,
                        kind: KongKind::Added,
                    });
                }
            }
        }
        actions.extend(
            unique_tiles(&self.players[seat as usize].hand)
                .into_iter()
                .map(|tile| ActionKind::Discard { tile }),
        );
        actions
    }

    fn count_in_hand(&self, seat: usize, tile: Tile) -> usize {
        self.players[seat]
            .hand
            .iter()
            .filter(|&&candidate| candidate == tile)
            .count()
    }

    fn chow_options(&self, seat: usize, tile: Tile) -> Vec<[Tile; 2]> {
        if !crate::rules::is_suited(tile) {
            return Vec::new();
        }
        let rank = tile % 10;
        let candidates = [
            (tile.checked_sub(2), tile.checked_sub(1)),
            (tile.checked_sub(1), tile.checked_add(1)),
            (tile.checked_add(1), tile.checked_add(2)),
        ];
        candidates
            .into_iter()
            .filter_map(|(a, b)| Some([a?, b?]))
            .filter(|tiles| {
                tiles[0] / 10 == tile / 10
                    && tiles[1] / 10 == tile / 10
                    && match rank {
                        1 => tiles[0] == tile + 1 && tiles[1] == tile + 2,
                        2 => true,
                        3..=7 => true,
                        8 => true,
                        9 => tiles[0] == tile - 2 && tiles[1] == tile - 1,
                        _ => false,
                    }
            })
            .filter(|tiles| {
                self.count_in_hand(seat, tiles[0]) > 0 && self.count_in_hand(seat, tiles[1]) > 0
            })
            .collect()
    }

    fn can_win(&self, seat: u8, tile: Tile, source: WinSource) -> bool {
        let index = seat as usize;
        let input = ScoreInput {
            concealed: self.players[index].hand.clone(),
            winning_tile: tile,
            winning_in_hand: source != WinSource::Discard,
            exposed: self.players[index].melds.clone(),
            flowers: self.players[index].flowers.clone(),
            winner: seat,
            card_owner: self.card_owner.unwrap_or(seat),
            dealer: self.dealer,
            round_wind: self.round_wind,
            door_wind: self.players[index].door_wind,
            consecutive_dealer: self.consecutive_dealer,
            wall_remaining: self.wall_remaining(),
            first_round: self.players[index].first_round,
            source,
        };
        score_hand(&input).is_some()
    }

    fn discard(&mut self, seat: u8, tile: Tile) -> Result<(), EngineError> {
        let player = &mut self.players[seat as usize];
        let position = player
            .hand
            .iter()
            .position(|&candidate| candidate == tile)
            .ok_or(EngineError::IllegalAction)?;
        player.hand.remove(position);
        player.hand.sort_unstable();
        player.discards.push(Discard {
            tile,
            claimed: false,
        });
        player.first_round = false;
        self.last_discard = Some(tile);
        self.card_owner = Some(seat);
        self.last_draw = None;
        self.last_draw_source = None;
        self.drawn_for_turn = false;
        self.current_seat = Some(seat);
        self.phase = Phase::Claim {
            discarder: seat,
            tile,
        };
        self.responses = std::array::from_fn(|_| None);
        self.status = format!(
            "{} 打出 {}，等待吃碰槓胡",
            self.players[seat as usize].name,
            tile_label(tile)
        );
        Ok(())
    }

    fn record_claim(
        &mut self,
        seat: u8,
        kind: ActionKind,
        events: &mut Vec<GameEvent>,
    ) -> Result<(), EngineError> {
        let tile = self.last_discard.ok_or(EngineError::IllegalAction)?;
        self.responses[seat as usize] = Some(kind.clone());
        let label = match kind {
            ActionKind::Pong { .. } => "pong",
            ActionKind::Chow { .. } => "chow",
            _ => "claim",
        };
        events.push(GameEvent::Claim {
            seat,
            kind: label.to_string(),
            tile,
        });
        self.resolve_claims(events)
    }

    fn resolve_claims(&mut self, events: &mut Vec<GameEvent>) -> Result<(), EngineError> {
        let Phase::Claim { discarder, tile } = self.phase else {
            return Ok(());
        };
        if (0..PLAYER_COUNT).any(|seat| seat as u8 != discarder && self.responses[seat].is_none()) {
            return Ok(());
        }

        let mut wins = Vec::new();
        for offset in 1..PLAYER_COUNT {
            let seat = (discarder as usize + offset) % PLAYER_COUNT;
            if matches!(self.responses[seat], Some(ActionKind::Win)) {
                wins.push(seat as u8);
            }
        }
        if let Some(&winner) = wins.first() {
            self.finish_win(winner, tile, WinSource::Discard)?;
            events.push(GameEvent::Win {
                seat: winner,
                source: WinSource::Discard,
            });
            return Ok(());
        }

        let mut konggers: Vec<u8> = (0..PLAYER_COUNT)
            .filter(|&seat| {
                matches!(
                    self.responses[seat],
                    Some(ActionKind::Kong {
                        kind: KongKind::Discard,
                        ..
                    })
                )
            })
            .map(|seat| seat as u8)
            .collect();
        konggers.sort_unstable();
        if let Some(&winner) = konggers.first() {
            self.apply_kong(winner, tile, KongKind::Discard)?;
            events.push(GameEvent::Claim {
                seat: winner,
                kind: "kong".to_string(),
                tile,
            });
            return Ok(());
        }

        let mut pongers: Vec<u8> = (0..PLAYER_COUNT)
            .filter(|&seat| matches!(self.responses[seat], Some(ActionKind::Pong { .. })))
            .map(|seat| seat as u8)
            .collect();
        pongers.sort_unstable();
        if let Some(&winner) = pongers.first() {
            self.apply_pong(winner, tile)?;
            events.push(GameEvent::Claim {
                seat: winner,
                kind: "pong".to_string(),
                tile,
            });
            return Ok(());
        }

        let chow_seat = next_seat(discarder);
        if let Some(ActionKind::Chow { tiles }) = &self.responses[chow_seat as usize] {
            self.apply_chow(chow_seat, tile, *tiles)?;
            events.push(GameEvent::Claim {
                seat: chow_seat,
                kind: "chow".to_string(),
                tile,
            });
            return Ok(());
        }

        let next = next_seat(discarder);
        self.current_seat = Some(next);
        self.card_owner = Some(discarder);
        self.phase = Phase::NeedDraw { seat: next };
        self.drawn_for_turn = false;
        self.pending_replacement = false;
        self.responses = std::array::from_fn(|_| None);
        self.status = format!("{} 等待摸牌", self.players[next as usize].name);
        Ok(())
    }

    fn apply_kong(&mut self, seat: u8, tile: Tile, kind: KongKind) -> Result<(), EngineError> {
        let index = seat as usize;
        match kind {
            KongKind::Discard => {
                for _ in 0..3 {
                    remove_one(&mut self.players[index].hand, tile)?;
                }
                self.mark_last_discard_claimed();
                self.players[index]
                    .melds
                    .push(Meld::kong(MeldKind::DiscardKong, tile, Some(tile)));
                self.current_seat = Some(seat);
                self.phase = Phase::NeedDraw { seat };
            }
            KongKind::Concealed => {
                for _ in 0..4 {
                    remove_one(&mut self.players[index].hand, tile)?;
                }
                self.players[index]
                    .melds
                    .push(Meld::kong(MeldKind::ConcealedKong, tile, None));
                self.current_seat = Some(seat);
                self.phase = Phase::NeedDraw { seat };
            }
            KongKind::Added => {
                remove_one(&mut self.players[index].hand, tile)?;
                let meld = self.players[index]
                    .melds
                    .iter_mut()
                    .find(|meld| meld.kind == MeldKind::Pong && meld.tiles.first() == Some(&tile))
                    .ok_or(EngineError::IllegalAction)?;
                meld.kind = MeldKind::AddedKong;
                meld.tiles = vec![tile; 4];
                self.current_seat = Some(seat);
                self.phase = Phase::NeedDraw { seat };
            }
        }
        self.pending_replacement = true;
        self.drawn_for_turn = false;
        self.responses = std::array::from_fn(|_| None);
        self.status = format!("{} 開槓，等待補牌", self.players[index].name);
        Ok(())
    }

    fn apply_pong(&mut self, seat: u8, tile: Tile) -> Result<(), EngineError> {
        let index = seat as usize;
        remove_one(&mut self.players[index].hand, tile)?;
        remove_one(&mut self.players[index].hand, tile)?;
        self.mark_last_discard_claimed();
        self.players[index].melds.push(Meld::pong(tile, Some(tile)));
        self.current_seat = Some(seat);
        self.phase = Phase::NeedDiscard { seat };
        self.drawn_for_turn = false;
        self.responses = std::array::from_fn(|_| None);
        self.status = format!("{} 碰牌後請出牌", self.players[index].name);
        Ok(())
    }

    fn apply_chow(&mut self, seat: u8, tile: Tile, chosen: [Tile; 2]) -> Result<(), EngineError> {
        let index = seat as usize;
        remove_one(&mut self.players[index].hand, chosen[0])?;
        remove_one(&mut self.players[index].hand, chosen[1])?;
        let mut tiles = [tile, chosen[0], chosen[1]];
        tiles.sort_unstable();
        self.mark_last_discard_claimed();
        self.players[index]
            .melds
            .push(Meld::chow(tiles, Some(tile)));
        self.current_seat = Some(seat);
        self.phase = Phase::NeedDiscard { seat };
        self.drawn_for_turn = false;
        self.responses = std::array::from_fn(|_| None);
        self.status = format!("{} 吃牌後請出牌", self.players[index].name);
        Ok(())
    }

    fn mark_last_discard_claimed(&mut self) {
        if let Some(seat) = self.card_owner {
            if let Some(discard) = self.players[seat as usize].discards.last_mut() {
                discard.claimed = true;
            }
        }
    }

    fn finish_win(&mut self, winner: u8, tile: Tile, source: WinSource) -> Result<(), EngineError> {
        let index = winner as usize;
        let card_owner = self.card_owner.unwrap_or(winner);
        let input = ScoreInput {
            concealed: self.players[index].hand.clone(),
            winning_tile: tile,
            winning_in_hand: source != WinSource::Discard,
            exposed: self.players[index].melds.clone(),
            flowers: self.players[index].flowers.clone(),
            winner,
            card_owner,
            dealer: self.dealer,
            round_wind: self.round_wind,
            door_wind: self.players[index].door_wind,
            consecutive_dealer: self.consecutive_dealer,
            wall_remaining: self.wall_remaining(),
            first_round: self.players[index].first_round,
            source,
        };
        let outcome = score_hand(&input).ok_or(EngineError::IllegalAction)?;
        let decomposition = WinningDecomposition {
            pair: outcome.pair,
            sets: outcome.sets.clone(),
            exposed: input.exposed.clone(),
        };
        if source == WinSource::Discard {
            self.mark_last_discard_claimed();
            self.players[index].hand.push(tile);
            self.players[index].hand.sort_unstable();
        }
        let changes = settlement(
            winner,
            card_owner,
            source,
            outcome.total_tai,
            self.dealer,
            self.consecutive_dealer,
        );
        for (seat, change) in changes.iter().enumerate() {
            self.players[seat].score += change;
        }
        let dealer_before = self.dealer;
        let consecutive_dealer_before = self.consecutive_dealer;
        let dealer_surcharge_tai =
            if winner != dealer_before && (source.self_draw() || card_owner == dealer_before) {
                1 + 2 * consecutive_dealer_before as i32
            } else {
                0
            };
        let dealer_continued = winner == dealer_before;
        if winner == self.dealer {
            self.consecutive_dealer = self.consecutive_dealer.saturating_add(1);
        } else {
            self.dealer = next_seat(self.dealer);
            self.consecutive_dealer = 0;
            if self.dealer == 0 {
                self.round_wind = if self.round_wind == 4 {
                    1
                } else {
                    self.round_wind + 1
                };
            }
        }
        let revealed_hands = self
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect();
        self.result = Some(RoundResult {
            winner: Some(winner),
            source: Some(source),
            winning_tile: Some(tile),
            decomposition: Some(decomposition),
            payment_source: (source == WinSource::Discard).then_some(card_owner),
            total_tai: outcome.total_tai,
            tai: outcome.tai,
            base_value: DEFAULT_BASE,
            tai_value: DEFAULT_TAI,
            changes,
            dealer_before,
            consecutive_dealer_before,
            dealer_continued,
            dealer_surcharge_tai,
            dealer_after: self.dealer,
            round_wind_after: self.round_wind,
            consecutive_dealer_after: self.consecutive_dealer,
            revealed_hands,
            draw: false,
        });
        self.phase = Phase::Result;
        self.current_seat = None;
        self.status = format!(
            "{} 胡牌，{} 台",
            self.players[index].name, outcome.total_tai
        );
        Ok(())
    }

    fn finish_draw(&mut self) {
        let consecutive_dealer_before = self.consecutive_dealer;
        self.consecutive_dealer = self.consecutive_dealer.saturating_add(1);
        let revealed_hands = self
            .players
            .iter()
            .map(|player| player.hand.clone())
            .collect();
        self.result = Some(RoundResult {
            winner: None,
            source: None,
            winning_tile: None,
            decomposition: None,
            payment_source: None,
            total_tai: 0,
            tai: Vec::new(),
            base_value: DEFAULT_BASE,
            tai_value: DEFAULT_TAI,
            changes: [0; PLAYER_COUNT],
            dealer_before: self.dealer,
            consecutive_dealer_before,
            dealer_continued: true,
            dealer_surcharge_tai: 0,
            dealer_after: self.dealer,
            round_wind_after: self.round_wind,
            consecutive_dealer_after: self.consecutive_dealer,
            revealed_hands,
            draw: true,
        });
        self.phase = Phase::Result;
        self.current_seat = None;
        self.status = "牌山保留十六張，流局".to_string();
    }

    fn auto_pass_human_if_unable(&mut self) -> Result<(), EngineError> {
        let Phase::Claim { .. } = self.phase else {
            return Ok(());
        };
        if self.responses[0].is_none() && self.legal_action_kinds(0).iter().all(ActionKind::is_pass)
        {
            self.responses[0] = Some(ActionKind::Pass);
            let mut ignored = Vec::new();
            self.resolve_claims(&mut ignored)?;
            if matches!(self.phase, Phase::Result) {
                return Ok(());
            }
        }
        Ok(())
    }

    fn bot_seat_to_move(&self) -> Option<u8> {
        match self.phase {
            Phase::NeedDraw { seat } | Phase::NeedDiscard { seat } => {
                (seat != HUMAN_SEAT).then_some(seat)
            }
            Phase::Claim { discarder, .. } => (1..PLAYER_COUNT)
                .map(|offset| (discarder as usize + offset) % PLAYER_COUNT)
                .find(|&seat| seat != HUMAN_SEAT as usize && self.responses[seat].is_none())
                .map(|seat| seat as u8),
            _ => None,
        }
    }

    fn human_needs_action(&self) -> bool {
        match self.phase {
            Phase::NeedDraw { seat } | Phase::NeedDiscard { seat } => seat == HUMAN_SEAT,
            Phase::Claim { .. } => {
                self.responses[0].is_none()
                    && self
                        .legal_action_kinds(0)
                        .iter()
                        .any(|kind| !kind.is_pass())
            }
            _ => false,
        }
    }
}

fn remove_one(hand: &mut Vec<Tile>, tile: Tile) -> Result<(), EngineError> {
    let index = hand
        .iter()
        .position(|&candidate| candidate == tile)
        .ok_or(EngineError::IllegalAction)?;
    hand.remove(index);
    Ok(())
}

fn unique_tiles(hand: &[Tile]) -> Vec<Tile> {
    let mut result = hand.to_vec();
    result.sort_unstable();
    result.dedup();
    result
}

fn next_seat(seat: u8) -> u8 {
    (seat + 1) % PLAYER_COUNT as u8
}

#[cfg(not(target_arch = "wasm32"))]
fn phase_seat(phase: &Phase) -> Option<u8> {
    match phase {
        Phase::NeedDraw { seat } | Phase::NeedDiscard { seat } => Some(*seat),
        Phase::Claim { discarder, .. } => Some(*discarder),
        _ => None,
    }
}

fn full_deck() -> Vec<Tile> {
    let mut deck = Vec::with_capacity(144);
    for tile in [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 21, 22, 23, 24, 25, 26, 27,
        28, 29, 31, 32, 33, 34, 41, 42, 43,
    ] {
        deck.extend([tile; 4]);
    }
    deck.extend(51..=58);
    deck
}

fn tile_counts(tiles: &[Tile]) -> [u8; 59] {
    let mut counts = [0u8; 59];
    for &tile in tiles {
        if let Some(count) = counts.get_mut(tile as usize) {
            *count = (*count).saturating_add(1);
        }
    }
    counts
}

fn mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn next_u64(state: &mut u64) -> u64 {
    let mut value = *state;
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    *state = value;
    value
}

fn shuffle(values: &mut [Tile], mut state: u64) {
    if state == 0 {
        state = 1;
    }
    for index in (1..values.len()).rev() {
        let swap = (next_u64(&mut state) % (index as u64 + 1)) as usize;
        values.swap(index, swap);
    }
}

fn settlement(
    winner: u8,
    card_owner: u8,
    source: WinSource,
    tai: i32,
    dealer: u8,
    consecutive_dealer: u32,
) -> [i64; PLAYER_COUNT] {
    let mut changes = [0; PLAYER_COUNT];
    let payer_amount = |payer: u8| {
        let surcharge = if payer == dealer {
            1 + 2 * consecutive_dealer as i32
        } else {
            0
        };
        DEFAULT_BASE + (tai + surcharge) as i64 * DEFAULT_TAI
    };
    if source.self_draw() {
        for payer in 0..PLAYER_COUNT as u8 {
            if payer != winner {
                let amount = payer_amount(payer);
                changes[payer as usize] -= amount;
                changes[winner as usize] += amount;
            }
        }
    } else {
        let amount = payer_amount(card_owner);
        changes[card_owner as usize] -= amount;
        changes[winner as usize] += amount;
    }
    changes
}

fn tile_label(tile: Tile) -> String {
    match tile {
        1..=9 => format!("{}萬", tile),
        11..=19 => format!("{}索", tile - 10),
        21..=29 => format!("{}筒", tile - 20),
        31 => "東".to_string(),
        32 => "南".to_string(),
        33 => "西".to_string(),
        34 => "北".to_string(),
        41 => "中".to_string(),
        42 => "白".to_string(),
        43 => "發".to_string(),
        51..=58 => format!("花{}", tile - 50),
        _ => tile.to_string(),
    }
}
