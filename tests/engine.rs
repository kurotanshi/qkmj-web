use qkmj_browser::{
    ai, score_hand, Action, ActionKind, Difficulty, Game, KongKind, Meld, MeldKind, Phase,
    ScoreInput, WinSource,
};

fn draw_or_discard(game: &mut Game) {
    if game.private_state(0).unwrap().needs_human {
        let actions = game.private_state(0).unwrap().legal_actions;
        let action = actions
            .iter()
            .find(|action| !matches!(action.kind, ActionKind::Pass))
            .cloned()
            .or_else(|| actions.into_iter().next())
            .unwrap();
        game.apply(action).unwrap();
    } else {
        game.bot_step().unwrap();
    }
}

#[test]
fn seeded_opening_conserves_tiles_and_rejects_stale_or_illegal_actions() {
    let mut game = Game::new(1);
    game.set_difficulty(1, Difficulty::Weak).unwrap();
    game.start().unwrap();
    assert!(game.tile_conservation());
    let before = game.clone();
    let bad = Action {
        revision: game.revision() + 1,
        actor: 0,
        kind: ActionKind::Discard { tile: 99 },
    };
    assert!(game.apply(bad).is_err());
    assert_eq!(game, before);
    assert!(game.apply_json("{not json}").is_err());
    assert!(game.apply_json(&"x".repeat(4097)).is_err());
    assert_eq!(game, before);
}

#[test]
fn public_state_has_no_hidden_wall_or_opponent_hands() {
    let game = Game::new(7);
    let json = serde_json::to_string(&game.public_state()).unwrap();
    assert!(json.contains("wall_remaining"));
    assert!(!json.contains("\"wall\""));
    assert!(!json.contains("\"hand\""));
    assert_eq!(game.private_state(0).unwrap().seat, 0);
}

#[test]
fn configuration_and_bot_step_are_seeded() {
    let mut game = Game::new(9);
    game.set_difficulty(1, Difficulty::Strong).unwrap();
    game.set_difficulty(2, Difficulty::Weak).unwrap();
    game.set_difficulty(3, Difficulty::Medium).unwrap();
    game.start().unwrap();
    for _ in 0..8 {
        draw_or_discard(&mut game);
        assert!(game.tile_conservation());
        if game.result().is_some() {
            break;
        }
    }
}

fn natural_result_is_draw(seed: u64) -> bool {
    let mut game = Game::new(seed);
    game.start().unwrap();
    for _ in 0..900 {
        if game.result().is_some() {
            break;
        }
        draw_or_discard(&mut game);
        assert!(game.tile_conservation(), "seed {seed} lost a tile");
    }
    game.result().expect("fixed seed must finish").draw
}

#[test]
fn fixed_natural_win_and_draw_seeds() {
    assert!(!natural_result_is_draw(3));
    assert!(natural_result_is_draw(1));
}

fn input(
    concealed: Vec<u8>,
    winning_tile: u8,
    winning_in_hand: bool,
    source: WinSource,
) -> ScoreInput {
    ScoreInput {
        concealed,
        winning_tile,
        winning_in_hand,
        exposed: Vec::new(),
        flowers: Vec::new(),
        winner: 0,
        card_owner: 1,
        dealer: 0,
        round_wind: 1,
        door_wind: 1,
        consecutive_dealer: 0,
        wall_remaining: 40,
        first_round: false,
        source,
    }
}

#[test]
fn scoring_has_complete_hand_and_correct_kong_source() {
    let concealed = vec![1, 2, 3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21];
    let mut normal = input(concealed.clone(), 21, false, WinSource::Discard);
    let normal_score = score_hand(&normal).unwrap();
    assert!(normal_score.total_tai > 0);
    normal.source = WinSource::FlowerReplacement;
    let flower_score = score_hand(&normal).unwrap();
    assert!(!flower_score.tai.iter().any(|tai| tai.id == 5));
    normal.source = WinSource::KongReplacement;
    let kong_score = score_hand(&normal).unwrap();
    assert!(kong_score.tai.iter().any(|tai| tai.id == 5));
}

#[test]
fn exposed_melds_use_actual_tiles() {
    let concealed = vec![31, 31, 31, 32, 32, 32, 33, 33, 33, 34];
    let input = ScoreInput {
        concealed,
        winning_tile: 34,
        winning_in_hand: false,
        exposed: vec![
            Meld::kong(MeldKind::DiscardKong, 41, Some(41)),
            Meld::pong(42, Some(42)),
        ],
        flowers: Vec::new(),
        winner: 0,
        card_owner: 1,
        dealer: 0,
        round_wind: 1,
        door_wind: 1,
        consecutive_dealer: 0,
        wall_remaining: 40,
        first_round: false,
        source: WinSource::Discard,
    };
    let score = score_hand(&input).unwrap();
    assert!(score.tai.iter().any(|tai| tai.id == 43 || tai.id == 41));
}

fn claim_fixture(hands: [Vec<u8>; 4], tile: u8) -> Game {
    Game::fixture(
        hands,
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::Claim { discarder: 0, tile },
        0,
        Some(0),
        None,
        None,
        false,
    )
}

fn pass(game: &mut Game, actor: u8) {
    game.apply(Action {
        revision: game.revision(),
        actor,
        kind: ActionKind::Pass,
    })
    .unwrap();
}

#[test]
fn all_chow_shapes_and_pong_are_legal() {
    let low = claim_fixture([vec![], vec![2, 3], vec![], vec![]], 1);
    assert!(low
        .legal_action_kinds(1)
        .contains(&ActionKind::Chow { tiles: [2, 3] }));
    let middle = claim_fixture([vec![], vec![4, 6], vec![], vec![]], 5);
    assert!(middle
        .legal_action_kinds(1)
        .contains(&ActionKind::Chow { tiles: [4, 6] }));
    let high = claim_fixture([vec![], vec![7, 8], vec![], vec![]], 9);
    assert!(high
        .legal_action_kinds(1)
        .contains(&ActionKind::Chow { tiles: [7, 8] }));
    let mut pong = claim_fixture([vec![], vec![5, 5], vec![], vec![]], 5);
    pong.apply(Action {
        revision: pong.revision(),
        actor: 1,
        kind: ActionKind::Pong { tile: 5 },
    })
    .unwrap();
    pass(&mut pong, 2);
    pass(&mut pong, 3);
    assert!(matches!(pong.phase(), Phase::NeedDiscard { seat: 1 }));
}

#[test]
fn discard_concealed_and_added_kongs_take_replacement_draws() {
    let mut discard = claim_fixture([vec![], vec![7, 7, 7], vec![], vec![]], 7);
    discard
        .apply(Action {
            revision: discard.revision(),
            actor: 1,
            kind: ActionKind::Kong {
                tile: 7,
                kind: KongKind::Discard,
            },
        })
        .unwrap();
    pass(&mut discard, 2);
    pass(&mut discard, 3);
    assert!(matches!(discard.phase(), Phase::NeedDraw { seat: 1 }));
    discard
        .apply(Action {
            revision: discard.revision(),
            actor: 1,
            kind: ActionKind::Draw,
        })
        .unwrap();
    assert!(matches!(discard.phase(), Phase::NeedDiscard { seat: 1 }));

    let mut concealed = Game::fixture(
        [
            vec![9, 9, 9, 9, 1, 2, 3, 4, 5, 6, 11, 12, 13, 21, 22, 23, 24],
            vec![],
            vec![],
            vec![],
        ],
        std::array::from_fn(|_| Vec::new()),
        vec![2; 79],
        Phase::NeedDiscard { seat: 0 },
        0,
        Some(0),
        Some(9),
        Some(WinSource::NormalDraw),
        true,
    );
    assert!(concealed.legal_action_kinds(0).contains(&ActionKind::Kong {
        tile: 9,
        kind: KongKind::Concealed,
    }));
    concealed
        .apply(Action {
            revision: concealed.revision(),
            actor: 0,
            kind: ActionKind::Kong {
                tile: 9,
                kind: KongKind::Concealed,
            },
        })
        .unwrap();
    assert!(matches!(concealed.phase(), Phase::NeedDraw { seat: 0 }));

    let mut added = Game::fixture(
        [
            vec![9, 1, 2, 3, 4, 5, 6, 11, 12, 13, 21, 22, 23, 24],
            vec![],
            vec![],
            vec![],
        ],
        [
            vec![Meld::pong(9, Some(9))],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ],
        vec![2; 79],
        Phase::NeedDiscard { seat: 0 },
        0,
        Some(0),
        Some(9),
        Some(WinSource::NormalDraw),
        true,
    );
    added
        .apply(Action {
            revision: added.revision(),
            actor: 0,
            kind: ActionKind::Kong {
                tile: 9,
                kind: KongKind::Added,
            },
        })
        .unwrap();
    assert!(matches!(added.phase(), Phase::NeedDraw { seat: 0 }));
}

#[test]
fn simultaneous_wins_use_first_clockwise_and_settle_zero_sum() {
    let winning_hand = vec![1, 2, 3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21];
    let mut game = claim_fixture([vec![], winning_hand.clone(), winning_hand, vec![]], 21);
    game.apply(Action {
        revision: game.revision(),
        actor: 2,
        kind: ActionKind::Win,
    })
    .unwrap();
    game.apply(Action {
        revision: game.revision(),
        actor: 1,
        kind: ActionKind::Win,
    })
    .unwrap();
    pass(&mut game, 3);
    let result = game.result().unwrap();
    assert_eq!(result.winner, Some(1));
    assert_eq!(result.changes.iter().sum::<i64>(), 0);
    assert_eq!(result.dealer_after, 1);
    assert_eq!(result.winning_tile, Some(21));
    assert_eq!(result.payment_source, Some(0));
    assert_eq!(result.consecutive_dealer_before, 0);
    assert!(!result.dealer_continued);
    assert_eq!(result.decomposition.as_ref().unwrap().exposed.len(), 0);
}

#[test]
fn reserve_draw_progresses_dealer_once_and_next_hand_is_available() {
    let mut game = Game::fixture(
        [vec![1; 16], vec![2; 16], vec![3; 16], vec![4; 16]],
        std::array::from_fn(|_| Vec::new()),
        vec![1; 16],
        Phase::NeedDraw { seat: 0 },
        0,
        Some(3),
        None,
        None,
        false,
    );
    game.apply(Action {
        revision: game.revision(),
        actor: 0,
        kind: ActionKind::Draw,
    })
    .unwrap();
    assert!(game.result().unwrap().draw);
    assert_eq!(game.result().unwrap().consecutive_dealer_after, 1);
    game.next_hand().unwrap();
    assert!(!matches!(game.phase(), Phase::Result));
    assert!(game.tile_conservation());
}

#[test]
fn dealer_payment_surcharge_and_wrap_are_reported() {
    let winning_hand = vec![1, 2, 3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21];
    let mut game = Game::fixture(
        [vec![], winning_hand.clone(), vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::Claim {
            discarder: 0,
            tile: 21,
        },
        0,
        Some(0),
        None,
        None,
        false,
    );
    game.apply(Action {
        revision: game.revision(),
        actor: 1,
        kind: ActionKind::Win,
    })
    .unwrap();
    pass(&mut game, 2);
    pass(&mut game, 3);
    let result = game.result().unwrap();
    let expected = -(500 + (result.total_tai as i64 + 1) * 200);
    assert_eq!(result.changes[0], expected);
    assert_eq!(result.dealer_before, 0);
    assert_eq!(result.dealer_after, 1);
    assert_eq!(result.round_wind_after, 1);
    assert_eq!(result.payment_source, Some(0));

    let mut wrapped = Game::fixture(
        [winning_hand, vec![], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::Claim {
            discarder: 3,
            tile: 21,
        },
        3,
        Some(3),
        None,
        None,
        false,
    );
    wrapped
        .apply(Action {
            revision: wrapped.revision(),
            actor: 0,
            kind: ActionKind::Win,
        })
        .unwrap();
    pass(&mut wrapped, 1);
    pass(&mut wrapped, 2);
    let result = wrapped.result().unwrap();
    assert_eq!(result.dealer_after, 0);
    assert_eq!(result.round_wind_after, 2);
    assert!(!result.dealer_continued);
    assert_eq!(result.changes.iter().sum::<i64>(), 0);
}

#[test]
fn find_policy_differences() {
    let hands = [
        vec![1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 29],
        vec![1, 2, 3, 4, 5, 7, 8, 9, 11, 11, 12, 13, 14, 15, 21, 31, 41],
        vec![1, 1, 1, 2, 3, 4, 5, 6, 11, 12, 13, 21, 22, 23, 31, 32, 43],
    ];
    let mut weak_medium = false;
    let mut medium_strong = false;
    for hand in hands {
        let game = Game::fixture(
            [vec![], hand.clone(), vec![], vec![]],
            std::array::from_fn(|_| Vec::new()),
            vec![1; 79],
            Phase::NeedDiscard { seat: 1 },
            0,
            Some(1),
            hand.last().copied(),
            Some(WinSource::NormalDraw),
            true,
        );
        let observation = game.observation(1);
        let legal = observation.legal_actions.clone();
        let weak = ai::choose_action(&observation, &legal, Difficulty::Weak, 3);
        let medium = ai::choose_action(&observation, &legal, Difficulty::Medium, 3);
        let strong = ai::choose_action(&observation, &legal, Difficulty::Strong, 3);
        println!("policy actions: {:?} {:?} {:?}", weak, medium, strong);
        weak_medium |= weak != medium;
        medium_strong |= medium != strong;
    }
    assert!(weak_medium);
    assert!(medium_strong);
}

#[test]
fn policies_ignore_hidden_wall_contents() {
    let left_hands = [
        vec![],
        vec![1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 29],
        vec![1; 16],
        vec![2; 16],
    ];
    let mut right_hands = left_hands.clone();
    right_hands[2] = vec![43; 16];
    right_hands[3] = vec![42; 16];
    let left = Game::fixture(
        left_hands,
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::NeedDiscard { seat: 1 },
        0,
        Some(1),
        Some(29),
        Some(WinSource::NormalDraw),
        true,
    );
    let right = Game::fixture(
        right_hands,
        std::array::from_fn(|_| Vec::new()),
        vec![43; 79],
        Phase::NeedDiscard { seat: 1 },
        0,
        Some(1),
        Some(29),
        Some(WinSource::NormalDraw),
        true,
    );
    for difficulty in [Difficulty::Weak, Difficulty::Medium, Difficulty::Strong] {
        let left_observation = left.observation(1);
        let right_observation = right.observation(1);
        let left_action = ai::choose_action(
            &left_observation,
            &left_observation.legal_actions,
            difficulty,
            99,
        );
        let right_action = ai::choose_action(
            &right_observation,
            &right_observation.legal_actions,
            difficulty,
            99,
        );
        assert_eq!(left_action, right_action);
    }
}

fn ids(score: &qkmj_browser::ScoreOutcome) -> Vec<u8> {
    score.tai.iter().map(|tai| tai.id).collect()
}

fn case_score(
    concealed: Vec<u8>,
    winning_tile: u8,
    source: WinSource,
    flowers: Vec<u8>,
) -> qkmj_browser::ScoreOutcome {
    let mut input = input(
        concealed,
        winning_tile,
        source != WinSource::Discard,
        source,
    );
    input.flowers = flowers;
    score_hand(&input).unwrap_or_else(|| panic!("invalid scoring fixture {:?}", input.concealed))
}

#[test]
fn legacy_tai_inventory_and_exclusions_are_regressed() {
    let double_wind = {
        let mut score_input = input(
            vec![34, 34, 34, 1, 1, 1, 2, 2, 2, 3, 3, 3, 11, 12, 13, 21, 21],
            34,
            true,
            WinSource::NormalDraw,
        );
        score_input.round_wind = 4;
        score_input.door_wind = 4;
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&double_wind).contains(&20));
    assert!(!ids(&double_wind).contains(&12));

    let two_double_sequences = case_score(
        vec![1, 2, 3, 1, 2, 3, 4, 5, 6, 4, 5, 6, 7, 8, 9, 21, 21],
        21,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&two_double_sequences).contains(&28));

    let three_concealed = case_score(
        vec![31, 31, 31, 41, 41, 41, 42, 42, 42, 1, 2, 3, 4, 5, 6, 11, 11],
        11,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&three_concealed).contains(&29));

    let four_concealed = case_score(
        vec![
            31, 31, 31, 41, 41, 41, 42, 42, 42, 43, 43, 43, 1, 2, 3, 11, 11,
        ],
        11,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&four_concealed).contains(&38));

    let five_concealed = case_score(
        vec![
            31, 31, 31, 32, 32, 32, 33, 33, 33, 34, 34, 34, 41, 41, 41, 42, 42,
        ],
        42,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&five_concealed).contains(&45));
    assert!(!ids(&five_concealed).contains(&33));

    let three_kongs = {
        let mut score_input = input(vec![1, 2, 3, 11, 12, 13, 21], 21, false, WinSource::Discard);
        score_input.exposed = vec![
            Meld::kong(MeldKind::DiscardKong, 31, Some(31)),
            Meld::kong(MeldKind::AddedKong, 32, Some(32)),
            Meld::kong(MeldKind::ConcealedKong, 33, None),
        ];
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&three_kongs).contains(&30));

    let four_kongs = {
        let mut score_input = input(vec![1, 2, 3, 4], 4, false, WinSource::Discard);
        score_input.exposed = vec![
            Meld::kong(MeldKind::DiscardKong, 31, Some(31)),
            Meld::kong(MeldKind::AddedKong, 32, Some(32)),
            Meld::kong(MeldKind::ConcealedKong, 33, None),
            Meld::kong(MeldKind::ConcealedKong, 34, None),
        ];
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&four_kongs).contains(&39));

    let three_color_triplets = case_score(
        vec![1, 1, 1, 11, 11, 11, 21, 21, 21, 2, 2, 2, 3, 3, 3, 4, 4],
        4,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&three_color_triplets).contains(&31));

    let ping_hu = case_score(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 22, 23, 24, 24],
        21,
        WinSource::Discard,
        vec![],
    );
    assert!(ids(&ping_hu).contains(&24));

    let mixed_outside = case_score(
        vec![1, 2, 3, 7, 8, 9, 1, 1, 1, 17, 18, 19, 31, 31, 31, 21, 21],
        21,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&mixed_outside).contains(&25));

    let three_color_sequences = case_score(
        vec![1, 2, 3, 11, 12, 13, 21, 22, 23, 4, 5, 6, 7, 8, 9, 24, 24],
        24,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&three_color_sequences).contains(&26));

    let one_long_straight = case_score(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 21, 21],
        21,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&one_long_straight).contains(&27));

    let mixed_one_suit = case_score(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 1, 1, 1, 31, 31, 31, 32, 32],
        32,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&mixed_one_suit).contains(&34));

    let pure_outside = case_score(
        vec![1, 2, 3, 7, 8, 9, 1, 1, 1, 17, 18, 19, 11, 12, 13, 21, 21],
        21,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&pure_outside).contains(&35));
    assert!(!ids(&pure_outside).contains(&25));

    let mixed_terminals = case_score(
        vec![1, 1, 1, 9, 9, 9, 11, 11, 11, 19, 19, 19, 31, 31, 31, 32, 32],
        32,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&mixed_terminals).contains(&36));
    assert!(!ids(&mixed_terminals).contains(&33));

    let small_dragons = case_score(
        vec![41, 41, 41, 42, 42, 42, 43, 43, 1, 1, 1, 2, 2, 2, 3, 3, 3],
        43,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&small_dragons).contains(&37));

    let big_dragons = case_score(
        vec![41, 41, 41, 42, 42, 42, 43, 43, 43, 1, 1, 1, 2, 2, 2, 3, 3],
        3,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&big_dragons).contains(&40));
    assert!(!ids(&big_dragons).contains(&37));

    let pure_suit = case_score(
        vec![1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 5, 6, 7, 8, 9, 4, 4],
        4,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&pure_suit).contains(&42));
    assert!(!ids(&pure_suit).contains(&34));

    let all_honors = case_score(
        vec![
            31, 31, 31, 32, 32, 32, 33, 33, 33, 41, 41, 41, 42, 42, 42, 43, 43,
        ],
        43,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&all_honors).contains(&43));
    assert!(!ids(&all_honors).contains(&33));

    let small_winds = case_score(
        vec![31, 31, 31, 32, 32, 32, 33, 33, 33, 1, 1, 1, 2, 2, 2, 34, 34],
        34,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&small_winds).contains(&41));

    let pure_terminals = case_score(
        vec![1, 1, 1, 9, 9, 9, 11, 11, 11, 19, 19, 19, 21, 21, 21, 29, 29],
        29,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&pure_terminals).contains(&46));

    let big_winds = case_score(
        vec![
            31, 31, 31, 32, 32, 32, 33, 33, 33, 34, 34, 34, 1, 1, 1, 2, 2,
        ],
        2,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&big_winds).contains(&47));
    assert!(!ids(&big_winds).contains(&41));

    let all_seasons = case_score(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24, 24],
        24,
        WinSource::NormalDraw,
        vec![51, 52, 53, 54],
    );
    assert!(ids(&all_seasons).contains(&21));
    let all_plants = case_score(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24, 24],
        24,
        WinSource::NormalDraw,
        vec![55, 56, 57, 58],
    );
    assert!(ids(&all_plants).contains(&22));
}

#[test]
fn direct_tai_flags_and_round_boundaries_are_regressed() {
    let ordinary = case_score(
        vec![2, 3, 4, 3, 4, 5, 4, 5, 6, 12, 13, 14, 22, 23, 24, 25, 25],
        25,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&ordinary).contains(&3));
    assert!(!ids(&ordinary).contains(&8));
    assert!(!ids(&ordinary).contains(&44));
    assert!(!ids(&ordinary).contains(&48));

    let one_double = case_score(
        vec![2, 3, 4, 2, 3, 4, 5, 6, 7, 12, 13, 14, 22, 23, 24, 25, 25],
        25,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&one_double).contains(&4));

    let mut sea = input(
        vec![2, 3, 4, 3, 4, 5, 4, 5, 6, 12, 13, 14, 22, 23, 24, 25, 25],
        25,
        true,
        WinSource::NormalDraw,
    );
    sea.card_owner = 0;
    sea.wall_remaining = 16;
    let sea = score_hand(&sea).unwrap();
    assert!(ids(&sea).contains(&6));

    let mut river = input(
        vec![2, 3, 4, 3, 4, 5, 4, 5, 6, 12, 13, 14, 22, 23, 24, 25],
        25,
        false,
        WinSource::Discard,
    );
    river.wall_remaining = 16;
    let river = score_hand(&river).unwrap();
    assert!(ids(&river).contains(&7));

    let mut wind = input(
        vec![31, 31, 31, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 21],
        31,
        true,
        WinSource::NormalDraw,
    );
    wind.round_wind = 1;
    wind.door_wind = 2;
    let wind = score_hand(&wind).unwrap();
    assert!(ids(&wind).contains(&9));

    let red_dragon = case_score(
        vec![41, 41, 41, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 21],
        41,
        WinSource::NormalDraw,
        vec![],
    );
    assert!(ids(&red_dragon).contains(&13));

    let flower = case_score(
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24, 24],
        24,
        WinSource::NormalDraw,
        vec![51],
    );
    assert!(ids(&flower).contains(&16));

    let all_claimed = {
        let mut score_input = input(vec![31], 31, false, WinSource::Discard);
        score_input.exposed = vec![
            Meld::chow([1, 2, 3], Some(1)),
            Meld::chow([11, 12, 13], Some(11)),
            Meld::chow([21, 22, 23], Some(21)),
            Meld::pong(31, Some(31)),
            Meld::pong(41, Some(41)),
        ];
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&all_claimed).contains(&23));

    let heavenly = {
        let mut score_input = input(
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24, 24],
            24,
            true,
            WinSource::Initial,
        );
        score_input.first_round = true;
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&heavenly).contains(&49));

    let earthly = {
        let mut score_input = input(
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24, 24],
            24,
            true,
            WinSource::NormalDraw,
        );
        score_input.winner = 1;
        score_input.dealer = 0;
        score_input.first_round = true;
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&earthly).contains(&50));

    let human = {
        let mut score_input = input(
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24],
            24,
            false,
            WinSource::Discard,
        );
        score_input.winner = 1;
        score_input.dealer = 0;
        score_input.card_owner = 0;
        score_input.first_round = true;
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&human).contains(&51));

    let consecutive = {
        let mut score_input = input(
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 24, 24],
            24,
            true,
            WinSource::NormalDraw,
        );
        score_input.consecutive_dealer = 3;
        score_hand(&score_input).unwrap()
    };
    assert!(ids(&consecutive).contains(&52));
}
