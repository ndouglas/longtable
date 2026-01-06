//! Line editor abstraction for the REPL.
//!
//! This module provides a trait-based abstraction over line editing libraries,
//! allowing the REPL to use rustyline while remaining swappable.

use crate::highlight::LongtableHighlighter;
use longtable_foundation::{Error, ErrorKind, Result};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::HistoryHinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Completer, Config, Context, Editor, Helper, Hinter, Validator as RLValidator};
use std::borrow::Cow;

/// Result of reading a line from the editor.
#[derive(Debug)]
pub enum ReadResult {
    /// A line was successfully read.
    Line(String),
    /// User pressed Ctrl+C.
    Interrupted,
    /// User pressed Ctrl+D (EOF).
    Eof,
}

/// Abstraction over line editing functionality.
///
/// This trait allows swapping out the underlying line editor implementation
/// (e.g., from rustyline to reedline) without changing the REPL code.
pub trait LineEditor {
    /// Read a line with the given prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the terminal fails.
    fn read_line(&mut self, prompt: &str) -> Result<ReadResult>;

    /// Read a continuation line (for multi-line input).
    ///
    /// # Errors
    ///
    /// Returns an error if reading from the terminal fails.
    fn read_continuation(&mut self, prompt: &str) -> Result<ReadResult>;

    /// Add a line to history.
    fn add_history(&mut self, line: &str);

    /// Set available completions for keywords.
    fn set_keywords(&mut self, keywords: Vec<String>);
}

/// Helper for rustyline that provides completion, hints, highlighting, and validation.
#[derive(Helper, Completer, Hinter, RLValidator)]
struct LongtableHelper {
    #[rustyline(Completer)]
    completer: LongtableCompleter,
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    #[rustyline(Validator)]
    validator: BracketValidator,
    highlighter: LongtableHighlighter,
}

impl Highlighter for LongtableHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Cow::Owned(format!("\x1b[1;32m{prompt}\x1b[0m"))
        } else {
            Cow::Borrowed(prompt)
        }
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2m{hint}\x1b[0m"))
    }
}

/// Completer for Longtable keywords and file paths.
struct LongtableCompleter {
    file_completer: FilenameCompleter,
    keywords: Vec<String>,
}

impl LongtableCompleter {
    fn new() -> Self {
        Self {
            file_completer: FilenameCompleter::new(),
            keywords: Self::default_keywords(),
        }
    }

    fn default_keywords() -> Vec<String> {
        vec![
            // Special forms
            "def".into(),
            "fn".into(),
            "let".into(),
            "if".into(),
            "do".into(),
            "quote".into(),
            "load".into(),
            "query".into(),
            // Declarations
            "component:".into(),
            "relationship:".into(),
            "rule:".into(),
            "derived:".into(),
            "constraint:".into(),
            // Declaration keywords
            ":where".into(),
            ":let".into(),
            ":guard".into(),
            ":then".into(),
            ":return".into(),
            ":salience".into(),
            ":once".into(),
            ":enabled".into(),
            ":for".into(),
            ":value".into(),
            ":check".into(),
            ":on-violation".into(),
            ":storage".into(),
            ":cardinality".into(),
            ":on-target-delete".into(),
            ":attributes".into(),
            ":default".into(),
            ":aggregate".into(),
            ":group-by".into(),
            ":order-by".into(),
            ":limit".into(),
            // Types
            ":int".into(),
            ":float".into(),
            ":bool".into(),
            ":string".into(),
            ":keyword".into(),
            ":symbol".into(),
            ":entity-ref".into(),
            ":map".into(),
            ":vec".into(),
            ":set".into(),
            // Common functions
            "+".into(),
            "-".into(),
            "*".into(),
            "/".into(),
            "mod".into(),
            "=".into(),
            "!=".into(),
            "<".into(),
            "<=".into(),
            ">".into(),
            ">=".into(),
            "not".into(),
            "and".into(),
            "or".into(),
            "nil?".into(),
            "some?".into(),
            "int?".into(),
            "float?".into(),
            "string?".into(),
            "keyword?".into(),
            "vector?".into(),
            "map?".into(),
            "set?".into(),
            "count".into(),
            "first".into(),
            "rest".into(),
            "nth".into(),
            "get".into(),
            "assoc".into(),
            "dissoc".into(),
            "conj".into(),
            "cons".into(),
            "contains?".into(),
            "keys".into(),
            "vals".into(),
            "str".into(),
            "str/len".into(),
            "str/upper".into(),
            "str/lower".into(),
            "abs".into(),
            "floor".into(),
            "ceil".into(),
            "round".into(),
            "sqrt".into(),
            "min".into(),
            "max".into(),
            "type".into(),
            "print!".into(),
            "spawn!".into(),
            "destroy!".into(),
            "set!".into(),
            "link!".into(),
            "unlink!".into(),
        ]
    }

    #[allow(dead_code)]
    fn set_keywords(&mut self, keywords: Vec<String>) {
        self.keywords = keywords;
    }
}

impl Completer for LongtableCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Find the start of the current word
        let start = line[..pos]
            .rfind(|c: char| c.is_whitespace() || "()[]{}".contains(c))
            .map_or(0, |i| i + 1);

        let word = &line[start..pos];

        // If inside a string (after a quote), complete file paths
        if line[..pos].chars().filter(|&c| c == '"').count() % 2 == 1 {
            return self.file_completer.complete(line, pos, ctx);
        }

        // Otherwise, complete keywords
        let candidates: Vec<Pair> = self
            .keywords
            .iter()
            .filter(|kw| kw.starts_with(word))
            .map(|kw| Pair {
                display: kw.clone(),
                replacement: kw.clone(),
            })
            .collect();

        Ok((start, candidates))
    }
}

/// Validator for bracket matching (enables multi-line input).
#[derive(Default)]
struct BracketValidator;

impl Validator for BracketValidator {
    fn validate(&self, ctx: &mut ValidationContext<'_>) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;

        for c in input.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '(' | '[' | '{' if !in_string => depth += 1,
                ')' | ']' | '}' if !in_string => depth -= 1,
                _ => {}
            }
        }

        if depth > 0 {
            // Show which characters are expected
            Ok(ValidationResult::Incomplete)
        } else {
            Ok(ValidationResult::Valid(None))
        }
    }
}

/// Line editor implementation using rustyline.
pub struct RustylineEditor {
    editor: Editor<LongtableHelper, DefaultHistory>,
}

impl RustylineEditor {
    /// Creates a new rustyline-based editor.
    ///
    /// # Errors
    ///
    /// Returns an error if rustyline initialization fails.
    ///
    /// # Panics
    ///
    /// Panics if the history size configuration is invalid (should not happen
    /// with hardcoded valid values).
    pub fn new() -> Result<Self> {
        let config = Config::builder()
            .auto_add_history(false)
            .max_history_size(1000)
            .expect("valid history size")
            .build();

        let helper = LongtableHelper {
            completer: LongtableCompleter::new(),
            hinter: HistoryHinter::new(),
            validator: BracketValidator,
            highlighter: LongtableHighlighter::new(),
        };

        let mut editor = Editor::with_config(config)
            .map_err(|e| Error::new(ErrorKind::Internal(e.to_string())))?;
        editor.set_helper(Some(helper));

        Ok(Self { editor })
    }
}

impl LineEditor for RustylineEditor {
    fn read_line(&mut self, prompt: &str) -> Result<ReadResult> {
        match self.editor.readline(prompt) {
            Ok(line) => Ok(ReadResult::Line(line)),
            Err(ReadlineError::Interrupted) => Ok(ReadResult::Interrupted),
            Err(ReadlineError::Eof) => Ok(ReadResult::Eof),
            Err(e) => Err(Error::new(ErrorKind::Internal(e.to_string()))),
        }
    }

    fn read_continuation(&mut self, prompt: &str) -> Result<ReadResult> {
        self.read_line(prompt)
    }

    fn add_history(&mut self, line: &str) {
        let _ = self.editor.add_history_entry(line);
    }

    fn set_keywords(&mut self, keywords: Vec<String>) {
        if let Some(helper) = self.editor.helper_mut() {
            helper.completer.keywords = keywords;
        }
    }
}
