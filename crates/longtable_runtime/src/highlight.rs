//! Syntax highlighting for the REPL.

use std::borrow::Cow;

/// Highlighter for Longtable DSL syntax.
pub struct LongtableHighlighter {
    // Could cache compiled regexes here if needed
}

impl LongtableHighlighter {
    /// Creates a new highlighter.
    pub const fn new() -> Self {
        Self {}
    }

    /// Highlight a line of input.
    #[allow(clippy::unused_self, clippy::too_many_lines)]
    pub fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let mut result = String::with_capacity(line.len() * 2);
        let mut chars = line.chars().peekable();
        let mut in_string = false;
        let mut in_comment = false;

        while let Some(c) = chars.next() {
            if in_comment {
                result.push(c);
                continue;
            }

            match c {
                // Comments
                ';' if !in_string => {
                    in_comment = true;
                    result.push_str("\x1b[2;3m"); // dim italic
                    result.push(c);
                }

                // Strings
                '"' => {
                    if in_string {
                        result.push(c);
                        result.push_str("\x1b[0m");
                        in_string = false;
                    } else {
                        result.push_str("\x1b[33m"); // yellow
                        result.push(c);
                        in_string = true;
                    }
                }

                // Escape in string
                '\\' if in_string => {
                    result.push(c);
                    if let Some(next) = chars.next() {
                        result.push(next);
                    }
                }

                // Keywords
                ':' if !in_string => {
                    result.push_str("\x1b[36m"); // cyan
                    result.push(c);
                    while let Some(&next) = chars.peek() {
                        if next.is_alphanumeric() || next == '-' || next == '/' || next == '_' {
                            result.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    result.push_str("\x1b[0m");
                }

                // Numbers
                c if c.is_ascii_digit() && !in_string => {
                    result.push_str("\x1b[35m"); // magenta
                    result.push(c);
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() || next == '.' || next == '_' {
                            result.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    result.push_str("\x1b[0m");
                }

                // Negative numbers
                '-' if !in_string => {
                    if let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            result.push_str("\x1b[35m"); // magenta
                            result.push(c);
                            while let Some(&next) = chars.peek() {
                                if next.is_ascii_digit() || next == '.' || next == '_' {
                                    result.push(chars.next().unwrap());
                                } else {
                                    break;
                                }
                            }
                            result.push_str("\x1b[0m");
                        } else {
                            result.push(c);
                        }
                    } else {
                        result.push(c);
                    }
                }

                // Variables (?name)
                '?' if !in_string => {
                    result.push_str("\x1b[34m"); // blue
                    result.push(c);
                    while let Some(&next) = chars.peek() {
                        if next.is_alphanumeric() || next == '-' || next == '_' {
                            result.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    result.push_str("\x1b[0m");
                }

                // Delimiters - bright
                '(' | ')' | '[' | ']' | '{' | '}' if !in_string => {
                    result.push_str("\x1b[1m"); // bold
                    result.push(c);
                    result.push_str("\x1b[0m");
                }

                // Special forms and declarations (check for symbol start)
                c if c.is_alphabetic() && !in_string => {
                    let mut word = String::new();
                    word.push(c);
                    while let Some(&next) = chars.peek() {
                        if next.is_alphanumeric()
                            || next == '-'
                            || next == '/'
                            || next == '_'
                            || next == '!'
                            || next == '?'
                            || next == ':'
                        {
                            word.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }

                    // Color based on word type
                    let color = match word.as_str() {
                        // Special forms - green
                        "def" | "fn" | "let" | "if" | "do" | "quote" | "loop" | "recur" | "try"
                        | "match" => "\x1b[32m",

                        // Declarations and query - bold green
                        "component:" | "relationship:" | "rule:" | "derived:" | "constraint:"
                        | "query" => "\x1b[1;32m",

                        // Booleans and nil - blue
                        "true" | "false" | "nil" => "\x1b[34m",

                        // Side-effecting functions - red
                        _ if word.ends_with('!') => "\x1b[31m",

                        // Predicates - yellow
                        _ if word.ends_with('?') => "\x1b[33m",

                        // Regular symbols
                        _ => "",
                    };

                    if color.is_empty() {
                        result.push_str(&word);
                    } else {
                        result.push_str(color);
                        result.push_str(&word);
                        result.push_str("\x1b[0m");
                    }
                }

                // Everything else in string or regular char
                _ => result.push(c),
            }
        }

        // Reset at end
        if in_comment || in_string {
            result.push_str("\x1b[0m");
        }

        Cow::Owned(result)
    }
}

impl Default for LongtableHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
