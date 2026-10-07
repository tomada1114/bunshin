//! Wrapped-row viewport facts supplied by the renderer.

use crate::board::{Post, PostId};

/// Wrapped-row scroll state for the board's post list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BoardViewport {
    top: usize,
    rows: usize,
    height: usize,
    following: bool,
    seen_through: Option<PostId>,
}

impl Default for BoardViewport {
    fn default() -> Self {
        Self {
            top: 0,
            rows: 0,
            height: 0,
            following: true,
            seen_through: None,
        }
    }
}

impl BoardViewport {
    pub(super) fn record_layout(&mut self, rows: usize, height: usize) {
        self.rows = rows;
        self.height = height;
        let end = rows.saturating_sub(height);
        self.top = if self.following {
            end
        } else {
            self.top.min(end)
        };
    }

    pub(super) const fn top(&self) -> usize {
        self.top
    }

    pub(super) const fn follows_latest(&self) -> bool {
        self.following
    }

    pub(super) fn new_post_count(&self, posts: &[Post]) -> usize {
        if self.following {
            return 0;
        }
        posts
            .iter()
            .filter(|post| self.seen_through.is_none_or(|seen| post.id.0 > seen.0))
            .count()
    }

    pub(super) fn first_unseen_post(&self, posts: &[Post]) -> Option<usize> {
        if self.following {
            return None;
        }
        posts
            .iter()
            .position(|post| self.seen_through.is_none_or(|seen| post.id.0 > seen.0))
    }

    pub(super) fn scroll_up(&mut self, posts: &[Post]) {
        self.leave_latest(posts);
        self.top = self.top.saturating_sub(1);
    }

    pub(super) fn scroll_down(&mut self, posts: &[Post]) {
        self.leave_latest(posts);
        let end = self.rows.saturating_sub(self.height);
        self.top = self.top.saturating_add(1).min(end);
        self.following = self.top == end;
    }

    pub(super) fn scroll_page_up(&mut self, posts: &[Post]) {
        self.leave_latest(posts);
        self.top = self.top.saturating_sub(self.height.max(1));
    }

    pub(super) fn scroll_page_down(&mut self, posts: &[Post]) {
        self.leave_latest(posts);
        let end = self.rows.saturating_sub(self.height);
        self.top = self.top.saturating_add(self.height.max(1)).min(end);
        self.following = self.top == end;
    }

    pub(super) fn follow_latest(&mut self) {
        self.following = true;
        self.top = self.rows.saturating_sub(self.height);
    }

    fn leave_latest(&mut self, posts: &[Post]) {
        if self.following {
            self.seen_through = posts.last().map(|post| post.id);
            self.following = false;
        }
    }
}
