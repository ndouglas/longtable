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

            // (why entity :component) or (why entity :component :depth N)
            Ast::Symbol(s, _) if s == "why" => self.handle_why(&list[1..]),

            // (explain-query (query ...)) or (explain-query (query ...) entity)
            Ast::Symbol(s, _) if s == "explain-query" => self.handle_explain_query(&list[1..]),

            // (trace :on) or (trace :off) or (trace :json :on) etc.
            Ast::Symbol(s, _) if s == "trace" => self.handle_trace(&list[1..]),

            // (get-traces :last N) or (get-traces :tick N) or (get-traces :all)
            Ast::Symbol(s, _) if s == "get-traces" => self.handle_get_traces(&list[1..]),

            // (break :rule name) or (break :tick N) etc.
            Ast::Symbol(s, _) if s == "break" => self.handle_break(&list[1..]),

            // (unbreak id)
            Ast::Symbol(s, _) if s == "unbreak" => self.handle_unbreak(&list[1..]),

            // (breakpoints)
            Ast::Symbol(s, _) if s == "breakpoints" => self.handle_breakpoints(),

            // (watch expr)
            Ast::Symbol(s, _) if s == "watch" => self.handle_watch(&list[1..]),

            // (unwatch id)
            Ast::Symbol(s, _) if s == "unwatch" => self.handle_unwatch(&list[1..]),

            // (watches)
            Ast::Symbol(s, _) if s == "watches" => self.handle_watches(),

            // (debug) - show debug status
            Ast::Symbol(s, _) if s == "debug" => self.handle_debug(),

            // (continue) - resume execution
            Ast::Symbol(s, _) if s == "continue" => self.handle_continue(),

            // (step-rule) - step to next rule
            Ast::Symbol(s, _) if s == "step-rule" => self.handle_step_rule(),

            // (step-phase) - step to next phase
            Ast::Symbol(s, _) if s == "step-phase" => self.handle_step_phase(),

            // (step-tick) - step to next tick
            Ast::Symbol(s, _) if s == "step-tick" => self.handle_step_tick(),

            // ==================== Time Travel Commands ====================

            // (rollback! N) - go back N ticks
            Ast::Symbol(s, _) if s == "rollback!" => self.handle_rollback(&list[1..]),

            // (goto-tick! N) - jump to tick N
            Ast::Symbol(s, _) if s == "goto-tick!" => self.handle_goto_tick(&list[1..]),

            // (branch! "name") - create branch at current tick
            Ast::Symbol(s, _) if s == "branch!" => self.handle_branch(&list[1..]),

            // (checkout! "name") - switch to branch
            Ast::Symbol(s, _) if s == "checkout!" => self.handle_checkout(&list[1..]),

            // (branches) - list all branches
            Ast::Symbol(s, _) if s == "branches" => self.handle_branches(),

            // (merge! "name") - merge branch into current
            Ast::Symbol(s, _) if s == "merge!" => self.handle_merge(&list[1..]),

            // (diff N M) or (diff :branches "a" "b") - compare ticks or branches
            Ast::Symbol(s, _) if s == "diff" => self.handle_diff(&list[1..]),

            // (history) or (history N) - show recent history
            Ast::Symbol(s, _) if s == "history" => self.handle_history(&list[1..]),

            // (timeline) - show timeline status
            Ast::Symbol(s, _) if s == "timeline" => self.handle_timeline(),

            // ==================== Parser Vocabulary Declarations ====================

            // (verb: name :synonyms [...])
            Ast::Symbol(s, _) if s == "verb:" => {
                if let Some(Declaration::Verb(verb_decl)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_verb(&verb_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid verb: form".to_string(),
                    )))
                }
            }

            // (direction: name :synonyms [...] :opposite ...)
            Ast::Symbol(s, _) if s == "direction:" => {
                if let Some(Declaration::Direction(dir_decl)) = DeclarationAnalyzer::analyze(form)?
                {
                    self.execute_direction(&dir_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid direction: form".to_string(),
                    )))
                }
            }

            // (preposition: name :implies ...)
            Ast::Symbol(s, _) if s == "preposition:" => {
                if let Some(Declaration::Preposition(prep_decl)) =
                    DeclarationAnalyzer::analyze(form)?
                {
                    self.execute_preposition(&prep_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid preposition: form".to_string(),
                    )))
                }
            }

            // (pronoun: name :gender ... :number ...)
            Ast::Symbol(s, _) if s == "pronoun:" => {
                if let Some(Declaration::Pronoun(pronoun_decl)) =
                    DeclarationAnalyzer::analyze(form)?
                {
                    self.execute_pronoun(&pronoun_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid pronoun: form".to_string(),
                    )))
                }
            }

            // (adverb: name)
            Ast::Symbol(s, _) if s == "adverb:" => {
                if let Some(Declaration::Adverb(adverb_decl)) = DeclarationAnalyzer::analyze(form)?
                {
                    self.execute_adverb(&adverb_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid adverb: form".to_string(),
                    )))
                }
            }

            // (type: name :extends [...] :where [...])
            Ast::Symbol(s, _) if s == "type:" => {
                if let Some(Declaration::NounType(type_decl)) = DeclarationAnalyzer::analyze(form)?
                {
                    self.execute_noun_type(&type_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid type: form".to_string(),
                    )))
                }
            }

            // (scope: name :extends [...] :where [...])
            Ast::Symbol(s, _) if s == "scope:" => {
                if let Some(Declaration::Scope(scope_decl)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_scope(&scope_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid scope: form".to_string(),
                    )))
                }
            }

            // (command: name :syntax [...] :action ...)
            Ast::Symbol(s, _) if s == "command:" => {
                if let Some(Declaration::Command(cmd_decl)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_command(&cmd_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid command: form".to_string(),
                    )))
                }
            }

            // (action: name :params [...] :preconditions [...] :do [...])
            Ast::Symbol(s, _) if s == "action:" => {
                if let Some(Declaration::Action(action_decl)) = DeclarationAnalyzer::analyze(form)?
                {
                    self.execute_action(&action_decl)?;
                    Ok(Some(Value::Nil))
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid action: form".to_string(),
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

    /// Executes a verb declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_verb(&mut self, verb_decl: &longtable_language::VerbDecl) -> Result<()> {
        use longtable_parser::vocabulary::Verb;
        use std::collections::HashSet;

        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&verb_decl.name);

        let synonyms: HashSet<_> = verb_decl
            .synonyms
            .iter()
            .map(|s| self.session.world_mut().interner_mut().intern_keyword(s))
            .collect();

        self.session
            .vocabulary_registry_mut()
            .register_verb(Verb { name, synonyms });
        Ok(())
    }

    /// Executes a direction declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_direction(&mut self, dir_decl: &longtable_language::DirectionDecl) -> Result<()> {
        use longtable_parser::vocabulary::Direction;
        use std::collections::HashSet;

        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&dir_decl.name);

        let synonyms: HashSet<_> = dir_decl
            .synonyms
            .iter()
            .map(|s| self.session.world_mut().interner_mut().intern_keyword(s))
            .collect();

        let opposite = dir_decl
            .opposite
            .as_ref()
            .map(|s| self.session.world_mut().interner_mut().intern_keyword(s));

        self.session
            .vocabulary_registry_mut()
            .register_direction(Direction {
                name,
                synonyms,
                opposite,
            });
        Ok(())
    }

    /// Executes a preposition declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_preposition(
        &mut self,
        prep_decl: &longtable_language::PrepositionDecl,
    ) -> Result<()> {
        use longtable_parser::vocabulary::Preposition;

        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&prep_decl.name);

        let implies = prep_decl
            .implies
            .as_ref()
            .map(|s| self.session.world_mut().interner_mut().intern_keyword(s));

        self.session
            .vocabulary_registry_mut()
            .register_preposition(Preposition { name, implies });
        Ok(())
    }

    /// Executes a pronoun declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_pronoun(&mut self, pronoun_decl: &longtable_language::PronounDecl) -> Result<()> {
        use longtable_language::declaration::PronounGender as DeclGender;
        use longtable_language::declaration::PronounNumber as DeclNumber;
        use longtable_parser::vocabulary::{Pronoun, PronounGender, PronounNumber};

        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&pronoun_decl.name);

        let gender = match pronoun_decl.gender {
            DeclGender::Masculine => PronounGender::Masculine,
            DeclGender::Feminine => PronounGender::Feminine,
            DeclGender::Neuter => PronounGender::Neuter,
        };

        let number = match pronoun_decl.number {
            DeclNumber::Singular => PronounNumber::Singular,
            DeclNumber::Plural => PronounNumber::Plural,
        };

        self.session
            .vocabulary_registry_mut()
            .register_pronoun(Pronoun {
                name,
                gender,
                number,
            });
        Ok(())
    }

    /// Executes an adverb declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_adverb(&mut self, adverb_decl: &longtable_language::AdverbDecl) -> Result<()> {
        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&adverb_decl.name);

        self.session.vocabulary_registry_mut().register_adverb(name);
        Ok(())
    }

    /// Executes a noun type declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_noun_type(&mut self, type_decl: &longtable_language::NounTypeDecl) -> Result<()> {
        use longtable_language::pretty::pretty_print;
        use longtable_parser::vocabulary::NounType;

        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&type_decl.name);

        let extends: Vec<_> = type_decl
            .extends
            .iter()
            .map(|s| self.session.world_mut().interner_mut().intern_keyword(s))
            .collect();

        // Convert pattern back to source string for later compilation
        let pattern_source = type_decl
            .pattern
            .clauses
            .iter()
            .map(|c| {
                let value_str = match &c.value {
                    longtable_language::PatternValue::Variable(v) => format!("?{v}"),
                    longtable_language::PatternValue::Literal(ast) => pretty_print(ast),
                    longtable_language::PatternValue::Wildcard => "_".to_string(),
                };
                format!("[?{} :{} {}]", c.entity_var, c.component, value_str)
            })
            .collect::<Vec<_>>()
            .join(" ");

        self.session
            .vocabulary_registry_mut()
            .register_type(NounType {
                name,
                extends,
                pattern_source,
            });
        Ok(())
    }

    /// Executes a scope declaration to register in the vocabulary.
    ///
    /// Scopes define visibility rules for noun resolution.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_scope(&mut self, scope_decl: &longtable_language::ScopeDecl) -> Result<()> {
        // For now, scopes are stored but not yet used in noun resolution
        // TODO: Implement proper scope storage and usage
        let _name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&scope_decl.name);
        // Scopes will be used by the natural language parser for noun resolution
        Ok(())
    }

    /// Executes a command declaration to register in the vocabulary.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_command(&mut self, cmd_decl: &longtable_language::CommandDecl) -> Result<()> {
        use longtable_parser::vocabulary::CommandSyntax;

        let name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&cmd_decl.name);

        let action = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&cmd_decl.action);

        // Convert syntax elements to source string for later compilation
        let syntax_source = cmd_decl
            .syntax
            .iter()
            .map(|elem| match elem {
                longtable_language::SyntaxElement::Verb => ":verb".to_string(),
                longtable_language::SyntaxElement::Direction { var } => {
                    format!("?{var} :direction")
                }
                longtable_language::SyntaxElement::Noun {
                    var,
                    type_constraint,
                } => {
                    let constraint = type_constraint
                        .as_ref()
                        .map(|t| format!(" :{t}"))
                        .unwrap_or_default();
                    format!("?{var}{constraint}")
                }
                longtable_language::SyntaxElement::OptionalNoun {
                    var,
                    type_constraint,
                } => {
                    let constraint = type_constraint
                        .as_ref()
                        .map(|t| format!(" :{t}"))
                        .unwrap_or_default();
                    format!("[?{var}{constraint}]")
                }
                longtable_language::SyntaxElement::Preposition(prep) => format!(":{prep}"),
                longtable_language::SyntaxElement::Literal(word) => format!("\"{word}\""),
            })
            .collect::<Vec<_>>()
            .join(" ");

        self.session
            .vocabulary_registry_mut()
            .register_command(CommandSyntax {
                name,
                action,
                priority: cmd_decl.priority,
                syntax_source,
            });
        Ok(())
    }

    /// Executes an action declaration to register in the vocabulary.
    ///
    /// Actions define what happens when a command matches.
    #[allow(clippy::unnecessary_wraps)]
    fn execute_action(&mut self, action_decl: &longtable_language::ActionDecl) -> Result<()> {
        // For now, actions are stored but not yet executed
        // TODO: Implement proper action registration and execution
        let _name = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&action_decl.name);
        // Actions will be compiled and stored for execution when commands match
        Ok(())
    }

    /// Handles the (why entity :component) or (why entity :component :depth N) form.
    ///
    /// Returns information about why an entity has a particular component value,
    /// tracing back through the provenance chain.
    fn handle_why(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::WhyQuery;
        use longtable_language::Ast;

        if args.len() < 2 || args.len() > 4 {
            return Err(Error::new(ErrorKind::Internal(
                "why requires 2-4 arguments: (why entity :component) or (why entity :component :depth N)".to_string(),
            )));
        }

        // Parse entity - support named entities from session
        let entity = if let Ast::Symbol(name, _) = &args[0] {
            // Try to resolve as a named entity
            self.session
                .get_entity(name)
                .ok_or_else(|| Error::new(ErrorKind::Internal(format!("unknown entity: {name}"))))?
        } else {
            let entity_val = self.eval_form(&args[0])?;
            match entity_val {
                Value::EntityRef(id) => id,
                Value::Int(idx) if idx >= 0 =>
                {
                    #[allow(clippy::cast_sign_loss)]
                    EntityId::new(idx as u64, 0)
                }
                other => {
                    return Err(Error::new(ErrorKind::Internal(format!(
                        "why entity must be an entity reference, got {:?}",
                        other.value_type()
                    ))));
                }
            }
        };

        // Parse component
        let component = match &args[1] {
            Ast::Keyword(name, _) => self.session.world_mut().interner_mut().intern_keyword(name),
            other => {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "why component must be a keyword, got {}",
                    other.type_name()
                ))));
            }
        };

        // Parse optional :depth N
        let depth = if args.len() >= 4 {
            match (&args[2], &args[3]) {
                (Ast::Keyword(k, _), Ast::Int(n, _)) if k == "depth" => {
                    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    {
                        *n as usize
                    }
                }
                _ => {
                    return Err(Error::new(ErrorKind::Internal(
                        "expected :depth N after component".to_string(),
                    )));
                }
            }
        } else {
            1
        };

        // Perform the why query
        let tracker = self.tick_executor.provenance();
        let query = WhyQuery::new(tracker);

        // Get current value for context (ignore errors - just for display)
        let current_value = self.session.world().get(entity, component).ok().flatten();

        let result = query.why_depth(entity, component, depth, current_value);

        // Format the result
        self.format_why_result(&result, entity, component);

        Ok(Some(Value::Nil))
    }

    /// Formats and prints a `WhyResult`.
    fn format_why_result(
        &self,
        result: &longtable_debug::WhyResult,
        entity: EntityId,
        component: longtable_foundation::KeywordId,
    ) {
        use longtable_debug::WhyResult;

        let component_name = self
            .session
            .world()
            .interner()
            .get_keyword(component)
            .unwrap_or("?");

        match result {
            WhyResult::Unknown => {
                println!("No provenance information for {entity} :{component_name}");
            }
            WhyResult::Single(None) => {
                println!("No write recorded for {entity} :{component_name}");
            }
            WhyResult::Single(Some(link)) => {
                let rule_name = self
                    .session
                    .world()
                    .interner()
                    .get_keyword(link.rule)
                    .unwrap_or("?");
                println!("Why {entity} :{component_name}?");
                println!("  Rule: :{rule_name}");
                println!("  Tick: {}", link.tick);
                if let Some(ref value) = link.value {
                    println!("  Value: {value}");
                }
                if let Some(ref prev) = link.previous_value {
                    println!("  Previous: {prev}");
                }
                if !link.context.is_empty() {
                    println!("  Context:");
                    for (var, eid) in &link.context {
                        println!("    {var} = {eid}");
                    }
                }
            }
            WhyResult::Chain(chain) => {
                println!("Why {entity} :{component_name}? (causal chain)");
                for (i, link) in chain.links.iter().enumerate() {
                    let rule_name = self
                        .session
                        .world()
                        .interner()
                        .get_keyword(link.rule)
                        .unwrap_or("?");
                    let comp_name = self
                        .session
                        .world()
                        .interner()
                        .get_keyword(link.component)
                        .unwrap_or("?");
                    println!("  [{i}] {} :{comp_name}", link.entity);
                    println!("      Rule: :{rule_name}, Tick: {}", link.tick);
                    if let Some(ref value) = link.value {
                        println!("      Value: {value}");
                    }
                }
                if chain.truncated {
                    println!("  ... (chain truncated at depth limit)");
                }
            }
        }
    }

    /// Handles the (explain-query (query ...)) form.
    ///
    /// Shows how a query was executed through its pipeline of clauses.
    fn handle_explain_query(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_language::Ast;

        if args.is_empty() || args.len() > 2 {
            return Err(Error::new(ErrorKind::Internal(
                "explain-query requires 1-2 arguments: (explain-query (query ...)) or (explain-query (query ...) entity)".to_string(),
            )));
        }

        // Parse the query
        let query_form = &args[0];
        let Some(Declaration::Query(query_decl)) = DeclarationAnalyzer::analyze(query_form)? else {
            return Err(Error::new(ErrorKind::Internal(
                "explain-query first argument must be a query form".to_string(),
            )));
        };

        // Optional: specific entity to explain - support named entities
        let target_entity = if args.len() == 2 {
            if let Ast::Symbol(name, _) = &args[1] {
                Some(self.session.get_entity(name).ok_or_else(|| {
                    Error::new(ErrorKind::Internal(format!("unknown entity: {name}")))
                })?)
            } else {
                let entity_val = self.eval_form(&args[1])?;
                match entity_val {
                    Value::EntityRef(id) => Some(id),
                    Value::Int(idx) if idx >= 0 =>
                    {
                        #[allow(clippy::cast_sign_loss)]
                        Some(EntityId::new(idx as u64, 0))
                    }
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "explain-query entity must be an entity reference, got {:?}",
                            other.value_type()
                        ))));
                    }
                }
            }
        } else {
            None
        };

        // Compile the query
        let compiled =
            QueryCompiler::compile(&query_decl, self.session.world_mut().interner_mut())?;

        // Execute the query (needed for statistics)
        let results = QueryExecutor::execute(&compiled, self.session.world())?;

        // Print explanation
        println!("Query Explanation:");
        println!("  Clauses: {}", compiled.pattern.clauses.len());
        println!("  Results: {}", results.len());

        if let Some(entity) = target_entity {
            self.print_entity_match_explanation(entity, &compiled);
        }

        Ok(Some(Value::Nil))
    }

    /// Prints entity-specific match explanation.
    fn print_entity_match_explanation(
        &self,
        entity: EntityId,
        compiled: &longtable_engine::CompiledQuery,
    ) {
        use longtable_engine::PatternMatcher;

        println!("\n  Entity {entity} match analysis:");

        let result =
            PatternMatcher::explain_entity(&compiled.pattern, entity, self.session.world());

        if result.matched {
            println!("    Status: MATCHED");
            for (var, val) in result.partial_bindings.iter() {
                println!("    ?{var} = {val}");
            }
        } else {
            println!("    Status: NOT MATCHED");
            if let Some(clause_idx) = result.failed_at_clause {
                println!("    Failed at clause: {clause_idx}");
                if clause_idx < compiled.pattern.clauses.len() {
                    let clause = &compiled.pattern.clauses[clause_idx];
                    let comp_name = self
                        .session
                        .world()
                        .interner()
                        .get_keyword(clause.component)
                        .unwrap_or("?");
                    println!("      [?{} :{comp_name} ...]", clause.entity_var);
                }
            }
            if let Some(ref reason) = result.failure_reason {
                self.print_match_failure_reason(reason);
            }
        }
    }

    /// Prints match failure reason.
    fn print_match_failure_reason(&self, reason: &longtable_engine::MatchFailure) {
        use longtable_engine::MatchFailure;

        match reason {
            MatchFailure::MissingComponent { component } => {
                let comp_name = self
                    .session
                    .world()
                    .interner()
                    .get_keyword(*component)
                    .unwrap_or("?");
                println!("    Reason: Entity missing component :{comp_name}");
            }
            MatchFailure::ValueMismatch { expected, actual } => {
                println!("    Reason: Value mismatch");
                println!("      Expected: {expected}");
                println!("      Actual: {actual}");
            }
            MatchFailure::UnificationFailure {
                var,
                expected,
                actual,
            } => {
                println!("    Reason: Unification failure for ?{var}");
                println!("      Previously bound to: {expected}");
                println!("      New value: {actual}");
            }
            MatchFailure::NegationMatched { component } => {
                let comp_name = self
                    .session
                    .world()
                    .interner()
                    .get_keyword(*component)
                    .unwrap_or("?");
                println!("    Reason: Entity has negated component :{comp_name}");
            }
            MatchFailure::GuardFailed { guard_index } => {
                println!("    Reason: Guard {guard_index} returned false");
            }
            MatchFailure::EntityNotFound => {
                println!("    Reason: Entity does not exist");
            }
        }
    }

    /// Handles the (trace :on/:off) form.
    ///
    /// Enables or disables tracing.
    fn handle_trace(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::TraceOutput;
        use longtable_language::Ast;

        if args.is_empty() {
            // Show current status
            let enabled = self.session.tracer().is_enabled();
            println!("Trace: {}", if enabled { "on" } else { "off" });
            return Ok(Some(Value::Bool(enabled)));
        }

        // Parse options
        let mut i = 0;
        while i < args.len() {
            match &args[i] {
                Ast::Keyword(k, _) if k == "on" => {
                    self.session.tracer_mut().enable();
                    self.session.tracer_mut().set_output(TraceOutput::Stderr);
                    println!("Trace enabled");
                }
                Ast::Keyword(k, _) if k == "off" => {
                    self.session.tracer_mut().disable();
                    self.session.tracer_mut().set_output(TraceOutput::None);
                    println!("Trace disabled");
                }
                Ast::Keyword(k, _) if k == "json" => {
                    self.session.tracer_mut().set_json_format(true);
                    println!("Trace output format: JSON");
                }
                Ast::Keyword(k, _) if k == "human" => {
                    self.session.tracer_mut().set_json_format(false);
                    println!("Trace output format: human-readable");
                }
                Ast::Keyword(k, _) if k == "clear" => {
                    self.session.tracer_mut().clear();
                    println!("Trace buffer cleared");
                }
                Ast::Keyword(k, _) if k == "stats" => {
                    let stats = self.session.tracer().stats();
                    println!("Trace statistics:");
                    println!("  Records: {}/{}", stats.record_count, stats.max_size);
                    if let (Some(oldest), Some(newest)) = (stats.oldest_tick, stats.newest_tick) {
                        println!("  Ticks: {oldest} - {newest}");
                    }
                    println!("  Event types:");
                    for (event_type, count) in &stats.event_counts {
                        println!("    {event_type}: {count}");
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::Internal(format!(
                        "unknown trace option: {other:?}"
                    ))));
                }
            }
            i += 1;
        }

        Ok(Some(Value::Nil))
    }

    /// Handles the (get-traces :last N) form.
    ///
    /// Retrieves and displays trace records.
    #[allow(clippy::items_after_statements, clippy::cast_possible_wrap)]
    fn handle_get_traces(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_language::Ast;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "get-traces requires an option: :last N, :tick N, :type TYPE, or :all".to_string(),
            )));
        }

        // First, figure out the query type and evaluate any parameters
        enum TraceQuery {
            All,
            Last(usize),
            Tick(u64),
            Type(String),
        }

        let query = match &args[0] {
            Ast::Keyword(k, _) if k == "all" => TraceQuery::All,

            Ast::Keyword(k, _) if k == "last" => {
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "get-traces :last requires a count".to_string(),
                    )));
                }
                let count = self.eval_form(&args[1])?;
                let Value::Int(n) = count else {
                    return Err(Error::new(ErrorKind::Internal(
                        "get-traces :last requires an integer count".to_string(),
                    )));
                };
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                TraceQuery::Last(n.max(0) as usize)
            }

            Ast::Keyword(k, _) if k == "tick" => {
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "get-traces :tick requires a tick number".to_string(),
                    )));
                }
                let tick = self.eval_form(&args[1])?;
                let Value::Int(t) = tick else {
                    return Err(Error::new(ErrorKind::Internal(
                        "get-traces :tick requires an integer tick number".to_string(),
                    )));
                };
                #[allow(clippy::cast_sign_loss)]
                TraceQuery::Tick(t.max(0) as u64)
            }

            Ast::Keyword(k, _) if k == "type" => {
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "get-traces :type requires an event type".to_string(),
                    )));
                }
                let Ast::Keyword(event_type, _) = &args[1] else {
                    return Err(Error::new(ErrorKind::Internal(
                        "get-traces :type requires a keyword event type".to_string(),
                    )));
                };
                TraceQuery::Type(event_type.clone())
            }

            other => {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "unknown get-traces option: {other:?}"
                ))));
            }
        };

        // Now borrow the buffer and execute the query
        let buffer = self.session.tracer().buffer();
        let records: Vec<_> = match query {
            TraceQuery::All => buffer.iter().collect(),
            TraceQuery::Last(n) => buffer.recent(n),
            TraceQuery::Tick(t) => buffer.records_for_tick(t),
            TraceQuery::Type(ref t) => buffer.by_event_type(t),
        };

        if records.is_empty() {
            println!("No traces found");
        } else {
            let interner = self.session.world().interner();
            let tracer = self.session.tracer();
            let output = tracer.format_records(&records, interner);
            println!("{output}");
        }

        Ok(Some(Value::Int(records.len() as i64)))
    }

    /// Handles the (break ...) form.
    ///
    /// Adds a breakpoint.
    #[allow(clippy::too_many_lines)]
    fn handle_break(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::BreakpointId;
        use longtable_language::Ast;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "break requires a type: :rule, :tick, :write, :read".to_string(),
            )));
        }

        let id: BreakpointId = match &args[0] {
            Ast::Keyword(k, _) if k == "rule" => {
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :rule requires a rule name".to_string(),
                    )));
                }
                let Ast::Keyword(rule_name, _) = &args[1] else {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :rule requires a keyword rule name".to_string(),
                    )));
                };
                let rule_id = self
                    .session
                    .world_mut()
                    .interner_mut()
                    .intern_keyword(rule_name);
                self.session
                    .debug_session_mut()
                    .breakpoints_mut()
                    .add_rule(rule_id)
            }

            Ast::Keyword(k, _) if k == "tick" => {
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :tick requires a tick number".to_string(),
                    )));
                }
                let tick_val = self.eval_form(&args[1])?;
                let Value::Int(tick) = tick_val else {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :tick requires an integer tick number".to_string(),
                    )));
                };
                #[allow(clippy::cast_sign_loss)]
                self.session
                    .debug_session_mut()
                    .breakpoints_mut()
                    .add_tick(tick.max(0) as u64)
            }

            Ast::Keyword(k, _) if k == "write" => {
                // (break :write :component) or (break :write entity :component)
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :write requires a component name".to_string(),
                    )));
                }

                let (entity, component_arg) = if args.len() >= 3 {
                    // Entity specified
                    let entity_val = self.eval_form(&args[1])?;
                    let entity = match entity_val {
                        Value::EntityRef(e) => Some(e),
                        _ => {
                            return Err(Error::new(ErrorKind::Internal(
                                "break :write entity must be an entity reference".to_string(),
                            )));
                        }
                    };
                    (entity, &args[2])
                } else {
                    (None, &args[1])
                };

                let Ast::Keyword(comp_name, _) = component_arg else {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :write requires a keyword component name".to_string(),
                    )));
                };
                let comp_id = self
                    .session
                    .world_mut()
                    .interner_mut()
                    .intern_keyword(comp_name);
                self.session
                    .debug_session_mut()
                    .breakpoints_mut()
                    .add_component_write(entity, comp_id)
            }

            Ast::Keyword(k, _) if k == "read" => {
                // (break :read :component) or (break :read entity :component)
                if args.len() < 2 {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :read requires a component name".to_string(),
                    )));
                }

                let (entity, component_arg) = if args.len() >= 3 {
                    let entity_val = self.eval_form(&args[1])?;
                    let entity = match entity_val {
                        Value::EntityRef(e) => Some(e),
                        _ => {
                            return Err(Error::new(ErrorKind::Internal(
                                "break :read entity must be an entity reference".to_string(),
                            )));
                        }
                    };
                    (entity, &args[2])
                } else {
                    (None, &args[1])
                };

                let Ast::Keyword(comp_name, _) = component_arg else {
                    return Err(Error::new(ErrorKind::Internal(
                        "break :read requires a keyword component name".to_string(),
                    )));
                };
                let comp_id = self
                    .session
                    .world_mut()
                    .interner_mut()
                    .intern_keyword(comp_name);
                self.session
                    .debug_session_mut()
                    .breakpoints_mut()
                    .add_component_read(entity, comp_id)
            }

            other => {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "unknown breakpoint type: {other:?}"
                ))));
            }
        };

        println!("Breakpoint {id} added");
        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(id.raw() as i64)))
    }

    /// Handles the (unbreak id) form.
    fn handle_unbreak(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::BreakpointId;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "unbreak requires a breakpoint id".to_string(),
            )));
        }

        let id_val = self.eval_form(&args[0])?;
        let Value::Int(id) = id_val else {
            return Err(Error::new(ErrorKind::Internal(
                "unbreak requires an integer breakpoint id".to_string(),
            )));
        };

        #[allow(clippy::cast_sign_loss)]
        let bp_id = BreakpointId::new(id.max(0) as u64);
        if self
            .session
            .debug_session_mut()
            .breakpoints_mut()
            .remove(bp_id)
            .is_some()
        {
            println!("Breakpoint {bp_id} removed");
            Ok(Some(Value::Bool(true)))
        } else {
            println!("Breakpoint {bp_id} not found");
            Ok(Some(Value::Bool(false)))
        }
    }

    /// Handles the (breakpoints) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_breakpoints(&self) -> Result<Option<Value>> {
        let registry = self.session.debug_session().breakpoints();

        if registry.is_empty() {
            println!("No breakpoints");
        } else {
            println!("Breakpoints:");
            for bp in registry.iter() {
                let status = if bp.is_enabled() {
                    "enabled"
                } else {
                    "disabled"
                };
                println!("  {} - {} ({})", bp.id(), bp.description(), status);
            }
        }

        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(registry.len() as i64)))
    }

    /// Handles the (watch expr) form.
    fn handle_watch(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_language::pretty::pretty_print;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "watch requires an expression".to_string(),
            )));
        }

        // Convert the AST back to source for display
        let source = pretty_print(&args[0]);
        let id = self.session.debug_session_mut().watches_mut().add(source);

        println!("Watch {id} added");
        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(id.raw() as i64)))
    }

    /// Handles the (unwatch id) form.
    fn handle_unwatch(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::WatchId;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "unwatch requires a watch id".to_string(),
            )));
        }

        let id_val = self.eval_form(&args[0])?;
        let Value::Int(id) = id_val else {
            return Err(Error::new(ErrorKind::Internal(
                "unwatch requires an integer watch id".to_string(),
            )));
        };

        #[allow(clippy::cast_sign_loss)]
        let watch_id = WatchId::new(id.max(0) as u64);
        if self
            .session
            .debug_session_mut()
            .watches_mut()
            .remove(watch_id)
            .is_some()
        {
            println!("Watch {watch_id} removed");
            Ok(Some(Value::Bool(true)))
        } else {
            println!("Watch {watch_id} not found");
            Ok(Some(Value::Bool(false)))
        }
    }

    /// Handles the (watches) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_watches(&self) -> Result<Option<Value>> {
        let registry = self.session.debug_session().watches();

        if registry.is_empty() {
            println!("No watches");
        } else {
            println!("Watches:");
            for watch in registry.iter() {
                let status = if watch.is_enabled() {
                    "enabled"
                } else {
                    "disabled"
                };
                let value_str = watch
                    .last_value()
                    .map_or("(not evaluated)".to_string(), |v| format!("{v}"));
                println!(
                    "  {} - {} = {} ({})",
                    watch.id(),
                    watch.source(),
                    value_str,
                    status
                );
            }
        }

        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(registry.len() as i64)))
    }

    /// Handles the (debug) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_debug(&self) -> Result<Option<Value>> {
        let session = self.session.debug_session();
        println!("{}", session.status_summary());
        Ok(Some(Value::Nil))
    }

    /// Handles the (continue) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_continue(&mut self) -> Result<Option<Value>> {
        self.session.debug_session_mut().resume();
        println!("Resumed execution");
        Ok(Some(Value::Nil))
    }

    /// Handles the (step-rule) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_step_rule(&mut self) -> Result<Option<Value>> {
        self.session.debug_session_mut().step_rule();
        println!("Stepping to next rule");
        Ok(Some(Value::Nil))
    }

    /// Handles the (step-phase) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_step_phase(&mut self) -> Result<Option<Value>> {
        self.session.debug_session_mut().step_phase();
        println!("Stepping to next phase");
        Ok(Some(Value::Nil))
    }

    /// Handles the (step-tick) form.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_step_tick(&mut self) -> Result<Option<Value>> {
        self.session.debug_session_mut().step_tick();
        println!("Stepping to next tick");
        Ok(Some(Value::Nil))
    }

    // ==================== Time Travel Handlers ====================

    /// Handles the (rollback! N) form.
    ///
    /// Goes back N ticks in the current branch's history.
    fn handle_rollback(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "rollback! requires a tick count: (rollback! N)".to_string(),
            )));
        }

        let count_val = self.eval_form(&args[0])?;
        let Value::Int(count) = count_val else {
            return Err(Error::new(ErrorKind::Internal(
                "rollback! requires an integer tick count".to_string(),
            )));
        };

        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let ticks_back = count.max(0) as usize;

        let Some(snapshot) = self.session.timeline().rollback(ticks_back) else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "cannot rollback {ticks_back} ticks - not enough history"
            ))));
        };

        let world = snapshot.world().clone();
        let tick = snapshot.tick();
        self.session.set_world(world);
        println!("Rolled back to tick {tick}");

        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(tick as i64)))
    }

    /// Handles the (goto-tick! N) form.
    ///
    /// Jumps to a specific tick in history.
    fn handle_goto_tick(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "goto-tick! requires a tick number: (goto-tick! N)".to_string(),
            )));
        }

        let tick_val = self.eval_form(&args[0])?;
        let Value::Int(tick) = tick_val else {
            return Err(Error::new(ErrorKind::Internal(
                "goto-tick! requires an integer tick number".to_string(),
            )));
        };

        #[allow(clippy::cast_sign_loss)]
        let target_tick = tick.max(0) as u64;

        let Some(snapshot) = self.session.timeline().goto_tick(target_tick) else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "tick {target_tick} not found in history"
            ))));
        };

        let world = snapshot.world().clone();
        self.session.set_world(world);
        println!("Jumped to tick {target_tick}");

        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(target_tick as i64)))
    }

    /// Handles the (branch! "name") form.
    ///
    /// Creates a new branch at the current tick.
    fn handle_branch(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_language::Ast;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "branch! requires a branch name: (branch! \"name\")".to_string(),
            )));
        }

        let name = match &args[0] {
            Ast::String(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "branch! name must be a string, got {}",
                    other.type_name()
                ))));
            }
        };

        let current_tick = self.session.world().tick();
        let Some(branch_id) = self
            .session
            .timeline_mut()
            .create_branch(name.clone(), current_tick)
        else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "failed to create branch '{name}' - name may already exist"
            ))));
        };

        println!("Created branch '{name}' at tick {current_tick} (id: {branch_id})");
        Ok(Some(Value::Nil))
    }

    /// Handles the (checkout! "name") form.
    ///
    /// Switches to a different branch.
    fn handle_checkout(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_language::Ast;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "checkout! requires a branch name: (checkout! \"name\")".to_string(),
            )));
        }

        let name = match &args[0] {
            Ast::String(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "checkout! name must be a string, got {}",
                    other.type_name()
                ))));
            }
        };

        if !self.session.timeline_mut().checkout(&name) {
            return Err(Error::new(ErrorKind::Internal(format!(
                "branch '{name}' not found"
            ))));
        }

        // Restore world from branch tip if available
        if let Some(snapshot) = self.session.timeline().latest_snapshot() {
            let world = snapshot.world().clone();
            self.session.set_world(world);
        }

        println!("Switched to branch '{name}'");
        Ok(Some(Value::Nil))
    }

    /// Handles the (branches) form.
    ///
    /// Lists all branches.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_branches(&self) -> Result<Option<Value>> {
        let timeline = self.session.timeline();
        let current = timeline.current_branch().name();
        let names = timeline.branch_names();

        println!("Branches:");
        for name in names {
            let marker = if name == current { " *" } else { "" };
            println!("  {name}{marker}");
        }

        Ok(Some(Value::Nil))
    }

    /// Handles the (merge! "name") form.
    ///
    /// Merges a branch into the current branch.
    fn handle_merge(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::{MergeStrategy, merge};
        use longtable_language::Ast;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "merge! requires a branch name: (merge! \"name\")".to_string(),
            )));
        }

        let name = match &args[0] {
            Ast::String(s, _) => s.clone(),
            other => {
                return Err(Error::new(ErrorKind::Internal(format!(
                    "merge! name must be a string, got {}",
                    other.type_name()
                ))));
            }
        };

        // Get the source branch tip
        let branches = self.session.timeline().branches();
        let Some(source_branch) = branches.get_by_name(&name) else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "branch '{name}' not found"
            ))));
        };

        let Some(source_snapshot) = source_branch.latest() else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "branch '{name}' has no snapshots"
            ))));
        };

        let incoming = source_snapshot.world();
        let current = self.session.world();

        // Use Replace strategy - simple overwrite
        let result = merge(current, current, incoming, MergeStrategy::Replace);

        if result.is_success() {
            if let Some(world) = result.into_world() {
                self.session.set_world(world);
                println!("Merged branch '{name}' into current branch");
            }
        } else if result.is_failed() {
            return Err(Error::new(ErrorKind::Internal(format!(
                "merge failed: branch '{name}'"
            ))));
        }

        Ok(Some(Value::Nil))
    }

    /// Handles the (diff N M) or (diff :branches "a" "b") form.
    ///
    /// Compares two ticks or two branches.
    fn handle_diff(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        use longtable_debug::format_diff;
        use longtable_language::Ast;

        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "diff requires arguments: (diff N M) or (diff :branches \"a\" \"b\")".to_string(),
            )));
        }

        // Check if comparing branches
        if let Ast::Keyword(k, _) = &args[0] {
            if k == "branches" {
                if args.len() < 3 {
                    return Err(Error::new(ErrorKind::Internal(
                        "diff :branches requires two branch names".to_string(),
                    )));
                }

                let name1 = match &args[1] {
                    Ast::String(s, _) => s.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "branch name must be a string, got {}",
                            other.type_name()
                        ))));
                    }
                };

                let name2 = match &args[2] {
                    Ast::String(s, _) => s.clone(),
                    other => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "branch name must be a string, got {}",
                            other.type_name()
                        ))));
                    }
                };

                let Some(diff) = self.session.timeline().diff_branches(&name1, &name2) else {
                    return Err(Error::new(ErrorKind::Internal(format!(
                        "cannot diff branches '{name1}' and '{name2}'"
                    ))));
                };

                let output = format_diff(&diff, self.session.world().interner(), 20);
                println!("Diff between branches '{name1}' and '{name2}':\n{output}");

                return Ok(Some(Value::Nil));
            }
        }

        // Compare ticks
        if args.len() < 2 {
            return Err(Error::new(ErrorKind::Internal(
                "diff requires two tick numbers: (diff N M)".to_string(),
            )));
        }

        let tick1_val = self.eval_form(&args[0])?;
        let Value::Int(tick1) = tick1_val else {
            return Err(Error::new(ErrorKind::Internal(
                "diff tick must be an integer".to_string(),
            )));
        };

        let tick2_val = self.eval_form(&args[1])?;
        let Value::Int(tick2) = tick2_val else {
            return Err(Error::new(ErrorKind::Internal(
                "diff tick must be an integer".to_string(),
            )));
        };

        #[allow(clippy::cast_sign_loss)]
        let (t1, t2) = (tick1.max(0) as u64, tick2.max(0) as u64);

        let Some(diff) = self.session.timeline().diff_ticks(t1, t2) else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "cannot diff ticks {t1} and {t2} - one or both not in history"
            ))));
        };

        let output = format_diff(&diff, self.session.world().interner(), 20);
        println!("Diff between tick {t1} and {t2}:\n{output}");

        Ok(Some(Value::Nil))
    }

    /// Handles the (history) or (history N) form.
    ///
    /// Shows recent tick history.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_history(&mut self, args: &[longtable_language::Ast]) -> Result<Option<Value>> {
        let count = if args.is_empty() {
            10
        } else {
            let count_val = self.eval_form(&args[0])?;
            let Value::Int(n) = count_val else {
                return Err(Error::new(ErrorKind::Internal(
                    "history count must be an integer".to_string(),
                )));
            };
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            {
                n.max(1) as usize
            }
        };

        let history = self.session.timeline().recent_history(count);

        if history.is_empty() {
            println!("No history available");
        } else {
            println!("Recent history ({} ticks):", history.len());
            for (tick, summary) in &history {
                println!("  Tick {tick}: {summary}");
            }
        }

        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(history.len() as i64)))
    }

    /// Handles the (timeline) form.
    ///
    /// Shows timeline status.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_timeline(&self) -> Result<Option<Value>> {
        let status = self.session.timeline().status();
        println!("{status}");
        Ok(Some(Value::Nil))
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

        // Verify entities exist (player, room, and the relationship entity from dual-write)
        assert_eq!(repl.session.world().entity_count(), 3);
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

    // ==================== Explain System Tests ====================

    #[test]
    fn why_unknown_entity() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // why with unknown entity name should error
        let result = repl.eval("(why player :health)");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown entity"));
    }

    #[test]
    fn why_requires_component() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // why with no component should error
        let result = repl.eval("(why 0)");
        assert!(result.is_err());
    }

    #[test]
    fn why_with_entity_ref() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define component
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();

        // Spawn entity
        repl.eval("(spawn: player :tag/player true)").unwrap();

        // why with entity by name should work (even if no provenance recorded)
        let result = repl.eval("(why player :tag/player)");
        assert!(result.is_ok());
    }

    #[test]
    fn why_with_depth() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define component
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();

        // Spawn entity
        repl.eval("(spawn: player :tag/player true)").unwrap();

        // why with depth should work
        let result = repl.eval("(why player :tag/player :depth 3)");
        assert!(result.is_ok());
    }

    #[test]
    fn explain_query_basic() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define component
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();

        // Spawn entity
        repl.eval("(spawn: player :tag/player true)").unwrap();

        // explain-query should work
        let result = repl.eval("(explain-query (query :where [[?e :tag/player]] :return ?e))");
        assert!(result.is_ok());
    }

    #[test]
    fn explain_query_with_entity() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Define component
        repl.eval("(component: tag/player :bool :default true)")
            .unwrap();

        // Spawn entity
        repl.eval("(spawn: player :tag/player true)").unwrap();

        // explain-query with entity should work
        let result =
            repl.eval("(explain-query (query :where [[?e :tag/player]] :return ?e) player)");
        assert!(result.is_ok());
    }

    #[test]
    fn explain_query_invalid_form() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // explain-query with non-query should error
        let result = repl.eval("(explain-query 42)");
        assert!(result.is_err());
    }

    // ==================== Time Travel Tests ====================

    #[test]
    fn timeline_shows_status() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // timeline command should work
        let result = repl.eval("(timeline)");
        assert!(result.is_ok());
    }

    #[test]
    fn branches_lists_branches() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // branches should list at least main
        let result = repl.eval("(branches)");
        assert!(result.is_ok());
    }

    #[test]
    fn branch_create_and_checkout() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Create a branch
        let result = repl.eval(r#"(branch! "test")"#);
        assert!(result.is_ok());

        // Checkout back to main
        let result = repl.eval(r#"(checkout! "main")"#);
        assert!(result.is_ok());

        // Checkout to test branch
        let result = repl.eval(r#"(checkout! "test")"#);
        assert!(result.is_ok());
    }

    #[test]
    fn branch_duplicate_fails() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Create branch
        repl.eval(r#"(branch! "duplicate")"#).unwrap();

        // Creating duplicate should fail
        let result = repl.eval(r#"(branch! "duplicate")"#);
        assert!(result.is_err());
    }

    #[test]
    fn checkout_nonexistent_fails() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // Checkout non-existent branch should fail
        let result = repl.eval(r#"(checkout! "nonexistent")"#);
        assert!(result.is_err());
    }

    #[test]
    fn history_empty_initially() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // history should work even when empty
        let result = repl.eval("(history)");
        assert!(result.is_ok());
        // Returns 0 when no history
        assert_eq!(result.unwrap(), Value::Int(0));
    }

    #[test]
    fn rollback_fails_without_history() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // rollback should fail when no history
        let result = repl.eval("(rollback! 1)");
        assert!(result.is_err());
    }

    #[test]
    fn goto_tick_fails_without_history() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // goto-tick should fail when tick not in history
        let result = repl.eval("(goto-tick! 42)");
        assert!(result.is_err());
    }

    #[test]
    fn diff_fails_without_history() {
        let editor = MockEditor::new(vec![]);
        let mut repl = Repl::with_editor(editor);

        // diff should fail when ticks not in history
        let result = repl.eval("(diff 1 2)");
        assert!(result.is_err());
    }
}
