//! Chat and model feedback wording, independent of drawing and worker I/O.
use bunshin_core::{
    UnavailableReason,
    prompt::answer::{ProposalField, RefusalReason},
    screen::ChatNotice,
};

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
            Change::Mute { after, .. } => after.map_or_else(
                || "ミュート解除（旧記録）".into(),
                |at| format!("ミュート 〜{}（旧記録）", chat_timestamp(local_at(at))),
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
        ChatNotice::Refused(reason) => refusal(reason).into(),
    }
}
fn refusal(reason: RefusalReason) -> &'static str {
    match reason {
        RefusalReason::Domain { reason } => super::day_error(reason),
        RefusalReason::InvalidTime => "時刻は 00:00〜23:59 で指定してください。",
        RefusalReason::MissingField { field } => match field {
            ProposalField::Task => "どのタスクか指定してください。",
            ProposalField::Title => "タスクのタイトルを指定してください。",
            ProposalField::Kind => "タスクの種類を指定してください。",
        },
    }
}
