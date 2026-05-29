use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextGuardAction {
    Pass(String),
    Suppress,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoopGuardSummary {
    pub suppressed_text_chunks: usize,
    pub repeated_segments: usize,
    pub duplicate_tool_calls: usize,
}

impl LoopGuardSummary {
    pub fn is_clean(&self) -> bool {
        self.suppressed_text_chunks == 0
            && self.repeated_segments == 0
            && self.duplicate_tool_calls == 0
    }

    pub fn to_log_summary(&self) -> Option<String> {
        if self.is_clean() {
            None
        } else {
            Some(format!(
                "loop_guard: suppressed_text={} repeated_segments={} duplicate_tool_calls={}",
                self.suppressed_text_chunks, self.repeated_segments, self.duplicate_tool_calls
            ))
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoopGuard {
    segment_hits: HashMap<String, usize>,
    recent_segments: Vec<String>,
    recent_exact_chunks: HashSet<String>,
    duplicate_streak: usize,
    summary: LoopGuardSummary,
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self {
            segment_hits: HashMap::new(),
            recent_segments: Vec::new(),
            recent_exact_chunks: HashSet::new(),
            duplicate_streak: 0,
            summary: LoopGuardSummary::default(),
        }
    }
}

impl LoopGuard {
    pub fn observe_text(&mut self, text: &str) -> TextGuardAction {
        if text.trim().is_empty() {
            return TextGuardAction::Pass(text.to_string());
        }

        let normalized_chunk = normalize_text(text);
        if normalized_chunk.chars().count() >= 24
            && self.recent_exact_chunks.contains(&normalized_chunk)
        {
            self.summary.suppressed_text_chunks += 1;
            self.duplicate_streak += 1;
            return TextGuardAction::Suppress;
        }

        let segments = stable_segments(text);
        let mut repeated = 0usize;
        let mut novel = 0usize;
        for segment in &segments {
            let hit = self.segment_hits.entry(segment.clone()).or_insert(0);
            if *hit >= 2 {
                repeated += 1;
            } else {
                novel += 1;
            }
            *hit += 1;
        }

        if !segments.is_empty() && repeated > 0 && novel == 0 {
            self.summary.suppressed_text_chunks += 1;
            self.summary.repeated_segments += repeated;
            self.duplicate_streak += 1;
            return TextGuardAction::Suppress;
        }

        if repeated >= 2 && repeated > novel * 2 {
            self.summary.suppressed_text_chunks += 1;
            self.summary.repeated_segments += repeated;
            self.duplicate_streak += 1;
            return TextGuardAction::Suppress;
        }

        self.duplicate_streak = self.duplicate_streak.saturating_sub(1);
        self.remember_chunk(normalized_chunk);
        for segment in segments {
            self.remember_segment(segment);
        }
        TextGuardAction::Pass(text.to_string())
    }

    pub fn observe_tool_call(&mut self, name: &str, arguments: &str) -> bool {
        let fingerprint = normalize_text(&format!("{name}:{arguments}"));
        let count = self
            .segment_hits
            .entry(format!("tool:{fingerprint}"))
            .or_insert(0);
        *count += 1;
        if *count > 1 {
            self.summary.duplicate_tool_calls += 1;
            true
        } else {
            false
        }
    }

    pub fn summary(&self) -> LoopGuardSummary {
        self.summary.clone()
    }

    fn remember_chunk(&mut self, normalized_chunk: String) {
        if normalized_chunk.chars().count() < 24 {
            return;
        }
        self.recent_exact_chunks.insert(normalized_chunk);
        if self.recent_exact_chunks.len() > 128 {
            self.recent_exact_chunks.clear();
        }
    }

    fn remember_segment(&mut self, segment: String) {
        self.recent_segments.push(segment);
        if self.recent_segments.len() > 256 {
            if let Some(old) = self.recent_segments.first().cloned() {
                self.segment_hits.remove(&old);
            }
            self.recent_segments.remove(0);
        }
    }
}

fn stable_segments(text: &str) -> Vec<String> {
    normalize_text(text)
        .split(|c: char| matches!(c, '.' | '!' | '?' | '。' | '！' | '？' | '\n' | ';' | '；'))
        .map(str::trim)
        .filter(|s| s.chars().count() >= 18)
        .map(|s| s.chars().take(220).collect::<String>())
        .collect()
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppresses_repeated_planning_text_without_stopping_session() {
        let mut guard = LoopGuard::default();
        let phrase =
            "Now let me fetch real-time market data and then provide a comprehensive analysis.";

        assert!(matches!(
            guard.observe_text(phrase),
            TextGuardAction::Pass(_)
        ));
        assert!(matches!(
            guard.observe_text(phrase),
            TextGuardAction::Suppress
        ));
        assert!(matches!(
            guard.observe_text(
                "The final portfolio recommendation has new details and should pass."
            ),
            TextGuardAction::Pass(_)
        ));
        assert!(guard.summary().suppressed_text_chunks >= 1);
    }

    #[test]
    fn allows_long_non_repeating_report() {
        let mut guard = LoopGuard::default();
        for idx in 0..80 {
            let text = format!(
                "Section {idx}: this paragraph contains unique analysis about a different portfolio factor and therefore should continue streaming."
            );
            assert!(matches!(
                guard.observe_text(&text),
                TextGuardAction::Pass(_)
            ));
        }
        assert!(guard.summary().is_clean());
    }

    #[test]
    fn detects_duplicate_tool_call_fingerprint() {
        let mut guard = LoopGuard::default();
        assert!(!guard.observe_tool_call("bash", "ls ~/.codex/skills"));
        assert!(guard.observe_tool_call("bash", "ls ~/.codex/skills"));
        assert_eq!(guard.summary().duplicate_tool_calls, 1);
    }
}
