#[derive(Default)]
pub struct SpeechChunks {
    buffer: String,
}

impl SpeechChunks {
    pub fn push(&mut self, text: &str, final_chunk: bool) -> Vec<String> {
        self.buffer.push_str(text);
        let mut chunks = Vec::new();
        while !self.buffer.trim().is_empty() {
            let chars: Vec<_> = self.buffer.char_indices().collect();
            let mut boundary = None;
            for (i, &(_, ch)) in chars.iter().enumerate() {
                if !".!?,;:".contains(ch) {
                    continue;
                }
                let sentence = ".!?".contains(ch);
                let mut next = i + 1;
                if sentence {
                    while next < chars.len() && ".!?\"'”’)".contains(chars[next].1) {
                        next += 1;
                    }
                }
                if next == chars.len() || !chars[next].1.is_whitespace() {
                    continue;
                }
                let end = chars[next].0;
                let prefix = self.buffer[..end].trim();
                if sentence && abbreviation(prefix) {
                    continue;
                }
                if sentence || prefix.chars().count() >= 45 {
                    boundary = Some(end);
                    break;
                }
            }
            if chars.len() > 180 {
                let limit = chars[180].0;
                if boundary.is_none_or(|end| end > limit) {
                    boundary = Some(
                        self.buffer[..limit]
                            .rfind(char::is_whitespace)
                            .filter(|&i| i > 0)
                            .unwrap_or(limit),
                    );
                }
            }
            let end = match boundary.or_else(|| final_chunk.then_some(self.buffer.len())) {
                Some(end) => end,
                None => break,
            };
            let chunk = self.buffer[..end].trim().to_string();
            self.buffer = self.buffer[end..].trim_start().to_string();
            if !chunk.is_empty() {
                chunks.push(chunk);
            }
        }
        chunks
    }
}

fn abbreviation(prefix: &str) -> bool {
    let Some(word) = prefix
        .strip_suffix('.')
        .and_then(|text| text.split_whitespace().last())
    else {
        return false;
    };
    matches!(
        word.to_ascii_lowercase().as_str(),
        "mr" | "mrs" | "ms" | "dr" | "prof" | "sr" | "jr" | "st" | "vs" | "etc"
    ) || (word.len() == 1 && word.chars().all(|ch| ch.is_ascii_alphabetic()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_sentences_and_preserves_split_decimals_and_abbreviations() {
        let mut chunks = SpeechChunks::default();
        assert!(chunks.push("Dr. Smith paid 3.", false).is_empty());
        assert_eq!(
            chunks.push("14 dollars. Hello there! Next", false),
            ["Dr. Smith paid 3.14 dollars.", "Hello there!"]
        );
        assert_eq!(chunks.push("", true), ["Next"]);
    }

    #[test]
    fn emits_clauses_and_bounds_unicode_chunks_without_losing_text() {
        let mut chunks = SpeechChunks::default();
        let clause = "When the recording finishes and the text is ready,";
        assert_eq!(chunks.push(&format!("{clause} next"), false), [clause]);
        assert_eq!(chunks.push("", true), ["next"]);
        let text = "é".repeat(401);
        let mut output = chunks.push(&text, false);
        output.extend(chunks.push("", true));
        assert_eq!(output.concat(), text);
        assert!(output.iter().all(|part| part.chars().count() <= 180));
    }
}
