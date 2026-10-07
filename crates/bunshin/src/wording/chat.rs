//! Chat and model feedback wording, independent of drawing and worker I/O.
use bunshin_core::{
    UnavailableReason,
    instructions::InstructionsOrigin,
    prompt::answer::{ProposalField, RefusalReason},
    screen::ChatNotice,
};
pub const THINKING: &str = "考え中…";
pub const THINKING_ROW: &str = "…";
pub const LONG_WAIT: &str = "まだ考えています（Esc で中止）";
pub const MODEL_UNAVAILABLE: &str = "モデル: 使えません";
pub const OWNER: &str = "あなた";
pub const SYSTEM: &str = "システム";
pub const CHANGE: &str = "変更";
pub const NEW_MESSAGE_DIVIDER: &str = "── ここから新着 ──";
pub const CANCELLED_MARK: &str = "（中止）";
const MUTE_GUIDE: &str = "（m で解除）";
pub const INSTRUCTIONS_TITLE: &str = " 指示文（読み取り専用） ";
pub const INSTRUCTIONS_EDIT: &str =
    "編集: 端末で bunshin instructions edit（次の呼び出しから反映）";
pub const INSTRUCTIONS_FOOTER: &str = "↑↓ スクロール  Esc/p 閉じる";
pub const INSTRUCTIONS_DEFAULT: &str = " 既定を使用中 ";
pub fn instructions_path(path: &std::path::Path) -> String {
    format!("ファイル: {}", path.display())
}
pub fn input_title(question_time: Option<&str>) -> String {
    question_time.map_or_else(
        || super::INPUT_TITLE.into(),
        |time| format!(" 入力（{time} の質問への返事） "),
    )
}
pub fn chat_timestamp(time: Option<bunshin_core::Now>) -> String {
    time.map_or_else(
        || "--:--".into(),
        |time| format!("{:02}:{:02}", time.local.hour(), time.local.minute()),
    )
}
pub fn chat_changes(
    set: &bunshin_core::day::ChangeSet,
    local_at: &dyn Fn(bunshin_core::UnixMillis) -> Option<bunshin_core::Now>,
) -> String {
    use bunshin_core::day::Change;
    let rows = set
        .changes
        .iter()
        .map(|change| match change {
            Change::Task { before, after } => task_change(before.as_ref(), after.as_ref()),
            Change::Mute { before: _, after } => after.map_or_else(
                || "ミュート解除".into(),
                |at| {
                    let guide = if set.undo { "" } else { MUTE_GUIDE };
                    format!("ミュート 〜{}{guide}", chat_timestamp(local_at(at)))
                },
            ),
        })
        .collect::<Vec<_>>()
        .join(" / ");
    if set.undo {
        format!("取り消し: {rows}")
    } else {
        rows
    }
}
fn task_change(
    before: Option<&bunshin_core::day::Task>,
    after: Option<&bunshin_core::day::Task>,
) -> String {
    use bunshin_core::day::TaskKind;
    match (before, after) {
        (None, Some(task)) => {
            let kind = match task.kind {
                TaskKind::Untimed => "",
                TaskKind::Deadline => "（締切）",
                TaskKind::Appointment => "（予定）",
            };
            format!("+ {} {}{}{kind}", task.number, task_clock(task), task.title)
        }
        (Some(task), None) => format!("削除 {} {}", task.number, task.title),
        (Some(before), Some(after)) => {
            if before.status != after.status {
                return format!(
                    "{} {} {}",
                    match after.status {
                        bunshin_core::day::TaskStatus::Done => "x",
                        bunshin_core::day::TaskStatus::Dropped => "-",
                        bunshin_core::day::TaskStatus::CarriedOver => ">",
                        bunshin_core::day::TaskStatus::Open => "~",
                    },
                    after.number,
                    after.title
                );
            }
            let title = if before.title == after.title {
                after.title.clone()
            } else {
                format!("{} → {}", before.title, after.title)
            };
            let clock = if before.kind == after.kind && before.time == after.time {
                String::new()
            } else {
                format!(
                    "{} → {} ",
                    task_clock(before).trim_end(),
                    task_clock(after).trim_end()
                )
            };
            format!("~ {} {clock}{title}", after.number)
        }
        (None, None) => String::new(),
    }
}
fn task_clock(task: &bunshin_core::day::Task) -> String {
    use bunshin_core::day::TaskKind;
    task.time.map_or_else(String::new, |at| {
        let prefix = match task.kind {
            TaskKind::Deadline => "〜",
            TaskKind::Appointment | TaskKind::Untimed => "",
        };
        format!("{prefix}{:02}:{:02} ", at.hour(), at.minute())
    })
}

pub fn input_count(chars: usize, limit: usize) -> String {
    format!(" {chars}/{limit} ")
}
pub fn chat_new_rows(count: usize) -> String {
    format!(" 新着 {count}（End で最新へ） ")
}
pub fn chat_notice(notice: ChatNotice) -> String {
    match notice {
        ChatNotice::Cancelled => "中止しました。変更はありません。".into(),
        ChatNotice::Failed => "うまく読み取れませんでした。もう一度送ってください。".into(),
        ChatNotice::Unavailable(reason) => match reason {
            UnavailableReason::UnsupportedOs => "この OS ではモデルを使えません。macOS 27 以降の Apple silicon Mac で使ってください。".into(),
            UnavailableReason::NotInstalled => "fm が見つかりません。macOS 27 以降で fm のインストールを確認してください。".into(),
            UnavailableReason::TermsNotAccepted => "モデルの利用規約への同意が必要です。端末で sudo fm license を実行してください。".into(),
        },
        ChatNotice::ModelBack => "モデルが使えるようになりました。".into(),
        ChatNotice::InstructionsFailure(error) => format!("{} 指示文は既定のものを使っています。", super::instructions_error(error)),
        ChatNotice::DefaultInstructions(origin) => {
            let reason = match origin {
                InstructionsOrigin::Owner => "",
                InstructionsOrigin::Missing => "指示文のファイルがありません。",
                InstructionsOrigin::Empty => "ファイルの指示文が空です。",
                InstructionsOrigin::TooLong => "ファイルの指示文が上限を超えています。",
            };
            format!("指示文は既定のものを使っています。{reason}自分の言葉で書くには、端末で bunshin instructions edit を実行してください。")
        }
        ChatNotice::Refused(reason) => refusal(reason).into(),
    }
}
fn refusal(reason: RefusalReason) -> &'static str {
    match reason {
        RefusalReason::Domain { reason } => super::day_error(reason),
        RefusalReason::InvalidTime => "時刻は 00:00〜23:59 で指定してください。",
        RefusalReason::MuteOutOfRange => "ミュートは5〜480分で指定してください。",
        RefusalReason::MuteOverflow => "ミュートの終了時刻を扱えませんでした。",
        RefusalReason::MissingField { field } => match field {
            ProposalField::Task => "どのタスクか指定してください。",
            ProposalField::Title => "タスクのタイトルを指定してください。",
            ProposalField::Kind => "タスクの種類を指定してください。",
            ProposalField::Minutes => "ミュートする分数を指定してください。",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bunshin_core::{
        Now, Tuning, UnixMillis,
        day::{Day, TaskKind, TaskOrigin},
    };

    #[test]
    fn change_rows_show_add_time_rename_reschedule_and_mute_end() {
        let now = Now {
            instant: UnixMillis(0),
            local: "2026-10-04T15:31:00".parse().unwrap(),
        };
        let (day, added) = Day::new(now.local.date(), Tuning::default())
            .add(
                "資料".into(),
                TaskKind::Deadline,
                Some("16:00".parse().unwrap()),
                TaskOrigin::Chat,
                now.instant,
            )
            .unwrap();
        assert_eq!(
            chat_changes(&added, &|at| now.at_fixed_offset(at)),
            "+ 1 〜16:00 資料（締切）"
        );
        let (day, renamed) = day
            .edit(
                1,
                "資料提出".into(),
                TaskKind::Deadline,
                Some("16:00".parse().unwrap()),
                now.instant,
            )
            .unwrap();
        assert_eq!(
            chat_changes(&renamed, &|at| now.at_fixed_offset(at)),
            "~ 1 資料 → 資料提出"
        );
        let (day, moved) = day
            .edit(
                1,
                "資料提出".into(),
                TaskKind::Deadline,
                Some("17:00".parse().unwrap()),
                now.instant,
            )
            .unwrap();
        assert_eq!(
            chat_changes(&moved, &|at| now.at_fixed_offset(at)),
            "~ 1 〜16:00 → 〜17:00 資料提出"
        );
        let (_, muted) = day.mute(UnixMillis(3_600_000), now.instant);
        assert_eq!(
            chat_changes(&muted, &|at| now.at_fixed_offset(at)),
            "ミュート 〜16:31（m で解除）"
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use bunshin_core::{
        Clock, Tuning,
        day::{Day, TaskKind, TaskOrigin},
    };
    use bunshin_test_support::FixedClock;

    #[test]
    fn task_change_marks_distinguish_done_dropped_and_undo() {
        let now = FixedClock::default().now();
        let day = Day::new(now.local.date(), Tuning::default())
            .add(
                "資料".into(),
                TaskKind::Untimed,
                None,
                TaskOrigin::Chat,
                now.instant,
            )
            .unwrap()
            .0;
        let (done, set) = day.clone().done(1, now.instant).unwrap();
        assert_eq!(
            chat_changes(&set, &|at| now.at_fixed_offset(at)),
            "x 1 資料"
        );
        assert_eq!(
            chat_changes(&done.undo(now.instant).unwrap().1, &|at| now
                .at_fixed_offset(at)),
            "取り消し: x 1 資料"
        );
        assert_eq!(
            chat_changes(&day.drop(1, now.instant).unwrap().1, &|at| now
                .at_fixed_offset(at)),
            "- 1 資料"
        );
    }
}

#[cfg(test)]
mod zone_tests {
    use super::*;
    use bunshin_core::{Now, Tuning, UnixMillis, day::Day};
    #[test]
    fn mute_end_uses_the_resolved_zone_offset_across_daylight_saving_changes() {
        for (instant, local, minutes, end_local, expected) in [
            (
                1_772_951_400_000,
                "2026-03-08T01:30:00",
                60,
                "2026-03-08T03:30:00",
                "ミュート 〜03:30（m で解除）",
            ),
            (
                1_793_511_000_000,
                "2026-11-01T01:30:00",
                120,
                "2026-11-01T02:30:00",
                "ミュート 〜02:30（m で解除）",
            ),
        ] {
            let now = Now {
                instant: UnixMillis(instant),
                local: local.parse().unwrap(),
            };
            let end = UnixMillis(instant + minutes * 60_000);
            let endpoint = Now {
                instant: end,
                local: end_local.parse().unwrap(),
            };
            let (_, set) = Day::new(now.local.date(), Tuning::default()).mute(end, now.instant);
            assert_eq!(
                chat_changes(&set, &|at| (at == end).then_some(endpoint)),
                expected
            );
            assert_ne!(now.at_fixed_offset(end).unwrap().local, endpoint.local);
        }
    }
}
