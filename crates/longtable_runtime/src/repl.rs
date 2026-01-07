//! The main REPL implementation.

use crate::editor::{LineEditor, ReadResult, RustylineEditor};
use crate::serialize;
use crate::session::Session;
use longtable_engine::{InputEvent, QueryCompiler, QueryExecutor, TickExecutor};
use longtable_foundation::{EntityId, Error, ErrorKind, Result, Value};
use longtable_foundation::{LtMap, Type};
use longtable_language::{
    Cardinality, Compiler, ComponentDecl, Declaration, DeclarationAnalyzer, NamespaceContext,
    NamespaceInfo, OnTargetDelete, RelationshipDecl, StorageKind, Vm, parse,
};
use longtable_storage::schema::{
    Cardinality as StorageCardinality, ComponentSchema, FieldSchema, OnDelete, RelationshipSchema,
    Storage,
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

    /// Tick executor for advancing simulation.
    tick_executor: TickExecutor,

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
            tick_executor: TickExecutor::new(),
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

    /// Tries to handle special REPL forms (def, load, save!, load-world!, tick!, inspect).
    #[allow(clippy::too_many_lines)]
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

            // (save! "path") - save world state to file
            Ast::Symbol(s, _) if s == "save!" => {
                if list.len() != 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "save! requires exactly 1 argument: (save! \"path\")".to_string(),
                    )));
                }

                let path = match &list[1] {
                    Ast::String(p, _) => p.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "save! path must be a string, got {}",
                            other.type_name()
                        ))));
                    }
                };

                let resolved = self.session.resolve_path(&path);
                serialize::save_to_file(self.session.world(), &resolved)?;
                println!("World saved to: {}", resolved.display());
                Ok(Some(Value::Nil))
            }

            // (load-world! "path") - load world state from file
            Ast::Symbol(s, _) if s == "load-world!" => {
                if list.len() != 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "load-world! requires exactly 1 argument: (load-world! \"path\")"
                            .to_string(),
                    )));
                }

                let path = match &list[1] {
                    Ast::String(p, _) => p.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "load-world! path must be a string, got {}",
                            other.type_name()
                        ))));
                    }
                };

                let resolved = self.session.resolve_path(&path);
                let world = serialize::load_from_file(&resolved)?;
                let entity_count = world.entity_count();
                let tick = world.tick();
                self.session.set_world(world);
                println!(
                    "World loaded from: {} ({} entities, tick {})",
                    resolved.display(),
                    entity_count,
                    tick
                );
                Ok(Some(Value::Nil))
            }

            // (tick!) or (tick! [events]) - advance world by one tick
            Ast::Symbol(s, _) if s == "tick!" => {
                let inputs: Vec<InputEvent> = if list.len() > 1 {
                    // Parse events from argument (placeholder - just use empty for now)
                    // Full event parsing would require more infrastructure
                    Vec::new()
                } else {
                    Vec::new()
                };

                let world = self.session.world().clone();
                let result = self.tick_executor.tick(world, &inputs)?;

                if result.success {
                    self.session.set_world(result.world);
                    println!(
                        "Tick {}: {} activations fired",
                        self.tick_executor.tick_number(),
                        result.activations_fired
                    );
                } else {
                    println!(
                        "Tick {} rolled back: {:?}",
                        self.tick_executor.tick_number(),
                        result.constraint_result
                    );
                }

                Ok(Some(Value::Nil))
            }

            // (inspect entity) - show entity details
            Ast::Symbol(s, _) if s == "inspect" => {
                if list.len() != 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "inspect requires exactly 1 argument: (inspect entity)".to_string(),
                    )));
                }

                // Evaluate the argument to get entity id
                let entity_val = self.eval_form(&list[1])?;
                let entity_id = match entity_val {
                    Value::EntityRef(id) => id,
                    Value::Int(idx) if idx >= 0 => {
                        // Allow using integer as entity index (for convenience)
                        // Use generation 0 as default for convenience lookup
                        #[allow(clippy::cast_sign_loss)]
                        EntityId::new(idx as u64, 0)
                    }
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "inspect argument must be an entity, got {:?}",
                            other.value_type()
                        ))));
                    }
                };

                let world = self.session.world();

                // Check if entity exists
                if !world.exists(entity_id) {
                    println!("Entity {entity_id} does not exist or is dead");
                    return Ok(Some(Value::Nil));
                }

                // Get entity info
                println!("Entity {entity_id}:");
                println!("  index: {}", entity_id.index);
                println!("  generation: {}", entity_id.generation);
                println!("  status: alive");

                // Note: Without a schema iterator, we can't easily list all components
                // This is a basic implementation - could be enhanced with schema listing
                println!("  (use (get entity :component) to query specific components)");

                Ok(Some(Value::Nil))
            }

            // (component: name ...) - define component schema
            Ast::Symbol(s, _) if s == "component:" => {
                if let Some(Declaration::Component(comp)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_component(&comp)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid component: form".to_string(),
                    )))
                }
            }

            // (relationship: name ...) - define relationship schema
            Ast::Symbol(s, _) if s == "relationship:" => {
                if let Some(Declaration::Relationship(rel)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_relationship(&rel)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid relationship: form".to_string(),
                    )))
                }
            }

            // (spawn: name :component value ...) - create entity
            Ast::Symbol(s, _) if s == "spawn:" => {
                // Use the declaration analyzer to parse the spawn form
                if let Some(Declaration::Spawn(spawn)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_spawn(&spawn)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid spawn: form".to_string(),
                    )))
                }
            }

            // (link: source :relationship target) - create relationship
            Ast::Symbol(s, _) if s == "link:" => {
                // Use the declaration analyzer to parse the link form
                if let Some(Declaration::Link(link)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_link(&link)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid link: form".to_string(),
                    )))
                }
            }

            // (query :where [...] :return ...) - execute query
            Ast::Symbol(s, _) if s == "query" => {
                if let Some(Declaration::Query(query_decl)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_query(&query_decl)
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid query form".to_string(),
                    )))
                }
            }

            _ => Ok(None),
        }
    }

    /// Executes a component declaration to register a schema.
    fn execute_component(&mut self, comp: &ComponentDecl) -> Result<()> {
        // Intern the component name as a keyword
        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&comp.name);

        // Build the schema
        let schema = if comp.is_tag {
            ComponentSchema::tag(name)
        } else {
            let mut schema = ComponentSchema::new(name);
            for field in &comp.fields {
                let field_name = self
                    .session
                    .world_mut()
                    .interner_mut()
                    .intern_keyword(&field.name);
                let field_type = Self::parse_type(&field.ty);

                // Create field schema
                let field_schema = if let Some(ref default_ast) = field.default {
                    let default_value = self.eval_form(default_ast)?;
                    FieldSchema::optional(field_name, field_type, default_value)
                } else {
                    FieldSchema::required(field_name, field_type)
                };
                schema = schema.with_field(field_schema);
            }
            schema
        };

        // Register in world (returns new world with immutable pattern)
        let world = self.session.world().clone();
        let new_world = world.register_component(schema)?;
        self.session.set_world(new_world);
        Ok(())
    }

    /// Executes a relationship declaration to register a schema.
    fn execute_relationship(&mut self, rel: &RelationshipDecl) -> Result<()> {
        // Intern the relationship name as a keyword
        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&rel.name);

        // Build the schema
        let storage = match rel.storage {
            StorageKind::Field => Storage::Field,
            StorageKind::Entity => Storage::Entity,
        };

        let cardinality = match rel.cardinality {
            Cardinality::OneToOne => StorageCardinality::OneToOne,
            Cardinality::OneToMany => StorageCardinality::OneToMany,
            Cardinality::ManyToOne => StorageCardinality::ManyToOne,
            Cardinality::ManyToMany => StorageCardinality::ManyToMany,
        };

        let on_delete = match rel.on_target_delete {
            OnTargetDelete::Remove => OnDelete::Remove,
            OnTargetDelete::Cascade => OnDelete::Cascade,
            OnTargetDelete::Nullify => OnDelete::Nullify,
        };

        let schema = RelationshipSchema::new(name)
            .with_storage(storage)
            .with_cardinality(cardinality)
            .with_on_delete(on_delete);

        // Register in world (returns new world with immutable pattern)
        let world = self.session.world().clone();
        let new_world = world.register_relationship(schema)?;
        self.session.set_world(new_world);
        Ok(())
    }

    /// Parses a type string into a Type.
    fn parse_type(ty: &str) -> Type {
        match ty {
            "int" => Type::Int,
            "float" => Type::Float,
            "string" => Type::String,
            "bool" => Type::Bool,
            "keyword" => Type::Keyword,
            "entity-ref" => Type::EntityRef,
            "nil" => Type::Nil,
            _ => Type::Any,
        }
    }

    /// Executes a spawn declaration to create an entity.
    fn execute_spawn(&mut self, spawn: &longtable_language::SpawnDecl) -> Result<()> {
        // Build component map
        let mut components = LtMap::new();

        for (comp_name, value_ast) in &spawn.components {
            // Evaluate the value AST to get a Value
            let value = self.eval_form(value_ast)?;

            // Convert map values: string keys like ":field" need to become proper keywords
            let value = self.convert_string_keys_to_keywords(value)?;

            // Intern the component keyword
            let keyword = self
                .session
                .world_mut()
                .interner_mut()
                .intern_keyword(comp_name);

            components = components.insert(Value::Keyword(keyword), value);
        }

        // Spawn the entity
        let world = self.session.world().clone();
        let (new_world, entity_id) = world.spawn(&components)?;

        // Update session
        self.session.set_world(new_world);
        self.session.register_entity(spawn.name.clone(), entity_id);

        Ok(())
    }

    /// Converts string keys that look like keywords (":foo") to proper `Value::Keyword`.
    /// This is needed because the compiler currently emits keywords as strings.
    fn convert_string_keys_to_keywords(&mut self, value: Value) -> Result<Value> {
        match value {
            Value::Map(map) => {
                let mut new_map = LtMap::new();
                for (k, v) in map.iter() {
                    let new_key = if let Value::String(s) = k {
                        if let Some(keyword_name) = s.strip_prefix(':') {
                            let kid = self
                                .session
                                .world_mut()
                                .interner_mut()
                                .intern_keyword(keyword_name);
                            Value::Keyword(kid)
                        } else {
                            k.clone()
                        }
                    } else {
                        k.clone()
                    };
                    // Recursively convert nested maps
                    let new_val = self.convert_string_keys_to_keywords(v.clone())?;
                    new_map = new_map.insert(new_key, new_val);
                }
                Ok(Value::Map(new_map))
            }
            Value::Vec(vec) => {
                let new_vec: Result<Vec<_>> = vec
                    .iter()
                    .map(|v| self.convert_string_keys_to_keywords(v.clone()))
                    .collect();
                Ok(Value::Vec(new_vec?.into_iter().collect()))
            }
            other => Ok(other),
        }
    }

    /// Executes a link declaration to create a relationship.
    fn execute_link(&mut self, link: &longtable_language::LinkDecl) -> Result<()> {
        // Resolve source entity
        let source = self.session.get_entity(&link.source).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown entity: {}",
                link.source
            )))
        })?;

        // Resolve target entity
        let target = self.session.get_entity(&link.target).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown entity: {}",
                link.target
            )))
        })?;

        // Intern the relationship keyword
        let relationship = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&link.relationship);

        // Create the link
        let world = self.session.world().clone();
        let new_world = world.link(source, relationship, target)?;

        // Update session
        self.session.set_world(new_world);

        Ok(())
    }

    /// Executes a query and returns results.
    fn execute_query(
        &mut self,
        query_decl: &longtable_language::declaration::QueryDecl,
    ) -> Result<Option<Value>> {
        // Compile the query
        let compiled = QueryCompiler::compile(query_decl, self.session.world_mut().interner_mut())?;

        // Print any warnings
        for warning in &compiled.warnings {
            eprintln!("Warning: {warning}");
        }

        // Execute the query
        let results = QueryExecutor::execute(&compiled, self.session.world())?;

        // Return as a vector
        Ok(Some(Value::Vec(results.into_iter().collect())))
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

    // ==================== Error Recovery Tests ====================

    #[test]
    fn error_recovery_syntax_error() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Syntax errors should not crash the REPL
        let result = repl.eval("(+ 1 2");
        assert!(result.is_err());

        // REPL should still be usable after error
        let result = repl.eval("(+ 1 2)");
        assert!(result.is_ok());
    }

    #[test]
    fn error_recovery_undefined_function() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Calling undefined function should return error
        let result = repl.eval("(undefined-fn 1 2)");
        assert!(result.is_err());

        // REPL should still be usable
        let result = repl.eval("42");
        assert!(result.is_ok());
    }

    #[test]
    fn error_recovery_division_by_zero() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Division by zero should return error
        let result = repl.eval("(/ 1 0)");
        assert!(result.is_err());

        // REPL should still be usable
        let result = repl.eval("(/ 10 2)");
        assert!(result.is_ok());
    }

    #[test]
    fn error_recovery_type_error() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Type error should return error
        let result = repl.eval(r#"(+ "hello" 1)"#);
        assert!(result.is_err());

        // REPL should still be usable
        let result = repl.eval("(+ 1 2)");
        assert!(result.is_ok());
    }

    // ==================== Interactive Scenario Tests ====================

    #[test]
    fn interactive_arithmetic_chain() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Complex expression evaluation
        let result = repl.eval("(+ (* 2 3) (- 10 5))").unwrap();
        assert_eq!(result, Value::Int(11));
    }

    #[test]
    fn interactive_let_expression() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Use let to bind a variable within a single expression
        let result = repl.eval("(let [x 10] (+ x 5))").unwrap();
        assert_eq!(result, Value::Int(15));

        // Nested let bindings
        let result = repl.eval("(let [x 10 y 20] (+ x y))").unwrap();
        assert_eq!(result, Value::Int(30));
    }

    #[test]
    fn interactive_collection_operations() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Create and manipulate collections
        let result = repl.eval("(first [1 2 3])").unwrap();
        assert_eq!(result, Value::Int(1));

        let result = repl.eval("(count [1 2 3 4 5])").unwrap();
        assert_eq!(result, Value::Int(5));
    }

    #[test]
    fn interactive_nested_collections() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Nested vector creation
        let result = repl.eval("[[1 2] [3 4]]").unwrap();
        // Just verify it's a vector
        assert!(matches!(result, Value::Vec(_)));

        // Map creation
        let result = repl.eval("{:a 1 :b 2}").unwrap();
        assert!(matches!(result, Value::Map(_)));
    }

    #[test]
    fn interactive_string_operations() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // String concatenation
        let result = repl.eval(r#"(str "hello" " " "world")"#).unwrap();
        assert_eq!(result, Value::String("hello world".into()));

        // String functions
        let result = repl.eval(r#"(str/upper "hello")"#).unwrap();
        assert_eq!(result, Value::String("HELLO".into()));
    }

    #[test]
    fn interactive_higher_order_functions() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Map with lambda - returns vector
        let result = repl.eval("(map (fn [x] (* x 2)) [1 2 3])").unwrap();
        assert!(matches!(result, Value::Vec(_)));

        // Reduce with lambda (native functions can't be passed directly)
        let result = repl
            .eval("(reduce (fn [acc x] (+ acc x)) 0 [1 2 3 4 5])")
            .unwrap();
        assert_eq!(result, Value::Int(15));
    }

    #[test]
    fn interactive_math_functions() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Basic math
        let result = repl.eval("(abs -5)").unwrap();
        assert_eq!(result, Value::Int(5));

        // Min/max
        let result = repl.eval("(max 1 5 3)").unwrap();
        assert_eq!(result, Value::Int(5));

        let result = repl.eval("(min 1 5 3)").unwrap();
        assert_eq!(result, Value::Int(1));
    }

    // ==================== Spawn and Link Tests ====================

    #[test]
    fn spawn_creates_entity() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // First define the component schema (tag component with :bool :default true)
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();

        // Spawn an entity
        repl.eval("(spawn: player :tag/player true)").unwrap();

        // Verify entity was created
        assert_eq!(repl.session.world().entity_count(), 1);

        // Verify entity is registered by name
        let entity_id = repl.session.get_entity("player");
        assert!(entity_id.is_some());
    }

    #[test]
    fn spawn_with_component_map() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // First define the component schema with optional fields with defaults
        repl.eval("(component: health :current :int :default 0 :max :int :default 100)")
            .unwrap();

        // Spawn an entity with a map component value
        repl.eval("(spawn: player :health {:current 100 :max 100})")
            .unwrap();

        // Verify entity was created
        assert_eq!(repl.session.world().entity_count(), 1);

        // Verify entity is registered by name
        let entity_id = repl.session.get_entity("player");
        assert!(entity_id.is_some());
    }

    #[test]
    fn link_creates_relationship() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define components and relationship (tag components)
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();
        repl.eval("(component: tag/room :bool :default true)")
            .unwrap();
        repl.eval("(relationship: in-room :cardinality :many-to-one)")
            .unwrap();

        // Spawn two entities
        repl.eval("(spawn: player :tag/player true)").unwrap();
        repl.eval("(spawn: room :tag/room true)").unwrap();

        // Link them
        repl.eval("(link: player :in-room room)").unwrap();

        // Verify entities exist
        assert_eq!(repl.session.world().entity_count(), 2);
    }

    #[test]
    fn link_unknown_source_fails() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define components and relationship
        repl.eval("(component: tag/room :bool :default true)")
            .unwrap();
        repl.eval("(relationship: in-room :cardinality :many-to-one)")
            .unwrap();

        // Spawn only target
        repl.eval("(spawn: room :tag/room true)").unwrap();

        // Try to link with unknown source
        let result = repl.eval("(link: player :in-room room)");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown entity"));
    }

    #[test]
    fn link_unknown_target_fails() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define components and relationship
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();
        repl.eval("(relationship: in-room :cardinality :many-to-one)")
            .unwrap();

        // Spawn only source
        repl.eval("(spawn: player :tag/player true)").unwrap();

        // Try to link with unknown target
        let result = repl.eval("(link: player :in-room room)");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown entity"));
    }

    #[test]
    fn component_defines_schema() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define a component with fields
        repl.eval("(component: health :current :int :max :int :default 100)")
            .unwrap();

        // Should not error - component is registered
    }

    #[test]
    fn relationship_defines_schema() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define a relationship
        repl.eval("(relationship: in-room :cardinality :many-to-one :on-target-delete :remove)")
            .unwrap();

        // Should not error - relationship is registered
    }
}
