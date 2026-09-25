use regex::Regex;
use std::sync::LazyLock;

// Capture the prefix instead of using lookahead: these patterns need no
// backtracking engine. Reuse them across streamed chunks and agent instances.
pub(super) static COMMAND_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*?)\(").unwrap());
pub(super) static SENTENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r".*?(?:\n|\r|\.|\?|!)").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_prefix_preserves_streaming_and_unicode_boundaries() {
        for (text, expected) in [
            ("speak(\"hello\")", Some("speak")),
            ("speak(\"unfinished", Some("speak")),
            ("\nspeak(", Some("speak")),
            ("first\nsecond(", Some("second")),
            (" first(another(", Some(" first")),
            ("話す(\"こんにちは\")", Some("話す")),
            ("(", Some("")),
            ("unfinished", None),
        ] {
            let captures = COMMAND_NAME.captures(text);
            assert_eq!(captures.as_ref().map(|c| &c[1]), expected, "{text:?}");
        }
    }

    #[test]
    fn sentences_leave_unterminated_stream_tail_unmatched() {
        let text = "Hello!世界。 Really?\r\nYes. unfinished";
        let sentences: Vec<_> = SENTENCE.find_iter(text).map(|m| m.as_str()).collect();
        assert_eq!(sentences, ["Hello!", "世界。 Really?", "\r", "\n", "Yes."]);
        assert_eq!(SENTENCE.find_iter("unfinished").count(), 0);
    }
}
