//! The main REPL implementation.

use crate::editor::{LineEditor, ReadResult, RustylineEditor};
use crate::serialize;
use crate::session::{Session, SessionContext};

/// Embedded core stdlib functions.
const STDLIB_CORE: &str = include_str!("../../longtable_stdlib/stdlib/core.lt");
use longtable_engine::{
    Bindings, InputEvent, PatternCompiler, PatternMatcher, QueryCompiler, QueryExecutor,
    TickExecutor,
};
use longtable_foundation::{EntityId, Error, ErrorKind, KeywordId, Result, Value};
use longtable_language::{
    Ast, Compiler, Declaration, DeclarationAnalyzer, NamespaceContext, NamespaceInfo, Vm, parse,
};
use longtable_parser::NounResolver;
use longtable_parser::parser::{NaturalLanguageParser, ParseError, ParseResult};
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

    /// The compiler (persists globals across invocations).
    compiler: Compiler,

    /// Tick executor for advancing simulation.
    tick_executor: TickExecutor,

    /// Whether to show the welcome banner.
    show_banner: bool,

    /// Primary prompt.
    prompt: String,

    /// Continuation prompt (for multi-line input).
    continuation_prompt: String,

    /// Whether we're in input/game mode (natural language input).
    input_mode: bool,

    /// Prompt to use in input mode.
    input_mode_prompt: String,
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
            compiler: Compiler::new(),
            tick_executor: TickExecutor::new(),
            show_banner: true,
            prompt: "λ> ".to_string(),
            continuation_prompt: ".. ".to_string(),
            input_mode: false,
            input_mode_prompt: "> ".to_string(),
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

    /// Starts the REPL in input mode (natural language commands).
    #[must_use]
    pub const fn with_input_mode(mut self) -> Self {
        self.input_mode = true;
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

    /// Loads the standard library functions into the REPL session.
    ///
    /// This is called automatically by `run()`, but can be called manually
    /// if you need stdlib functions before starting the REPL loop.
    ///
    /// # Errors
    ///
    /// Returns an error if the stdlib fails to parse or evaluate.
    pub fn load_stdlib(&mut self) -> Result<()> {
        // Parse and evaluate the core stdlib
        self.eval(STDLIB_CORE)?;
        Ok(())
    }

    /// Runs the REPL loop.
    ///
    /// # Errors
    ///
    /// Returns an error if reading input or evaluation fails fatally.
    pub fn run(&mut self) -> Result<()> {
        // Load standard library first
        if let Err(e) = self.load_stdlib() {
            eprintln!("Warning: Failed to load stdlib: {e}");
        }

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

        // In input mode, dispatch to natural language handler unless it's an S-expression
        if self.input_mode {
            // S-expressions (starting with '(') are still evaluated normally
            // This allows (quit), (save!), etc. to work in input mode
            if trimmed.starts_with('(') {
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
            } else {
                // Natural language input
                match self.dispatch_input(&input) {
                    Ok(Some(value)) => {
                        if value != Value::Nil {
                            // dispatch_input typically prints via (say), so don't print again
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        self.print_error(&e);
                    }
                }
            }
        } else {
            // Normal REPL mode - evaluate S-expressions
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
        }

        Ok(true)
    }

    /// Reads a potentially multi-line input.
    fn read_input(&mut self) -> Result<Option<String>> {
        let mut input = String::new();
        let mut first_line = true;

        loop {
            let prompt = if first_line {
                if self.input_mode {
                    &self.input_mode_prompt
                } else {
                    &self.prompt
                }
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

    /// Evaluates input as an S-expression and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing, compilation, or execution fails.
    pub fn eval(&mut self, input: &str) -> Result<Value> {
        // Parse as S-expression
        let forms = parse(input)?;

        // Evaluate all forms
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
        // Prepare the persistent compiler for a new compilation
        // This preserves globals while clearing per-compilation state
        self.compiler.prepare_for_compilation();

        // Sync interner to compiler for declaration compilation (keyword resolution)
        let interner = self.session.world().interner().clone();
        self.compiler.set_interner(interner);

        let program = self.compiler.compile(&[form.clone()])?;

        // Sync interner back to session (compiler may have added keywords)
        if let Some(interner) = self.compiler.take_interner() {
            self.session.world_mut().set_interner(interner);
        }

        // Sync compiler's globals map to VM for late-bound lookups (forward references)
        for (name, &slot) in self.compiler.globals() {
            self.vm.register_global(name.clone(), slot);
        }

        // Execute with full RuntimeContext for registration opcode support
        let mut ctx = SessionContext::new(&mut self.session);
        let result = self.vm.execute_with_runtime_context(&program, &mut ctx)?;

        // Apply any effects produced by VM execution (Link, Unlink, SetComponent, etc.)
        self.apply_vm_effects()?;

        // Print any output from print/println/say calls
        for line in self.vm.output() {
            print!("{line}");
        }
        self.vm.clear_output();

        Ok(result)
    }

    /// Applies VM effects to the world.
    ///
    /// Effects like `Link`, `Unlink`, `SetComponent`, etc. are collected during VM execution
    /// and must be applied to persist the changes to the world state.
    ///
    /// Mergeable effects (`VecRemove`, `VecAdd`, `SetRemove`, `SetAdd`) on the same (entity, component, field)
    /// are grouped and merged before application. This ensures that multiple operations on the same
    /// field within a single expression all take effect.
    #[allow(clippy::too_many_lines, clippy::items_after_statements)]
    fn apply_vm_effects(&mut self) -> Result<()> {
        use longtable_foundation::{KeywordId, LtSet, Type};
        use longtable_language::VmEffect;
        use std::collections::HashMap;

        let effects = self.vm.take_effects();

        // Group mergeable effects by (entity, component, field)
        // Each entry contains (values_to_remove, values_to_add)
        type FieldKey = (EntityId, KeywordId, KeywordId);
        let mut vec_field_ops: HashMap<FieldKey, (Vec<Value>, Vec<Value>)> = HashMap::new();
        let mut set_field_ops: HashMap<FieldKey, (Vec<Value>, Vec<Value>)> = HashMap::new();

        // Mapping from temporary spawn IDs to actual entity IDs.
        // With spawn_with_id, temp IDs become real IDs, so this map stays empty.
        // Kept for future flexibility if spawn semantics change.
        let temp_to_real_id: HashMap<EntityId, EntityId> = HashMap::new();

        // Helper to translate entity IDs (temp -> real)
        let translate_id = |id: EntityId, map: &HashMap<EntityId, EntityId>| -> EntityId {
            *map.get(&id).unwrap_or(&id)
        };

        for effect in effects {
            match effect {
                // Non-mergeable effects: apply immediately
                VmEffect::Link {
                    source,
                    relationship,
                    target,
                } => {
                    let real_source = translate_id(source, &temp_to_real_id);
                    let real_target = translate_id(target, &temp_to_real_id);
                    let new_world =
                        self.session
                            .world()
                            .link(real_source, relationship, real_target)?;
                    *self.session.world_mut() = new_world;
                }
                VmEffect::Unlink {
                    source,
                    relationship,
                    target,
                } => {
                    let real_source = translate_id(source, &temp_to_real_id);
                    let real_target = translate_id(target, &temp_to_real_id);
                    let new_world =
                        self.session
                            .world()
                            .unlink(real_source, relationship, real_target)?;
                    *self.session.world_mut() = new_world;
                }
                VmEffect::SetComponent {
                    entity,
                    component,
                    value,
                } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let new_world = self.session.world().set(real_entity, component, value)?;
                    *self.session.world_mut() = new_world;
                }
                VmEffect::SetField {
                    entity,
                    component,
                    field,
                    value,
                } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let new_world =
                        self.session
                            .world()
                            .set_field(real_entity, component, field, value)?;
                    *self.session.world_mut() = new_world;
                }
                VmEffect::Spawn {
                    temp_id,
                    components,
                } => {
                    // Use spawn_with_id to create the entity with the temp_id as its
                    // permanent ID. This ensures EntityRefs returned from spawn! remain
                    // valid after effects are applied.
                    let (new_world, _) =
                        self.session.world().spawn_with_id(temp_id, &components)?;
                    *self.session.world_mut() = new_world;
                }
                VmEffect::Destroy { entity } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let new_world = self.session.world().destroy(real_entity)?;
                    *self.session.world_mut() = new_world;
                }
                VmEffect::RemoveComponent { entity, component } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let new_world = self
                        .session
                        .world()
                        .remove_component(real_entity, component)?;
                    *self.session.world_mut() = new_world;
                }

                // Mergeable effects: collect for later merging
                // Translate entity IDs from temp to real for all mergeable effects
                VmEffect::VecRemove {
                    entity,
                    component,
                    field,
                    value,
                } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let key = (real_entity, component, field);
                    let entry = vec_field_ops.entry(key).or_insert_with(|| (vec![], vec![]));
                    entry.0.push(value);
                }
                VmEffect::VecAdd {
                    entity,
                    component,
                    field,
                    value,
                } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let key = (real_entity, component, field);
                    let entry = vec_field_ops.entry(key).or_insert_with(|| (vec![], vec![]));
                    entry.1.push(value);
                }
                VmEffect::SetRemove {
                    entity,
                    component,
                    field,
                    value,
                } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let key = (real_entity, component, field);
                    let entry = set_field_ops.entry(key).or_insert_with(|| (vec![], vec![]));
                    entry.0.push(value);
                }
                VmEffect::SetAdd {
                    entity,
                    component,
                    field,
                    value,
                } => {
                    let real_entity = translate_id(entity, &temp_to_real_id);
                    let key = (real_entity, component, field);
                    let entry = set_field_ops.entry(key).or_insert_with(|| (vec![], vec![]));
                    entry.1.push(value);
                }

                // State management effects are now handled directly through RuntimeContext
                // during VM execution, not deferred as effects. These match arms are kept
                // for completeness but should not be reached.
                VmEffect::SaveState { .. } | VmEffect::RestoreState { .. } => {
                    // No-op: these are handled immediately during VM execution
                }
            }
        }

        // Apply merged vector field operations
        for ((entity, component, field), (to_remove, to_add)) in vec_field_ops {
            // Get current field value
            let current = self.session.world().get_field(entity, component, field)?;

            // Extract current vector elements
            let mut elements: Vec<Value> = match current {
                Some(Value::Vec(v)) => v.iter().cloned().collect(),
                Some(Value::Nil) | None => vec![],
                Some(other) => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: Type::vec(Type::Any),
                        actual: other.value_type(),
                    }));
                }
            };

            // Remove values (preserve order, remove first occurrence of each)
            for val in &to_remove {
                if let Some(pos) = elements.iter().position(|e| e == val) {
                    elements.remove(pos);
                }
            }

            // Add values
            for val in to_add {
                elements.push(val);
            }

            // Convert back to Value::Vec
            let new_vec = Value::Vec(elements.into_iter().collect());

            // Apply the merged change
            let new_world = self
                .session
                .world()
                .set_field(entity, component, field, new_vec)?;
            *self.session.world_mut() = new_world;
        }

        // Apply merged set field operations
        for ((entity, component, field), (to_remove, to_add)) in set_field_ops {
            // Get current field value
            let current = self.session.world().get_field(entity, component, field)?;

            // Extract current set elements
            let mut elements: LtSet<Value> = match current {
                Some(Value::Set(s)) => s,
                Some(Value::Nil) | None => LtSet::new(),
                Some(other) => {
                    return Err(Error::new(ErrorKind::TypeMismatch {
                        expected: Type::set(Type::Any),
                        actual: other.value_type(),
                    }));
                }
            };

            // Remove values
            for val in to_remove {
                elements = elements.remove(&val);
            }

            // Add values
            for val in to_add {
                elements = elements.insert(val);
            }

            // Convert back to Value::Set
            let new_set = Value::Set(elements);

            // Apply the merged change
            let new_world = self
                .session
                .world()
                .set_field(entity, component, field, new_set)?;
            *self.session.world_mut() = new_world;
        }

        Ok(())
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
            // NOTE: (say) is replaced by (println) native function

            // (def name value) - define a variable in the session
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

                // Evaluate the value expression
                let value = self.eval_form(&list[2])?;

                // Store in session variables
                self.session.set_variable(name.clone(), value.clone());
                Ok(Some(value))
            }

            // (run) - enter input/game mode for natural language commands
            Ast::Symbol(s, _) if s == "run" => {
                if list.len() != 1 {
                    return Err(Error::new(ErrorKind::Internal(
                        "run takes no arguments".to_string(),
                    )));
                }

                self.input_mode = true;
                println!("Entering input mode. S-expressions still work. Use (repl) to exit.");
                Ok(Some(Value::Nil))
            }

            // (repl) - exit input mode and return to normal REPL
            Ast::Symbol(s, _) if s == "repl" => {
                if list.len() != 1 {
                    return Err(Error::new(ErrorKind::Internal(
                        "repl takes no arguments".to_string(),
                    )));
                }

                self.input_mode = false;
                println!("Returning to REPL mode.");
                Ok(Some(Value::Nil))
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

            // NOTE: component:, relationship:, rule: are now handled by compiler opcodes

            // (spawn: name :component value ...) - spawn an entity with components
            Ast::Symbol(s, _) if s == "spawn:" => {
                if let Some(Declaration::Spawn(spawn_decl)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_spawn(&spawn_decl)
                } else {
                    Err(Error::new(ErrorKind::Internal(
                        "invalid spawn: form".to_string(),
                    )))
                }
            }

            // (link: source :relationship target) - create a relationship between entities
            Ast::Symbol(s, _) if s == "link:" => {
                if let Some(Declaration::Link(link_decl)) = DeclarationAnalyzer::analyze(form)? {
                    self.execute_link(&link_decl)
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

            // ==================== Backtracking Support ====================

            // (save-state) - save current world state, returns snapshot ID
            Ast::Symbol(s, _) if s == "save-state" => self.handle_save_state(),

            // (restore-state ID) - restore world to a saved snapshot
            Ast::Symbol(s, _) if s == "restore-state" => self.handle_restore_state(&list[1..]),

            // ==================== Natural Language Input ====================

            // (input! "command text") - parse and execute natural language command
            Ast::Symbol(s, _) if s == "input!" => self.handle_input(&list[1..]),

            // NOTE: (entity-ref) is now a compiler form
            // NOTE: Parser vocabulary declarations (verb:, direction:, preposition:, etc.)
            //       are now handled by compiler opcodes
            _ => Ok(None),
        }
    }

    // NOTE: execute_component(), execute_relationship(), execute_spawn(), execute_link()
    //       removed - now handled by compiler forms/opcodes

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

    /// Executes a spawn: declaration.
    ///
    /// Creates an entity with the specified components and registers it by name.
    fn execute_spawn(
        &mut self,
        spawn_decl: &longtable_language::declaration::SpawnDecl,
    ) -> Result<Option<Value>> {
        use longtable_foundation::LtMap;

        // Build a components map from the declaration
        let mut components: LtMap<Value, Value> = LtMap::new();

        for (comp_name, comp_value_ast) in &spawn_decl.components {
            // Intern the component keyword
            let comp_kw = self
                .session
                .world_mut()
                .interner_mut()
                .intern_keyword(comp_name);

            // Evaluate the component value AST
            let comp_value = self.eval_form(comp_value_ast)?;

            components = components.insert(Value::Keyword(comp_kw), comp_value);
        }

        // Spawn the entity
        let (new_world, entity_id) = self.session.world().spawn(&components)?;
        self.session.set_world(new_world);

        // Register the entity by name
        self.session
            .register_entity(spawn_decl.name.clone(), entity_id);

        Ok(Some(Value::EntityRef(entity_id)))
    }

    /// Executes a link: declaration.
    ///
    /// Creates a relationship between two entities by name.
    fn execute_link(
        &mut self,
        link_decl: &longtable_language::declaration::LinkDecl,
    ) -> Result<Option<Value>> {
        // Lookup source entity by name
        let source_id = self.session.get_entity(&link_decl.source).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown entity: {}",
                link_decl.source
            )))
        })?;

        // Lookup target entity by name
        let target_id = self.session.get_entity(&link_decl.target).ok_or_else(|| {
            Error::new(ErrorKind::Internal(format!(
                "unknown entity: {}",
                link_decl.target
            )))
        })?;

        // Intern the relationship keyword
        let rel_kw = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&link_decl.relationship);

        // Create the relationship
        let (new_world, _rel_entity) = self
            .session
            .world()
            .create_relationship(rel_kw, source_id, target_id)?;
        self.session.set_world(new_world);

        Ok(Some(Value::Nil))
    }

    // NOTE: execute_verb(), execute_direction(), execute_preposition(), execute_pronoun(),
    //       execute_adverb(), execute_noun_type(), execute_scope(), execute_command(),
    //       execute_rule(), execute_action() removed - now handled by compiler opcodes

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

    /// Handles the (input "command") form.
    ///
    /// Parses natural language input and executes the corresponding action.
    fn handle_input(&mut self, args: &[Ast]) -> Result<Option<Value>> {
        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "input requires a string argument".to_string(),
            )));
        }

        // Get the input string
        let input_str = match &args[0] {
            Ast::String(s, _) => s.as_str(),
            other => {
                // Try to evaluate it
                let value = self.eval_form(other)?;
                match value {
                    Value::String(ref s) => {
                        return self.dispatch_input(s);
                    }
                    _ => {
                        return Err(Error::new(ErrorKind::Internal(format!(
                            "input expects a string, got {}",
                            value.value_type()
                        ))));
                    }
                }
            }
        };

        self.dispatch_input(input_str)
    }

    /// Dispatches natural language input to the appropriate action.
    fn dispatch_input(&mut self, input: &str) -> Result<Option<Value>> {
        // Get the player entity as the actor
        let Some(actor) = self.session.get_entity("player") else {
            println!("No player entity found.");
            return Ok(Some(Value::Nil));
        };

        // Try to parse with NaturalLanguageParser
        let parse_result = {
            // Build parser with current vocabulary and syntaxes
            let vocab = self.session.vocabulary_registry().clone();
            let mut parser = NaturalLanguageParser::new(vocab);

            // Add all compiled syntaxes
            for syntax in self.session.compiled_syntaxes() {
                parser.add_syntax(syntax.clone());
            }

            // Configure noun resolver with appropriate keywords
            let name_kw = self.session.world().interner().lookup_keyword("name");
            let value_kw = self.session.world().interner().lookup_keyword("value");
            let aliases_kw = self.session.world().interner().lookup_keyword("aliases");
            let adjectives_kw = self.session.world().interner().lookup_keyword("adjectives");

            if let (Some(name_kw), Some(value_kw)) = (name_kw, value_kw) {
                let resolver = NounResolver::new(
                    name_kw,
                    value_kw,
                    aliases_kw.unwrap_or(name_kw),    // fallback
                    adjectives_kw.unwrap_or(name_kw), // fallback
                );
                parser = parser.with_noun_resolver(resolver);
            }

            // Parse the input
            parser.parse(input, actor, self.session.world())
        };

        match parse_result {
            ParseResult::Success(cmd) => {
                self.execute_parsed_command(cmd.action, actor, cmd.direction, cmd.noun_bindings)
            }
            ParseResult::Multiple(cmds) => {
                // Execute each command in sequence
                for cmd in cmds {
                    self.execute_parsed_command(
                        cmd.action,
                        actor,
                        cmd.direction,
                        cmd.noun_bindings,
                    )?;
                }
                Ok(Some(Value::Nil))
            }
            ParseResult::Ambiguous(disamb) => {
                println!("{}", disamb.question);
                for (i, (desc, _)) in disamb.options.iter().enumerate() {
                    println!("  {}. {}", i + 1, desc);
                }
                // TODO: Store pending parse state for disambiguation
                Ok(Some(Value::Nil))
            }
            ParseResult::Error(err) => {
                // Fall back to simple verb lookup for backwards compatibility
                self.dispatch_input_simple(input, actor).or_else(|_| {
                    match err {
                        ParseError::EmptyInput => println!("What?"),
                        ParseError::NoMatch => {
                            let verb = input.split_whitespace().next().unwrap_or(input);
                            println!("I don't understand '{verb}'.");
                        }
                        ParseError::UnknownWord(word) => {
                            println!("I don't know the word '{word}'.");
                        }
                        ParseError::NotFound(noun) => {
                            println!("I don't see any '{noun}' here.");
                        }
                        ParseError::WrongType { noun, expected } => {
                            println!("You can't do that with the {noun} ({expected} expected).");
                        }
                        ParseError::NoReferent(pronoun) => {
                            println!("I don't know what '{pronoun}' refers to.");
                        }
                    }
                    Ok(Some(Value::Nil))
                })
            }
        }
    }

    /// Executes a parsed command.
    fn execute_parsed_command(
        &mut self,
        action: KeywordId,
        actor: EntityId,
        direction: Option<(String, KeywordId)>,
        noun_bindings: std::collections::HashMap<String, EntityId>,
    ) -> Result<Option<Value>> {
        // Get the full action declaration
        let Some(action_decl) = self.session.get_action_decl(action).cloned() else {
            let action_name = self
                .session
                .world()
                .interner()
                .get_keyword(action)
                .unwrap_or("unknown");
            println!("Action '{action_name}' has no declaration.");
            return Ok(Some(Value::Nil));
        };

        // Build bindings
        let mut bindings = Bindings::new();

        // Bind actor
        if action_decl.params.contains(&"actor".to_string()) {
            bindings.set("actor".to_string(), Value::EntityRef(actor));
        }

        // Bind direction if present
        // Note: We use "direction" as the binding name since that's what the `go` action expects
        // TODO: Properly use the :bindings mapping from command declarations
        if let Some((_var_name, dir_kw)) = direction {
            bindings.set("direction".to_string(), Value::Keyword(dir_kw));
        }

        // Bind all noun bindings (e.g., target, item, container, etc.)
        for (var_name, entity_id) in noun_bindings {
            bindings.set(var_name, Value::EntityRef(entity_id));
        }

        // Evaluate preconditions
        if !action_decl.preconditions.is_empty() {
            match self.evaluate_preconditions(&action_decl, &bindings) {
                Ok(Some(new_bindings)) => bindings = new_bindings,
                Ok(None) => return Ok(Some(Value::Nil)),
                Err(e) => return Err(e),
            }
        }

        // Execute handlers
        for handler in &action_decl.handler {
            self.execute_action_handler(handler, &bindings)?;
        }

        Ok(Some(Value::Nil))
    }

    /// Simple input dispatch - fallback for when parser fails.
    fn dispatch_input_simple(&mut self, input: &str, actor: EntityId) -> Result<Option<Value>> {
        let tokens: Vec<&str> = input.split_whitespace().collect();

        if tokens.is_empty() {
            return Err(Error::new(ErrorKind::Internal("Empty input".to_string())));
        }

        let verb = tokens[0].to_lowercase();
        let action_name_kw = self
            .session
            .world_mut()
            .interner_mut()
            .intern_keyword(&verb);

        // Check if we have an action registered with this name
        if self.session.action_registry().get(action_name_kw).is_none() {
            return Err(Error::new(ErrorKind::Internal(format!(
                "Unknown action: {verb}"
            ))));
        }

        let Some(action_decl) = self.session.get_action_decl(action_name_kw).cloned() else {
            return Err(Error::new(ErrorKind::Internal(format!(
                "Action '{verb}' has no declaration."
            ))));
        };

        let mut bindings = Bindings::new();

        if action_decl.params.contains(&"actor".to_string()) {
            bindings.set("actor".to_string(), Value::EntityRef(actor));
        }

        // Bind direction if present
        if action_decl.params.contains(&"direction".to_string()) && tokens.len() > 1 {
            let direction_str = tokens[1].to_lowercase();
            let direction_kw = self
                .session
                .world_mut()
                .interner_mut()
                .intern_keyword(&direction_str);
            bindings.set("direction".to_string(), Value::Keyword(direction_kw));
        }

        if !action_decl.preconditions.is_empty() {
            match self.evaluate_preconditions(&action_decl, &bindings) {
                Ok(Some(new_bindings)) => bindings = new_bindings,
                Ok(None) => return Ok(Some(Value::Nil)),
                Err(e) => return Err(e),
            }
        }

        for handler in &action_decl.handler {
            self.execute_action_handler(handler, &bindings)?;
        }

        Ok(Some(Value::Nil))
    }

    /// Evaluates action preconditions and returns bindings if they all pass.
    fn evaluate_preconditions(
        &mut self,
        action_decl: &longtable_language::ActionDecl,
        initial_bindings: &Bindings,
    ) -> Result<Option<Bindings>> {
        // Evaluate all preconditions and accumulate bindings
        let mut result = initial_bindings.clone();

        for precondition in &action_decl.preconditions {
            // Compile the pattern
            let compiled = PatternCompiler::compile(
                &precondition.pattern,
                self.session.world_mut().interner_mut(),
            )?;

            // Try to match against the world
            let all_matches = PatternMatcher::match_pattern(&compiled, self.session.world());

            // Filter matches to those compatible with our initial bindings
            let compatible_matches: Vec<_> = all_matches
                .into_iter()
                .filter(|m| {
                    // Check that all variables in initial_bindings match this result
                    for (var, expected_val) in result.iter() {
                        if let Some(actual_val) = m.get(var) {
                            if actual_val != expected_val {
                                return false;
                            }
                        }
                    }
                    true
                })
                .collect();

            if compatible_matches.is_empty() {
                // No match - precondition failed
                // TODO: Print the failure message from precondition.message
                println!("You can't do that.");
                return Ok(None);
            }

            // Use the first compatible match and merge with accumulated bindings
            for (var, value) in compatible_matches[0].iter() {
                result.set(var.clone(), value.clone());
            }
        }

        Ok(Some(result))
    }

    /// Executes a single action handler expression with variable bindings.
    fn execute_action_handler(&mut self, handler: &Ast, bindings: &Bindings) -> Result<Value> {
        // Convert Vector to List for evaluation (handlers may be deserialized as vectors)
        let handler = match handler {
            Ast::Vector(elements, span) => Ast::List(elements.clone(), *span),
            other => other.clone(),
        };

        // Check for special forms like (say "...")
        if let Ast::List(ref elements, _) = handler {
            if let Some(Ast::Symbol(name, _)) = elements.first() {
                if name == "say" {
                    // Handle (say expr) specially
                    if let Some(msg_ast) = elements.get(1) {
                        let msg = self.eval_with_bindings(msg_ast, bindings)?;
                        if let Value::String(s) = msg {
                            println!("{s}");
                        } else {
                            println!("{msg}");
                        }
                        return Ok(Value::Nil);
                    }
                }
            }
        }

        // Fall back to general evaluation with bindings
        self.eval_with_bindings(&handler, bindings)
    }

    /// Evaluates an AST with variable bindings.
    fn eval_with_bindings(&mut self, ast: &Ast, bindings: &Bindings) -> Result<Value> {
        // If it's a variable reference (symbol starting with ?), look it up
        if let Ast::Symbol(name, _) = ast {
            if let Some(stripped) = name.strip_prefix('?') {
                if let Some(value) = bindings.get(stripped) {
                    return Ok(value.clone());
                }
                // Variable not bound - return as error or nil
                return Err(Error::new(ErrorKind::Internal(format!(
                    "unbound variable: {name}"
                ))));
            }
        }

        // For function calls, substitute variables in arguments
        // Handle both List and Vector (vectors from deserialization should be treated as lists)
        let (Ast::List(elements, span) | Ast::Vector(elements, span)) = ast else {
            return self.eval_form(ast);
        };

        if !elements.is_empty() {
            // Substitute variables in each element
            let substituted: Vec<Ast> = elements
                .iter()
                .map(|elem| self.substitute_variables(elem, bindings))
                .collect();

            // Evaluate the substituted form as a list (function call)
            let new_ast = Ast::List(substituted, *span);
            return self.eval_form(&new_ast);
        }

        // Fall back to normal evaluation
        self.eval_form(ast)
    }

    /// Substitutes bound variables in an AST.
    fn substitute_variables(&self, ast: &Ast, bindings: &Bindings) -> Ast {
        match ast {
            Ast::Symbol(name, span) => {
                if let Some(stripped) = name.strip_prefix('?') {
                    if let Some(value) = bindings.get(stripped) {
                        // Convert Value back to AST
                        return self.value_to_ast(value, *span);
                    }
                }
                ast.clone()
            }
            Ast::List(elements, span) => {
                let substituted: Vec<Ast> = elements
                    .iter()
                    .map(|elem| self.substitute_variables(elem, bindings))
                    .collect();
                Ast::List(substituted, *span)
            }
            // Vectors - substitute but keep as vectors (needed for let bindings, etc.)
            Ast::Vector(elements, span) => {
                let substituted: Vec<Ast> = elements
                    .iter()
                    .map(|elem| self.substitute_variables(elem, bindings))
                    .collect();
                Ast::Vector(substituted, *span)
            }
            // Other AST types pass through unchanged
            _ => ast.clone(),
        }
    }

    /// Converts a Value back to an AST for substitution.
    #[allow(clippy::cast_possible_wrap)]
    fn value_to_ast(&self, value: &Value, span: longtable_language::Span) -> Ast {
        match value {
            Value::Bool(b) => Ast::Bool(*b, span),
            Value::Int(n) => Ast::Int(*n, span),
            Value::Float(f) => Ast::Float(*f, span),
            Value::String(s) => Ast::String(s.to_string(), span),
            Value::Keyword(kw) => {
                // Look up the keyword name
                let name = self
                    .session
                    .world()
                    .interner()
                    .get_keyword(*kw)
                    .unwrap_or("unknown");
                Ast::Keyword(name.to_string(), span)
            }
            Value::EntityRef(id) => {
                // Represent as a special form the evaluator can understand
                Ast::List(
                    vec![
                        Ast::Symbol("entity-ref".to_string(), span),
                        Ast::Int(id.index as i64, span),
                        Ast::Int(i64::from(id.generation), span),
                    ],
                    span,
                )
            }
            // Nil and collections fall through to Nil
            _ => Ast::Nil(span),
        }
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

    // ==================== Backtracking Support Handlers ====================

    /// Handles the (save-state) form.
    ///
    /// Saves the current world state and returns a snapshot ID.
    /// This is used by the Sudoku solver for backtracking.
    #[allow(clippy::unnecessary_wraps)]
    fn handle_save_state(&mut self) -> Result<Option<Value>> {
        let id = self.session.save_state();
        #[allow(clippy::cast_possible_wrap)]
        Ok(Some(Value::Int(id as i64)))
    }

    /// Handles the (restore-state ID) form.
    ///
    /// Restores the world to a previously saved snapshot.
    fn handle_restore_state(&mut self, args: &[Ast]) -> Result<Option<Value>> {
        if args.is_empty() {
            return Err(Error::new(ErrorKind::Internal(
                "restore-state requires a snapshot ID: (restore-state ID)".to_string(),
            )));
        }

        let id_val = self.eval_form(&args[0])?;
        let Value::Int(id) = id_val else {
            return Err(Error::new(ErrorKind::Internal(
                "restore-state requires an integer snapshot ID".to_string(),
            )));
        };

        #[allow(clippy::cast_sign_loss)]
        let id = id as u64;

        self.session.restore_state(id)?;
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

    /// Formats a value for display, resolving keywords via the world's interner.
    fn format_value(&self, value: &Value) -> String {
        let formatted = self.format_value_inner(value);
        format!("\x1b[1m{formatted}\x1b[0m")
    }

    /// Recursively formats a value, resolving keywords to their string names.
    fn format_value_inner(&self, value: &Value) -> String {
        match value {
            Value::Nil => "nil".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(n) => {
                if n.fract() == 0.0 {
                    format!("{n}.0")
                } else {
                    n.to_string()
                }
            }
            Value::String(s) => format!("\"{s}\""),
            Value::Symbol(id) => self
                .session
                .world()
                .interner()
                .get_symbol(*id)
                .map_or_else(|| format!("Symbol({})", id.index()), str::to_string),
            Value::Keyword(id) => self
                .session
                .world()
                .interner()
                .get_keyword(*id)
                .map_or_else(|| format!("Keyword({})", id.index()), |s| format!(":{s}")),
            Value::EntityRef(id) => format!("Entity({}, {})", id.index, id.generation),
            Value::Vec(v) => {
                let items: Vec<_> = v.iter().map(|v| self.format_value_inner(v)).collect();
                format!("[{}]", items.join(" "))
            }
            Value::List(l) => {
                let items: Vec<_> = l.iter().map(|v| self.format_value_inner(v)).collect();
                format!("({})", items.join(" "))
            }
            Value::Set(s) => {
                let items: Vec<_> = s.iter().map(|v| self.format_value_inner(v)).collect();
                format!("#{{{}}}", items.join(" "))
            }
            Value::Map(m) => {
                let pairs: Vec<_> = m
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{} {}",
                            self.format_value_inner(k),
                            self.format_value_inner(v)
                        )
                    })
                    .collect();
                format!("{{{}}}", pairs.join(" "))
            }
            Value::Fn(_) => "<fn>".to_string(),
        }
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
