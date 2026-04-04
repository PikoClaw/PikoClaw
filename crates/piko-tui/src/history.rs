/// Stores submitted input strings for ↑/↓ navigation in the input bar.
///
/// Design Spec 03 — Input History Navigation:
/// - `↑` when input is **empty** (or already navigating) → replace with previous submitted message
/// - `↓` when navigating → move forward toward the current (empty) position
/// - Cursor is moved to end of the recalled string by the caller.
#[derive(Debug, Default)]
pub struct InputHistory {
    entries: Vec<String>,
    /// Index into `entries` while navigating. `None` = at the live "current" position.
    idx: Option<usize>,
}

impl InputHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a submitted entry.  Whitespace-only strings are silently ignored.
    /// Resets the navigation index back to the "current" (live) position.
    pub fn push(&mut self, entry: String) {
        if !entry.trim().is_empty() {
            self.entries.push(entry);
        }
        self.idx = None;
    }

    /// Move one step backward in history (↑ key).
    ///
    /// Returns the recalled string when history is non-empty, or `None` when history is empty.
    /// Pressing ↑ repeatedly at the oldest entry keeps returning that entry.
    pub fn backward(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        match self.idx {
            None => {
                // First ↑ press: jump to the most recent entry.
                let i = self.entries.len() - 1;
                self.idx = Some(i);
            }
            Some(0) => {
                // Already at the oldest entry — stay there.
            }
            Some(i) => {
                self.idx = Some(i - 1);
            }
        }
        self.idx.map(|i| self.entries[i].as_str())
    }

    /// Move one step forward in history (↓ key).
    ///
    /// Returns `Some(entry)` when there is a newer history item to show, or `None` when
    /// moving past the newest entry (returning to the live/empty input).
    pub fn forward(&mut self) -> Option<&str> {
        match self.idx {
            // Not navigating at all → nothing to do.
            None => None,
            // Moving past the last (newest) entry → back to live current input.
            Some(i) if i + 1 >= self.entries.len() => {
                self.idx = None;
                None
            }
            Some(i) => {
                self.idx = Some(i + 1);
                Some(self.entries[i + 1].as_str())
            }
        }
    }

    /// Reset the navigation index (e.g. when the user starts typing while browsing history).
    pub fn reset(&mut self) {
        self.idx = None;
    }

    /// Returns `true` when the user is actively browsing history (idx is Some).
    pub fn is_navigating(&self) -> bool {
        self.idx.is_some()
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no entries have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction ──────────────────────────────────────────────────────────

    #[test]
    fn new_history_is_empty() {
        let h = InputHistory::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert!(!h.is_navigating());
    }

    // ── Empty history edge-cases ──────────────────────────────────────────────

    #[test]
    fn empty_history_prev_returns_none() {
        let mut h = InputHistory::new();
        assert!(h.backward().is_none());
        assert!(!h.is_navigating(), "empty prev should not start navigating");
    }

    #[test]
    fn empty_history_next_returns_none() {
        let mut h = InputHistory::new();
        assert!(h.forward().is_none());
    }

    // ── Whitespace filtering ──────────────────────────────────────────────────

    #[test]
    fn whitespace_only_entries_are_not_stored() {
        let mut h = InputHistory::new();
        h.push("   ".to_string());
        h.push("\t\n".to_string());
        h.push("".to_string());
        assert!(h.is_empty());
        assert!(h.backward().is_none());
    }

    // ── Single-entry navigation ───────────────────────────────────────────────

    #[test]
    fn single_entry_prev_recalls_it() {
        let mut h = InputHistory::new();
        h.push("hello world".to_string());
        assert_eq!(h.backward(), Some("hello world"));
        assert!(h.is_navigating());
    }

    #[test]
    fn single_entry_prev_twice_stays_at_oldest() {
        let mut h = InputHistory::new();
        h.push("only".to_string());
        h.backward();
        // Second ↑ at oldest entry must stay there.
        assert_eq!(h.backward(), Some("only"));
    }

    #[test]
    fn single_entry_prev_then_next_returns_none_and_exits_navigation() {
        let mut h = InputHistory::new();
        h.push("one".to_string());
        h.backward(); // go to entry 0
        let result = h.forward(); // back to current
        assert!(result.is_none(), "next past newest must return None");
        assert!(!h.is_navigating());
    }

    // ── Multi-entry backward navigation ──────────────────────────────────────

    #[test]
    fn multiple_entries_navigate_backwards_most_recent_first() {
        let mut h = InputHistory::new();
        h.push("first".to_string());
        h.push("second".to_string());
        h.push("third".to_string());

        assert_eq!(h.backward(), Some("third"));
        assert_eq!(h.backward(), Some("second"));
        assert_eq!(h.backward(), Some("first"));
    }

    #[test]
    fn pressing_up_at_oldest_stays_at_oldest() {
        let mut h = InputHistory::new();
        h.push("a".to_string());
        h.push("b".to_string());

        h.backward(); // b
        h.backward(); // a  (oldest)
        assert_eq!(h.backward(), Some("a")); // must not wrap
    }

    // ── Forward navigation ────────────────────────────────────────────────────

    #[test]
    fn navigate_forward_after_backward() {
        let mut h = InputHistory::new();
        h.push("first".to_string());
        h.push("second".to_string());
        h.push("third".to_string());

        h.backward(); // third
        h.backward(); // second
        assert_eq!(h.forward(), Some("third"));
    }

    #[test]
    fn navigate_forward_past_newest_returns_none() {
        let mut h = InputHistory::new();
        h.push("a".to_string());
        h.push("b".to_string());

        h.backward(); // b  (newest)
        let result = h.forward(); // past newest → current
        assert!(result.is_none());
        assert!(!h.is_navigating());
    }

    #[test]
    fn next_when_not_navigating_returns_none() {
        let mut h = InputHistory::new();
        h.push("hello".to_string());
        // Never pressed ↑, so not navigating.
        assert!(h.forward().is_none());
        assert!(!h.is_navigating());
    }

    // ── Reset behaviour ───────────────────────────────────────────────────────

    #[test]
    fn reset_clears_navigation_index() {
        let mut h = InputHistory::new();
        h.push("hello".to_string());
        h.backward();
        assert!(h.is_navigating());
        h.reset();
        assert!(!h.is_navigating());
    }

    #[test]
    fn reset_on_non_navigating_is_safe() {
        let mut h = InputHistory::new();
        h.reset(); // must not panic
        assert!(!h.is_navigating());
    }

    // ── Push resets index ─────────────────────────────────────────────────────

    #[test]
    fn push_while_navigating_resets_index() {
        let mut h = InputHistory::new();
        h.push("first".to_string());
        h.backward();
        assert!(h.is_navigating());
        h.push("second".to_string());
        assert!(!h.is_navigating(), "push must reset navigation index");
    }

    #[test]
    fn after_push_prev_returns_newest_entry() {
        let mut h = InputHistory::new();
        h.push("first".to_string());
        h.push("second".to_string());
        // Navigate to first, then push new entry.
        h.backward();
        h.backward();
        h.push("third".to_string());
        // ↑ should now recall "third" (the newest).
        assert_eq!(h.backward(), Some("third"));
    }

    // ── Full round-trip ───────────────────────────────────────────────────────

    #[test]
    fn full_round_trip_three_entries() {
        let mut h = InputHistory::new();
        h.push("msg1".to_string());
        h.push("msg2".to_string());
        h.push("msg3".to_string());

        // Navigate all the way back.
        assert_eq!(h.backward(), Some("msg3"));
        assert_eq!(h.backward(), Some("msg2"));
        assert_eq!(h.backward(), Some("msg1"));
        // Clamped at oldest.
        assert_eq!(h.backward(), Some("msg1"));

        // Navigate all the way forward.
        assert_eq!(h.forward(), Some("msg2"));
        assert_eq!(h.forward(), Some("msg3"));
        // Past newest → back to current (None).
        assert!(h.forward().is_none());
        assert!(!h.is_navigating());
    }

    // ── len / is_empty ────────────────────────────────────────────────────────

    #[test]
    fn len_counts_valid_entries_only() {
        let mut h = InputHistory::new();
        h.push("a".to_string());
        h.push("  ".to_string()); // filtered
        h.push("b".to_string());
        assert_eq!(h.len(), 2);
        assert!(!h.is_empty());
    }
}
