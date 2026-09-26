use regex::Regex;
use std::sync::LazyLock;

// Capture the prefix instead of using lookahead: these patterns need no
// backtracking engine. Reuse them across streamed chunks and agent instances.
pub(super) static COMMAND_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*?)\(").unwrap());
pub(super) static SENTENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r".*?(?:\r\n|\n|\r|[.!?。！？])").unwrap());

pub(super) fn split_sentences(input: &str) -> (Vec<&str>, &str) {
    let mut sentences = Vec::new();
    let mut tail_start = 0;
    for sentence in SENTENCE.find_iter(input) {
        sentences.push(sentence.as_str());
        tail_start = sentence.end();
    }
    (sentences, &input[tail_start..])
}

pub(super) fn split_speech(input: &str, flush_tail: bool) -> (Vec<&str>, &str) {
    let (mut sentences, tail) = split_sentences(input);
    if flush_tail && !tail.trim().is_empty() {
        sentences.push(tail);
        (sentences, "")
    } else {
        (sentences, tail)
    }
}

pub(super) fn is_speakable_sentence(sentence: &str) -> bool {
    !sentence
        .trim()
        .trim_matches(['.', ',', '!', '?', '。', '！', '？'])
        .trim()
        .is_empty()
}

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
        assert_eq!(sentences, ["Hello!", "世界。 Really?", "\r\n", "Yes."]);
        assert_eq!(SENTENCE.find_iter("unfinished").count(), 0);
    }

    #[test]
    fn sentence_split_preserves_unicode_and_exact_tail() {
        let text = "你好!Café? final tail";
        let (sentences, tail) = split_sentences(text);
        assert_eq!(sentences, ["你好!", "Café?"]);
        assert_eq!(tail, " final tail");
    }

    #[test]
    fn complete_speech_flushes_unpunctuated_tail() {
        let (sentences, tail) = split_speech("Hello there", true);
        assert_eq!(sentences, ["Hello there"]);
        assert_eq!(tail, "");
    }

    #[test]
    fn streaming_speech_retains_unpunctuated_tail() {
        let (sentences, tail) = split_speech("Hello wor", false);
        assert!(sentences.is_empty());
        assert_eq!(tail, "Hello wor");
    }

    #[test]
    fn crlf_is_one_delimiter_and_whitespace_is_not_speakable() {
        let (sentences, tail) = split_sentences("Hello\r\nworld");
        assert_eq!(sentences, ["Hello\r\n"]);
        assert_eq!(tail, "world");
        assert!(!is_speakable_sentence(" \r\n "));
    }

    #[test]
    fn apostrophes_and_unicode_sentence_punctuation_are_preserved() {
        let (sentences, tail) = split_speech("HERE'S a poem!こんにちは。次です！", true);
        assert_eq!(sentences, ["HERE'S a poem!", "こんにちは。", "次です！"]);
        assert_eq!(tail, "");
    }
}
