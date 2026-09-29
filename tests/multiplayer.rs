use qkmj_browser::{Action, ActionKind, Game, Phase};

fn claim_fixture() -> Game {
    Game::fixture(
        [vec![], vec![5], vec![6], vec![]],
        std::array::from_fn(|_| Vec::new()),
        vec![1; 79],
        Phase::Claim {
            discarder: 0,
            tile: 5,
        },
        0,
        Some(0),
        None,
        None,
        false,
    )
}

#[test]
fn stale_claim_replies_are_rejected_until_retried() {
    let mut game = claim_fixture();
    let revision = game.revision();

    game.apply(Action {
        revision,
        actor: 1,
        kind: ActionKind::Pass,
    })
    .unwrap();
    assert_eq!(game.revision(), revision + 1);
    let current = game.snapshot_for(2).unwrap();

    assert_eq!(
        game.apply(Action {
            revision,
            actor: 2,
            kind: ActionKind::Pass,
        })
        .unwrap_err(),
        qkmj_browser::EngineError::StaleRevision
    );
    assert_eq!(game.revision(), revision + 1);
    assert_eq!(game.snapshot_for(2).unwrap(), current);

    let retry = Action {
        revision: current.public.revision,
        actor: 2,
        kind: ActionKind::Pass,
    };
    game.apply(retry).unwrap();
    assert_eq!(game.revision(), revision + 2);

    game.apply(Action {
        revision: game.revision(),
        actor: 3,
        kind: ActionKind::Pass,
    })
    .unwrap();
    assert_eq!(game.revision(), revision + 3);
}

#[test]
fn each_seat_gets_a_private_snapshot_without_changing_json_admission() {
    let game = claim_fixture();
    for seat in 0..4 {
        let snapshot = game.snapshot_for(seat).unwrap();
        assert_eq!(snapshot.private.seat, seat);
        assert_eq!(snapshot.public.players.len(), 4);
    }
}
