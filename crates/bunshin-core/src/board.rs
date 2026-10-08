//! The deterministic model of the prototype's shared, in-memory board.

use std::time::Duration;

use serde::Deserialize;

use crate::{ModelAnswer, ModelError, ModelRequest, Tuning, UnixMillis};

const POST_SCHEMA: &str = r#"{"type":"object","properties":{"body":{"type":"string"}},"required":["body"],"additionalProperties":false,"title":"Post","x-order":["body"]}"#;
const ZERO_SEED: u64 = 0x9e37_79b9_7f4a_7c15;
const TOPIC_DEPTHS: [TopicDepth; 3] = [TopicDepth::Casual, TopicDepth::Deeper, TopicDepth::Expert];
const TOPIC_MOODS: [TopicMood; 3] = [TopicMood::Relaxed, TopicMood::Excited, TopicMood::Debatable];

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
    /// Maximum Unicode scalar values accepted from an owner post.
    pub input_max_chars: usize,
}

impl Default for BoardTuning {
    fn default() -> Self {
        Self {
            post_interval: Duration::from_secs(30),
            context_posts: 12,
            max_posts: 200,
            body_max_chars: 120,
            asked_max_chars: 80,
            input_max_chars: 400,
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
                "楽観的で好奇心旺盛。話題を始めるのが好きで、「〜じゃない？」「やばい！」のようなくだけた口調。"
            }
            Self::Shizuku => {
                "慎重で心配性。前提を問い直し、「でもそれって…」のような丁寧さの混じる口調。"
            }
            Self::Gen => {
                "博識で落ち着いており、話をまとめる。「〜だな」「なあ」のような穏やかな口調。"
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

/// The twelve fixed categories that can seed a new topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopicCategory {
    /// Travel.
    Travel,
    /// Information technology and technology.
    Technology,
    /// Economics and money.
    Economics,
    /// Food and cooking.
    Food,
    /// Movies and television dramas.
    Movies,
    /// Music.
    Music,
    /// Sports.
    Sports,
    /// Science.
    Science,
    /// History.
    History,
    /// Health and everyday life.
    Health,
    /// Work and careers.
    Career,
    /// Hobbies and play.
    Hobbies,
}

impl TopicCategory {
    const ALL: [Self; 12] = [
        Self::Travel,
        Self::Technology,
        Self::Economics,
        Self::Food,
        Self::Movies,
        Self::Music,
        Self::Sports,
        Self::Science,
        Self::History,
        Self::Health,
        Self::Career,
        Self::Hobbies,
    ];

    /// The fixed Japanese label used in a topic prompt.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Travel => "旅行",
            Self::Technology => "IT・テクノロジー",
            Self::Economics => "経済・お金",
            Self::Food => "食べ物・料理",
            Self::Movies => "映画・ドラマ",
            Self::Music => "音楽",
            Self::Sports => "スポーツ",
            Self::Science => "科学",
            Self::History => "歴史",
            Self::Health => "健康・暮らし",
            Self::Career => "仕事・キャリア",
            Self::Hobbies => "趣味・遊び",
        }
    }
}

/// How deeply a new topic should be explored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopicDepth {
    /// A light conversation.
    Casual,
    /// A little more depth.
    Deeper,
    /// A topic for someone familiar with it.
    Expert,
}

impl TopicDepth {
    /// The fixed Japanese label used in a topic prompt.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Casual => "気軽な雑談",
            Self::Deeper => "ちょっと掘り下げる",
            Self::Expert => "詳しい人向け",
        }
    }
}

/// The fixed mood for a new topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopicMood {
    /// Relaxed.
    Relaxed,
    /// Excited.
    Excited,
    /// A little divisive.
    Debatable,
}

impl TopicMood {
    /// The fixed Japanese label used in a topic prompt.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Relaxed => "のんびり",
            Self::Excited => "盛り上がる",
            Self::Debatable => "ちょっと意見が分かれる",
        }
    }
}

/// The fixed parameters selected for a new topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Topic {
    /// The selected subject area.
    pub category: TopicCategory,
    /// The desired discussion depth.
    pub depth: TopicDepth,
    /// The desired conversation mood.
    pub mood: TopicMood,
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
        /// The category, depth, and mood to use.
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
    /// The response was not the required body object.
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
    previous_topic_category: Option<TopicCategory>,
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
            previous_topic_category: None,
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

    /// Build one schema-backed model request for the active turn.
    #[must_use]
    pub fn request(&self, turn: &Turn, model_tuning: Tuning) -> Option<ModelRequest> {
        if self.in_flight != Some(turn.id) || self.generation != turn.generation {
            return None;
        }

        let instructions = format!(
            "あなたは{}として掲示板に参加します。{}\n日本語で投稿本文を一つ書いてください。本文は{}文字以内にし、名前を付けず、本文だけをJSONで返してください。",
            turn.speaker.name(),
            turn.speaker.persona(),
            self.tuning.asked_max_chars
        );
        let mut prompt = self.context_prompt();
        let instruction = self.turn_instruction(turn);
        if !prompt.is_empty() {
            prompt.push_str("\n\n");
        }
        prompt.push_str(&instruction);

        Some(ModelRequest::new(
            &instructions,
            &prompt,
            POST_SCHEMA,
            model_tuning,
        ))
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
            Ok(answer) => match serde_json::from_str::<PostAnswer>(&answer.json) {
                Ok(parsed) if !parsed.body.trim().is_empty() => parsed
                    .body
                    .trim()
                    .chars()
                    .take(self.tuning.body_max_chars)
                    .collect(),
                Ok(_) => return Outcome::Failed(FailureKind::EmptyBody),
                Err(_) => return Outcome::Failed(FailureKind::Malformed),
            },
            Err(error) => return Outcome::Failed(failure_kind(error)),
        };
        if body.is_empty() {
            return Outcome::Failed(FailureKind::EmptyBody);
        }

        let post = self.make_post(Author::Character(turn.speaker), body, now);
        if let TurnKind::NewTopic { topic } = turn.kind {
            self.previous_topic_category = Some(topic.category);
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

    fn choose_topic(&mut self, rng: &mut Rng) -> Topic {
        let categories = TopicCategory::ALL
            .into_iter()
            .filter(|category| Some(*category) != self.previous_topic_category)
            .collect::<Vec<_>>();
        let category = categories[rng.index(categories.len())];

        Topic {
            category,
            depth: TOPIC_DEPTHS[rng.index(TOPIC_DEPTHS.len())],
            mood: TOPIC_MOODS[rng.index(TOPIC_MOODS.len())],
        }
    }

    fn context_prompt(&self) -> String {
        self.posts
            .iter()
            .rev()
            .take(self.tuning.context_posts)
            .rev()
            .map(|post| format!("{}: {}", post.author.name(), post.body))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn turn_instruction(&self, turn: &Turn) -> String {
        match turn.kind {
            TurnKind::Reply { target } => {
                let body = self.target_body(target, turn);
                format!("直近の投稿に自然に返信してください。対象の投稿: {body}")
            }
            TurnKind::ChimeIn { older, latest } => {
                let older_body = self.target_body(older, turn);
                let latest_body = self.target_body(latest, turn);
                format!(
                    "直近二つの投稿を踏まえて会話に加わってください。少し前: {older_body} / 最新: {latest_body}"
                )
            }
            TurnKind::NewTopic { topic } => format!(
                "全員に向けて新しい話題を始めてください。カテゴリ: {}、深さ: {}、雰囲気: {}。",
                topic.category.label(),
                topic.depth.label(),
                topic.mood.label()
            ),
            TurnKind::OwnerReply { target } => {
                let body = self.target_body(target, turn);
                format!("あなたの投稿に返信してください。対象の投稿: {body}")
            }
            TurnKind::OwnerReaction { target } => {
                let body = self.target_body(target, turn);
                format!(
                    "あなたの同じ投稿に、前の返事とは違う観点から反応してください。対象の投稿: {body}"
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
#[serde(deny_unknown_fields)]
struct PostAnswer {
    body: String,
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
