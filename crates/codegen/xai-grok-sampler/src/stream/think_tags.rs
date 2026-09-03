//! Split third-party `<think>` / `<thinking>` tags out of Chat Completions `content`.
//!
//! Official Grok models send reasoning on `reasoning_content`. Many OpenAI-compatible
//! gateways instead dump XML tags into `content`, often without a closing tag when the
//! stream is cancelled. This module is a local, self-contained transform so upstream
//! merges stay a one-call-site hook in `chat_completions.rs`.
//!
//! Tags inside fenced code blocks (``` / ~~~) are left as text.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkSink {
    Text,
    Reasoning,
}

const OPEN_TAGS: &[&str] = &["<thinking>", "<think>"];
const CLOSE_TAGS: &[&str] = &["</thinking>", "</think>"];

#[derive(Debug, Default)]
pub struct ThinkTagSplitter {
    buf: String,
    in_think: bool,
    /// Opening fence tick char and count when inside a text-mode code fence.
    fence: Option<(char, usize)>,
    /// True when `buf` is empty or ended with `\n` (for fence-at-line-start).
    at_line_start: bool,
}

impl ThinkTagSplitter {
    pub fn new() -> Self {
        Self {
            at_line_start: true,
            ..Self::default()
        }
    }

    pub fn push(&mut self, chunk: &str) -> Vec<(ThinkSink, String)> {
        if chunk.is_empty() {
            return Vec::new();
        }
        self.buf.push_str(chunk);
        self.drain(false)
    }

    /// Flush leftovers. An unclosed think region becomes Reasoning so the UI can
    /// finish the thinking block instead of leaving a raw open tag in the message.
    pub fn finish(&mut self) -> Vec<(ThinkSink, String)> {
        self.drain(true)
    }

    fn drain(&mut self, eof: bool) -> Vec<(ThinkSink, String)> {
        let mut out = Vec::new();
        loop {
            if self.buf.is_empty() {
                return out;
            }
            if let Some((tick, n)) = self.fence {
                match find_closing_fence(&self.buf, tick, n) {
                    Some(end) => {
                        self.emit(&mut out, ThinkSink::Text, end);
                        self.fence = None;
                        self.at_line_start = self.buf.starts_with('\n') || self.buf.is_empty();
                    }
                    None => {
                        let hold = if eof {
                            0
                        } else {
                            trailing_fence_hold(&self.buf, tick)
                        };
                        if self.buf.len() > hold {
                            let take = self.buf.len() - hold;
                            self.emit(&mut out, ThinkSink::Text, take);
                        }
                        return out;
                    }
                }
                continue;
            }

            if self.in_think {
                match find_tag(&self.buf, CLOSE_TAGS) {
                    Some((start, end)) => {
                        self.emit(&mut out, ThinkSink::Reasoning, start);
                        let _ = self.buf.drain(..end - start);
                        self.in_think = false;
                        self.at_line_start = self.buf.starts_with('\n') || self.buf.is_empty();
                    }
                    None => {
                        let hold = if eof { 0 } else { incomplete_tag_hold(&self.buf, CLOSE_TAGS) };
                        if self.buf.len() > hold {
                            let take = self.buf.len() - hold;
                            self.emit(&mut out, ThinkSink::Reasoning, take);
                        }
                        if eof {
                            self.in_think = false;
                        }
                        return out;
                    }
                }
                continue;
            }

            let fence = find_opening_fence(&self.buf, self.at_line_start);
            let tag = find_tag(&self.buf, OPEN_TAGS);
            match (fence, tag) {
                (Some((f0, f1, tick, n)), Some((t0, _))) if f0 <= t0 => {
                    self.emit(&mut out, ThinkSink::Text, f1);
                    let _ = f0;
                    self.fence = Some((tick, n));
                    self.at_line_start = false;
                }
                (Some((_f0, f1, tick, n)), None) => {
                    self.emit(&mut out, ThinkSink::Text, f1);
                    self.fence = Some((tick, n));
                    self.at_line_start = false;
                }
                (_, Some((start, end))) => {
                    self.emit(&mut out, ThinkSink::Text, start);
                    let _ = self.buf.drain(..end - start);
                    self.in_think = true;
                    self.at_line_start = self.buf.starts_with('\n') || self.buf.is_empty();
                }
                (None, None) => {
                    let hold = if eof {
                        0
                    } else {
                        incomplete_tag_hold(&self.buf, OPEN_TAGS).max(trailing_open_fence_hold(
                            &self.buf,
                            self.at_line_start,
                        ))
                    };
                    if self.buf.len() > hold {
                        let take = self.buf.len() - hold;
                        self.emit(&mut out, ThinkSink::Text, take);
                    }
                    return out;
                }
            }
        }
    }

    fn emit(&mut self, out: &mut Vec<(ThinkSink, String)>, sink: ThinkSink, end: usize) {
        if end == 0 {
            return;
        }
        let piece: String = self.buf.drain(..end).collect();
        if piece.is_empty() {
            return;
        }
        self.at_line_start = piece.ends_with('\n');
        if let Some(last) = out.last_mut().filter(|(s, _)| *s == sink) {
            last.1.push_str(&piece);
        } else {
            out.push((sink, piece));
        }
    }
}

fn find_tag(hay: &str, tags: &[&str]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for tag in tags {
        if let Some(i) = find_ci(hay, tag) {
            let end = i + tag.len();
            best = Some(match best {
                Some((b, e)) if b < i || (b == i && e >= end) => (b, e),
                _ => (i, end),
            });
        }
    }
    best
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let n = needle.len();
    if n == 0 || hay.len() < n {
        return None;
    }
    hay.as_bytes()
        .windows(n)
        .position(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
        .filter(|&i| hay.is_char_boundary(i))
}

fn incomplete_tag_hold(s: &str, tags: &[&str]) -> usize {
    let max = tags.iter().map(|t| t.len()).max().unwrap_or(0);
    let start = s.len().saturating_sub(max);
    // Walk char starts only: a CJK chunk like `已` is 3 bytes, and
    // `start..len` includes 1 and 2, which are not char boundaries.
    for (i, _) in s.char_indices() {
        if i < start {
            continue;
        }
        let suf = &s[i..];
        if tags.iter().any(|tag| {
            suf.len() < tag.len()
                && tag.as_bytes()[..suf.len()].eq_ignore_ascii_case(suf.as_bytes())
        }) {
            return s.len() - i;
        }
    }
    0
}

fn find_opening_fence(s: &str, at_line_start: bool) -> Option<(usize, usize, char, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut line_start = at_line_start;
    while i < b.len() {
        if line_start && (b[i] == b'`' || b[i] == b'~') {
            let tick = b[i] as char;
            let mut n = 0;
            while i + n < b.len() && b[i + n] == tick as u8 {
                n += 1;
            }
            if n >= 3 {
                return Some((i, i + n, tick, n));
            }
            i += n.max(1);
            line_start = false;
            continue;
        }
        line_start = b[i] == b'\n';
        i += 1;
    }
    None
}

fn find_closing_fence(s: &str, tick: char, n: usize) -> Option<usize> {
    let b = s.as_bytes();
    let t = tick as u8;
    let mut i = 0;
    let mut line_start = true;
    while i < b.len() {
        if line_start && b[i] == t {
            let mut m = 0;
            while i + m < b.len() && b[i + m] == t {
                m += 1;
            }
            if m >= n {
                return Some(i + m);
            }
            i += m.max(1);
            line_start = false;
            continue;
        }
        line_start = b[i] == b'\n';
        i += 1;
    }
    None
}

fn trailing_fence_hold(s: &str, tick: char) -> usize {
    let t = tick as u8;
    let b = s.as_bytes();
    let mut i = b.len();
    while i > 0 && b[i - 1] == t {
        i -= 1;
    }
    if i < b.len() && (i == 0 || b[i - 1] == b'\n') {
        b.len() - i
    } else {
        0
    }
}

fn trailing_open_fence_hold(s: &str, at_line_start: bool) -> usize {
    let b = s.as_bytes();
    if b.is_empty() {
        return 0;
    }
    let last = b[b.len() - 1];
    if last != b'`' && last != b'~' {
        return 0;
    }
    let mut i = b.len();
    while i > 0 && b[i - 1] == last {
        i -= 1;
    }
    let at_start = i == 0 && at_line_start || i > 0 && b[i - 1] == b'\n';
    if at_start && b.len() - i < 3 {
        b.len() - i
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_all(chunks: &[&str]) -> Vec<(ThinkSink, String)> {
        let mut s = ThinkTagSplitter::new();
        let mut out = Vec::new();
        for c in chunks {
            out.extend(s.push(c));
        }
        out.extend(s.finish());
        let mut merged: Vec<(ThinkSink, String)> = Vec::new();
        for (sink, piece) in out {
            if let Some(last) = merged.last_mut().filter(|(sk, _)| *sk == sink) {
                last.1.push_str(&piece);
            } else {
                merged.push((sink, piece));
            }
        }
        merged
    }

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(
            split_all(&["Hello, ", "world!"]),
            vec![(ThinkSink::Text, "Hello, world!".into())]
        );
    }

    #[test]
    fn think_block_then_answer() {
        assert_eq!(
            split_all(&["<think>secret</think>\nanswer"]),
            vec![
                (ThinkSink::Reasoning, "secret".into()),
                (ThinkSink::Text, "\nanswer".into()),
            ]
        );
    }

    #[test]
    fn thinking_alias_and_case() {
        assert_eq!(
            split_all(&["<THINKING>a</Thinking>b"]),
            vec![
                (ThinkSink::Reasoning, "a".into()),
                (ThinkSink::Text, "b".into()),
            ]
        );
    }

    #[test]
    fn tags_split_across_chunks() {
        assert_eq!(
            split_all(&["<thi", "nk>ab", "c</th", "ink>ok"]),
            vec![
                (ThinkSink::Reasoning, "abc".into()),
                (ThinkSink::Text, "ok".into()),
            ]
        );
    }

    #[test]
    fn unclosed_think_becomes_reasoning_at_eof() {
        assert_eq!(
            split_all(&["<think>\npartial"]),
            vec![(ThinkSink::Reasoning, "\npartial".into())]
        );
    }

    #[test]
    fn unclosed_open_tag_at_eof_is_text() {
        assert_eq!(
            split_all(&["hello <thi"]),
            vec![(ThinkSink::Text, "hello <thi".into())]
        );
    }

    #[test]
    fn tags_inside_fence_stay_text() {
        let src = "```\n<think>nope</think>\n```\nlater";
        assert_eq!(
            split_all(&[src]),
            vec![(ThinkSink::Text, src.into())]
        );
    }

    #[test]
    fn less_than_in_prose_is_not_a_tag() {
        assert_eq!(
            split_all(&["if a < b then c"]),
            vec![(ThinkSink::Text, "if a < b then c".into())]
        );
    }

    #[test]
    fn multiple_think_regions() {
        assert_eq!(
            split_all(&["<think>one</think>mid<think>two</think>end"]),
            vec![
                (ThinkSink::Reasoning, "one".into()),
                (ThinkSink::Text, "mid".into()),
                (ThinkSink::Reasoning, "two".into()),
                (ThinkSink::Text, "end".into()),
            ]
        );
    }

    #[test]
    fn stray_close_tag_is_text() {
        assert_eq!(
            split_all(&["nope</think>ok"]),
            vec![(ThinkSink::Text, "nope</think>ok".into())]
        );
    }

    #[test]
    fn fence_split_across_chunks_hides_tags() {
        assert_eq!(
            split_all(&["``", "`\n<think>nope</think>\n", "```\nok"]),
            vec![(ThinkSink::Text, "```\n<think>nope</think>\n```\nok".into())]
        );
    }

    #[test]
    fn longer_thinking_tag_wins_over_think() {
        assert_eq!(
            split_all(&["<thinking>x</thinking>y"]),
            vec![
                (ThinkSink::Reasoning, "x".into()),
                (ThinkSink::Text, "y".into()),
            ]
        );
    }

    #[test]
    fn unclosed_thinking_alias_at_eof() {
        assert_eq!(
            split_all(&["<thinking>\npartial"]),
            vec![(ThinkSink::Reasoning, "\npartial".into())]
        );
    }

    #[test]
    fn cjk_chunk_is_not_sliced_mid_char() {
        assert_eq!(
            split_all(&["已", "完成"]),
            vec![(ThinkSink::Text, "已完成".into())]
        );
    }

    #[test]
    fn cjk_then_incomplete_open_tag_at_eof() {
        assert_eq!(
            split_all(&["已<thi"]),
            vec![(ThinkSink::Text, "已<thi".into())]
        );
    }
}
