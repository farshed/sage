use crate::tts::vocab::VOCAB;

/// Tokenizes the given phonemes string into a vector of token indices.
///
/// This function takes a text string as input and converts it into a vector of token indices
/// by looking up each character in the global `VOCAB` map and mapping it to the corresponding
/// token index. The resulting vector contains the token indices for the input text.
///
/// # Arguments
/// * `text` - The input text string to be tokenized.
///
/// # Returns
/// A vector of `i64` token indices representing the input text.
pub fn tokenize(phonemes: &str) -> Vec<i64> {
    phonemes
        .chars()
        .filter_map(|c| VOCAB.get(&c))
        .map(|&idx| idx as i64)
        .collect()
}

use crate::tts::vocab::REVERSE_VOCAB;

pub fn tokens_to_phonemes(tokens: &[i64]) -> String {
    tokens
        .iter()
        .filter_map(|&t| REVERSE_VOCAB.get(&(t as usize)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_phonemes_and_punctuation() {
        let text = "heɪ ðɪs ɪz ˈlʌvliː!";
        assert_eq!(tokens_to_phonemes(&tokenize(text)), text);
        assert_eq!(tokenize("..."), [4, 4, 4]);
    }

    #[test]
    fn handles_empty_input_and_unknown_symbols() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("😀").is_empty());
        assert_eq!(tokens_to_phonemes(&[]), "");
        assert_eq!(tokens_to_phonemes(&[-1, 9999]), "");
    }
}
