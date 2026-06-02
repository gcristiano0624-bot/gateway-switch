use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
};

use serde_json::Value;

pub const TOOL_RESULT_CHAR_LIMIT: usize = 50_000;
const TOOL_RESULT_HEAD_CHARS: usize = 1_200;
const TOOL_RESULT_TAIL_CHARS: usize = 400;
const TOOL_CALL_WINDOW: usize = 5;
const TOOL_CALL_REPEAT_THRESHOLD: usize = 3;
const DEBUG_ENV: &str = "GATEWAY_SWITCH_LOOP_GUARD_DEBUG";

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
    pub large_tool_results: usize,
    pub compressed_tool_result_chars: usize,
    pub tool_loop_hints: usize,
}

impl LoopGuardSummary {
    pub fn is_clean(&self) -> bool {
        self.suppressed_text_chunks == 0
            && self.repeated_segments == 0
            && self.duplicate_tool_calls == 0
            && self.large_tool_results == 0
            && self.compressed_tool_result_chars == 0
            && self.tool_loop_hints == 0
    }

    pub fn to_log_summary(&self) -> Option<String> {
        if self.is_clean() {
            None
        } else {
            Some(format!(
                "loop_guard: suppressed_text={} repeated_segments={} duplicate_tool_calls={} large_tool_results={} compressed_tool_result_chars={} tool_loop_hints={}",
                self.suppressed_text_chunks,
                self.repeated_segments,
                self.duplicate_tool_calls,
                self.large_tool_results,
                self.compressed_tool_result_chars,
                self.tool_loop_hints
            ))
        }
    }

    pub fn merge(&mut self, other: &LoopGuardSummary) {
        self.suppressed_text_chunks += other.suppressed_text_chunks;
        self.repeated_segments += other.repeated_segments;
        self.duplicate_tool_calls += other.duplicate_tool_calls;
        self.large_tool_results += other.large_tool_results;
        self.compressed_tool_result_chars += other.compressed_tool_result_chars;
        self.tool_loop_hints += other.tool_loop_hints;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolLoopHint {
    pub tool_name: String,
    pub repeats: usize,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct LoopGuard {
    segment_hits: HashMap<String, usize>,
    recent_segments: Vec<String>,
    recent_exact_chunks: HashSet<String>,
    recent_tool_calls: Vec<String>,
    hinted_tool_calls: HashSet<String>,
    duplicate_streak: usize,
    summary: LoopGuardSummary,
}

impl Default for LoopGuard {
    fn default() -> Self {
        Self {
            segment_hits: HashMap::new(),
            recent_segments: Vec::new(),
            recent_exact_chunks: HashSet::new(),
            recent_tool_calls: Vec::new(),
            hinted_tool_calls: HashSet::new(),
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
        self.observe_tool_call_pattern(name, arguments).is_some()
            || self
                .recent_tool_calls
                .iter()
                .filter(|fp| *fp == &tool_fingerprint(name, arguments))
                .count()
                > 1
    }

    pub fn observe_tool_call_pattern(
        &mut self,
        name: &str,
        arguments: &str,
    ) -> Option<ToolLoopHint> {
        let normalized_args = normalize_tool_arguments(arguments);
        let fingerprint = tool_fingerprint(name, arguments);
        self.recent_tool_calls.push(fingerprint.clone());
        if self.recent_tool_calls.len() > TOOL_CALL_WINDOW {
            self.recent_tool_calls.remove(0);
        }

        let repeats = self
            .recent_tool_calls
            .iter()
            .filter(|recent| *recent == &fingerprint)
            .count();
        if repeats > 1 {
            self.summary.duplicate_tool_calls += 1;
        }

        debug_log(|| {
            format!(
                "tool_call name={name} fingerprint={fingerprint} repeats={repeats}/{TOOL_CALL_REPEAT_THRESHOLD} window={}/{} normalized_args_preview=\"{}\"",
                self.recent_tool_calls.len(),
                TOOL_CALL_WINDOW,
                preview_for_log(&normalized_args, 220)
            )
        });

        if repeats >= TOOL_CALL_REPEAT_THRESHOLD && !self.hinted_tool_calls.contains(&fingerprint) {
            self.hinted_tool_calls.insert(fingerprint.clone());
            self.summary.tool_loop_hints += 1;
            debug_log(|| {
                format!(
                    "tool_loop_hint triggered name={name} fingerprint={fingerprint} repeats={repeats} duplicate_tool_calls={} hints={}",
                    self.summary.duplicate_tool_calls,
                    self.summary.tool_loop_hints
                )
            });
            return Some(ToolLoopHint {
                tool_name: name.to_string(),
                repeats,
                fingerprint,
            });
        }

        None
    }

    pub fn compress_tool_result(&mut self, tool_use_id: &str, content: &str) -> String {
        let original_chars = content.chars().count();
        if original_chars <= TOOL_RESULT_CHAR_LIMIT {
            debug_log(|| {
                format!(
                    "tool_result pass tool_use_id={tool_use_id} chars={original_chars} limit={TOOL_RESULT_CHAR_LIMIT}"
                )
            });
            return content.to_string();
        }

        self.summary.large_tool_results += 1;
        self.summary.compressed_tool_result_chars +=
            original_chars.saturating_sub(TOOL_RESULT_HEAD_CHARS + TOOL_RESULT_TAIL_CHARS);

        let head = take_chars(content, TOOL_RESULT_HEAD_CHARS);
        let tail = take_last_chars(content, TOOL_RESULT_TAIL_CHARS);
        let compressed = format!(
            "[Gateway Switch LoopGuard: tool_result compressed]\n\
tool_use_id: {tool_use_id}\n\
original_chars: {original_chars}\n\
retained_head_chars: {TOOL_RESULT_HEAD_CHARS}\n\
retained_tail_chars: {TOOL_RESULT_TAIL_CHARS}\n\
reason: oversized tool_result would crowd out conversation history.\n\n\
--- retained head ---\n{head}\n\n\
--- retained tail ---\n{tail}"
        );
        debug_log(|| {
            format!(
                "tool_result compressed tool_use_id={tool_use_id} original_chars={original_chars} compressed_chars={} saved_chars={} preview=\"{}\"",
                compressed.chars().count(),
                original_chars.saturating_sub(compressed.chars().count()),
                preview_for_log(&compressed, 260)
            )
        });
        compressed
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

fn tool_fingerprint(name: &str, arguments: &str) -> String {
    let normalized_args = normalize_tool_arguments(arguments);
    format!("{}:{}", normalize_text(name), stable_hash(&normalized_args))
}

fn normalize_tool_arguments(arguments: &str) -> String {
    serde_json::from_str::<Value>(arguments)
        .map(|value| canonical_json(&value))
        .unwrap_or_else(|_| normalize_text(arguments))
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut entries = map
                .iter()
                .map(|(key, value)| format!("{key}:{}", canonical_json(value)))
                .collect::<Vec<_>>();
            entries.sort();
            format!("{{{}}}", entries.join(","))
        }
        Value::Array(items) => {
            let values = items.iter().map(canonical_json).collect::<Vec<_>>();
            format!("[{}]", values.join(","))
        }
        Value::String(value) => normalize_text(value),
        _ => value.to_string(),
    }
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn take_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn take_last_chars(value: &str, limit: usize) -> String {
    let mut chars = value.chars().rev().take(limit).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn debug_enabled() -> bool {
    std::env::var(DEBUG_ENV)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn debug_log(message: impl FnOnce() -> String) {
    if debug_enabled() {
        eprintln!("[loop_guard][debug] {}", message());
    }
}

fn preview_for_log(value: &str, limit: usize) -> String {
    let mut preview = value
        .chars()
        .take(limit)
        .collect::<String>()
        .replace('\n', "\\n");
    if value.chars().count() > limit {
        preview.push_str("...");
    }
    preview
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

    #[test]
    fn emits_tool_loop_hint_after_three_recent_repeats() {
        let mut guard = LoopGuard::default();
        assert!(guard
            .observe_tool_call_pattern("Read", r#"{"file_path":"/tmp/a"}"#)
            .is_none());
        assert!(guard
            .observe_tool_call_pattern("Read", r#"{"file_path":"/tmp/a"}"#)
            .is_none());
        let hint = guard
            .observe_tool_call_pattern("Read", r#"{"file_path":"/tmp/a"}"#)
            .unwrap();

        assert_eq!(hint.tool_name, "Read");
        assert_eq!(hint.repeats, 3);
        assert_eq!(guard.summary().tool_loop_hints, 1);
    }

    #[test]
    fn compresses_large_tool_result() {
        let mut guard = LoopGuard::default();
        let content = "a".repeat(TOOL_RESULT_CHAR_LIMIT + 10);
        let compressed = guard.compress_tool_result("toolu_1", &content);

        assert!(compressed.contains("tool_result compressed"));
        assert!(compressed.contains("original_chars: 50010"));
        assert!(compressed.chars().count() < content.chars().count());
        assert_eq!(guard.summary().large_tool_results, 1);
    }

    #[test]
    fn simulation_repeated_tool_calls_and_large_result_debug_trace() {
        let mut guard = LoopGuard::default();
        let args = r#"{"file_path":"/tmp/openai-form.html","offset":1,"limit":2000}"#;

        let first = guard.observe_tool_call_pattern("Read", args);
        let second = guard.observe_tool_call_pattern("Read", args);
        let third = guard.observe_tool_call_pattern("Read", args);
        let large_html = format!(
            "<html><body>{}</body></html>",
            "large form body ".repeat(4_000)
        );
        let compressed = guard.compress_tool_result("toolu_large_html", &large_html);
        let summary = guard.summary();

        eprintln!("[loop_guard][simulation] first_hint={first:?}");
        eprintln!("[loop_guard][simulation] second_hint={second:?}");
        eprintln!("[loop_guard][simulation] third_hint={third:?}");
        eprintln!(
            "[loop_guard][simulation] compressed_preview=\"{}\"",
            preview_for_log(&compressed, 360)
        );
        eprintln!("[loop_guard][simulation] summary={summary:?}");

        assert!(first.is_none());
        assert!(second.is_none());
        assert_eq!(
            third.as_ref().map(|hint| hint.tool_name.as_str()),
            Some("Read")
        );
        assert!(compressed.contains("tool_result compressed"));
        assert_eq!(summary.duplicate_tool_calls, 2);
        assert_eq!(summary.tool_loop_hints, 1);
        assert_eq!(summary.large_tool_results, 1);
    }
}
