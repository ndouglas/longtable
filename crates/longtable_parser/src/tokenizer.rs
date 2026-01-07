//! Input tokenization.
//!
//! Converts raw player input into a stream of tokens.

/// A token from player input.
#[derive(Clone, Debug, PartialEq)]
pub enum InputToken {
    /// A lowercase word
    Word(String),
    /// A quoted string (preserved as-is)
    QuotedString(String),
    /// End of input
    End,
}

/// Tokenizes player input.
pub struct InputTokenizer;

impl InputTokenizer {
    /// Tokenizes a raw input string into tokens.
    ///
    /// - Converts words to lowercase
    /// - Strips punctuation (except within quotes)
    /// - Preserves quoted strings as atomic units
    #[must_use]
    pub fn tokenize(input: &str) -> Vec<InputToken> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();
        let mut current_word = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                // Start of quoted string
                '"' => {
                    // Flush current word if any
                    if !current_word.is_empty() {
                        tokens.push(InputToken::Word(current_word.to_lowercase()));
                        current_word.clear();
                    }
                    // Collect quoted string
                    let mut quoted = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '"' {
                            chars.next();
                            break;
                        }
                        quoted.push(chars.next().unwrap());
                    }
                    tokens.push(InputToken::QuotedString(quoted));
                }
                // Whitespace - end of word
                ' ' | '\t' | '\n' | '\r' => {
                    if !current_word.is_empty() {
                        tokens.push(InputToken::Word(current_word.to_lowercase()));
                        current_word.clear();
                    }
                }
                // Punctuation to strip
                '.' | ',' | '!' | '?' | ';' | ':' | '\'' => {
                    // Skip punctuation (could handle contractions here)
                }
                // Regular character
                _ => {
                    current_word.push(ch);
                }
            }
        }

        // Flush final word
        if !current_word.is_empty() {
            tokens.push(InputToken::Word(current_word.to_lowercase()));
        }

        tokens.push(InputToken::End);
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let tokens = InputTokenizer::tokenize("take sword");
        assert_eq!(
            tokens,
            vec![
                InputToken::Word("take".to_string()),
                InputToken::Word("sword".to_string()),
                InputToken::End,
            ]
        );
    }

    #[test]
    fn test_tokenize_lowercase() {
        let tokens = InputTokenizer::tokenize("Take SWORD");
        assert_eq!(
            tokens,
            vec![
                InputToken::Word("take".to_string()),
                InputToken::Word("sword".to_string()),
                InputToken::End,
            ]
        );
    }

    #[test]
    fn test_tokenize_strips_punctuation() {
        let tokens = InputTokenizer::tokenize("take sword!");
        assert_eq!(
            tokens,
            vec![
                InputToken::Word("take".to_string()),
                InputToken::Word("sword".to_string()),
                InputToken::End,
            ]
        );
    }

    #[test]
    fn test_tokenize_quoted_string() {
        let tokens = InputTokenizer::tokenize("say \"Hello world\"");
        assert_eq!(
            tokens,
            vec![
                InputToken::Word("say".to_string()),
                InputToken::QuotedString("Hello world".to_string()),
                InputToken::End,
            ]
        );
    }
}
