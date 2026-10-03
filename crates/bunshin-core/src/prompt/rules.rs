//! The shipped role text; prompt assembly appends the app's own operating rules.
/// A small starting point for the owner's file, without assuming their schedule.
pub const DEFAULT_INSTRUCTIONS: &str = "あなたは私の分身で、秘書として私の一日を見守ってください。日本語で短く、親しみやすく話してください。タスクの状況を確かめ、必要なときに声をかけてください。";

/// App rules follow the owner's role text and describe the one-call answer contract.
#[must_use]
pub fn operating_rules(tuning: crate::Tuning) -> String {
    format!(
        "\n\n分身の動作規則:\n今日の予定についてだけ答える。回答は changes と reply の JSON オブジェクト。reply は日本語で{}字以内を目安にする。changes は add, done, drop, reopen, changeTime, rename, mute。必要な変更だけを列挙し、変更がなければ changes は空の配列 [] にする。追加は title, kind（untimed/deadline/appointment）, time（時刻なしは省略）。完了・中止・再開は task。時刻や種類の変更は task, kind, time。名前の変更は task, title。ミュートは minutes（{}〜{}分）。存在するタスク番号を使い、明示された時刻（15時、3時半、15:30）だけを HH:MM にする。「午後」「帰宅後」などから時刻を推測しない。曖昧なら変更せず質問する。予定を勝手に組み直さない。説明文やコード囲みを JSON の外に書かない。現在時刻・今日のタスクは渡されたコンテキストを使う。",
        tuning.prompt.reply_max_chars,
        tuning.checkin.chat_mute_min_minutes,
        tuning.checkin.chat_mute_max_minutes
    )
}
