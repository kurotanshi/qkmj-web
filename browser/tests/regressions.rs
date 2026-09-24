use qkmj_browser::{
    ai, score_hand, Action, ActionKind, Game, GameEvent, Meld, Phase, ScoreInput, WinSource,
};

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

fn ids(score: &qkmj_browser::ScoreOutcome) -> Vec<u8> {
    score.tai.iter().map(|tai| tai.id).collect()
}

fn value(score: &qkmj_browser::ScoreOutcome, id: u8) -> Option<i32> {
    score
        .tai
        .iter()
        .find(|tai| tai.id == id)
        .map(|tai| tai.value)
}

#[test]
fn all_claimed_requires_five_exposed_melds_and_one_tile() {
    let mut positive = input(vec![31], 31, false, WinSource::Discard);
    positive.exposed = vec![
        Meld::chow([1, 2, 3], Some(1)),
        Meld::chow([11, 12, 13], Some(11)),
        Meld::chow([21, 22, 23], Some(21)),
        Meld::pong(34, Some(34)),
        Meld::pong(41, Some(41)),
    ];
    assert!(ids(&score_hand(&positive).unwrap()).contains(&23));

    let mut negative = input(vec![1, 2, 3, 31], 31, false, WinSource::Discard);
    negative.exposed = vec![
        Meld::chow([1, 2, 3], Some(1)),
        Meld::chow([11, 12, 13], Some(11)),
        Meld::chow([21, 22, 23], Some(21)),
        Meld::pong(41, Some(41)),
    ];
    assert!(!ids(&score_hand(&negative).unwrap()).contains(&23));
}

#[test]
fn pinfu_rejects_closed_or_edge_waits() {
    let closed_wait = input(
        vec![1, 3, 11, 12, 13, 21, 22, 23, 24, 25, 26, 27, 28, 29, 5, 5],
        2,
        false,
        WinSource::Discard,
    );
    assert!(!ids(&score_hand(&closed_wait).unwrap()).contains(&24));

    let edge_wait = input(
        vec![1, 2, 11, 12, 13, 21, 22, 23, 24, 25, 26, 27, 28, 29, 5, 5],
        3,
        false,
        WinSource::Discard,
    );
    assert!(!ids(&score_hand(&edge_wait).unwrap()).contains(&24));

    let open_wait = input(
        vec![1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 6],
        1,
        false,
        WinSource::Discard,
    );
    assert!(ids(&score_hand(&open_wait).unwrap()).contains(&24));
}

#[test]
fn outside_tai_requires_an_outside_pair_and_concealed_suited_group() {
    let mixed = input(
        vec![1, 2, 3, 7, 8, 9, 11, 12, 13, 17, 18, 19, 31, 31, 31, 25],
        25,
        false,
        WinSource::NormalDraw,
    );
    assert!(!ids(&score_hand(&mixed).unwrap()).contains(&25));

    let pure = input(
        vec![1, 2, 3, 7, 8, 9, 11, 12, 13, 17, 18, 19, 21, 22, 23, 5],
        5,
        false,
        WinSource::NormalDraw,
    );
    assert!(!ids(&score_hand(&pure).unwrap()).contains(&35));
}

#[test]
fn five_concealed_triplets_clear_all_triplets() {
    let score = input(
        vec![2, 2, 2, 5, 5, 5, 12, 12, 12, 16, 16, 16, 25, 25, 25, 31, 31],
        31,
        true,
        WinSource::NormalDraw,
    );
    let ids = ids(&score_hand(&score).unwrap());
    assert!(ids.contains(&45));
    assert!(!ids.contains(&33));
}

#[test]
fn apply_json_is_human_only_and_atomic() {
    let game = Game::fixture(
        [vec![], vec![1, 2, 3], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![2; 80],
        Phase::NeedDiscard { seat: 1 },
        0,
        Some(1),
        Some(3),
        Some(WinSource::NormalDraw),
        true,
    );
    let mut game = game;
    let before = game.clone();
    let json = serde_json::to_string(&Action {
        revision: game.revision(),
        actor: 1,
        kind: ActionKind::Discard { tile: 1 },
    })
    .unwrap();
    assert!(game.apply_json(&json).is_err());
    assert_eq!(game, before);

    let mut boundary = Game::fixture(
        [vec![1], vec![], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![2; 80],
        Phase::NeedDiscard { seat: 0 },
        0,
        Some(0),
        None,
        None,
        false,
    );
    let before = boundary.clone();
    let stale = serde_json::to_string(&Action {
        revision: boundary.revision() + 1,
        actor: 0,
        kind: ActionKind::Discard { tile: 1 },
    })
    .unwrap();
    assert!(boundary.apply_json(&stale).is_err());
    assert_eq!(boundary, before);
    assert!(boundary.apply_json("not json").is_err());
    assert_eq!(boundary, before);
    assert!(boundary.apply_json(&"x".repeat(4097)).is_err());
    assert_eq!(boundary, before);
}

#[test]
fn tile_conservation_checks_the_tile_multiset() {
    let game = Game::fixture(
        [vec![1; 16], vec![1; 16], vec![1; 16], vec![1; 16]],
        std::array::from_fn(|_| Vec::new()),
        vec![0; 80],
        Phase::NeedDraw { seat: 0 },
        0,
        Some(0),
        None,
        None,
        false,
    );
    assert!(!game.tile_conservation());
}

#[test]
fn bot_step_pauses_for_a_human_claim() {
    let mut game = Game::fixture(
        [vec![5, 5], vec![], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![2; 80],
        Phase::Claim {
            discarder: 1,
            tile: 5,
        },
        0,
        Some(1),
        None,
        None,
        false,
    );
    let before = game.clone();
    game.bot_step().unwrap();
    assert_eq!(game, before);
}

#[test]
fn dealer_surcharge_metadata_matches_the_actual_payer() {
    let winning_hand = vec![1, 2, 3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 21];
    let mut nondealer = Game::fixture(
        [vec![], winning_hand.clone(), vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::NeedDiscard { seat: 1 },
        0,
        Some(1),
        Some(21),
        Some(WinSource::NormalDraw),
        true,
    );
    nondealer
        .apply(Action {
            revision: nondealer.revision(),
            actor: 1,
            kind: ActionKind::Win,
        })
        .unwrap();
    let result = nondealer.result().unwrap();
    assert_eq!(result.dealer_surcharge_tai, 1);
    assert_eq!(result.changes.iter().sum::<i64>(), 0);

    let mut dealer = Game::fixture(
        [winning_hand, vec![], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::NeedDiscard { seat: 0 },
        0,
        Some(0),
        Some(21),
        Some(WinSource::NormalDraw),
        true,
    );
    dealer
        .apply(Action {
            revision: dealer.revision(),
            actor: 0,
            kind: ActionKind::Win,
        })
        .unwrap();
    assert_eq!(dealer.result().unwrap().dealer_surcharge_tai, 0);
}

#[test]
fn standard_shanten_targets_five_melds() {
    let complete = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 21, 22, 23, 31, 31];
    assert_eq!(ai::standard_shanten(&complete, 0), -1);
    assert_eq!(ai::standard_shanten(&complete[..complete.len() - 1], 0), 0);
    assert_eq!(ai::standard_shanten(&[31, 31], 5), -1);
    assert_eq!(ai::standard_shanten(&[31], 5), 0);
}

#[test]
fn standard_shanten_counts_five_non_overlapping_partial_groups() {
    let hand = [1, 2, 4, 5, 11, 12, 14, 15, 21, 22, 31, 31, 32, 33, 34, 41];
    assert_eq!(ai::standard_shanten(&hand, 0), 4);
    for fixed_melds in 0..=5 {
        assert_eq!(
            ai::standard_shanten(&[31, 31], fixed_melds),
            9 - 2 * fixed_melds as i32
        );
    }
}

#[test]
fn discard_status_uses_the_legacy_suit_labels() {
    let wall = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13, 14, 15, 16, 17, 18, 19, 22, 23, 24,
    ]
    .into_iter()
    .flat_map(|tile| [tile; 4])
    .collect::<Vec<_>>();
    let mut bamboo = Game::fixture(
        [vec![11], vec![], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        wall.clone(),
        Phase::NeedDiscard { seat: 0 },
        0,
        Some(0),
        None,
        None,
        true,
    );
    let events = bamboo
        .apply(Action {
            revision: bamboo.revision(),
            actor: 0,
            kind: ActionKind::Discard { tile: 11 },
        })
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [GameEvent::Discard { seat: 0, tile: 11 }]
    ));
    assert!(bamboo.public_state().status.contains("1索"));

    let mut dots = Game::fixture(
        [vec![21], vec![], vec![], vec![]],
        std::array::from_fn(|_| Vec::new()),
        wall,
        Phase::NeedDiscard { seat: 0 },
        0,
        Some(0),
        None,
        None,
        true,
    );
    let events = dots
        .apply(Action {
            revision: dots.revision(),
            actor: 0,
            kind: ActionKind::Discard { tile: 21 },
        })
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [GameEvent::Discard { seat: 0, tile: 21 }]
    ));
    assert!(dots.public_state().status.contains("1筒"));
}

#[test]
fn medium_claims_only_when_the_claim_improves_readiness() {
    let game = Game::fixture(
        [
            vec![],
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13, 31, 33, 41, 41],
            vec![],
            vec![],
        ],
        std::array::from_fn(|_| Vec::new()),
        vec![2; 79],
        Phase::Claim {
            discarder: 0,
            tile: 41,
        },
        0,
        Some(0),
        None,
        None,
        false,
    );
    let observation = game.observation(1);
    let action = ai::choose_action(
        &observation,
        &observation.legal_actions,
        qkmj_browser::Difficulty::Medium,
        99,
    );
    assert!(matches!(action, ActionKind::Pong { tile: 41 }));
}

#[test]
fn door_winds_are_a_rotation() {
    let game = Game::new(1);
    assert_eq!(
        game.private_state(0).unwrap().hand,
        vec![1, 2, 5, 7, 12, 13, 15, 17, 24, 24, 25, 27, 27, 31, 33, 34, 41]
    );
    assert_eq!(
        game.public_state()
            .players
            .iter()
            .map(|player| player.door_wind)
            .collect::<Vec<_>>(),
        vec![3, 4, 1, 2]
    );
    let doors: Vec<u8> = game
        .public_state()
        .players
        .iter()
        .map(|player| player.door_wind)
        .collect();
    assert!((0..4).any(|start| {
        doors
            == (0..4)
                .map(|offset| ((start + offset) % 4 + 1) as u8)
                .collect::<Vec<_>>()
    }));
}

#[test]
fn opening_flower_fixture_uses_sorted_original_packet() {
    let game = Game::new(2);
    assert_eq!(
        game.private_state(0).unwrap().hand,
        vec![1, 3, 6, 6, 7, 11, 13, 15, 17, 22, 27, 31, 32, 32, 32, 34, 41]
    );
    assert_eq!(game.public_state().players[0].flowers, vec![54]);
    assert!(game.tile_conservation());
}

#[test]
fn active_tai_values_match_the_legacy_profile() {
    assert_eq!(
        qkmj_browser::TAI_VALUES,
        [
            1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 2, 3, 4, 4, 4, 4, 4, 6, 6, 8, 8, 8, 8, 0, 8, 8, 16, 0, 16, 16, 16, 2,
        ]
    );

    let mut all_claimed = input(vec![31], 31, false, WinSource::Discard);
    all_claimed.exposed = vec![
        Meld::chow([1, 2, 3], Some(1)),
        Meld::chow([11, 12, 13], Some(11)),
        Meld::chow([21, 22, 23], Some(21)),
        Meld::pong(34, Some(34)),
        Meld::pong(41, Some(41)),
    ];
    let all_claimed = score_hand(&all_claimed).unwrap();
    assert_eq!(value(&all_claimed, 23), Some(2));

    let pinfu = score_hand(&input(
        vec![1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 6],
        1,
        false,
        WinSource::Discard,
    ))
    .unwrap();
    assert_eq!(value(&pinfu, 24), Some(2));

    let outside = score_hand(&input(
        vec![1, 2, 3, 7, 8, 9, 11, 12, 13, 17, 18, 19, 31, 31, 31, 21],
        21,
        false,
        WinSource::NormalDraw,
    ))
    .unwrap();
    assert_eq!(value(&outside, 25), Some(2));

    let pure_outside = score_hand(&input(
        vec![1, 2, 3, 7, 8, 9, 11, 12, 13, 17, 18, 19, 21, 22, 23, 1],
        1,
        false,
        WinSource::NormalDraw,
    ))
    .unwrap();
    assert_eq!(value(&pure_outside, 35), Some(4));

    let mut closed_self_draw = input(
        vec![2, 3, 4, 3, 4, 5, 4, 5, 6, 12, 13, 14, 22, 23, 24, 25, 25],
        25,
        true,
        WinSource::NormalDraw,
    );
    closed_self_draw.card_owner = 0;
    assert_eq!(value(&score_hand(&closed_self_draw).unwrap(), 32), Some(3));

    let five = score_hand(&input(
        vec![2, 2, 2, 5, 5, 5, 12, 12, 12, 16, 16, 16, 25, 25, 25, 31, 31],
        31,
        true,
        WinSource::NormalDraw,
    ))
    .unwrap();
    assert_eq!(value(&five, 45), Some(8));
    assert_eq!(value(&five, 33), None);
}
