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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UnixMillis, board::Author};

    fn posts(ids: &[u64]) -> Vec<Post> {
        ids.iter()
            .map(|id| Post {
                id: PostId(*id),
                author: Author::Owner,
                body: format!("post {id}"),
                at: UnixMillis(i64::try_from(*id).unwrap_or_default()),
            })
            .collect()
    }

    #[test]
    fn scrolling_tracks_unseen_posts_and_clamps_to_the_available_rows() {
        let initial = posts(&[1, 2, 3]);
        let mut viewport = BoardViewport::default();

        assert!(viewport.follows_latest());
        assert_eq!(viewport.new_post_count(&initial), 0);
        assert_eq!(viewport.first_unseen_post(&initial), None);

        viewport.record_layout(12, 4);
        assert_eq!(viewport.top(), 8);
        viewport.scroll_up(&initial);
        assert_eq!(viewport.top(), 7);
        assert!(!viewport.follows_latest());

        viewport.follow_latest();
        assert_eq!(viewport.top(), 8);
        assert!(viewport.follows_latest());
        viewport.scroll_up(&initial);

        let appended = posts(&[1, 2, 3, 4, 5]);
        assert_eq!(viewport.new_post_count(&appended), 2);
        assert_eq!(viewport.first_unseen_post(&appended), Some(3));

        viewport.record_layout(20, 4);
        assert_eq!(viewport.top(), 7);
        viewport.record_layout(10, 4);
        assert_eq!(viewport.top(), 6);

        viewport.scroll_up(&appended);
        assert_eq!(viewport.top(), 5);
        viewport.scroll_page_up(&appended);
        assert_eq!(viewport.top(), 1);
        viewport.scroll_page_up(&appended);
        assert_eq!(viewport.top(), 0);
        viewport.scroll_up(&appended);
        assert_eq!(viewport.top(), 0);

        viewport.scroll_down(&appended);
        assert_eq!(viewport.top(), 1);
        viewport.scroll_page_down(&appended);
        assert_eq!(viewport.top(), 5);
        viewport.scroll_page_down(&appended);
        assert_eq!(viewport.top(), 6);
        assert!(viewport.follows_latest());
        assert_eq!(viewport.new_post_count(&appended), 0);
        assert_eq!(viewport.first_unseen_post(&appended), None);

        viewport.record_layout(2, 5);
        assert_eq!(viewport.top(), 0);
    }

    #[test]
    fn scrolling_before_any_post_treats_the_existing_posts_as_unseen() {
        let appended = posts(&[1, 2]);
        let mut viewport = BoardViewport::default();

        viewport.record_layout(3, 1);
        viewport.scroll_up(&[]);

        assert_eq!(viewport.new_post_count(&appended), 2);
        assert_eq!(viewport.first_unseen_post(&appended), Some(0));

        viewport.scroll_down(&appended);
        assert!(viewport.follows_latest());
        assert_eq!(viewport.new_post_count(&appended), 0);
    }
}
