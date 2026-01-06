//! The main REPL implementation.

use crate::editor::{LineEditor, ReadResult, RustylineEditor};
use crate::session::Session;
use longtable_foundation::{Error, ErrorKind, Result, Value};
use longtable_language::{
    Compiler, Declaration, DeclarationAnalyzer, NamespaceContext, NamespaceInfo, Vm, parse,
};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// The interactive REPL.
pub struct Repl<E: LineEditor = RustylineEditor> {
    /// The line editor for input.
    editor: E,

    /// Session state (world, variables).
    session: Session,

    /// The bytecode VM for evaluation.
    vm: Vm,

    /// Whether to show the welcome banner.
    show_banner: bool,

    /// Primary prompt.
    prompt: String,

    /// Continuation prompt (for multi-line input).
    continuation_prompt: String,
}

impl Repl<RustylineEditor> {
    /// Creates a new REPL with the default rustyline editor.
    ///
    /// # Errors
    ///
    /// Returns an error if the editor fails to initialize.
    pub fn new() -> Result<Self> {
        let editor = RustylineEditor::new()?;
        Ok(Self::with_editor(editor))
    }
}

impl<E: LineEditor> Repl<E> {
    /// Creates a new REPL with the given editor.
    pub fn with_editor(editor: E) -> Self {
        Self {
            editor,
            session: Session::new(),
            vm: Vm::new(),
            show_banner: true,
            prompt: "λ> ".to_string(),
            continuation_prompt: ".. ".to_string(),
        }
    }

    /// Sets the session for this REPL.
    #[must_use]
    pub fn with_session(mut self, session: Session) -> Self {
        self.session = session;
        self
    }

    /// Disables the welcome banner.
    #[must_use]
    pub const fn without_banner(mut self) -> Self {
        self.show_banner = false;
        self
    }

    /// Sets the primary prompt.
    #[must_use]
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Returns a reference to the session.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// Returns a mutable reference to the session.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.session
    }

    /// Runs the REPL loop.
    ///
    /// # Errors
    ///
    /// Returns an error if reading input or evaluation fails fatally.
    pub fn run(&mut self) -> Result<()> {
        if self.show_banner {
            self.print_banner();
        }

        loop {
            match self.read_eval_print() {
                Ok(true) => {}
                Ok(false) => break,
                Err(e) => {
                    self.print_error(&e);
                }
            }
        }

        println!("\nGoodbye!");
        Ok(())
    }

    /// Executes one read-eval-print iteration.
    ///
    /// Returns `Ok(true)` to continue, `Ok(false)` to exit.
    fn read_eval_print(&mut self) -> Result<bool> {
        // Read input
        let Some(input) = self.read_input()? else {
            return Ok(false); // EOF
        };

        // Skip empty lines
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(true);
        }

        // Add to history
        self.editor.add_history(&input);

        // Eval and print
        match self.eval(&input) {
            Ok(value) => {
                if value != Value::Nil {
                    println!("{}", self.format_value(&value));
                }
            }
            Err(e) => {
                self.print_error(&e);
            }
        }

        Ok(true)
    }

    /// Reads a potentially multi-line input.
    fn read_input(&mut self) -> Result<Option<String>> {
        let mut input = String::new();
        let mut first_line = true;

        loop {
            let prompt = if first_line {
                &self.prompt
            } else {
                &self.continuation_prompt
            };

            match self.editor.read_line(prompt)? {
                ReadResult::Line(line) => {
                    if first_line {
                        input = line;
                    } else {
                        input.push('\n');
                        input.push_str(&line);
                    }

                    // Check if input is complete
                    if self.is_complete(&input) {
                        return Ok(Some(input));
                    }

                    first_line = false;
                }
                ReadResult::Interrupted => {
                    if first_line {
                        println!();
                        return Ok(Some(String::new()));
                    }
                    println!("\nInput cancelled.");
                    return Ok(Some(String::new()));
                }
                ReadResult::Eof => {
                    if first_line {
                        return Ok(None);
                    }
                    return Err(Error::new(ErrorKind::Internal(
                        "unexpected EOF in multi-line input".to_string(),
                    )));
                }
            }
        }
    }

    /// Checks if input is syntactically complete (balanced brackets).
    #[allow(clippy::unused_self)]
    fn is_complete(&self, input: &str) -> bool {
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

        depth <= 0 && !in_string
    }

    /// Evaluates input and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, compilation, or execution fails.
    pub fn eval(&mut self, input: &str) -> Result<Value> {
        // Parse
        let forms = parse(input)?;

        // Evaluate each form
        let mut result = Value::Nil;
        for form in forms {
            result = self.eval_form(&form)?;
        }

        Ok(result)
    }

    /// Evaluates a single form.
    fn eval_form(&mut self, form: &longtable_language::Ast) -> Result<Value> {
        // Check for special REPL forms
        if let Some(result) = self.try_special_form(form)? {
            return Ok(result);
        }

        // Compile and execute
        // Use a fresh compiler for each expression to avoid state leakage
        let mut compiler = Compiler::new();
        let program = compiler.compile(&[form.clone()])?;

        self.vm.execute(&program)
    }

    /// Tries to handle special REPL forms (def, load).
    fn try_special_form(&mut self, form: &longtable_language::Ast) -> Result<Option<Value>> {
        use longtable_language::Ast;

        let list = match form {
            Ast::List(elements, _) if !elements.is_empty() => elements,
            _ => return Ok(None),
        };

        match &list[0] {
            // (def name value)
            Ast::Symbol(s, _) if s == "def" => {
                if list.len() != 3 {
                    return Err(Error::new(ErrorKind::Internal(
                        "def requires exactly 2 arguments: (def name value)".to_string(),
                    )));
                }

                let name = match &list[1] {
                    Ast::Symbol(n, _) => n.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "def name must be a symbol, got {}",
                            other.type_name()
                        ))));
                    }
                };

                // Evaluate the value
                let value = self.eval_form(&list[2])?;

                // Store in session
                self.session.set_variable(name, value.clone());

                Ok(Some(value))
            }

            // (load "path")
            Ast::Symbol(s, _) if s == "load" => {
                if list.len() != 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "load requires exactly 1 argument: (load \"path\")".to_string(),
                    )));
                }

                let path = match &list[1] {
                    Ast::String(p, _) => p.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "load path must be a string, got {}",
                            other.type_name()
                        ))));
                    }
                };

                self.load_file(&path)?;
                Ok(Some(Value::Nil))
            }

            _ => Ok(None),
        }
    }

    /// Loads and evaluates a file.
    ///
    /// If the path is a directory containing a `_.lt` file, that file is loaded.
    /// Uses cycle detection to prevent recursive loading.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, contains cyclic loads, or evaluation fails.
    pub fn load_file(&mut self, path: &str) -> Result<()> {
        // Resolve path relative to current load path
        let resolved = self.session.resolve_path(path);

        // Try with .lt extension if not specified
        let has_lt_ext = Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lt"));

        let file_path = if resolved.is_dir() {
            // If it's a directory, look for _.lt inside
            let entry_file = resolved.join("_.lt");
            if entry_file.exists() {
                entry_file
            } else {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "directory '{path}' does not contain _.lt entry file"
                ))));
            }
        } else if resolved.exists() {
            resolved
        } else if !has_lt_ext {
            let with_ext = self.session.resolve_path(&format!("{path}.lt"));
            if with_ext.exists() {
                with_ext
            } else {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "file not found: {path}"
                ))));
            }
        } else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "file not found: {path}"
            ))));
        };

        // Canonicalize for consistent cycle detection
        let canonical = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone());

        // Check if already loaded (skip re-loading)
        if self.session.module_registry().has_file(&canonical) {
            return Ok(());
        }

        // Begin loading (cycle detection)
        self.session
            .module_registry_mut()
            .begin_loading(canonical.clone())?;

        // Read file
        let source = fs::read_to_string(&file_path).map_err(|e| {
            // Clean up loading state on error
            self.session
                .module_registry_mut()
                .finish_loading(&canonical);
            Error::new(ErrorKind::Internal(format!(
                "failed to read {}: {e}",
                file_path.display()
            )))
        })?;

        // Save and update load path
        let old_path = self.session.load_path().clone();
        if let Some(parent) = file_path.parent() {
            self.session.set_load_path(parent.to_path_buf());
        }

        // Evaluate with file context
        let result = self.eval_with_file_context(&source, &canonical);

        // Restore load path
        self.session.set_load_path(old_path);

        // Finish loading (remove from loading stack)
        self.session
            .module_registry_mut()
            .finish_loading(&canonical);

        result.map(|_| ())
    }

    /// Evaluates source code within a file context.
    ///
    /// Handles namespace declarations and registers the file in the module registry.
    fn eval_with_file_context(
        &mut self,
        source: &str,
        file_path: &std::path::Path,
    ) -> Result<Value> {
        // Parse
        let forms = parse(source)?;

        // Check for namespace declaration at the beginning
        if let Some(first_form) = forms.first() {
            if let Some(Declaration::Namespace(ns_decl)) = DeclarationAnalyzer::analyze(first_form)?
            {
                // Build namespace context from declaration
                let ns_context = NamespaceContext::from_decl(&ns_decl);

                // Register the namespace
                let ns_info = NamespaceInfo::new(ns_decl, file_path.to_path_buf());
                self.session
                    .module_registry_mut()
                    .register_namespace(ns_info);

                // Set as current namespace context for compilation
                self.session.set_namespace_context(ns_context);
            }
        }

        // Evaluate each form
        let mut result = Value::Nil;
        for form in forms {
            result = self.eval_form(&form)?;
        }

        Ok(result)
    }

    /// Evaluates a file without changing the load path (for CLI batch mode).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or evaluated.
    pub fn eval_file(&mut self, path: &Path) -> Result<Value> {
        let source = fs::read_to_string(path).map_err(|e| {
            Error::new(ErrorKind::Internal(format!(
                "failed to read {}: {e}",
                path.display()
            )))
        })?;

        // Set load path to file's directory
        if let Some(parent) = path.parent() {
            self.session.set_load_path(parent.to_path_buf());
        }

        self.eval(&source)
    }

    /// Formats a value for display.
    #[allow(clippy::unused_self)]
    fn format_value(&self, value: &Value) -> String {
        // Use Display formatting from Value
        format!("\x1b[1m{value}\x1b[0m")
    }

    /// Prints an error to stderr.
    #[allow(clippy::unused_self)]
    fn print_error(&self, error: &Error) {
        eprintln!("\x1b[31mError: {error}\x1b[0m");
    }

    /// Prints the welcome banner.
    #[allow(clippy::unused_self)]
    fn print_banner(&self) {
        println!("\x1b[1;36m");
        println!("  _                        _        _     _      ");
        println!(" | |    ___  _ __   __ _ _| |_ __ _| |__ | | ___ ");
        println!(" | |   / _ \\| '_ \\ / _` |_   _/ _` | '_ \\| |/ _ \\");
        println!(" | |__| (_) | | | | (_| | | || (_| | |_) | |  __/");
        println!(" |_____\\___/|_| |_|\\__, | |_| \\__,_|_.__/|_|\\___|");
        println!("                   |___/                         ");
        println!("\x1b[0m");
        println!("Welcome to Longtable REPL v{}", env!("CARGO_PKG_VERSION"));
        println!("Type expressions to evaluate. Use Ctrl+D to exit.\n");

        // Flush to ensure banner appears
        let _ = io::stdout().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple mock editor for testing.
    struct MockEditor {
        inputs: Vec<String>,
        index: usize,
    }

    impl MockEditor {
        fn new(inputs: Vec<&str>) -> Self {
            Self {
                inputs: inputs.into_iter().map(String::from).collect(),
                index: 0,
            }
        }
    }

    impl LineEditor for MockEditor {
        fn read_line(&mut self, _prompt: &str) -> Result<ReadResult> {
            if self.index < self.inputs.len() {
                let line = self.inputs[self.index].clone();
                self.index += 1;
                Ok(ReadResult::Line(line))
            } else {
                Ok(ReadResult::Eof)
            }
        }

        fn read_continuation(&mut self, prompt: &str) -> Result<ReadResult> {
            self.read_line(prompt)
        }

        fn add_history(&mut self, _line: &str) {}

        fn set_keywords(&mut self, _keywords: Vec<String>) {}
    }

    #[test]
    fn eval_simple_expression() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        let result = repl.eval("(+ 1 2)").unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn eval_def_creates_variable() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        repl.eval("(def x 42)").unwrap();
        assert_eq!(repl.session.get_variable("x"), Some(&Value::Int(42)));

        // Note: Using session variables in subsequent expressions requires
        // VM global variable support, which is not yet implemented.
        // For now, we just verify the variable is stored in the session.
    }

    #[test]
    fn eval_def_with_expression() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        repl.eval("(def sum (+ 10 20))").unwrap();
        assert_eq!(repl.session.get_variable("sum"), Some(&Value::Int(30)));
    }

    #[test]
    fn is_complete_balanced() {
        let editor = MockEditor::new(vec![]);
        let repl = Repl::with_editor(editor);

        assert!(repl.is_complete("(+ 1 2)"));
        assert!(repl.is_complete("[1 2 3]"));
        assert!(repl.is_complete("{:a 1}"));
        assert!(repl.is_complete("42"));
        assert!(repl.is_complete(""));
    }

    #[test]
    fn is_complete_unbalanced() {
        let editor = MockEditor::new(vec![]);
        let repl = Repl::with_editor(editor);

        assert!(!repl.is_complete("(+ 1"));
        assert!(!repl.is_complete("[1 2"));
        assert!(!repl.is_complete("{:a"));
        assert!(!repl.is_complete("\"hello"));
    }

    #[test]
    fn is_complete_nested() {
        let editor = MockEditor::new(vec![]);
        let repl = Repl::with_editor(editor);

        assert!(repl.is_complete("(if (> x 0) (+ x 1) (- x 1))"));
        assert!(!repl.is_complete("(if (> x 0) (+ x 1)"));
    }

    #[test]
    fn is_complete_string_with_brackets() {
        let editor = MockEditor::new(vec![]);
        let repl = Repl::with_editor(editor);

        // Brackets inside strings should be ignored
        assert!(repl.is_complete("\"hello (world\""));
        assert!(repl.is_complete("(str \"[test]\")"));
    }
}
