//! Model use stays bounded, preserves tasks, and yields typed delivery effects.
use bunshin_core::{
    Now, Tuning, UnixMillis,
    day::{Day, TaskKind, TaskOrigin, Trigger, TriggerKind},
    instructions::InstructionsState,
    prompt::{chat::ContextExtras, checkin::build_checkin},
};

fn apply_delivery_guard(
    mut day: Day,
    now: &mut Now,
    guard: &str,
) -> Result<Day, bunshin_core::day::file::DayFileError> {
    if guard == "hours" {
        now.local = now.local.date().at(7, 0, 0, 0);
    }
    if guard == "mute" {
        day = day.mute(UnixMillis(3_600_000), now.instant).0;
    }
    if guard == "gap" {
        let mut data = day.data().clone();
        data.last_unprompted_at = Some(now.instant);
        let format = bunshin_core::day::file::FORMAT;
        day = bunshin_core::day::file::DayFile { format, data }.into_day(Tuning::default())?;
    }
    Ok(day)
}

#[test]
fn removing_the_last_queued_or_in_flight_deadline_keeps_untimed_work_on_a_planned_look() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{
            calls::{CheckinCalls, CheckinEffect},
            plan_look,
        },
    };
    for default_minutes in [120, 45] {
        let mut tuning = Tuning::default();
        tuning.checkin.planned_default_minutes = default_minutes;
        for in_flight in [false, true] {
            for deleted in [false, true] {
                for preserve in [false, true] {
                    let (day, owner, _, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
                    let day = day
                        .add(
                            "時刻なしの作業".into(),
                            TaskKind::Untimed,
                            None,
                            TaskOrigin::Key,
                            now.instant,
                        )
                        .unwrap()
                        .0;
                    let day = if preserve {
                        plan_look(day, now, Some(7), tuning)
                    } else {
                        day
                    };
                    let previous = day.data().next_planned_look;
                    let mut calls = CheckinCalls::new(tuning);
                    queue(&mut calls, &day, TriggerKind::BeforeDeadline, now.instant);
                    let start = calls.prepare(
                        day,
                        &owner,
                        ContextExtras::default(),
                        bunshin_core::checkin::calls::CallContext {
                            input_has_text: !in_flight,
                            ..context(now)
                        },
                        fixed,
                    );
                    let day = if deleted {
                        start.day.delete(1, now.instant).unwrap().0
                    } else {
                        start.day.done(1, now.instant).unwrap().0
                    };
                    let tasks = day.tasks().to_vec();
                    let update = if in_flight {
                        calls.finish(day, start.request.unwrap().id,
                            Ok(ModelAnswer { json: r#"{"kind":"note","task":1,"message":"obsolete","next_look_minutes":5}"#.into() }),
                            context(now), fixed)
                    } else {
                        calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed)
                    };
                    let expected = previous.unwrap_or_else(|| {
                        now.local
                            .checked_add(jiff::SignedDuration::from_mins(i64::from(
                                default_minutes,
                            )))
                            .unwrap()
                    });
                    assert_eq!(update.day.data().next_planned_look, Some(expected));
                    assert_eq!(update.effects, vec![CheckinEffect::Save]);
                    assert_eq!(update.day.tasks(), tasks);
                    assert!(
                        update
                            .day
                            .messages()
                            .iter()
                            .all(|row| row.unprompted.is_none())
                    );
                    assert!(update.day.data().held_triggers.is_empty());
                    assert!(update.request.is_none());
                }
            }
        }
    }
}

#[test]
fn a_stale_mixed_answer_requeues_the_surviving_look_for_the_next_actual_tick() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{
            Checkin,
            calls::{CheckinCalls, CheckinEffect},
            plan_look,
        },
    };
    let (day, owner, _, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    let day = plan_look(day, now, Some(5), Tuning::default());
    now.instant = UnixMillis(240_000);
    now.local = now.local.date().at(14, 34, 0, 0);
    let timer = Checkin::new(now, Tuning::default());
    now.instant = UnixMillis(300_000);
    now.local = now.local.date().at(14, 35, 0, 0);
    let scheduled = timer.tick(day, now, false);
    assert_eq!(scheduled.day.data().next_planned_look, None);
    let batch = scheduled.ready.unwrap();
    let surviving = batch
        .triggers
        .iter()
        .filter(|event| event.kind == TriggerKind::PlannedLook)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(batch.triggers.len(), 2);
    assert_eq!(surviving.len(), 1);
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(&scheduled.day, batch);
    let start = calls.prepare(
        scheduled.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let day = start.day.done(1, now.instant).unwrap().0;
    let tasks = day.tasks().to_vec();
    let held = calls.finish(
        day,
        start.request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"note","task":1,"message":"obsolete","next_look_minutes":5}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert_eq!(held.day.data().held_triggers, surviving);
    assert_eq!(held.day.data().next_planned_look, None);
    assert_eq!(held.effects, vec![CheckinEffect::Save]);
    assert!(
        held.day
            .messages()
            .iter()
            .all(|row| row.unprompted.is_none())
    );
    assert_eq!(held.day.tasks(), tasks);
    let between = calls.prepare(
        held.day,
        &owner,
        ContextExtras::default(),
        bunshin_core::checkin::calls::CallContext {
            is_tick: false,
            ..context(now)
        },
        fixed,
    );
    assert!(between.request.is_none());
    now.instant = UnixMillis(360_000);
    now.local = now.local.date().at(14, 36, 0, 0);
    let retry = calls.prepare(
        between.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let delivered = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"note","message":"有効な見回りの回答","next_look_minutes":5}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert_eq!(
        delivered.effects,
        vec![CheckinEffect::Bell, CheckinEffect::Save]
    );
    assert_eq!(
        delivered.day.messages().last().unwrap().text,
        "有効な見回りの回答"
    );
    assert_eq!(delivered.day.tasks(), tasks);
    assert!(delivered.day.data().held_triggers.is_empty());
    assert_eq!(
        delivered.day.data().next_planned_look,
        Some(now.local.date().at(14, 41, 0, 0))
    );
}

#[test]
fn planned_responses_accept_unchanged_closed_tasks_and_reject_reopened_tasks() {
    use bunshin_core::{ModelAnswer, checkin::calls::CheckinEffect};
    for status in ["done", "dropped"] {
        for kind in ["note", "question", "silent"] {
            for reopened in [false, true] {
                let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
                let day = if status == "done" {
                    day.done(1, now.instant).unwrap().0
                } else {
                    day.drop(1, now.instant).unwrap().0
                };
                let start =
                    calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
                let day = if reopened {
                    start.day.reopen(1, now.instant).unwrap().0
                } else {
                    start.day
                };
                let tasks = day.tasks().to_vec();
                let update = calls.finish(day, start.request.unwrap().id,
                    Ok(ModelAnswer { json: serde_json::json!({"kind":kind,"task":1,"message":"完了した作業の確認","next_look_minutes":5}).to_string() }),
                    context(now), fixed);
                assert_eq!(
                    update.day.data().next_planned_look,
                    (!reopened).then(|| now
                        .local
                        .checked_add(jiff::SignedDuration::from_mins(5))
                        .unwrap())
                );
                let rows = update
                    .day
                    .messages()
                    .iter()
                    .filter(|row| row.unprompted.is_some())
                    .collect::<Vec<_>>();
                let delivers = !reopened && kind != "silent";
                assert_eq!(rows.len(), usize::from(delivers));
                if delivers {
                    assert_eq!(rows[0].text, "完了した作業の確認");
                    let metadata = rows[0].unprompted.as_ref().unwrap();
                    assert_eq!(metadata.task, Some(1));
                    assert_eq!(metadata.trigger.kind, TriggerKind::PlannedLook);
                }
                assert_eq!(
                    update.effects,
                    if delivers {
                        vec![CheckinEffect::Bell, CheckinEffect::Save]
                    } else if reopened {
                        vec![]
                    } else {
                        vec![CheckinEffect::Save]
                    }
                );
                assert_eq!(update.day.tasks(), tasks);
                assert_eq!(
                    update.day.data().held_triggers,
                    if reopened {
                        vec![Trigger {
                            kind: TriggerKind::PlannedLook,
                            task: Some(1),
                            due_at: now.instant,
                        }]
                    } else {
                        vec![]
                    }
                );
                assert!(update.request.is_none());
            }
        }
    }
}

#[test]
fn spent_retries_keep_an_existing_look_and_plan_a_default_only_when_missing() {
    use bunshin_core::{
        ModelError,
        checkin::{
            calls::{CheckinCalls, CheckinEffect},
            plan_look,
        },
    };
    for default_minutes in [120, 45] {
        let mut tuning = Tuning::default();
        tuning.checkin.planned_default_minutes = default_minutes;
        for error in [
            ModelError::TimedOut,
            ModelError::Cancelled,
            ModelError::Refused,
            ModelError::Malformed,
            ModelError::Failed,
        ] {
            for preserve in [false, true] {
                let (day, owner, _, mut now) = fixture(TriggerKind::EveningReview).unwrap();
                let day = if preserve {
                    plan_look(day, now, Some(7), tuning)
                } else {
                    day
                };
                let previous = day.data().next_planned_look;
                let tasks = day.tasks().to_vec();
                let messages = day.messages().to_vec();
                let mut calls = CheckinCalls::new(tuning);
                queue(&mut calls, &day, TriggerKind::EveningReview, now.instant);
                let first =
                    calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
                let failed = calls.finish(
                    first.day,
                    first.request.unwrap().id,
                    Err(error),
                    context(now),
                    fixed,
                );
                assert_eq!(failed.day.data().next_planned_look, previous);
                now.instant = UnixMillis(60_000);
                now.local = now.local.date().at(14, 31, 0, 0);
                let retry = calls.prepare(
                    failed.day,
                    &owner,
                    ContextExtras::default(),
                    context(now),
                    fixed,
                );
                let spent = calls.finish(
                    retry.day,
                    retry.request.unwrap().id,
                    Err(error),
                    context(now),
                    fixed,
                );
                let expected = previous.unwrap_or_else(|| {
                    now.local
                        .checked_add(jiff::SignedDuration::from_mins(i64::from(default_minutes)))
                        .unwrap()
                });
                assert_eq!(spent.day.data().next_planned_look, Some(expected));
                assert_eq!(spent.effects, vec![CheckinEffect::Save]);
                assert_eq!(spent.day.tasks(), tasks);
                assert_eq!(spent.day.messages(), messages);
                assert!(spent.day.data().held_triggers.is_empty());
                assert!(spent.request.is_none());
            }
        }
    }
}

#[test]
fn deadline_only_failures_schedule_a_default_look_without_replacing_an_existing_one() {
    use bunshin_core::{
        Availability, ModelError, UnavailableReason,
        checkin::{
            calls::{CheckinCalls, CheckinEffect},
            plan_look,
        },
    };
    for default_minutes in [120, 45] {
        let mut tuning = Tuning::default();
        tuning.checkin.planned_default_minutes = default_minutes;
        for kind in [TriggerKind::BeforeDeadline, TriggerKind::AfterDeadline] {
            for error in [
                ModelError::Unavailable(UnavailableReason::NotInstalled),
                ModelError::TimedOut,
                ModelError::Cancelled,
                ModelError::Refused,
                ModelError::Malformed,
                ModelError::Failed,
            ] {
                for preserve in [false, true] {
                    let (day, owner, _, now) = fixture(kind).unwrap();
                    let day = if preserve {
                        plan_look(day, now, Some(7), tuning)
                    } else {
                        day
                    };
                    let tasks = day.tasks().to_vec();
                    let mut calls = CheckinCalls::new(tuning);
                    queue(&mut calls, &day, kind, now.instant);
                    let update = if let ModelError::Unavailable(reason) = error {
                        calls.prepare(
                            day,
                            &owner,
                            ContextExtras::default(),
                            bunshin_core::checkin::calls::CallContext {
                                availability: Availability::Unavailable(reason),
                                ..context(now)
                            },
                            fixed,
                        )
                    } else {
                        let start = calls.prepare(
                            day,
                            &owner,
                            ContextExtras::default(),
                            context(now),
                            fixed,
                        );
                        calls.finish(
                            start.day,
                            start.request.unwrap().id,
                            Err(error),
                            context(now),
                            fixed,
                        )
                    };
                    let delay = if preserve { 7 } else { default_minutes };
                    assert_eq!(
                        update.day.data().next_planned_look,
                        Some(
                            now.local
                                .checked_add(jiff::SignedDuration::from_mins(i64::from(delay)))
                                .unwrap()
                        )
                    );
                    assert_eq!(
                        update.day.messages().last().unwrap().text,
                        format!("fixed {kind:?}")
                    );
                    assert_eq!(
                        update.effects,
                        vec![CheckinEffect::Bell, CheckinEffect::Save]
                    );
                    assert_eq!(update.day.tasks(), tasks);
                    assert!(update.day.data().held_triggers.is_empty());
                    assert!(update.request.is_none());
                }
            }
        }
    }
}

#[test]
fn mixed_batch_keeps_general_answers_but_discards_references_to_the_expired_deadline() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{BatchReason, ReadyBatch, calls::CheckinEffect},
    };
    for kind in ["note", "question", "silent"] {
        for task in [None, Some(999_u64), Some(1)] {
            let (day, owner, mut calls, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
            now.local = now.local.date().at(14, 59, 59, 0);
            let original_tasks = day.tasks().to_vec();
            calls.enqueue(
                &day,
                ReadyBatch {
                    reason: BatchReason::Tick,
                    triggers: vec![Trigger {
                        kind: TriggerKind::PlannedLook,
                        task: None,
                        due_at: now.instant,
                    }],
                },
            );
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let mut json =
                serde_json::json!({"kind":kind,"message":"一般の回答","next_look_minutes":5});
            if let Some(task) = task {
                json["task"] = serde_json::json!(task);
            }
            now.instant = UnixMillis(3_000);
            now.local = now.local.date().at(15, 0, 1, 0);
            let update = calls.finish(
                start.day,
                start.request.unwrap().id,
                Ok(ModelAnswer {
                    json: json.to_string(),
                }),
                context(now),
                fixed,
            );
            let valid = task != Some(1);
            assert_eq!(
                update.day.data().next_planned_look,
                valid.then(|| now
                    .local
                    .checked_add(jiff::SignedDuration::from_mins(5))
                    .unwrap())
            );
            let rows = update
                .day
                .messages()
                .iter()
                .filter(|row| row.unprompted.is_some())
                .collect::<Vec<_>>();
            let delivers = valid && kind != "silent";
            assert_eq!(rows.len(), usize::from(delivers));
            if delivers {
                assert_eq!(rows[0].text, "一般の回答");
                let metadata = rows[0].unprompted.as_ref().unwrap();
                assert_eq!(metadata.trigger.kind, TriggerKind::PlannedLook);
                assert_eq!(metadata.task, None);
                assert_eq!(metadata.suppressed, None);
            }
            assert_eq!(
                update.effects,
                if delivers {
                    vec![CheckinEffect::Bell, CheckinEffect::Save]
                } else {
                    vec![CheckinEffect::Save]
                }
            );
            assert_eq!(update.day.tasks(), original_tasks);
            assert_eq!(
                update.day.data().held_triggers,
                if valid {
                    vec![]
                } else {
                    vec![Trigger {
                        kind: TriggerKind::PlannedLook,
                        task: None,
                        due_at: UnixMillis(0),
                    }]
                }
            );
            assert!(update.request.is_none());
        }
    }
}

#[test]
fn expired_before_deadline_work_does_not_suppress_the_next_overdue_tick() {
    use bunshin_core::{
        Availability, UnavailableReason,
        checkin::{
            Checkin,
            calls::{CheckinCalls, CheckinEffect},
        },
    };
    for availability in [
        Availability::Available,
        Availability::Unavailable(UnavailableReason::NotInstalled),
    ] {
        let (day, owner, _, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let original_messages = day.messages().to_vec();
        now.local = now.local.date().at(14, 59, 59, 0);
        let scheduled = Checkin::new(now, Tuning::default()).open(day, now, false);
        let mut calls = CheckinCalls::new(Tuning::default());
        calls.enqueue(&scheduled.day, scheduled.ready.unwrap());
        let waiting = calls.prepare(
            scheduled.day,
            &owner,
            ContextExtras::default(),
            bunshin_core::checkin::calls::CallContext {
                input_has_text: true,
                ..context(now)
            },
            fixed,
        );
        assert!(waiting.request.is_none());
        now.instant = UnixMillis(3_000);
        now.local = now.local.date().at(15, 0, 1, 0);
        let expired = calls.prepare(
            waiting.day,
            &owner,
            ContextExtras::default(),
            bunshin_core::checkin::calls::CallContext {
                availability,
                is_tick: false,
                ..context(now)
            },
            fixed,
        );
        assert!(expired.request.is_none());
        assert_eq!(expired.day.messages(), original_messages);
        assert!(expired.day.data().held_triggers.is_empty());
        assert_eq!(expired.day.data().last_unprompted_at, None);
        assert_eq!(expired.effects, vec![CheckinEffect::Save]);
        now.instant = UnixMillis(60_000);
        now.local = now.local.date().at(15, 0, 59, 0);
        let next = scheduled.checkin.tick(expired.day, now, false);
        calls.enqueue(&next.day, next.ready.unwrap());
        let delivered = calls.prepare(
            next.day,
            &owner,
            ContextExtras::default(),
            bunshin_core::checkin::calls::CallContext {
                availability: Availability::Unavailable(UnavailableReason::NotInstalled),
                ..context(now)
            },
            fixed,
        );
        let rows = delivered
            .day
            .messages()
            .iter()
            .filter(|row| row.unprompted.is_some())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text, "fixed AfterDeadline");
        let metadata = rows[0].unprompted.as_ref().unwrap();
        assert_eq!(metadata.trigger.kind, TriggerKind::AfterDeadline);
        assert_eq!(metadata.suppressed, None);
        assert_eq!(
            delivered.effects,
            vec![CheckinEffect::Bell, CheckinEffect::Save]
        );
        assert_eq!(delivered.day.data().triggers_fired.len(), 2);
    }
}

#[test]
fn before_deadline_answers_and_fallbacks_expire_during_the_call_or_completion_wait() {
    use bunshin_core::{ModelAnswer, ModelError};
    for outcome in ["success", "timeout", "held"] {
        let (day, owner, mut calls, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let original_messages = day.messages().to_vec();
        now.local = now.local.date().at(14, 59, 59, 0);
        let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
        let result = if outcome == "timeout" {
            Err(ModelError::TimedOut)
        } else {
            Ok(ModelAnswer {
                json: r#"{"kind":"note","task":1,"message":"approaching","next_look_minutes":5}"#
                    .into(),
            })
        };
        let finish_now = if outcome == "held" {
            now
        } else {
            Now {
                instant: UnixMillis(3_000),
                local: now.local.date().at(15, 0, 1, 0),
            }
        };
        let finished = calls.finish(
            start.day,
            start.request.unwrap().id,
            result,
            bunshin_core::checkin::calls::CallContext {
                owner_waiting: outcome == "held",
                ..context(finish_now)
            },
            fixed,
        );
        let update = if outcome == "held" {
            now.instant = UnixMillis(3_000);
            now.local = now.local.date().at(15, 0, 1, 0);
            calls.prepare(
                finished.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed,
            )
        } else {
            finished
        };
        assert!(update.request.is_none());
        assert_eq!(update.day.messages(), original_messages, "{outcome}");
        assert!(update.day.data().held_triggers.is_empty());
        assert_eq!(update.day.data().last_unprompted_at, None);
        assert_eq!(
            update.day.data().next_planned_look,
            Some(now.local.date().at(17, 0, 1, 0))
        );
        assert_eq!(
            update.effects,
            vec![bunshin_core::checkin::calls::CheckinEffect::Save]
        );
    }
}

#[test]
fn scheduler_releases_during_a_call_cannot_remove_the_workers_persisted_events() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{
            Checkin,
            calls::{CheckinCalls, CheckinEffect},
        },
    };
    for completed in [false, true] {
        let (day, owner, _, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let scheduled = Checkin::new(now, Tuning::default()).open(day, now, false);
        let batch = scheduled.ready.unwrap();
        let events = batch.triggers.clone();
        let mut calls = CheckinCalls::new(Tuning::default());
        calls.enqueue(&scheduled.day, batch);
        let start = calls.prepare(
            scheduled.day,
            &owner,
            ContextExtras::default(),
            context(now),
            fixed,
        );
        let day = if completed {
            calls
                .finish(
                    start.day,
                    start.request.unwrap().id,
                    Ok(ModelAnswer {
                        json: r#"{"kind":"note","task":1,"message":"held answer"}"#.into(),
                    }),
                    bunshin_core::checkin::calls::CallContext {
                        owner_waiting: true,
                        ..context(now)
                    },
                    fixed,
                )
                .day
        } else {
            start.day
        };
        now.instant = UnixMillis(60_000);
        now.local = now.local.date().at(14, 31, 0, 0);
        let released = scheduled.checkin.tick(day, now, false);
        calls.enqueue(&released.day, released.ready.unwrap());
        let held = calls.prepare(
            released.day,
            &owner,
            ContextExtras::default(),
            bunshin_core::checkin::calls::CallContext {
                owner_waiting: true,
                ..context(now)
            },
            fixed,
        );
        assert_eq!(held.day.data().held_triggers, events);
        assert_eq!(held.effects, vec![CheckinEffect::Save]);
        assert!(held.request.is_none());
    }
}

#[test]
fn in_flight_deadlines_remain_persisted_and_can_be_recovered_after_restart() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{
            Checkin,
            calls::{CheckinCalls, CheckinEffect},
        },
        day::file::DayFile,
    };
    let (day, owner, _, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    let scheduled = Checkin::new(now, Tuning::default()).open(day, now, false);
    let batch = scheduled.ready.unwrap();
    let events = batch.triggers.clone();
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(&scheduled.day, batch);
    let start = calls.prepare(
        scheduled.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    assert!(start.request.is_some());
    assert_eq!(start.day.data().held_triggers, events);
    assert!(start.effects.contains(&CheckinEffect::Save));
    let restored = DayFile::from(&start.day)
        .into_day(Tuning::default())
        .unwrap();
    let reopened = Checkin::new(now, Tuning::default()).open(restored, now, false);
    let mut fresh = CheckinCalls::new(Tuning::default());
    fresh.enqueue(&reopened.day, reopened.ready.unwrap());
    let retry = fresh.prepare(
        reopened.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let done = fresh.finish(
        retry.day,
        retry.request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(done.day.data().held_triggers.is_empty());
    assert_eq!(done.day.data().triggers_fired, events);
}

#[test]
fn mixed_opening_batches_hold_routine_answers_after_hours_or_when_muted_during_the_call() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{
            Checkin,
            calls::{CheckinCalls, CheckinEffect},
        },
    };
    for (day_start, muted) in [(false, false), (false, true), (true, false), (true, true)] {
        let (day, owner, _, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let day = day
            .edit(
                1,
                "資料作成".into(),
                TaskKind::Deadline,
                Some("21:00".parse().unwrap()),
                now.instant,
            )
            .unwrap()
            .0
            .add(
                "routine".into(),
                TaskKind::Deadline,
                Some("22:15".parse().unwrap()),
                TaskOrigin::Key,
                now.instant,
            )
            .unwrap()
            .0;
        now.local = now.local.date().at(21, 0, 0, 0);
        let scheduler = Checkin::new(now, Tuning::default());
        let opening = if day_start {
            scheduler.day_start(day, now, true)
        } else {
            scheduler.open(day, now, true)
        };
        now.instant = UnixMillis(2_700_000);
        now.local = now.local.date().at(21, 45, 0, 0);
        let later = opening.checkin.tick(opening.day, now, true);
        let released = later.checkin.release_held(later.day, now, false);
        let mut calls = CheckinCalls::new(Tuning::default());
        calls.enqueue(&released.day, released.ready.unwrap());
        let start = calls.prepare(
            released.day,
            &owner,
            ContextExtras::default(),
            context(now),
            fixed,
        );
        let day = if muted {
            start.day.mute(UnixMillis(9_000_000), now.instant).0
        } else {
            now.local = now.local.date().at(22, 0, 20, 0);
            start.day
        };
        let count = day.messages().len();
        let held = calls.finish(
            day,
            start.request.unwrap().id,
            Ok(ModelAnswer {
                json: r#"{"kind":"note","task":2,"message":"routine answer"}"#.into(),
            }),
            context(now),
            fixed,
        );
        assert_eq!(held.day.messages().len(), count);
        assert!(!held.effects.contains(&CheckinEffect::Bell));
        assert!(!held.day.data().held_triggers.is_empty());
        assert!(
            calls
                .prepare(
                    held.day,
                    &owner,
                    ContextExtras::default(),
                    context(now),
                    fixed
                )
                .request
                .is_none()
        );
    }
}

#[test]
fn successful_overdue_notes_choose_after_deadline_when_both_phases_are_ready() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{BatchReason, ReadyBatch, calls::CheckinCalls},
    };
    let (day, owner, _, now) = fixture(TriggerKind::AfterDeadline).unwrap();
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(
        &day,
        ReadyBatch {
            reason: BatchReason::Tick,
            triggers: vec![
                Trigger {
                    kind: TriggerKind::BeforeDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
                Trigger {
                    kind: TriggerKind::AfterDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
            ],
        },
    );
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let done = calls.finish(
        start.day,
        start.request.unwrap().id,
        Ok(ModelAnswer {
            json: r#"{"kind":"note","task":1,"message":"overdue"}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert_eq!(
        done.day
            .messages()
            .last()
            .unwrap()
            .unprompted
            .as_ref()
            .unwrap()
            .trigger
            .kind,
        TriggerKind::AfterDeadline
    );
}

#[test]
fn title_only_edits_before_dispatch_keep_the_queued_deadline_with_its_current_title() {
    use bunshin_core::{Availability, UnavailableReason, checkin::calls::CallContext};
    for availability in [
        Availability::Available,
        Availability::Unavailable(UnavailableReason::NotInstalled),
    ] {
        let (day, owner, mut calls, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let held = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            CallContext {
                owner_waiting: true,
                ..context(now)
            },
            fixed,
        );
        let day = held
            .day
            .edit(
                1,
                "新しい資料".into(),
                TaskKind::Deadline,
                Some("15:00".parse().unwrap()),
                now.instant,
            )
            .unwrap()
            .0;
        let update = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            CallContext {
                availability,
                ..context(now)
            },
            |note| {
                assert_eq!(note.task.title, "新しい資料");
                "current title deadline".into()
            },
        );
        match availability {
            Availability::Available => assert!(update.request.is_some()),
            Availability::Unavailable(_) => assert_eq!(
                update.day.messages().last().unwrap().text,
                "current title deadline"
            ),
        }
    }
}

#[test]
fn a_task_created_during_a_call_cannot_capture_the_models_previously_unknown_reference() {
    use bunshin_core::{ModelAnswer, checkin::calls::CheckinEffect};
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let day = start
        .day
        .add(
            "後で追加".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap()
        .0;
    assert_eq!(day.tasks().last().unwrap().number, 2);
    let answer = ModelAnswer {
        json: r#"{"kind":"question","task":2,"message":"general answer"}"#.into(),
    };
    let update = calls.finish(
        day,
        start.request.unwrap().id,
        Ok(answer),
        context(now),
        fixed,
    );
    let message = update.day.messages().last().unwrap();
    assert_eq!(message.text, "general answer");
    assert_eq!(message.unprompted.as_ref().unwrap().task, None);
    assert!(update.effects.contains(&CheckinEffect::Bell));
}

#[test]
fn checkin_prompt_describes_the_configured_before_deadline_offset() {
    let (day, owner, _, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    let mut tuning = Tuning::default();
    tuning.checkin.before_deadline_minutes = 15;
    let built = build_checkin(
        &day,
        &owner,
        now,
        &[Trigger {
            kind: TriggerKind::BeforeDeadline,
            task: Some(1),
            due_at: now.instant,
        }],
        ContextExtras::default(),
        tuning,
    )
    .unwrap();
    assert!(built.request.instructions.contains("b=締切15分前"));
    assert!(!built.request.instructions.contains("b=締切30分前"));
}

#[test]
fn opening_exempt_batches_cannot_carry_later_routine_events_past_delivery_guards() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{BatchReason, ReadyBatch},
    };
    let extras = ContextExtras::default();
    for reason in [BatchReason::Open, BatchReason::DayStart] {
        for guard in ["hours", "mute", "gap"] {
            let (day, owner, _, mut now) = fixture(TriggerKind::PlannedLook).unwrap();
            let day = apply_delivery_guard(day, &mut now, guard).unwrap();
            let opening = Trigger {
                kind: TriggerKind::DayStart,
                task: None,
                due_at: now.instant,
            };
            let routine = Trigger {
                kind: TriggerKind::PlannedLook,
                task: None,
                due_at: UnixMillis(1),
            };
            let mut calls = bunshin_core::checkin::calls::CheckinCalls::new(Tuning::default());
            calls.enqueue(
                &day,
                ReadyBatch {
                    reason,
                    triggers: vec![opening.clone()],
                },
            );
            let held = calls.prepare(
                day,
                &owner,
                extras,
                bunshin_core::checkin::calls::CallContext {
                    owner_waiting: true,
                    ..context(now)
                },
                fixed,
            );
            calls.enqueue(
                &held.day,
                ReadyBatch {
                    reason: BatchReason::Tick,
                    triggers: vec![routine.clone()],
                },
            );
            let start = calls.prepare(held.day, &owner, extras, context(now), fixed);
            assert_eq!(
                start.day.data().held_triggers,
                vec![opening, routine.clone()]
            );
            assert!(
                !start
                    .request
                    .as_ref()
                    .unwrap()
                    .request
                    .prompt
                    .contains("[\"p\",")
            );
            let done = calls.finish(
                start.day,
                start.request.unwrap().id,
                Ok(ModelAnswer {
                    json: r#"{"kind":"silent","message":""}"#.into(),
                }),
                context(now),
                fixed,
            );
            let blocked = calls.prepare(done.day, &owner, extras, context(now), fixed);
            assert!(blocked.request.is_none());
            now.instant = UnixMillis(3_600_001);
            now.local = now.local.date().at(14, 30, 0, 0);
            let next = calls.prepare(blocked.day, &owner, extras, context(now), fixed);
            let expected = build_checkin(
                &next.day,
                &owner,
                now,
                &[routine],
                extras,
                Tuning::default(),
            )
            .unwrap();
            assert_eq!(
                next.request.unwrap().request.prompt,
                expected.request.prompt
            );
        }
    }
}

#[test]
fn restarted_held_deadlines_moved_into_the_future_are_removed_before_model_or_fallback() {
    use bunshin_core::{
        Availability, UnavailableReason,
        checkin::{BatchReason, ReadyBatch, calls::CheckinEffect},
    };
    for kind in [TriggerKind::BeforeDeadline, TriggerKind::AfterDeadline] {
        for availability in [
            Availability::Available,
            Availability::Unavailable(UnavailableReason::NotInstalled),
        ] {
            let (day, owner, _, now) = fixture(kind).unwrap();
            let event = Trigger {
                kind,
                task: Some(1),
                due_at: now.instant,
            };
            let mut data = day.data().clone();
            data.held_triggers = vec![event.clone()];
            data.triggers_fired = vec![event.clone()];
            data.tasks[0].triggers_fired = vec![event.clone()];
            let day = bunshin_core::day::file::DayFile { format: 1, data }
                .into_day(Tuning::default())
                .unwrap()
                .edit(
                    1,
                    "資料作成".into(),
                    TaskKind::Deadline,
                    Some("17:00".parse().unwrap()),
                    now.instant,
                )
                .unwrap()
                .0;
            let restored = bunshin_core::day::file::DayFile {
                format: 1,
                data: day.data().clone(),
            }
            .into_day(Tuning::default())
            .unwrap();
            let mut calls = bunshin_core::checkin::calls::CheckinCalls::new(Tuning::default());
            calls.enqueue(
                &restored,
                ReadyBatch {
                    reason: BatchReason::Open,
                    triggers: vec![event],
                },
            );
            let count = restored.messages().len();
            let update = calls.prepare(
                restored,
                &owner,
                ContextExtras::default(),
                bunshin_core::checkin::calls::CallContext {
                    availability,
                    ..context(now)
                },
                fixed,
            );
            assert!(update.request.is_none());
            assert_eq!(update.day.messages().len(), count);
            assert!(update.day.data().held_triggers.is_empty());
            assert_eq!(update.effects, vec![CheckinEffect::Save]);
        }
    }
}

#[test]
fn a_trigger_queued_during_a_call_joins_its_failed_trigger_at_the_next_tick() {
    use bunshin_core::{ModelAnswer, ModelError};
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    queue(
        &mut calls,
        &start.day,
        TriggerKind::EveningReview,
        UnixMillis(1),
    );
    let held = calls.prepare(
        start.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let failed = calls.finish(
        held.day,
        start.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    let blocked = calls.prepare(
        failed.day,
        &owner,
        ContextExtras::default(),
        bunshin_core::checkin::calls::CallContext {
            is_tick: false,
            ..context(now)
        },
        fixed,
    );
    assert!(blocked.request.is_none());
    let next = calls.prepare(
        blocked.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let expected = build_checkin(
        &next.day,
        &owner,
        now,
        &[
            Trigger {
                kind: TriggerKind::EveningReview,
                task: Some(1),
                due_at: UnixMillis(1),
            },
            Trigger {
                kind: TriggerKind::PlannedLook,
                task: Some(1),
                due_at: now.instant,
            },
        ],
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    let request = next.request.unwrap();
    assert_eq!(request.request.prompt, expected.request.prompt);
    let done = calls.finish(
        next.day,
        request.id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(
        calls
            .prepare(
                done.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed
            )
            .request
            .is_none()
    );
}

#[test]
fn merging_a_fresh_event_into_a_retry_preserves_each_events_one_retry() {
    use bunshin_core::{
        ModelError,
        checkin::{BatchReason, ReadyBatch},
    };
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let failed = calls.finish(
        start.day,
        start.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    let new = Trigger {
        kind: TriggerKind::EveningReview,
        task: None,
        due_at: UnixMillis(1),
    };
    calls.enqueue(
        &failed.day,
        ReadyBatch {
            reason: BatchReason::Tick,
            triggers: vec![new.clone()],
        },
    );
    let retry = calls.prepare(
        failed.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let second = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    assert_eq!(second.day.data().held_triggers, vec![new.clone()]);
    let fresh_retry = calls.prepare(
        second.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let expected = build_checkin(
        &fresh_retry.day,
        &owner,
        now,
        &[new],
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    let request = fresh_retry.request.unwrap();
    assert_eq!(request.request.prompt, expected.request.prompt);
    let spent = calls.finish(
        fresh_retry.day,
        request.id,
        Err(ModelError::Refused),
        context(now),
        fixed,
    );
    assert!(spent.day.data().held_triggers.is_empty());
    assert!(
        calls
            .prepare(
                spent.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed
            )
            .request
            .is_none()
    );
}

#[test]
fn unavailable_held_triggers_and_new_events_recover_in_one_call() {
    use bunshin_core::{
        Availability, ModelAnswer, UnavailableReason,
        checkin::{BatchReason, ReadyBatch},
    };
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let held = calls.prepare(
        day,
        &owner,
        ContextExtras::default(),
        bunshin_core::checkin::calls::CallContext {
            availability: Availability::Unavailable(UnavailableReason::NotInstalled),
            ..context(now)
        },
        fixed,
    );
    let old = held.day.data().held_triggers[0].clone();
    let new = Trigger {
        kind: TriggerKind::EveningReview,
        task: None,
        due_at: UnixMillis(1),
    };
    calls.enqueue(
        &held.day,
        ReadyBatch {
            reason: BatchReason::Tick,
            triggers: vec![old, new],
        },
    );
    let start = calls.prepare(
        held.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let expected = build_checkin(
        &start.day,
        &owner,
        now,
        &[
            Trigger {
                kind: TriggerKind::PlannedLook,
                task: Some(1),
                due_at: now.instant,
            },
            Trigger {
                kind: TriggerKind::EveningReview,
                task: None,
                due_at: UnixMillis(1),
            },
        ],
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    let request = start.request.unwrap();
    assert_eq!(request.request.prompt, expected.request.prompt);
    let done = calls.finish(
        start.day,
        request.id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(done.day.data().held_triggers.is_empty());
    assert!(
        calls
            .prepare(
                done.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed
            )
            .request
            .is_none()
    );
}

#[test]
fn checkin_rejects_blank_notes_and_questions_but_accepts_silent_empty_text() {
    use bunshin_core::{ModelError, checkin::answer::parse_checkin};
    let (day, _, _, _) = fixture(TriggerKind::PlannedLook).unwrap();
    for kind in ["note", "question"] {
        for message in ["", " \t\n", "　"] {
            let json = serde_json::json!({"kind":kind,"message":message}).to_string();
            assert_eq!(
                parse_checkin(&json, &day, Tuning::default()),
                Err(ModelError::Malformed)
            );
        }
    }
    assert!(parse_checkin(r#"{"kind":"silent","message":""}"#, &day, Tuning::default()).is_ok());
}

#[test]
fn queued_deadlines_edited_behind_owner_work_are_removed_before_model_or_fallback_dispatch() {
    use bunshin_core::{
        Availability, UnavailableReason,
        checkin::calls::{CallContext, CheckinEffect},
    };
    for availability in [
        Availability::Available,
        Availability::Unavailable(UnavailableReason::NotInstalled),
    ] {
        let (day, owner, mut calls, now) = fixture(TriggerKind::AfterDeadline).unwrap();
        let held = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            CallContext {
                owner_waiting: true,
                ..context(now)
            },
            fixed,
        );
        assert_eq!(held.day.data().held_triggers.len(), 1);
        let day = held
            .day
            .edit(
                1,
                "資料作成".into(),
                TaskKind::Deadline,
                Some("17:00".parse().unwrap()),
                now.instant,
            )
            .unwrap()
            .0;
        let count = day.messages().len();
        let result = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            CallContext {
                availability,
                ..context(now)
            },
            fixed,
        );
        assert!(result.request.is_none());
        assert_eq!(result.day.messages().len(), count);
        assert!(result.day.data().held_triggers.is_empty());
        assert_eq!(result.effects, vec![CheckinEffect::Save]);
    }
}

#[test]
fn discarding_a_stale_response_preserves_the_current_planned_look_for_notes_and_silence() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{calls::CheckinEffect, plan_look},
    };
    for action in ["done", "drop", "delete", "edit"] {
        for kind in ["note", "silent"] {
            let (day, owner, mut calls, now) = fixture(TriggerKind::AfterDeadline).unwrap();
            let day = plan_look(day, now, Some(60), Tuning::default());
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let day = match action {
                "done" => start.day.done(1, now.instant).unwrap().0,
                "drop" => start.day.drop(1, now.instant).unwrap().0,
                "delete" => start.day.delete(1, now.instant).unwrap().0,
                "edit" => {
                    start
                        .day
                        .edit(
                            1,
                            "資料作成".into(),
                            TaskKind::Deadline,
                            Some("17:00".parse().unwrap()),
                            now.instant,
                        )
                        .unwrap()
                        .0
                }
                _ => panic!("fixture action"),
            };
            let planned = day.data().next_planned_look;
            let answer = ModelAnswer { json:serde_json::json!({"kind":kind,"task":1,"message":"old facts","next_look_minutes":5}).to_string() };
            let result = calls.finish(
                day,
                start.request.unwrap().id,
                Ok(answer),
                context(now),
                fixed,
            );
            assert_eq!(result.day.data().next_planned_look, planned);
            assert!(!result.effects.contains(&CheckinEffect::Bell));
        }
    }
}

#[test]
fn completed_checkins_wait_for_current_owner_input_and_queued_conversation_to_clear() {
    use bunshin_core::{
        ModelAnswer, ModelError,
        checkin::calls::{CallContext, CheckinEffect},
    };
    for failure in [false, true] {
        for typing in [false, true] {
            let (day, owner, mut calls, now) = fixture(TriggerKind::AfterDeadline).unwrap();
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let busy = CallContext {
                input_has_text: typing,
                owner_waiting: !typing,
                ..context(now)
            };
            let result = if failure {
                Err(ModelError::TimedOut)
            } else {
                Ok(ModelAnswer {
                    json: r#"{"kind":"note","task":1,"message":"retained answer"}"#.into(),
                })
            };
            let held = calls.finish(start.day, start.request.unwrap().id, result, busy, fixed);
            assert!(
                held.day
                    .messages()
                    .iter()
                    .all(|row| row.unprompted.is_none())
            );
            assert_eq!(held.effects, vec![]);
            let still_held = calls.prepare(held.day, &owner, ContextExtras::default(), busy, fixed);
            assert!(still_held.request.is_none());
            assert!(
                still_held
                    .day
                    .messages()
                    .iter()
                    .all(|row| row.unprompted.is_none())
            );
            let posted = calls.prepare(
                still_held.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed,
            );
            assert!(posted.request.is_none());
            assert_eq!(
                posted.effects,
                vec![CheckinEffect::Bell, CheckinEffect::Save]
            );
            assert_eq!(
                posted.day.messages().last().unwrap().text,
                if failure {
                    "fixed AfterDeadline"
                } else {
                    "retained answer"
                }
            );
        }
    }
}

#[test]
fn deadline_responses_and_fallbacks_are_discarded_after_a_time_title_or_kind_edit() {
    use bunshin_core::{ModelAnswer, ModelError, checkin::calls::CheckinEffect};
    for failure in [false, true] {
        for (title, kind, time) in [
            ("資料作成", TaskKind::Deadline, "17:00"),
            ("別の資料", TaskKind::Deadline, "15:00"),
            ("資料作成", TaskKind::Appointment, "15:00"),
        ] {
            let (day, owner, mut calls, now) = fixture(TriggerKind::AfterDeadline).unwrap();
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let day = start
                .day
                .edit(
                    1,
                    title.into(),
                    kind,
                    Some(time.parse().unwrap()),
                    now.instant,
                )
                .unwrap()
                .0;
            let count = day.messages().len();
            let tasks = day.tasks().to_vec();
            let result = if failure {
                Err(ModelError::TimedOut)
            } else {
                Ok(ModelAnswer {
                    json: r#"{"kind":"note","task":1,"message":"old deadline facts"}"#.into(),
                })
            };
            let update = calls.finish(day, start.request.unwrap().id, result, context(now), fixed);
            assert_eq!(update.day.messages().len(), count);
            assert_eq!(update.day.tasks(), tasks);
            assert!(!update.effects.contains(&CheckinEffect::Bell));
        }
    }
}

#[test]
fn checkin_completion_is_held_after_hours_or_during_mute_without_another_model_call() {
    use bunshin_core::{ModelAnswer, ModelError, checkin::calls::CheckinEffect};
    for failure in [false, true] {
        for muted in [false, true] {
            let (day, owner, mut calls, mut now) = fixture(TriggerKind::AfterDeadline).unwrap();
            now.local = now.local.date().at(21, 59, 50, 0);
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let id = start.request.unwrap().id;
            let day = if muted {
                start.day.mute(UnixMillis(600_000), now.instant).0
            } else {
                start.day
            };
            let before = day.messages().len();
            now.instant.0 = 30_000;
            if !muted {
                now.local = now.local.date().at(22, 0, 20, 0);
            }
            let result = if failure {
                Err(ModelError::TimedOut)
            } else {
                Ok(ModelAnswer {
                    json: r#"{"kind":"note","task":1,"message":"complete answer"}"#.into(),
                })
            };
            let held = calls.finish(day, id, result, context(now), fixed);
            assert_eq!(held.day.messages().len(), before);
            assert_eq!(held.effects, vec![]);
            assert_eq!(held.day.data().held_triggers.len(), 1);
            let waiting = calls.prepare(
                held.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed,
            );
            assert!(waiting.request.is_none());
            now.instant.0 = 600_000;
            now.local = now.local.date().at(8, 0, 0, 0);
            let delivered = calls.prepare(
                waiting.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed,
            );
            assert!(delivered.request.is_none());
            assert_eq!(
                delivered.effects,
                vec![CheckinEffect::Bell, CheckinEffect::Save]
            );
            assert!(delivered.day.data().held_triggers.is_empty());
            assert_eq!(
                delivered.day.messages().last().unwrap().text,
                if failure {
                    "fixed AfterDeadline"
                } else {
                    "complete answer"
                }
            );
        }
    }
}

#[test]
fn checkin_success_drops_messages_for_deadline_tasks_closed_dropped_or_deleted_during_the_call() {
    use bunshin_core::ModelAnswer;
    for action in ["done", "drop", "delete"] {
        let (day, owner, mut calls, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
        let day = match action {
            "done" => start.day.done(1, now.instant).unwrap().0,
            "drop" => start.day.drop(1, now.instant).unwrap().0,
            "delete" => start.day.delete(1, now.instant).unwrap().0,
            _ => panic!("fixture action"),
        };
        let count = day.messages().len();
        let update = calls.finish(
            day,
            start.request.unwrap().id,
            Ok(ModelAnswer {
                json: r#"{"kind":"question","task":1,"message":"stale deadline"}"#.into(),
            }),
            context(now),
            fixed,
        );
        assert_eq!(update.day.messages().len(), count);
        assert!(
            update
                .day
                .messages()
                .iter()
                .all(|m| !m.text.contains("stale deadline"))
        );
        assert!(
            !update
                .effects
                .contains(&bunshin_core::checkin::calls::CheckinEffect::Bell)
        );
    }
}

#[test]
fn checkin_failed_non_deadline_is_saved_as_held_until_the_retry_is_spent() {
    use bunshin_core::ModelError;
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let failed = calls.finish(
        start.day,
        start.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    assert_eq!(failed.day.data().held_triggers.len(), 1);
    // The held event was saved at dispatch; the failure saves its retry record.
    assert_eq!(
        failed.effects,
        vec![bunshin_core::checkin::calls::CheckinEffect::Save]
    );
    assert_eq!(
        failed.day.data().retrying_triggers,
        failed.day.data().held_triggers
    );
    let later = Now {
        instant: UnixMillis(now.instant.0 + 60_000),
        ..now
    };
    let retry = calls.prepare(
        failed.day,
        &owner,
        ContextExtras::default(),
        context(later),
        fixed,
    );
    assert_eq!(
        retry.day.data().held_triggers,
        vec![Trigger {
            kind: TriggerKind::PlannedLook,
            task: Some(1),
            due_at: now.instant
        }]
    );
    let spent = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(later),
        fixed,
    );
    assert!(spent.day.data().held_triggers.is_empty());
    assert!(spent.day.data().retrying_triggers.is_empty());
    assert!(spent.day.data().next_planned_look.is_some());
    assert!(
        calls
            .prepare(
                spent.day,
                &owner,
                ContextExtras::default(),
                context(later),
                fixed
            )
            .request
            .is_none()
    );
}

#[test]
fn restarting_does_not_grant_a_failed_trigger_another_retry() {
    use bunshin_core::{
        ModelError,
        checkin::{BatchReason, ReadyBatch, calls::CheckinCalls, plan_look},
    };
    for during_retry in [false, true] {
        for preserve in [false, true] {
            let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let failed = calls.finish(
                start.day,
                start.request.unwrap().id,
                Err(ModelError::TimedOut),
                context(now),
                fixed,
            );
            let later = after(now, 60);
            let saved = if during_retry {
                calls
                    .prepare(
                        failed.day,
                        &owner,
                        ContextExtras::default(),
                        context(later),
                        fixed,
                    )
                    .day
            } else {
                failed.day
            };
            let restored = restart(&saved);
            assert_eq!(
                restored.data().retrying_triggers,
                restored.data().held_triggers
            );
            let restored = if preserve {
                plan_look(restored, later, Some(7), Tuning::default())
            } else {
                restored
            };
            let expected = restored.data().next_planned_look.unwrap_or_else(|| {
                later
                    .local
                    .checked_add(jiff::SignedDuration::from_mins(120))
                    .unwrap()
            });
            let tasks = restored.tasks().to_vec();
            let held = restored.data().held_triggers.clone();
            let mut recovered = CheckinCalls::new(Tuning::default());
            recovered.enqueue(
                &restored,
                ReadyBatch {
                    triggers: held,
                    reason: BatchReason::Tick,
                },
            );
            let retry = recovered.prepare(
                restored,
                &owner,
                ContextExtras::default(),
                context(later),
                fixed,
            );
            let spent = recovered.finish(
                retry.day,
                retry.request.unwrap().id,
                Err(ModelError::TimedOut),
                context(later),
                fixed,
            );
            assert!(spent.day.data().held_triggers.is_empty());
            assert!(spent.day.data().retrying_triggers.is_empty());
            assert_eq!(spent.day.data().next_planned_look, Some(expected));
            assert_eq!(spent.day.tasks(), tasks);
            assert!(
                spent
                    .day
                    .messages()
                    .iter()
                    .all(|row| row.unprompted.is_none())
            );
            assert!(
                recovered
                    .prepare(
                        spent.day,
                        &owner,
                        ContextExtras::default(),
                        context(later),
                        fixed,
                    )
                    .request
                    .is_none()
            );
        }
    }
}

#[test]
fn a_restored_retry_waits_for_an_actual_tick_and_every_delivery_guard() {
    use bunshin_core::{
        ModelError,
        checkin::{Checkin, calls::CheckinCalls},
    };
    let (day, owner, now) = untimed().unwrap();
    let look = planned_look(now.instant);
    let failed = fail_once(day, &owner, now, &look);
    assert_eq!(failed.data().retrying_triggers, vec![look.clone()]);
    let at = after(now, 60);
    let recovered = || {
        let reopened = Checkin::new(at, Tuning::default()).open(restart(&failed), at, false);
        let mut calls = CheckinCalls::new(Tuning::default());
        calls.enqueue(&reopened.day, reopened.ready.clone().unwrap());
        (calls, reopened.day)
    };
    // The scheduler released the look to the queue; a save in that gap stays loadable
    // and the queue still restores the record.
    let (_, released) = recovered();
    assert!(released.data().held_triggers.is_empty());
    assert!(restart(&released).data().retrying_triggers.is_empty());
    for blocked in ["no tick", "owner", "typing", "hours", "mute", "gap"] {
        let (mut calls, day) = recovered();
        let mut now = at;
        let day = apply_delivery_guard(day, &mut now, blocked).unwrap();
        let update = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            bunshin_core::checkin::calls::CallContext {
                is_tick: blocked != "no tick",
                owner_waiting: blocked == "owner",
                input_has_text: blocked == "typing",
                ..context(now)
            },
            fixed,
        );
        assert!(update.request.is_none(), "{blocked}");
        assert_eq!(update.day.data().held_triggers, vec![look.clone()]);
        assert_eq!(update.day.data().retrying_triggers, vec![look.clone()]);
    }
    let (mut calls, day) = recovered();
    let tasks = day.tasks().to_vec();
    let retry = calls.prepare(day, &owner, ContextExtras::default(), context(at), fixed);
    let spent = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Err(ModelError::Malformed),
        context(at),
        fixed,
    );
    assert!(spent.day.data().held_triggers.is_empty());
    assert!(spent.day.data().retrying_triggers.is_empty());
    assert_eq!(
        spent.day.data().next_planned_look,
        at.local
            .checked_add(jiff::SignedDuration::from_mins(120))
            .ok()
    );
    assert_eq!(spent.day.tasks(), tasks);
}

#[test]
fn a_restored_held_trigger_that_never_failed_keeps_its_first_attempt() {
    use bunshin_core::{
        ModelError,
        checkin::calls::{CallContext, CheckinCalls},
    };
    let (day, owner, now) = untimed().unwrap();
    let look = planned_look(now.instant);
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(&day, tick_batch(vec![look.clone()]));
    let held = calls.prepare(
        day,
        &owner,
        ContextExtras::default(),
        CallContext {
            owner_waiting: true,
            ..context(now)
        },
        fixed,
    );
    assert!(held.request.is_none());
    let restored = restart(&held.day);
    assert_eq!(restored.data().held_triggers, vec![look.clone()]);
    assert!(restored.data().retrying_triggers.is_empty());
    let mut fresh = CheckinCalls::new(Tuning::default());
    fresh.enqueue(&restored, tick_batch(vec![look.clone()]));
    let first = fresh.prepare(
        restored,
        &owner,
        ContextExtras::default(),
        CallContext {
            is_tick: false,
            ..context(now)
        },
        fixed,
    );
    let failed = fresh.finish(
        first.day,
        first.request.expect("a first attempt needs no tick").id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    assert_eq!(failed.day.data().held_triggers, vec![look.clone()]);
    assert_eq!(failed.day.data().retrying_triggers, vec![look]);
}

#[test]
fn a_restored_retry_record_survives_dispatch_and_a_guarded_completion_until_delivery() {
    use bunshin_core::{
        ModelAnswer,
        checkin::calls::{CallContext, CheckinCalls, CheckinEffect},
    };
    let (day, owner, now) = untimed().unwrap();
    let look = planned_look(now.instant);
    let restored = restart(&fail_once(day, &owner, now, &look));
    let at = after(now, 60);
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(&restored, tick_batch(vec![look.clone()]));
    let dispatched = calls.prepare(
        restored,
        &owner,
        ContextExtras::default(),
        context(at),
        fixed,
    );
    let id = dispatched.request.unwrap().id;
    assert_eq!(
        restart(&dispatched.day).data().retrying_triggers,
        vec![look.clone()]
    );
    let waiting = calls.finish(
        dispatched.day,
        id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        CallContext {
            input_has_text: true,
            ..context(at)
        },
        fixed,
    );
    assert!(waiting.request.is_none());
    assert_eq!(waiting.day.data().held_triggers, vec![look.clone()]);
    assert_eq!(
        restart(&waiting.day).data().retrying_triggers,
        vec![look.clone()]
    );
    let delivered = calls.prepare(
        waiting.day,
        &owner,
        ContextExtras::default(),
        context(at),
        fixed,
    );
    assert!(delivered.request.is_none());
    assert!(delivered.day.data().held_triggers.is_empty());
    assert!(delivered.day.data().retrying_triggers.is_empty());
    assert!(delivered.effects.contains(&CheckinEffect::Save));
}

#[test]
fn a_fresh_event_merged_into_a_restored_retry_keeps_its_own_retry() {
    use bunshin_core::{
        ModelError,
        checkin::calls::{CallContext, CheckinCalls},
    };
    let (day, owner, now) = untimed().unwrap();
    let look = planned_look(now.instant);
    let restored = restart(&fail_once(day, &owner, now, &look));
    let at = after(now, 60);
    let fresh_look = planned_look(at.instant);
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(
        &restored,
        tick_batch(vec![look.clone(), fresh_look.clone()]),
    );
    let waiting = calls.prepare(
        restored,
        &owner,
        ContextExtras::default(),
        CallContext {
            is_tick: false,
            ..context(at)
        },
        fixed,
    );
    assert!(
        waiting.request.is_none(),
        "the merged batch waits for a tick"
    );
    let retry = calls.prepare(
        waiting.day,
        &owner,
        ContextExtras::default(),
        context(at),
        fixed,
    );
    let failed = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(at),
        fixed,
    );
    assert_eq!(failed.day.data().held_triggers, vec![fresh_look.clone()]);
    assert_eq!(failed.day.data().retrying_triggers, vec![fresh_look]);
    let later = after(at, 60);
    let retry = calls.prepare(
        restart(&failed.day),
        &owner,
        ContextExtras::default(),
        context(later),
        fixed,
    );
    let spent = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(later),
        fixed,
    );
    assert!(spent.day.data().held_triggers.is_empty());
    assert!(spent.day.data().retrying_triggers.is_empty());
}

#[test]
fn unavailability_does_not_spend_a_restored_retry() {
    use bunshin_core::{
        Availability, ModelError, UnavailableReason,
        checkin::calls::{CallContext, CallError, CheckinCalls},
    };
    let (day, owner, now) = untimed().unwrap();
    let look = planned_look(now.instant);
    let restored = restart(&fail_once(day, &owner, now, &look));
    let at = after(now, 60);
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(&restored, tick_batch(vec![look.clone()]));
    let unavailable = calls.prepare(
        restored,
        &owner,
        ContextExtras::default(),
        CallContext {
            availability: Availability::Unavailable(UnavailableReason::NotInstalled),
            ..context(at)
        },
        fixed,
    );
    assert!(unavailable.request.is_none());
    assert!(matches!(
        unavailable.error,
        Some(CallError::Model(ModelError::Unavailable(_)))
    ));
    assert_eq!(unavailable.day.data().held_triggers, vec![look.clone()]);
    assert_eq!(unavailable.day.data().retrying_triggers, vec![look]);
    let retry = calls.prepare(
        unavailable.day,
        &owner,
        ContextExtras::default(),
        context(at),
        fixed,
    );
    let spent = calls.finish(
        retry.day,
        retry.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(at),
        fixed,
    );
    assert!(spent.day.data().held_triggers.is_empty());
    assert!(spent.day.data().retrying_triggers.is_empty());
}

#[test]
fn checkin_retry_waits_for_active_hours_mute_and_delivery_gap_to_clear() {
    use bunshin_core::{
        ModelError,
        checkin::{
            BatchReason, ReadyBatch,
            calls::{CheckinCalls, CheckinEffect},
        },
    };
    let (day, owner, _, mut now) = fixture(TriggerKind::AfterDeadline).unwrap();
    now.local = now.local.date().at(21, 59, 0, 0);
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(
        &day,
        ReadyBatch {
            reason: BatchReason::Tick,
            triggers: vec![
                Trigger {
                    kind: TriggerKind::AfterDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
                Trigger {
                    kind: TriggerKind::PlannedLook,
                    task: None,
                    due_at: now.instant,
                },
            ],
        },
    );
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let done = calls.finish(
        start.day,
        start.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    assert_eq!(done.effects, vec![CheckinEffect::Bell, CheckinEffect::Save]);
    let start = calls.prepare(
        done.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    assert!(
        start.request.is_none(),
        "the deadline fallback starts the minimum gap"
    );
    now.instant.0 += 60_000;
    now.local = now.local.date().at(22, 0, 0, 0);
    let night = calls.prepare(
        start.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    assert!(night.request.is_none());
    now.instant.0 += 600_000;
    now.local = now.local.date().at(8, 0, 0, 0);
    let day = night
        .day
        .mute(UnixMillis(now.instant.0 + 60_000), now.instant)
        .0;
    let muted = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    assert!(muted.request.is_none());
    now.instant.0 += 60_000;
    assert!(
        calls
            .prepare(
                muted.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed
            )
            .request
            .is_some()
    );
}

#[test]
fn checkin_previous_day_worker_remains_busy_until_its_matching_completion() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{BatchReason, ReadyBatch},
    };
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let old = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let old_id = old.request.unwrap().id;
    let new = Day::new(jiff::civil::date(2026, 10, 4), Tuning::default());
    calls.enqueue(
        &new,
        ReadyBatch {
            reason: BatchReason::DayStart,
            triggers: vec![Trigger {
                kind: TriggerKind::DayStart,
                task: None,
                due_at: now.instant,
            }],
        },
    );
    let waiting = calls.prepare(new, &owner, ContextExtras::default(), context(now), fixed);
    assert!(waiting.request.is_none());
    let released = calls.finish(
        waiting.day,
        old_id,
        Ok(ModelAnswer {
            json: r#"{"kind":"note","message":"yesterday"}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(released.effects.is_empty());
    assert!(released.day.messages().is_empty());
    assert!(
        calls
            .prepare(
                released.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed
            )
            .request
            .is_some()
    );
}

#[test]
fn checkin_open_and_sleep_batches_preserve_catchup_on_both_model_and_fixed_delivery() {
    use bunshin_core::{
        ModelAnswer, ModelError,
        checkin::{BatchReason, ReadyBatch, calls::CheckinCalls},
    };
    for reason in [BatchReason::Open, BatchReason::Sleep] {
        for result in [
            Ok(ModelAnswer {
                json: r#"{"kind":"note","task":1,"message":"確認"}"#.into(),
            }),
            Err(ModelError::TimedOut),
        ] {
            let (day, owner, _, now) = fixture(TriggerKind::AfterDeadline).unwrap();
            let mut calls = CheckinCalls::new(Tuning::default());
            calls.enqueue(
                &day,
                ReadyBatch {
                    reason,
                    triggers: vec![Trigger {
                        kind: TriggerKind::AfterDeadline,
                        task: Some(1),
                        due_at: now.instant,
                    }],
                },
            );
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let update = calls.finish(
                start.day,
                start.request.unwrap().id,
                result,
                context(now),
                fixed,
            );
            let row = update.day.messages().last().unwrap();
            assert_eq!(
                row.unprompted.as_ref().unwrap().trigger.kind,
                TriggerKind::CatchUp
            );
            assert_eq!(row.unprompted.as_ref().unwrap().task, Some(1));
        }
    }
}

#[test]
fn checkin_fixed_batch_emits_one_bell_for_each_delivered_row_and_none_for_suppression() {
    use bunshin_core::{
        ModelError,
        checkin::{
            BatchReason, ReadyBatch,
            calls::{CheckinCalls, CheckinEffect},
        },
    };
    let (day, owner, _, now) = fixture(TriggerKind::AfterDeadline).unwrap();
    let day = day
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some(jiff::civil::time(15, 0, 0, 0)),
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap()
        .0;
    let mut calls = CheckinCalls::new(Tuning::default());
    calls.enqueue(
        &day,
        ReadyBatch {
            reason: BatchReason::Sleep,
            triggers: vec![
                Trigger {
                    kind: TriggerKind::BeforeDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
                Trigger {
                    kind: TriggerKind::AfterDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
                Trigger {
                    kind: TriggerKind::AfterDeadline,
                    task: Some(2),
                    due_at: now.instant,
                },
            ],
        },
    );
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let update = calls.finish(
        start.day,
        start.request.unwrap().id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    assert_eq!(
        update.effects,
        vec![
            CheckinEffect::Bell,
            CheckinEffect::Bell,
            CheckinEffect::Save
        ]
    );
    assert_eq!(
        update
            .day
            .messages()
            .iter()
            .filter(|m| m
                .unprompted
                .as_ref()
                .is_some_and(|u| u.suppressed.is_none()))
            .count(),
        2
    );
}

#[test]
fn checkin_request_retains_all_fifty_tasks_and_both_deadline_triggers_within_window() {
    let tuning = Tuning::default();
    let now = Now {
        instant: UnixMillis(0),
        local: jiff::civil::date(2026, 10, 3).at(15, 1, 0, 0),
    };
    let mut day = Day::new(now.local.date(), tuning);
    let mut triggers = Vec::new();
    for number in 1..=50 {
        day = day
            .add(
                "題".repeat(80),
                TaskKind::Deadline,
                Some(jiff::civil::time(15, 0, 0, 0)),
                TaskOrigin::Key,
                now.instant,
            )
            .unwrap()
            .0;
        for kind in [TriggerKind::BeforeDeadline, TriggerKind::AfterDeadline] {
            triggers.push(Trigger {
                kind,
                task: Some(number),
                due_at: now.instant,
            });
        }
    }
    let owner =
        InstructionsState::resolve(Some(&"指".repeat(600)), "instructions.md".into(), tuning);
    let built = build_checkin(
        &day,
        &owner,
        now,
        &triggers,
        ContextExtras::default(),
        tuning,
    )
    .expect("largest supported deadline batch fits");
    let json: serde_json::Value = serde_json::from_str(&built.request.prompt).unwrap();
    assert_eq!(json["openTasks"].as_array().unwrap().len(), 50);
    assert_eq!(json["triggers"].as_array().unwrap().len(), 100);
    assert_eq!(json["triggers"][0], serde_json::json!(["b", 1]));
    assert_eq!(json["triggers"][1], serde_json::json!(["a", 1]));
    assert!(built.budget.estimated_tokens <= 4096);
    assert_eq!(built.request.timeout, std::time::Duration::from_secs(30));
    assert!(!built.request.schema.contains("changes"));
    assert_eq!(day.tasks()[49].title, "題".repeat(80));
}

#[test]
fn checkin_answer_is_strict_clamps_look_and_drops_only_unknown_task_reference() {
    use bunshin_core::{
        ModelError,
        checkin::answer::{CheckinKind, parse_checkin},
    };
    let tuning = Tuning::default();
    let now = UnixMillis(0);
    let day = Day::new(jiff::civil::date(2026, 10, 3), tuning)
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some(jiff::civil::time(15, 0, 0, 0)),
            TaskOrigin::Key,
            now,
        )
        .unwrap()
        .0;
    for (proposal, expected) in [
        ("4", 5),
        ("121", 120),
        ("-9", 5),
        ("999999999999999999999", 120),
    ] {
        let json = format!(
            "{{\"kind\":\"question\",\"task\":1,\"message\":\"どこまで？\",\"next_look_minutes\":{proposal}}}"
        );
        let answer = parse_checkin(&json, &day, tuning).unwrap();
        assert_eq!(answer.kind, CheckinKind::Question);
        assert_eq!(answer.task, Some(1));
        assert_eq!(answer.next_look_minutes, expected);
    }
    let answer = parse_checkin(
        r#"{"kind":"note","task":90,"message":"確認"}"#,
        &day,
        tuning,
    )
    .unwrap();
    assert_eq!(answer.task, None);
    assert_eq!(answer.next_look_minutes, 120);
    for json in [
        "not json",
        r#"{"kind":"no","message":"x"}"#,
        r#"{"kind":"silent"}"#,
        r#"{"kind":"note","message":3}"#,
        r#"{"kind":"note","message":"x","task":null}"#,
        r#"{"kind":"note","message":"x","next_look_minutes":2.5}"#,
        r#"{"kind":"note","message":"x","changes":[]}"#,
    ] {
        assert_eq!(
            parse_checkin(json, &day, tuning),
            Err(ModelError::Malformed)
        );
    }
}

#[test]
fn checkin_queue_waits_behind_owner_and_delivers_one_question_without_changing_tasks_or_undo() {
    use bunshin_core::{
        Availability, CancelFlag, LanguageModel, ModelAnswer,
        checkin::{
            BatchReason, ReadyBatch,
            calls::{CallContext, CheckinCalls, CheckinEffect},
        },
    };
    use bunshin_test_support::ScriptedLanguageModel;
    let tuning = Tuning::default();
    let now = Now {
        instant: UnixMillis(0),
        local: jiff::civil::date(2026, 10, 3).at(14, 30, 0, 0),
    };
    let day = Day::new(now.local.date(), tuning)
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some(jiff::civil::time(15, 0, 0, 0)),
            TaskOrigin::Key,
            now.instant,
        )
        .unwrap()
        .0;
    let before = day.tasks().to_vec();
    let expected_undo = day.clone().undo(UnixMillis(1)).unwrap().0.tasks().to_vec();
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let mut calls = CheckinCalls::new(tuning);
    calls.enqueue(
        &day,
        ReadyBatch {
            triggers: vec![Trigger {
                kind: TriggerKind::BeforeDeadline,
                task: Some(1),
                due_at: now.instant,
            }],
            reason: BatchReason::Tick,
        },
    );
    let owner_context = CallContext {
        now,
        owner_waiting: true,
        input_has_text: false,
        is_tick: false,
        availability: Availability::Available,
    };
    let waiting = calls.prepare(day, &owner, ContextExtras::default(), owner_context, fixed);
    assert!(waiting.request.is_none());
    assert_eq!(waiting.effects, vec![CheckinEffect::Save]);
    assert_eq!(waiting.day.data().held_triggers.len(), 1);
    let ready = calls.prepare(
        waiting.day,
        &owner,
        ContextExtras::default(),
        CallContext {
            owner_waiting: false,
            ..owner_context
        },
        fixed,
    );
    let request = ready.request.unwrap();
    let busy = calls.prepare(
        ready.day,
        &owner,
        ContextExtras::default(),
        CallContext {
            owner_waiting: false,
            ..owner_context
        },
        fixed,
    );
    assert!(busy.request.is_none());
    let model=ScriptedLanguageModel::new([Ok(ModelAnswer{json:r#"{"kind":"question","task":1,"message":"資料、どこまで進んだ？","next_look_minutes":30}"#.into()})]);
    let result = model.respond(&request.request, &CancelFlag::default());
    let update = calls.finish(busy.day, request.id, result, context(now), fixed);
    assert_eq!(model.requests().len(), 1);
    assert_eq!(update.day.tasks(), before);
    assert_eq!(
        update.effects,
        vec![CheckinEffect::Bell, CheckinEffect::Save]
    );
    let row = update.day.messages().last().unwrap();
    assert_eq!(row.text, "資料、どこまで進んだ？");
    let extra = row.unprompted.as_ref().unwrap();
    assert_eq!(extra.kind, bunshin_core::day::UnpromptedKind::Question);
    assert_eq!(extra.inbox_state, bunshin_core::day::InboxState::Open);
    assert_eq!(extra.task, Some(1));
    assert_eq!(extra.trigger.kind, TriggerKind::BeforeDeadline);
    assert_eq!(
        update.day.data().next_planned_look,
        Some(
            now.local
                .checked_add(jiff::SignedDuration::from_mins(30))
                .unwrap()
        )
    );
    assert_eq!(
        update.day.undo(UnixMillis(1)).unwrap().0.tasks(),
        expected_undo
    );
}

fn fixture(
    kind: TriggerKind,
) -> Result<
    (
        Day,
        InstructionsState,
        bunshin_core::checkin::calls::CheckinCalls,
        Now,
    ),
    bunshin_core::day::DayError,
> {
    let tuning = Tuning::default();
    let mut now = Now {
        instant: UnixMillis(0),
        local: jiff::civil::date(2026, 10, 3).at(14, 30, 0, 0),
    };
    if kind == TriggerKind::AfterDeadline {
        now.local = now.local.date().at(15, 0, 1, 0);
    }
    let day = Day::new(now.local.date(), tuning)
        .add(
            "資料作成".into(),
            TaskKind::Deadline,
            Some(jiff::civil::time(15, 0, 0, 0)),
            TaskOrigin::Key,
            now.instant,
        )?
        .0;
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    let mut calls = bunshin_core::checkin::calls::CheckinCalls::new(tuning);
    queue(&mut calls, &day, kind, now.instant);
    Ok((day, owner, calls, now))
}
fn queue(
    calls: &mut bunshin_core::checkin::calls::CheckinCalls,
    day: &Day,
    kind: TriggerKind,
    at: UnixMillis,
) {
    calls.enqueue(
        day,
        bunshin_core::checkin::ReadyBatch {
            triggers: vec![Trigger {
                kind,
                task: Some(1),
                due_at: at,
            }],
            reason: bunshin_core::checkin::BatchReason::Tick,
        },
    );
}
fn untimed() -> Result<(Day, InstructionsState, Now), bunshin_core::day::DayError> {
    let tuning = Tuning::default();
    let now = Now {
        instant: UnixMillis(0),
        local: jiff::civil::date(2026, 10, 3).at(14, 30, 0, 0),
    };
    let day = Day::new(now.local.date(), tuning)
        .add(
            "資料作成".into(),
            TaskKind::Untimed,
            None,
            TaskOrigin::Key,
            now.instant,
        )?
        .0;
    let owner = InstructionsState::resolve(None, "instructions.md".into(), tuning);
    Ok((day, owner, now))
}
const fn planned_look(at: UnixMillis) -> Trigger {
    Trigger {
        kind: TriggerKind::PlannedLook,
        task: None,
        due_at: at,
    }
}
const fn tick_batch(triggers: Vec<Trigger>) -> bunshin_core::checkin::ReadyBatch {
    bunshin_core::checkin::ReadyBatch {
        triggers,
        reason: bunshin_core::checkin::BatchReason::Tick,
    }
}
fn after(now: Now, seconds: i64) -> Now {
    Now {
        instant: UnixMillis(now.instant.0 + seconds * 1_000),
        local: now
            .local
            .saturating_add(jiff::SignedDuration::from_secs(seconds)),
    }
}
/// A save and a reload through JSON, as a restart does.
fn restart(day: &Day) -> Day {
    use bunshin_core::day::file::DayFile;
    let reloaded = serde_json::to_string(&DayFile::from(day))
        .and_then(|json| serde_json::from_str::<DayFile>(&json));
    match reloaded.map(|file| file.into_day(Tuning::default())) {
        Ok(Ok(day)) => day,
        Ok(Err(error)) => panic!("reload refused: {error:?}"),
        Err(error) => panic!("JSON round trip failed: {error}"),
    }
}
/// Dispatch `look` once and fail it, leaving it held for its one retry.
fn fail_once(day: Day, owner: &InstructionsState, now: Now, look: &Trigger) -> Day {
    let mut calls = bunshin_core::checkin::calls::CheckinCalls::new(Tuning::default());
    calls.enqueue(&day, tick_batch(vec![look.clone()]));
    let start = calls.prepare(day, owner, ContextExtras::default(), context(now), fixed);
    let Some(request) = start.request else {
        panic!("the first attempt dispatches");
    };
    calls
        .finish(
            start.day,
            request.id,
            Err(bunshin_core::ModelError::TimedOut),
            context(now),
            fixed,
        )
        .day
}
fn context(now: Now) -> bunshin_core::checkin::calls::CallContext {
    bunshin_core::checkin::calls::CallContext {
        now,
        owner_waiting: false,
        input_has_text: false,
        is_tick: true,
        availability: bunshin_core::Availability::Available,
    }
}
fn fixed(note: &bunshin_core::checkin::calls::FixedDeadline) -> String {
    assert_eq!(note.task.title, "資料作成");
    assert_eq!(note.task.time, Some(jiff::civil::time(15, 0, 0, 0)));
    format!("fixed {:?}", note.trigger.kind)
}
#[test]
fn checkin_silent_only_plans_and_note_preserves_complete_model_text() {
    use bunshin_core::{ModelAnswer, checkin::calls::CheckinEffect};
    for (kind, expected) in [
        ("silent", vec![CheckinEffect::Save]),
        ("note", vec![CheckinEffect::Bell, CheckinEffect::Save]),
    ] {
        let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
        let before = day.tasks().to_vec();
        let count = day.messages().len();
        let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
        let request = start.request.unwrap();
        let text = "全".repeat(1000);
        let json = serde_json::json!({"kind":kind,"task":999,"message":text}).to_string();
        let update = calls.finish(
            start.day,
            request.id,
            Ok(ModelAnswer { json }),
            context(now),
            fixed,
        );
        assert_eq!(update.effects, expected);
        assert_eq!(update.day.tasks(), before);
        assert_eq!(
            update.day.data().next_planned_look,
            Some(
                now.local
                    .checked_add(jiff::SignedDuration::from_mins(120))
                    .unwrap()
            )
        );
        if kind == "silent" {
            assert_eq!(update.day.messages().len(), count);
            assert_eq!(update.day.data().last_unprompted_at, None);
        } else {
            let row = update.day.messages().last().unwrap();
            assert_eq!(row.text, text);
            assert_eq!(row.unprompted.as_ref().unwrap().task, None);
            assert_eq!(update.day.data().last_unprompted_at, Some(now.instant));
        }
    }
}
#[test]
fn checkin_every_model_failure_posts_both_deadline_kinds_as_fixed_notes() {
    use bunshin_core::{
        ModelError, UnavailableReason,
        checkin::calls::{CallError, CheckinEffect},
    };
    for kind in [TriggerKind::BeforeDeadline, TriggerKind::AfterDeadline] {
        for error in [
            ModelError::Unavailable(UnavailableReason::NotInstalled),
            ModelError::TimedOut,
            ModelError::Cancelled,
            ModelError::Refused,
            ModelError::Malformed,
            ModelError::Failed,
        ] {
            let (day, owner, mut calls, now) = fixture(kind).unwrap();
            let before = day.tasks().to_vec();
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
            let id = start.request.unwrap().id;
            let update = calls.finish(start.day, id, Err(error), context(now), fixed);
            assert_eq!(update.error, Some(CallError::Model(error)));
            assert_eq!(
                update.effects,
                vec![CheckinEffect::Bell, CheckinEffect::Save]
            );
            assert_eq!(update.day.tasks(), before);
            let row = update.day.messages().last().unwrap();
            assert_eq!(row.text, format!("fixed {kind:?}"));
            assert_eq!(
                row.unprompted.as_ref().unwrap().kind,
                bunshin_core::day::UnpromptedKind::Note
            );
            assert!(
                calls
                    .prepare(
                        update.day,
                        &owner,
                        ContextExtras::default(),
                        context(now),
                        fixed
                    )
                    .request
                    .is_none()
            );
        }
    }
}
#[test]
fn checkin_unavailable_model_holds_other_triggers_and_recovers_without_duplicate_calls() {
    use bunshin_core::{
        Availability, ModelAnswer, UnavailableReason, checkin::calls::CheckinEffect,
    };
    let (day, owner, mut calls, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    queue(&mut calls, &day, TriggerKind::PlannedLook, now.instant);
    let unavailable = bunshin_core::checkin::calls::CallContext {
        availability: Availability::Unavailable(UnavailableReason::TermsNotAccepted),
        ..context(now)
    };
    let first = calls.prepare(day, &owner, ContextExtras::default(), unavailable, fixed);
    assert!(first.request.is_none());
    assert_eq!(
        first.effects,
        vec![CheckinEffect::Bell, CheckinEffect::Save]
    );
    let waiting = calls.prepare(
        first.day,
        &owner,
        ContextExtras::default(),
        unavailable,
        fixed,
    );
    assert!(waiting.request.is_none());
    assert_eq!(waiting.day.data().held_triggers.len(), 1);
    queue(
        &mut calls,
        &waiting.day,
        TriggerKind::PlannedLook,
        now.instant,
    );
    let recovered_now = Now {
        instant: UnixMillis(now.instant.0 + 300_000),
        ..now
    };
    let recovered = calls.prepare(
        waiting.day,
        &owner,
        ContextExtras::default(),
        context(recovered_now),
        fixed,
    );
    let id = recovered.request.unwrap().id;
    assert_eq!(
        recovered.day.data().held_triggers,
        vec![Trigger {
            kind: TriggerKind::PlannedLook,
            task: Some(1),
            due_at: now.instant
        }]
    );
    let done = calls.finish(
        recovered.day,
        id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(
        calls
            .prepare(
                done.day,
                &owner,
                ContextExtras::default(),
                context(now),
                fixed
            )
            .request
            .is_none()
    );
}
#[test]
fn checkin_other_failures_retry_once_at_next_tick_and_then_schedule_default_look() {
    use bunshin_core::{ModelError, checkin::calls::CheckinEffect};
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let id = start.request.unwrap().id;
    let failed = calls.finish(
        start.day,
        id,
        Err(ModelError::TimedOut),
        context(now),
        fixed,
    );
    assert_eq!(failed.effects, vec![CheckinEffect::Save]);
    assert_eq!(failed.day.data().held_triggers.len(), 1);
    assert_eq!(
        failed.day.data().retrying_triggers,
        failed.day.data().held_triggers
    );
    let early = Now {
        instant: UnixMillis(59_999),
        ..now
    };
    let waiting = calls.prepare(
        failed.day,
        &owner,
        ContextExtras::default(),
        bunshin_core::checkin::calls::CallContext {
            is_tick: false,
            ..context(early)
        },
        fixed,
    );
    assert!(waiting.request.is_none());
    let tick = Now {
        instant: UnixMillis(60_000),
        local: now
            .local
            .checked_add(jiff::SignedDuration::from_mins(1))
            .unwrap(),
    };
    let retry = calls.prepare(
        waiting.day,
        &owner,
        ContextExtras::default(),
        context(tick),
        fixed,
    );
    let id = retry.request.unwrap().id;
    let spent = calls.finish(
        retry.day,
        id,
        Err(ModelError::Refused),
        context(tick),
        fixed,
    );
    assert_eq!(spent.effects, vec![CheckinEffect::Save]);
    assert_eq!(spent.day.messages().len(), 1);
    assert_eq!(
        spent.day.data().next_planned_look,
        Some(
            tick.local
                .checked_add(jiff::SignedDuration::from_mins(120))
                .unwrap()
        )
    );
    assert!(
        calls
            .prepare(
                spent.day,
                &owner,
                ContextExtras::default(),
                context(tick),
                fixed
            )
            .request
            .is_none()
    );
}
#[test]
fn checkin_same_task_suppression_does_not_ring_or_extend_delivered_time() {
    use bunshin_core::{ModelAnswer, checkin::calls::CheckinEffect, day::SuppressionReason};
    let (mut day, owner, mut calls, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    for (index, at) in [0, 1_799_999, 1_800_000].into_iter().enumerate() {
        let current = Now {
            instant: UnixMillis(at),
            ..now
        };
        if index > 0 {
            queue(&mut calls, &day, TriggerKind::PlannedLook, current.instant);
        }
        let start = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            context(current),
            fixed,
        );
        let id = start.request.unwrap().id;
        let result = calls.finish(
            start.day,
            id,
            Ok(ModelAnswer {
                json: r#"{"kind":"question","task":1,"message":"確認"}"#.into(),
            }),
            context(current),
            fixed,
        );
        let extra = result
            .day
            .messages()
            .last()
            .unwrap()
            .unprompted
            .as_ref()
            .unwrap();
        if index == 1 {
            assert_eq!(extra.suppressed, Some(SuppressionReason::SameTask));
            assert_eq!(result.effects, vec![CheckinEffect::Save]);
            assert_eq!(result.day.data().last_unprompted_at, Some(UnixMillis(0)));
            assert!(result.day.messages().last().unwrap().text.is_empty());
        } else {
            assert_eq!(extra.suppressed, None);
            assert_eq!(
                result.effects,
                vec![CheckinEffect::Bell, CheckinEffect::Save]
            );
            assert_eq!(result.day.data().last_unprompted_at, Some(current.instant));
        }
        day = result.day;
    }
}
#[test]
fn checkin_old_tokens_and_previous_day_completions_preserve_current_flight() {
    use bunshin_core::ModelAnswer;
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let id = start.request.unwrap().id;
    let wrong = calls.finish(
        start.day,
        id + 1,
        Ok(ModelAnswer { json: "bad".into() }),
        context(now),
        fixed,
    );
    assert!(wrong.effects.is_empty());
    assert!(wrong.error.is_none());
    let done = calls.finish(
        wrong.day,
        id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(done.error.is_none());
    queue(
        &mut calls,
        &done.day,
        TriggerKind::PlannedLook,
        UnixMillis(1),
    );
    let old = calls.prepare(
        done.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let old_id = old.request.unwrap().id;
    let next = Day::new(now.local.date().tomorrow().unwrap(), Tuning::default());
    queue(&mut calls, &next, TriggerKind::PlannedLook, now.instant);
    let waiting = calls.prepare(next, &owner, ContextExtras::default(), context(now), fixed);
    assert!(waiting.request.is_none());
    let released = calls.finish(
        waiting.day,
        old_id,
        Ok(ModelAnswer { json: "bad".into() }),
        context(now),
        fixed,
    );
    assert!(released.effects.is_empty());
    let fresh = calls.prepare(
        released.day,
        &owner,
        ContextExtras::default(),
        context(now),
        fixed,
    );
    let new_id = fresh.request.unwrap().id;
    let ignored = calls.finish(
        fresh.day,
        old_id,
        Ok(ModelAnswer { json: "bad".into() }),
        context(now),
        fixed,
    );
    assert!(ignored.effects.is_empty());
    let done = calls.finish(
        ignored.day,
        new_id,
        Ok(ModelAnswer {
            json: r#"{"kind":"silent","message":""}"#.into(),
        }),
        context(now),
        fixed,
    );
    assert!(done.error.is_none());
    assert_eq!(done.day.date(), now.local.date().tomorrow().unwrap());
}

#[test]
fn checkin_retry_uses_the_next_scheduler_tick_after_a_slow_failure() {
    use bunshin_core::ModelError;
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let id = start.request.unwrap().id;
    let finished = Now {
        instant: UnixMillis(30_000),
        ..now
    };
    let failed = calls.finish(
        start.day,
        id,
        Err(ModelError::TimedOut),
        context(finished),
        fixed,
    );
    let tick = Now {
        instant: UnixMillis(60_000),
        ..now
    };
    let next = calls.prepare(
        failed.day,
        &owner,
        ContextExtras::default(),
        context(tick),
        fixed,
    );
    assert!(
        next.request.is_some(),
        "the next tick is measured by the scheduler, rather than a full minute after completion"
    );
}

#[test]
fn checkin_integer_task_references_outside_the_domain_are_dropped_without_rejecting_the_note() {
    let (day, _, _, _) = fixture(TriggerKind::PlannedLook).unwrap();
    for task in ["-1", "18446744073709551616"] {
        let json = format!("{{\"kind\":\"note\",\"task\":{task},\"message\":\"確認\"}}");
        let answer = bunshin_core::checkin::answer::parse_checkin(&json, &day, Tuning::default())
            .expect("schema-valid integer");
        assert_eq!(answer.task, None);
        assert_eq!(answer.message, "確認");
    }
}

#[test]
fn malformed_checkin_answer_falls_back_and_closed_tasks_are_not_announced_after_a_pending_call() {
    use bunshin_core::{
        CancelFlag, LanguageModel, ModelAnswer, ModelError,
        checkin::calls::{CallError, CheckinEffect},
    };
    use bunshin_test_support::ScriptedLanguageModel;
    for close in [false, true] {
        let (day, owner, mut calls, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
        let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
        let request = start.request.unwrap();
        let day = if close {
            start.day.done(1, UnixMillis(1)).unwrap().0
        } else {
            start.day
        };
        let before = day.tasks().to_vec();
        let model = ScriptedLanguageModel::new([Ok(ModelAnswer {
            json: r#"{"kind":"note","message":"秘密","changes":[{"op":"done","task":1}]}"#.into(),
        })]);
        let result = model.respond(&request.request, &CancelFlag::default());
        let update = calls.finish(day, request.id, result, context(now), fixed);
        assert_eq!(update.error, Some(CallError::Model(ModelError::Malformed)));
        assert_eq!(update.day.tasks(), before);
        assert!(
            update
                .day
                .messages()
                .iter()
                .all(|row| !row.text.contains("秘密"))
        );
        if close {
            assert_eq!(update.effects, vec![CheckinEffect::Save]);
        } else {
            assert_eq!(
                update.effects,
                vec![CheckinEffect::Bell, CheckinEffect::Save]
            );
        }
    }
}
#[test]
fn checkin_prompt_refusal_sends_no_request_and_uses_typed_fallback_facts() {
    use bunshin_core::checkin::calls::{CallError, CheckinCalls, CheckinEffect};
    let (day, owner, _, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    let mut tuning = Tuning::default();
    tuning.prompt.context_tokens = 1;
    let mut calls = CheckinCalls::new(tuning);
    queue(&mut calls, &day, TriggerKind::BeforeDeadline, now.instant);
    let before = day.tasks().to_vec();
    let update = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    assert!(update.request.is_none());
    assert_eq!(
        update.error,
        Some(CallError::Prompt(
            bunshin_core::prompt::chat::PromptError::RequiredContextTooLong
        ))
    );
    assert_eq!(
        update.effects,
        vec![CheckinEffect::Bell, CheckinEffect::Save]
    );
    assert_eq!(update.day.tasks(), before);
}

#[test]
fn suppressed_checkin_rows_never_enter_visible_chat_history_sent_to_the_model() {
    use bunshin_core::{ModelAnswer, prompt::chat::build_chat};
    let (mut day, owner, mut calls, now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    for at in [0, 300_000] {
        let current = Now {
            instant: UnixMillis(at),
            ..now
        };
        if at > 0 {
            queue(&mut calls, &day, TriggerKind::PlannedLook, current.instant);
        }
        let start = calls.prepare(
            day,
            &owner,
            ContextExtras::default(),
            context(current),
            fixed,
        );
        let id = start.request.unwrap().id;
        day = calls
            .finish(
                start.day,
                id,
                Ok(ModelAnswer {
                    json: r#"{"kind":"note","task":1,"message":"確認"}"#.into(),
                }),
                context(current),
                fixed,
            )
            .day;
    }
    let built = build_chat(
        &day,
        &owner,
        "了解",
        now,
        ContextExtras::default(),
        Tuning::default(),
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&built.request.prompt).unwrap();
    assert_eq!(
        json["chat"],
        serde_json::json!([{"author":"bunshin","text":"確認"}])
    );
}

#[test]
fn checkin_overdue_catchup_announces_only_the_current_deadline_phase() {
    use bunshin_core::{Availability, UnavailableReason, checkin::calls::CheckinEffect};
    let (day, owner, mut calls, mut now) = fixture(TriggerKind::BeforeDeadline).unwrap();
    now.local = now.local.date().at(15, 0, 1, 0);
    // One scheduler batch contains the elapsed before/after events for this task.
    let mut calls2 = bunshin_core::checkin::calls::CheckinCalls::new(Tuning::default());
    calls2.enqueue(
        &day,
        bunshin_core::checkin::ReadyBatch {
            reason: bunshin_core::checkin::BatchReason::Open,
            triggers: vec![
                Trigger {
                    kind: TriggerKind::BeforeDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
                Trigger {
                    kind: TriggerKind::AfterDeadline,
                    task: Some(1),
                    due_at: now.instant,
                },
            ],
        },
    );
    let unavailable = bunshin_core::checkin::calls::CallContext {
        availability: Availability::Unavailable(UnavailableReason::NotInstalled),
        ..context(now)
    };
    let update = calls2.prepare(day, &owner, ContextExtras::default(), unavailable, fixed);
    let rows = update
        .day
        .messages()
        .iter()
        .filter(|row| row.unprompted.is_some())
        .collect::<Vec<_>>();
    assert_eq!(rows[0].text, "fixed AfterDeadline");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].unprompted.as_ref().unwrap().suppressed, None);
    assert_eq!(
        update.effects,
        vec![CheckinEffect::Bell, CheckinEffect::Save]
    );
    // The other fixture queue is independent of the consolidated batch.
    assert!(
        calls
            .prepare(
                update.day,
                &owner,
                ContextExtras::default(),
                unavailable,
                fixed
            )
            .request
            .is_none()
    );
}

#[test]
fn checkin_retry_follows_a_rebased_scheduler_tick_after_clock_rollback() {
    use bunshin_core::ModelError;
    let (day, owner, mut calls, now) = fixture(TriggerKind::PlannedLook).unwrap();
    let start = calls.prepare(day, &owner, ContextExtras::default(), context(now), fixed);
    let id = start.request.unwrap().id;
    let failed = calls.finish(
        start.day,
        id,
        Err(ModelError::TimedOut),
        context(Now {
            instant: UnixMillis(30_000),
            ..now
        }),
        fixed,
    );
    // The scheduler has already rebased its clock and now reports an actual tick.
    let rebased = Now {
        instant: UnixMillis(-10_000),
        ..now
    };
    assert!(
        calls
            .prepare(
                failed.day,
                &owner,
                ContextExtras::default(),
                context(rebased),
                fixed
            )
            .request
            .is_some()
    );
}

#[test]
fn a_restored_opening_with_fresh_and_retrying_triggers_yields_one_catch_up() {
    use bunshin_core::{
        ModelAnswer,
        checkin::{BatchReason, ReadyBatch, calls::CheckinCalls},
    };
    for reason in [BatchReason::Open, BatchReason::GuardedOpen] {
        let (day, owner, _, now) = fixture(TriggerKind::PlannedLook).unwrap();
        let look = planned_look(now.instant);
        let restored = restart(&fail_once(day, &owner, now, &look));
        assert_eq!(restored.data().retrying_triggers, vec![look.clone()]);
        let mut calls = CheckinCalls::new(Tuning::default());
        calls.enqueue(
            &restored,
            ReadyBatch {
                reason,
                triggers: vec![
                    Trigger {
                        kind: TriggerKind::BeforeDeadline,
                        task: Some(1),
                        due_at: now.instant,
                    },
                    look.clone(),
                ],
            },
        );
        let mut day = restored;
        for step in 0..4 {
            let at = after(now, step * 31 * 60);
            let start = calls.prepare(day, &owner, ContextExtras::default(), context(at), fixed);
            day = match start.request {
                Some(request) => {
                    calls
                        .finish(
                            start.day,
                            request.id,
                            Ok(ModelAnswer {
                                json: r#"{"kind":"note","task":1,"message":"確認"}"#.into(),
                            }),
                            context(at),
                            fixed,
                        )
                        .day
                }
                None => start.day,
            };
        }
        let catch_ups = day
            .messages()
            .iter()
            .filter(|row| {
                row.unprompted
                    .as_ref()
                    .is_some_and(|u| u.trigger.kind == TriggerKind::CatchUp)
            })
            .count();
        assert_eq!(catch_ups, 1, "{reason:?}");
        assert!(day.data().held_triggers.is_empty(), "{reason:?}");
    }
}
