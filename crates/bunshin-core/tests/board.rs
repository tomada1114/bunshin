//! Integration tests for the deterministic board model.

use std::time::Duration;

use bunshin_core::{
    ModelAnswer, ModelError, Tuning, UnavailableReason, UnixMillis,
    board::{
        Author, Board, BoardTuning, FailureKind, Outcome, Rng, Speaker, TopicCategory, TopicDepth,
        TopicMood, Turn, TurnKind,
    },
};

fn at(milliseconds: i64) -> UnixMillis {
    UnixMillis(milliseconds)
}

fn board_with_interval(interval: Duration) -> Board {
    Board::new(BoardTuning {
        post_interval: interval,
        ..BoardTuning::default()
    })
}

fn finish_post(board: &mut Board, turn: &Turn, milliseconds: i64) -> Outcome {
    board.finish(
        turn,
        Ok(ModelAnswer {
            json: r#"{"body":"  いいですね。  "}"#.into(),
        }),
        at(milliseconds),
    )
}

fn start_turn(board: &mut Board, milliseconds: i64, rng: &mut Rng) -> Turn {
    match board.next_turn(at(milliseconds), rng) {
        Some(turn) => turn,
        None => panic!("turn is due"),
    }
}

fn post_count(outcome: &Outcome) -> usize {
    match outcome {
        Outcome::Posted(_) => 1,
        Outcome::Failed(_) | Outcome::Stale => 0,
    }
}

#[test]
fn defaults_match_the_board_prototype_values() {
    let tuning = BoardTuning::default();

    assert_eq!(tuning.post_interval, Duration::from_secs(30));
    assert_eq!(tuning.context_posts, 12);
    assert_eq!(tuning.max_posts, 200);
    assert_eq!(tuning.body_max_chars, 120);
    assert_eq!(tuning.asked_max_chars, 80);
}

#[test]
fn zero_rng_seed_maps_to_the_fixed_nonzero_seed() {
    assert_eq!(Rng::from_seed(0), Rng::from_seed(0x9e37_79b9_7f4a_7c15));
}

#[test]
fn owner_posts_are_immediate_and_whitespace_only_posts_are_ignored() {
    let mut board = Board::default();

    assert!(board.owner_post(" \n　\t", at(1)).is_none());
    assert!(board.posts().is_empty());

    let owner = board
        .owner_post("今日はいい天気", at(2))
        .expect("owner post");
    assert_eq!(owner.author, Author::Owner);
    assert_eq!(owner.body, "今日はいい天気");
    assert_eq!(owner.at, at(2));
    assert!(board.is_due(at(2)));
}

#[test]
fn first_turn_starts_immediately_and_an_empty_board_falls_back_to_a_new_topic() {
    let mut board = Board::default();
    let mut rng = Rng::from_seed(12);

    let turn = start_turn(&mut board, 0, &mut rng);
    assert!(matches!(turn.kind, TurnKind::NewTopic { .. }));
    assert!(!board.is_due(at(0)));
    assert!(board.next_turn(at(0), &mut rng).is_none());
}

#[test]
fn only_one_model_turn_can_be_in_flight() {
    let mut board = Board::default();
    let mut rng = Rng::from_seed(17);

    let _turn = start_turn(&mut board, 0, &mut rng);

    assert!(board.next_turn(at(30_000), &mut rng).is_none());
    assert!(!board.is_due(at(30_000)));
}

#[test]
fn request_uses_the_speaker_rules_schema_and_latest_context() {
    let mut board = Board::default();
    let owner = board
        .owner_post("週末に旅行したい", at(10))
        .expect("owner post");
    let mut rng = Rng::from_seed(20);
    let turn = start_turn(&mut board, 10, &mut rng);
    let request = board
        .request(&turn, Tuning::default())
        .expect("active request");

    assert!(request.instructions.contains(turn.speaker.name()));
    assert!(request.instructions.contains("80文字"));
    assert!(request.instructions.contains("名前を付けず"));
    assert!(request.prompt.contains(&format!("あなた: {}", owner.body)));
    assert!(request.prompt.contains("あなたの投稿に返信"));
    assert_eq!(request.timeout, Duration::from_secs(30));
    assert_eq!(
        request.schema,
        r#"{"type":"object","properties":{"body":{"type":"string"}},"required":["body"],"additionalProperties":false,"title":"Post","x-order":["body"]}"#
    );
}

#[test]
fn owner_post_gets_two_responses_about_the_same_post_by_different_characters() {
    let mut board = board_with_interval(Duration::from_secs(30));
    let owner = board
        .owner_post("札幌に行きたい", at(100))
        .expect("owner post");
    let mut rng = Rng::from_seed(21);

    let first = start_turn(&mut board, 100, &mut rng);
    assert_eq!(first.kind, TurnKind::OwnerReply { target: owner.id });
    assert_eq!(post_count(&finish_post(&mut board, &first, 200)), 1);
    assert!(!board.is_due(at(30_199)));

    let second = start_turn(&mut board, 30_200, &mut rng);
    assert_eq!(second.kind, TurnKind::OwnerReaction { target: owner.id });
    assert_ne!(second.speaker, first.speaker);
    assert_eq!(post_count(&finish_post(&mut board, &second, 30_300)), 1);

    let next = start_turn(&mut board, 60_300, &mut rng);
    assert!(!matches!(
        next.kind,
        TurnKind::OwnerReply { .. } | TurnKind::OwnerReaction { .. }
    ));
}

#[test]
fn a_new_owner_post_can_be_answered_by_the_previous_character_again() {
    let mut reused_previous_speaker = false;

    for seed in 1..=100 {
        let mut board = Board::default();
        let mut rng = Rng::from_seed(seed);
        board.owner_post("最初の投稿", at(0)).expect("owner post");

        let first = start_turn(&mut board, 0, &mut rng);
        finish_post(&mut board, &first, 1);
        let second = start_turn(&mut board, 30_001, &mut rng);
        finish_post(&mut board, &second, 30_002);

        board
            .owner_post("もう一度投稿", at(30_003))
            .expect("owner post");
        let reply = start_turn(&mut board, 30_003, &mut rng);
        assert_eq!(
            reply.kind,
            TurnKind::OwnerReply {
                target: board.posts()[3].id
            }
        );
        if reply.speaker == second.speaker {
            reused_previous_speaker = true;
            break;
        }
    }

    assert!(reused_previous_speaker);
}

#[test]
fn a_new_owner_post_resets_an_in_flight_turn_and_discards_its_result() {
    let mut board = Board::default();
    let old_owner = board
        .owner_post("古い話題", at(1))
        .expect("first owner post");
    let mut rng = Rng::from_seed(33);
    let old_turn = start_turn(&mut board, 1, &mut rng);
    assert_eq!(
        old_turn.kind,
        TurnKind::OwnerReply {
            target: old_owner.id
        }
    );

    let newest_owner = board
        .owner_post("新しい話題", at(2))
        .expect("second owner post");
    assert_eq!(post_count(&finish_post(&mut board, &old_turn, 3)), 0);
    assert_eq!(board.posts().len(), 2);
    assert_eq!(board.posts()[1].id, newest_owner.id);

    let new_turn = start_turn(&mut board, 3, &mut rng);
    assert_eq!(
        new_turn.kind,
        TurnKind::OwnerReply {
            target: newest_owner.id
        }
    );
}

#[test]
fn failed_attempts_add_nothing_and_restart_the_interval_at_finish_time() {
    let mut board = board_with_interval(Duration::from_secs(30));
    let mut rng = Rng::from_seed(44);
    let turn = start_turn(&mut board, 1_000, &mut rng);

    assert_eq!(
        board.finish(&turn, Err(ModelError::TimedOut), at(15_000)),
        Outcome::Failed(FailureKind::TimedOut)
    );
    assert!(board.posts().is_empty());
    assert!(!board.is_due(at(44_999)));
    assert!(board.is_due(at(45_000)));
}

#[test]
fn malformed_answers_unknown_fields_and_empty_bodies_fail_without_a_post() {
    let cases = [
        ("{", FailureKind::Malformed),
        (r#"{"body":"ok","name":"Haru"}"#, FailureKind::Malformed),
        (r#"{"body":"   \n"}"#, FailureKind::EmptyBody),
    ];

    for (index, (json, expected)) in cases.into_iter().enumerate() {
        let mut board = Board::default();
        let mut rng = Rng::from_seed(index as u64 + 1);
        let turn = start_turn(&mut board, 0, &mut rng);
        let outcome = board.finish(&turn, Ok(ModelAnswer { json: json.into() }), at(1));

        assert_eq!(outcome, Outcome::Failed(expected));
        assert!(board.posts().is_empty());
    }
}

#[test]
fn every_model_error_is_reduced_to_a_typed_failure_kind() {
    let failures = [
        (
            ModelError::Unavailable(UnavailableReason::NotInstalled),
            FailureKind::Unavailable,
        ),
        (ModelError::TimedOut, FailureKind::TimedOut),
        (ModelError::Cancelled, FailureKind::Cancelled),
        (ModelError::Refused, FailureKind::Refused),
        (ModelError::Malformed, FailureKind::Malformed),
        (ModelError::Failed, FailureKind::Failed),
    ];

    for (index, (error, expected)) in failures.into_iter().enumerate() {
        let mut board = Board::default();
        let mut rng = Rng::from_seed(index as u64 + 9);
        let turn = start_turn(&mut board, 0, &mut rng);

        assert_eq!(
            board.finish(&turn, Err(error), at(1)),
            Outcome::Failed(expected)
        );
    }
}

#[test]
fn normal_turns_follow_the_reply_chime_topic_weights_and_chime_prompt_context() {
    let mut rng = Rng::from_seed(0x5eed);
    let mut replies = 0;
    let mut chimes = 0;
    let mut topics = 0;
    let mut checked_chime_context = false;

    for index in 0..1_000 {
        let mut board = board_with_interval(Duration::ZERO);
        board
            .owner_post(&format!("owner-{index}"), at(0))
            .expect("owner post");
        let first = start_turn(&mut board, 0, &mut rng);
        let first_answer = ModelAnswer {
            json: format!(r#"{{"body":"first-{index}"}}"#),
        };
        assert!(matches!(
            board.finish(&first, Ok(first_answer), at(1)),
            Outcome::Posted(_)
        ));
        let second = start_turn(&mut board, 1, &mut rng);
        let second_answer = ModelAnswer {
            json: format!(r#"{{"body":"second-{index}"}}"#),
        };
        assert!(matches!(
            board.finish(&second, Ok(second_answer), at(2)),
            Outcome::Posted(_)
        ));

        let turn = start_turn(&mut board, 3, &mut rng);
        match turn.kind {
            TurnKind::Reply { .. } => replies += 1,
            TurnKind::ChimeIn { .. } => {
                chimes += 1;
                let request = board
                    .request(&turn, Tuning::default())
                    .expect("active chime request");
                assert!(request.prompt.contains(&format!("first-{index}")));
                assert!(request.prompt.contains(&format!("second-{index}")));
                checked_chime_context = true;
            }
            TurnKind::NewTopic { .. } => topics += 1,
            TurnKind::OwnerReply { .. } | TurnKind::OwnerReaction { .. } => {
                panic!("normal turn must not resume an owner cycle")
            }
        }
    }

    assert!((450..=550).contains(&replies), "reply count: {replies}");
    assert!((200..=300).contains(&chimes), "chime count: {chimes}");
    assert!((200..=300).contains(&topics), "topic count: {topics}");
    assert!(checked_chime_context);
}

#[test]
fn finished_bodies_are_trimmed_and_truncated_by_unicode_scalar_values() {
    let mut board = Board::new(BoardTuning {
        body_max_chars: 3,
        ..BoardTuning::default()
    });
    let mut rng = Rng::from_seed(50);
    let turn = start_turn(&mut board, 0, &mut rng);
    let outcome = board.finish(
        &turn,
        Ok(ModelAnswer {
            json: r#"{"body":"  あいうえ  "}"#.into(),
        }),
        at(1),
    );

    let Outcome::Posted(post) = outcome else {
        panic!("expected a post");
    };
    assert_eq!(post.body, "あいう");
    assert_eq!(post.author, Author::Character(turn.speaker));
}

#[test]
fn bounded_history_drops_the_oldest_posts() {
    let mut board = Board::new(BoardTuning {
        max_posts: 3,
        ..BoardTuning::default()
    });

    for index in 0..5 {
        board
            .owner_post(&format!("owner-{index}"), at(index))
            .expect("owner posts are accepted");
    }

    assert_eq!(board.posts().len(), 3);
    assert_eq!(board.posts()[0].body, "owner-2");
    assert_eq!(board.posts()[2].body, "owner-4");
}

#[test]
fn model_prompt_contains_only_the_latest_configured_number_of_posts() {
    let mut board = Board::new(BoardTuning {
        context_posts: 12,
        ..BoardTuning::default()
    });
    for index in 0..15 {
        board
            .owner_post(&format!("owner-{index}"), at(index))
            .expect("owner posts are accepted");
    }
    let mut rng = Rng::from_seed(77);
    let turn = start_turn(&mut board, 15, &mut rng);
    let request = board
        .request(&turn, Tuning::default())
        .expect("active request");

    assert!(!request.prompt.contains("owner-2"));
    assert!(request.prompt.contains("owner-3"));
    assert!(request.prompt.contains("owner-14"));
}

#[test]
fn character_speakers_never_repeat_in_adjacent_successful_posts() {
    let mut board = board_with_interval(Duration::ZERO);
    let mut rng = Rng::from_seed(9_381);
    let mut previous = None;

    for tick in 0..500 {
        let now = tick * 2;
        let turn = start_turn(&mut board, now, &mut rng);
        assert_ne!(Some(turn.speaker), previous);
        assert_eq!(post_count(&finish_post(&mut board, &turn, now + 1)), 1);
        previous = Some(turn.speaker);
    }
}

#[test]
fn new_topic_categories_do_not_repeat_and_uniform_dimensions_cover_the_fixed_sets() {
    let mut board = board_with_interval(Duration::ZERO);
    let mut rng = Rng::from_seed(1_337);
    let mut previous_category = None;
    let mut categories = Vec::new();
    let mut depths = Vec::new();
    let mut moods = Vec::new();

    for tick in 0..1_000 {
        let now = tick * 2;
        let turn = start_turn(&mut board, now, &mut rng);
        if let TurnKind::NewTopic { topic } = turn.kind {
            assert_ne!(Some(topic.category), previous_category);
            previous_category = Some(topic.category);
            categories.push(topic.category);
            depths.push(topic.depth);
            moods.push(topic.mood);
        }
        finish_post(&mut board, &turn, now + 1);
    }

    for expected in [
        TopicCategory::Travel,
        TopicCategory::Technology,
        TopicCategory::Economics,
        TopicCategory::Food,
        TopicCategory::Movies,
        TopicCategory::Music,
        TopicCategory::Sports,
        TopicCategory::Science,
        TopicCategory::History,
        TopicCategory::Health,
        TopicCategory::Career,
        TopicCategory::Hobbies,
    ] {
        assert!(
            categories.contains(&expected),
            "missing category: {expected:?}"
        );
    }
    assert!(
        categories.len() >= 200,
        "new-topic count: {}",
        categories.len()
    );
    for expected in [TopicDepth::Casual, TopicDepth::Deeper, TopicDepth::Expert] {
        assert!(depths.contains(&expected), "missing depth: {expected:?}");
    }
    for expected in [TopicMood::Relaxed, TopicMood::Excited, TopicMood::Debatable] {
        assert!(moods.contains(&expected), "missing mood: {expected:?}");
    }
}

#[test]
fn successful_character_turns_are_appended_with_the_selected_speaker() {
    let mut board = board_with_interval(Duration::ZERO);
    let mut rng = Rng::from_seed(101);
    let turn = start_turn(&mut board, 12, &mut rng);

    let Outcome::Posted(post) = finish_post(&mut board, &turn, 13) else {
        panic!("expected a post");
    };
    assert_eq!(post.author, Author::Character(turn.speaker));
    assert_eq!(post.body, "いいですね。");
    assert_eq!(post.at, at(13));
    assert_eq!(board.posts(), &[post]);
}

#[test]
fn speaker_names_are_japanese_and_owner_name_is_fixed() {
    assert_eq!(Speaker::Haru.name(), "ハル");
    assert_eq!(Speaker::Shizuku.name(), "シズク");
    assert_eq!(Speaker::Gen.name(), "ゲン");
    assert_eq!(Author::Owner.name(), "あなた");
}

#[test]
fn a_turn_from_a_previous_attempt_cannot_finish_the_current_attempt() {
    let mut board = board_with_interval(Duration::ZERO);
    let mut rng = Rng::from_seed(500);
    let old_turn = start_turn(&mut board, 0, &mut rng);
    assert_eq!(post_count(&finish_post(&mut board, &old_turn, 1)), 1);

    let current_turn = start_turn(&mut board, 2, &mut rng);
    assert_eq!(
        board.finish(&old_turn, Err(ModelError::Failed), at(3)),
        Outcome::Stale
    );
    assert_eq!(post_count(&finish_post(&mut board, &current_turn, 4)), 1);
}

#[test]
fn topic_values_have_stable_user_facing_labels() {
    assert_eq!(TopicCategory::Travel.label(), "旅行");
    assert_eq!(TopicCategory::Technology.label(), "IT・テクノロジー");
    assert_eq!(TopicCategory::Economics.label(), "経済・お金");
    assert_eq!(TopicCategory::Food.label(), "食べ物・料理");
    assert_eq!(TopicCategory::Movies.label(), "映画・ドラマ");
    assert_eq!(TopicCategory::Music.label(), "音楽");
    assert_eq!(TopicCategory::Sports.label(), "スポーツ");
    assert_eq!(TopicCategory::Science.label(), "科学");
    assert_eq!(TopicCategory::History.label(), "歴史");
    assert_eq!(TopicCategory::Health.label(), "健康・暮らし");
    assert_eq!(TopicCategory::Career.label(), "仕事・キャリア");
    assert_eq!(TopicCategory::Hobbies.label(), "趣味・遊び");
    assert_eq!(TopicDepth::Casual.label(), "気軽な雑談");
    assert_eq!(TopicDepth::Deeper.label(), "ちょっと掘り下げる");
    assert_eq!(TopicDepth::Expert.label(), "詳しい人向け");
    assert_eq!(TopicMood::Relaxed.label(), "のんびり");
    assert_eq!(TopicMood::Excited.label(), "盛り上がる");
    assert_eq!(TopicMood::Debatable.label(), "ちょっと意見が分かれる");
}
