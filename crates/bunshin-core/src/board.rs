//! The deterministic model of the prototype's shared, in-memory board.

use std::time::Duration;

use serde::Deserialize;

use crate::{ModelAnswer, ModelError, ModelRequest, Tuning, UnixMillis};

const ZERO_SEED: u64 = 0x9e37_79b9_7f4a_7c15;
/// The owner's name in model prompts. あなた is the display name, but in a prompt
/// that also says "あなたは…" the model reads it as itself and ignores the owner.
const OWNER_PROMPT_NAME: &str = "ユーザー";
/// How many recent New topic subjects a new draw avoids.
const RECENT_TOPICS: usize = 8;
/// Markers after which a model body is leftover structure rather than prose.
const BODY_CUT_MARKERS: [&str; 5] = ["```", "<|", "<<", "{", "}"];
const TOPICS: [&str; 40] = [
    "コンビニの新作スイーツ",
    "休日の朝ごはん",
    "最近ハマってるゲーム",
    "スタバの期間限定ドリンク",
    "最近見て面白かったドラマやアニメ",
    "推しのお菓子",
    "カップ麺の最強の食べ方",
    "行ってみたい旅行先",
    "雨の日の過ごし方",
    "最近買ってよかったもの",
    "朝型か夜型か",
    "好きなおにぎりの具",
    "スマホの便利なアプリ",
    "最近聴いてる音楽",
    "子どもの頃に好きだった遊び",
    "地元の名物",
    "ラーメンは何味派か",
    "寝る前のルーティン",
    "猫派か犬派か",
    "ちょっとした贅沢",
    "冬に食べたくなるもの",
    "休日に行きたいカフェ",
    "最近の小さなラッキー",
    "部屋に置きたいもの",
    "好きなパンの種類",
    "100円ショップの掘り出し物",
    "家でできる気分転換",
    "サウナや温泉",
    "最近覚えた料理",
    "好きな季節とその理由",
    "ついやってしまう癖",
    "おすすめの散歩コース",
    "目玉焼きに何をかけるか",
    "買ってよかったキッチングッズ",
    "最近笑ったこと",
    "コンビニのホットスナック",
    "好きなアイス",
    "週末にやりたいこと",
    "最近ちょっと面倒だったこと",
    "映画館で食べたいもの",
];

/// Fixed starting values for the board's scheduling and display bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardTuning {
    /// Delay measured from the finish of one character attempt to the next.
    pub post_interval: Duration,
    /// Maximum recent posts included in a model prompt.
    pub context_posts: usize,
    /// Maximum number of posts kept in memory.
    pub max_posts: usize,
    /// Maximum Unicode scalar values shown for a model-authored body.
    pub body_max_chars: usize,
    /// Character limit asked of the model in its instructions.
    pub asked_max_chars: usize,
    /// Maximum Unicode scalar values accepted by the screen's input line.
    pub screen_input_max_chars: usize,
}

impl Default for BoardTuning {
    fn default() -> Self {
        Self {
            post_interval: Duration::from_secs(30),
            context_posts: 8,
            max_posts: 200,
            body_max_chars: 120,
            asked_max_chars: 60,
            screen_input_max_chars: 400,
        }
    }
}

/// A deterministic xorshift64* generator seeded by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a repeatable generator from seed; zero is mapped to a fixed seed.
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        Self {
            state: if seed == 0 { ZERO_SEED } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn index(&mut self, upper_bound: usize) -> usize {
        let Ok(bound) = u64::try_from(upper_bound) else {
            return 0;
        };
        if bound == 0 {
            return 0;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return usize::try_from(value % bound).unwrap_or_default();
            }
        }
    }

    fn percent(&mut self) -> usize {
        self.index(100)
    }
}

/// One of the three characters on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Speaker {
    /// Haru, the optimistic and curious topic starter.
    Haru,
    /// Shizuku, the cautious worrier who probes assumptions.
    Shizuku,
    /// Gen, the well-read and calm summarizer.
    Gen,
}

impl Speaker {
    /// The character's fixed Japanese display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Haru => "ハル",
            Self::Shizuku => "シズク",
            Self::Gen => "ゲン",
        }
    }

    const fn persona(self) -> &'static str {
        match self {
            Self::Haru => {
                "楽観的で好奇心旺盛。話題を振るのが好きで、「〜じゃない？」「やばい！」のようなくだけた口調。"
            }
            Self::Shizuku => {
                "現実的なツッコミ役だけど明るい。「いやそれ〜でしょ笑」「わかる、でも〜」のような口調。"
            }
            Self::Gen => {
                "雑学好きのおっちゃん。豆知識をひとこと添える。「〜なんだよな」「なあ」のような穏やかな口調。"
            }
        }
    }
}

/// Who authored a board post.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Author {
    /// The owner, shown as あなた.
    Owner,
    /// A fixed board character.
    Character(Speaker),
}

impl Author {
    /// The fixed Japanese display name for this author.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Owner => "あなた",
            Self::Character(speaker) => speaker.name(),
        }
    }

    const fn prompt_name(self) -> &'static str {
        match self {
            Self::Owner => OWNER_PROMPT_NAME,
            Self::Character(speaker) => speaker.name(),
        }
    }
}

/// Stable identity for one post during the current process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostId(pub u64);

/// One visible post in the board's in-memory history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    /// Identity used to keep a turn aimed at the post it selected.
    pub id: PostId,
    /// Owner or character who wrote the post.
    pub author: Author,
    /// Display text.
    pub body: String,
    /// Wall-clock instant supplied by the caller.
    pub at: UnixMillis,
}

/// One concrete, casual subject that seeds a new topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Topic {
    /// The fixed Japanese subject named in the prompt.
    pub subject: &'static str,
}

impl Topic {
    /// Every subject a New topic can draw, fixed in code.
    pub fn all() -> impl Iterator<Item = Self> {
        TOPICS.into_iter().map(|subject| Self { subject })
    }
}

/// What one selected character turn should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnKind {
    /// Reply to the latest post.
    Reply {
        /// The post receiving a reply.
        target: PostId,
    },
    /// Join the conversation using its latest two posts.
    ChimeIn {
        /// The older of the two referenced posts.
        older: PostId,
        /// The latest referenced post.
        latest: PostId,
    },
    /// Start a new topic for everyone.
    NewTopic {
        /// The subject to start talking about.
        topic: Topic,
    },
    /// Immediately answer a new owner post.
    OwnerReply {
        /// The newest owner post receiving the reply.
        target: PostId,
    },
    /// Answer the newest owner post on the following scheduled turn.
    OwnerReaction {
        /// The owner post receiving the second response.
        target: PostId,
    },
}

/// One scheduled character turn, including a private completion token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// Character selected to write this post.
    pub speaker: Speaker,
    /// Purpose and context of the post.
    pub kind: TurnKind,
    id: u64,
    generation: u64,
    target_snapshot: Option<Post>,
}

/// A reason a character attempt failed without adding a post.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The model is unavailable.
    Unavailable,
    /// The model call exceeded its timeout.
    TimedOut,
    /// The model call was cancelled.
    Cancelled,
    /// The model refused the request.
    Refused,
    /// The model returned text that is not UTF-8.
    Malformed,
    /// The model adapter failed for another reason.
    Failed,
    /// The returned body contained only whitespace.
    EmptyBody,
}

/// What happened when a scheduled character turn finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A post was added to the board.
    Posted(Post),
    /// The attempt failed and added no post.
    Failed(FailureKind),
    /// An owner post made the in-flight turn obsolete.
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerPhase {
    First,
    Second { first_speaker: Speaker },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerCycle {
    target: Post,
    phase: OwnerPhase,
    immediate: bool,
}

/// In-memory board state and deterministic turn selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    tuning: BoardTuning,
    posts: Vec<Post>,
    next_post_id: u64,
    next_turn_id: u64,
    in_flight: Option<u64>,
    generation: u64,
    last_attempt_finished: Option<UnixMillis>,
    recent_topics: Vec<Topic>,
    owner_cycle: Option<OwnerCycle>,
}

impl Default for Board {
    fn default() -> Self {
        Self::new(BoardTuning::default())
    }
}

impl Board {
    /// Create an empty board with the supplied fixed limits.
    #[must_use]
    pub const fn new(tuning: BoardTuning) -> Self {
        Self {
            tuning,
            posts: Vec::new(),
            next_post_id: 1,
            next_turn_id: 1,
            in_flight: None,
            generation: 0,
            last_attempt_finished: None,
            recent_topics: Vec::new(),
            owner_cycle: None,
        }
    }

    /// The retained posts in chronological order.
    #[must_use]
    pub fn posts(&self) -> &[Post] {
        &self.posts
    }

    /// Add a nonblank owner post immediately and reset the two-response cycle.
    ///
    /// # Returns
    /// The post that was accepted, or None for whitespace-only input.
    pub fn owner_post(&mut self, body: &str, at: UnixMillis) -> Option<Post> {
        let body = body.trim();
        if body.is_empty() {
            return None;
        }

        let post = self.make_post(Author::Owner, body.to_owned(), at);
        self.append(post.clone());
        self.generation = self.generation.wrapping_add(1);
        self.owner_cycle = Some(OwnerCycle {
            target: post.clone(),
            phase: OwnerPhase::First,
            immediate: true,
        });
        Some(post)
    }

    /// Whether a character attempt may start at this supplied instant.
    #[must_use]
    pub fn is_due(&self, now: UnixMillis) -> bool {
        if self.in_flight.is_some() {
            return false;
        }
        if self
            .owner_cycle
            .as_ref()
            .is_some_and(|cycle| cycle.immediate)
        {
            return true;
        }
        self.interval_elapsed(now)
    }

    /// Select and reserve the next turn when one is due.
    pub fn next_turn(&mut self, now: UnixMillis, rng: &mut Rng) -> Option<Turn> {
        if !self.is_due(now) {
            return None;
        }

        let (speaker, kind, target_snapshot) = if let Some(cycle) = self.owner_cycle.as_mut() {
            cycle.immediate = false;
            let target = cycle.target.clone();
            match cycle.phase {
                OwnerPhase::First => {
                    let speaker = self.choose_speaker(rng, &[]);
                    (
                        speaker,
                        TurnKind::OwnerReply { target: target.id },
                        Some(target),
                    )
                }
                OwnerPhase::Second { first_speaker } => {
                    let speaker = self.choose_speaker(rng, &[first_speaker]);
                    (
                        speaker,
                        TurnKind::OwnerReaction { target: target.id },
                        Some(target),
                    )
                }
            }
        } else {
            self.choose_normal_turn(rng)
        };

        let turn = Turn {
            speaker,
            kind,
            id: self.next_turn_id,
            generation: self.generation,
            target_snapshot,
        };
        self.next_turn_id = self.next_turn_id.wrapping_add(1);
        self.in_flight = Some(turn.id);
        Some(turn)
    }

    /// Build one plain-text model request for the active turn.
    #[must_use]
    pub fn request(&self, turn: &Turn, model_tuning: Tuning) -> Option<ModelRequest> {
        if self.in_flight != Some(turn.id) || self.generation != turn.generation {
            return None;
        }

        let instructions = format!(
            "あなたは「{}」。気軽な雑談掲示板の常連です。性格: {}\nルール: 投稿本文だけを日本語で1〜2文、{}文字以内で書く。名前・かぎかっこ・記号・前置きは付けない。具体的な商品名や体験を入れる。政治やニュースの話はしない。前の投稿の言い回しをまねしない。",
            turn.speaker.name(),
            turn.speaker.persona(),
            self.tuning.asked_max_chars
        );
        // A small model copies whatever the context dwells on, so a new topic starts
        // without the old conversation.
        let mut prompt = match turn.kind {
            TurnKind::NewTopic { .. } => String::new(),
            TurnKind::Reply { .. }
            | TurnKind::ChimeIn { .. }
            | TurnKind::OwnerReply { .. }
            | TurnKind::OwnerReaction { .. } => self.context_prompt(),
        };
        let instruction = self.turn_instruction(turn);
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(&instruction);

        Some(ModelRequest::new(&instructions, &prompt, model_tuning))
    }

    /// Finish one active model attempt and update the board or its failure state.
    pub fn finish(
        &mut self,
        turn: &Turn,
        answer: Result<ModelAnswer, ModelError>,
        now: UnixMillis,
    ) -> Outcome {
        if self.in_flight != Some(turn.id) {
            return Outcome::Stale;
        }
        self.in_flight = None;
        self.last_attempt_finished = Some(now);

        if self.generation != turn.generation {
            return Outcome::Stale;
        }
        self.advance_owner_cycle(turn);

        let body: String = match answer {
            Ok(answer) => clean_body(&answer.text, turn.speaker)
                .chars()
                .take(self.tuning.body_max_chars)
                .collect(),
            Err(error) => return Outcome::Failed(failure_kind(error)),
        };
        if body.is_empty() {
            return Outcome::Failed(FailureKind::EmptyBody);
        }

        let post = self.make_post(Author::Character(turn.speaker), body, now);
        if let TurnKind::NewTopic { topic } = turn.kind {
            if self.recent_topics.len() >= RECENT_TOPICS {
                self.recent_topics.remove(0);
            }
            self.recent_topics.push(topic);
        }
        self.append(post.clone());
        Outcome::Posted(post)
    }

    fn advance_owner_cycle(&mut self, turn: &Turn) {
        let mut clear_cycle = false;
        if let Some(cycle) = self.owner_cycle.as_mut() {
            match turn.kind {
                TurnKind::OwnerReply { target }
                    if target == cycle.target.id && cycle.phase == OwnerPhase::First =>
                {
                    cycle.phase = OwnerPhase::Second {
                        first_speaker: turn.speaker,
                    };
                }
                TurnKind::OwnerReaction { target }
                    if target == cycle.target.id
                        && matches!(cycle.phase, OwnerPhase::Second { .. }) =>
                {
                    clear_cycle = true;
                }
                TurnKind::OwnerReply { .. }
                | TurnKind::OwnerReaction { .. }
                | TurnKind::Reply { .. }
                | TurnKind::ChimeIn { .. }
                | TurnKind::NewTopic { .. } => {}
            }
        }
        if clear_cycle {
            self.owner_cycle = None;
        }
    }

    fn make_post(&mut self, author: Author, body: String, at: UnixMillis) -> Post {
        let id = PostId(self.next_post_id);
        self.next_post_id = self.next_post_id.wrapping_add(1);
        Post {
            id,
            author,
            body,
            at,
        }
    }

    fn append(&mut self, post: Post) {
        if self.tuning.max_posts == 0 {
            return;
        }
        while self.posts.len() >= self.tuning.max_posts {
            self.posts.remove(0);
        }
        self.posts.push(post);
    }

    fn interval_elapsed(&self, now: UnixMillis) -> bool {
        let Some(finished) = self.last_attempt_finished else {
            return true;
        };
        let elapsed = i128::from(now.0) - i128::from(finished.0);
        elapsed < 0
            || u128::try_from(elapsed)
                .is_ok_and(|millis| millis >= self.tuning.post_interval.as_millis())
    }

    fn choose_normal_turn(&mut self, rng: &mut Rng) -> (Speaker, TurnKind, Option<Post>) {
        let draw = rng.percent();
        let posts_len = self.posts.len();
        let selected = if draw < 50 {
            if let Some(target) = self.posts.last() {
                let speaker = self.choose_speaker(rng, &[]);
                Some((
                    speaker,
                    TurnKind::Reply { target: target.id },
                    Some(target.clone()),
                ))
            } else {
                None
            }
        } else if draw < 75 {
            self.chime_turn(rng)
        } else {
            None
        };
        if let Some(selected) = selected {
            return selected;
        }

        let topic = self.choose_topic(rng);
        let speaker = self.choose_speaker(rng, &[]);
        let target_snapshot = if posts_len == 0 {
            None
        } else {
            self.posts.last().cloned()
        };
        (speaker, TurnKind::NewTopic { topic }, target_snapshot)
    }

    fn chime_turn(&self, rng: &mut Rng) -> Option<(Speaker, TurnKind, Option<Post>)> {
        if self.posts.len() < 2 {
            return None;
        }
        let older = &self.posts[self.posts.len() - 2];
        let latest = &self.posts[self.posts.len() - 1];
        let mut excluded = Vec::with_capacity(3);
        if let Author::Character(speaker) = older.author {
            excluded.push(speaker);
        }
        if let Author::Character(speaker) = latest.author {
            excluded.push(speaker);
        }
        let eligible = Self::speakers_excluding(&excluded);
        if eligible.is_empty() {
            return None;
        }
        let speaker = eligible[rng.index(eligible.len())];
        Some((
            speaker,
            TurnKind::ChimeIn {
                older: older.id,
                latest: latest.id,
            },
            Some(latest.clone()),
        ))
    }

    fn choose_speaker(&self, rng: &mut Rng, additional_exclusions: &[Speaker]) -> Speaker {
        let mut excluded = Vec::with_capacity(additional_exclusions.len() + 1);
        if let Some(Author::Character(previous)) = self.posts.last().map(|post| post.author) {
            excluded.push(previous);
        }
        excluded.extend_from_slice(additional_exclusions);
        let eligible = Self::speakers_excluding(&excluded);
        eligible[rng.index(eligible.len())]
    }

    fn speakers_excluding(excluded: &[Speaker]) -> Vec<Speaker> {
        [Speaker::Haru, Speaker::Shizuku, Speaker::Gen]
            .into_iter()
            .filter(|speaker| !excluded.contains(speaker))
            .collect()
    }

    fn choose_topic(&self, rng: &mut Rng) -> Topic {
        let topics = Topic::all()
            .filter(|topic| !self.recent_topics.contains(topic))
            .collect::<Vec<_>>();
        topics[rng.index(topics.len())]
    }

    fn context_prompt(&self) -> String {
        let lines = self
            .posts
            .iter()
            .rev()
            .take(self.tuning.context_posts)
            .rev()
            .map(|post| format!("{}: {}", post.author.prompt_name(), post.body))
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return String::new();
        }
        format!("これまでの流れ:\n{}", lines.join("\n"))
    }

    fn turn_instruction(&self, turn: &Turn) -> String {
        match turn.kind {
            TurnKind::Reply { target } => {
                let body = self.target_body(target, turn);
                format!("直近の投稿に自然に返事してください。対象: {body}")
            }
            TurnKind::ChimeIn { older, latest } => {
                let older_body = self.target_body(older, turn);
                let latest_body = self.target_body(latest, turn);
                format!(
                    "直近二つの投稿を踏まえて会話に加わってください。少し前: {older_body} / 最新: {latest_body}"
                )
            }
            TurnKind::NewTopic { topic } => format!(
                "みんなに向けて新しい話題を振ってください。お題: {}。自分の体験を一つ入れて、みんなが返しやすい問いかけで終える。",
                topic.subject
            ),
            TurnKind::OwnerReply { target } => {
                let body = self.target_body(target, turn);
                format!(
                    "{OWNER_PROMPT_NAME}が今こう書き込みました: 「{body}」\n{OWNER_PROMPT_NAME}の書き込みの内容にまっすぐ応えてください。話題を変えたいと言われたら、その話題に乗ってください。"
                )
            }
            TurnKind::OwnerReaction { target } => {
                let body = self.target_body(target, turn);
                format!(
                    "{OWNER_PROMPT_NAME}が少し前にこう書き込みました: 「{body}」\n前の返事とは違う角度から、{OWNER_PROMPT_NAME}の書き込みに反応してください。"
                )
            }
        }
    }

    fn target_body(&self, id: PostId, turn: &Turn) -> String {
        self.posts
            .iter()
            .find(|post| post.id == id)
            .or(turn.target_snapshot.as_ref().filter(|post| post.id == id))
            .map_or_else(
                || "(対象の投稿は履歴から削除済み)".to_owned(),
                |post| post.body.clone(),
            )
    }
}

#[derive(Debug, Deserialize)]
struct PostAnswer {
    body: String,
}

/// Turn the model's raw text into one display line. The model sometimes still wraps
/// its answer as JSON, prefixes its name or quotes, or runs on into JSON or code
/// fences after the sentence; each is removed rather than shown.
fn clean_body(raw: &str, speaker: Speaker) -> String {
    let raw = raw.trim();
    let unwrapped = serde_json::from_str::<PostAnswer>(raw)
        .map_or_else(|_| raw.to_owned(), |answer| answer.body);
    let mut text = unwrapped.as_str();
    if let Some(cut) = BODY_CUT_MARKERS
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
    {
        text = &text[..cut];
    }
    let mut body = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    for prefix in [speaker.name(), OWNER_PROMPT_NAME] {
        for separator in [":", "：", "「"] {
            if let Some(rest) = body.strip_prefix(&format!("{prefix}{separator}")) {
                body = rest.trim_start().to_owned();
            }
        }
    }
    let mut body = body.trim();
    for (open, close) in [('「', '」'), ('『', '』'), ('"', '"')] {
        if let Some(inner) = body
            .strip_prefix(open)
            .and_then(|rest| rest.strip_suffix(close))
            && !inner.contains([open, close])
        {
            body = inner.trim();
        }
    }
    // A cut at a stray brace often leaves the closing bracket of a quote that never opened.
    for (open, close) in [('「', '」'), ('『', '』')] {
        if body.ends_with(close) && body.matches(open).count() < body.matches(close).count() {
            body = body[..body.len() - close.len_utf8()].trim_end();
        }
    }
    body.to_owned()
}

fn failure_kind(error: ModelError) -> FailureKind {
    match error {
        ModelError::Unavailable(_) => FailureKind::Unavailable,
        ModelError::TimedOut => FailureKind::TimedOut,
        ModelError::Cancelled => FailureKind::Cancelled,
        ModelError::Refused => FailureKind::Refused,
        ModelError::Malformed => FailureKind::Malformed,
        ModelError::Failed => FailureKind::Failed,
    }
}
