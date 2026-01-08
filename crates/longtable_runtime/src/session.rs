//! Session state for the REPL.
//!
//! The session holds the current world state and session-local variables.
//!
//! This module also provides [`SessionContext`], which implements the
//! [`RuntimeContext`] trait for VM execution with full runtime access.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use longtable_debug::{DebugSession, Timeline, Tracer};
use longtable_foundation::{EntityId, Error, ErrorKind, Interner, KeywordId, Result, Type, Value};
use longtable_language::{ActionDecl, ModuleRegistry, NamespaceContext, RuntimeContext, VmContext};
use longtable_parser::scope::CompiledScope;
use longtable_parser::vocabulary::{
    CommandSyntax, Direction, NounType, Preposition, Pronoun, PronounGender, PronounNumber, Verb,
};
use longtable_parser::{ActionRegistry, VocabularyRegistry};
use longtable_storage::World;
use longtable_storage::schema::{
    Cardinality, ComponentSchema, FieldSchema, OnDelete, RelationshipSchema,
};

/// Session state for an interactive REPL session.
#[allow(clippy::struct_field_names)]
pub struct Session {
    /// The current world state.
    world: World,

    /// Session-local variable bindings (from `def`).
    variables: HashMap<String, Value>,

    /// Entity name registry (from `spawn:` declarations).
    /// Maps symbolic names (e.g., "player", "cave-entrance") to `EntityId`s.
    entity_names: HashMap<String, EntityId>,

    /// Current load path for relative file resolution.
    load_path: PathBuf,

    /// Whether to auto-commit world mutations.
    auto_commit: bool,

    /// Registry for tracking loaded modules and namespaces.
    module_registry: ModuleRegistry,

    /// Current namespace context for symbol resolution.
    namespace_context: NamespaceContext,

    /// Tracer for observability.
    tracer: Tracer,

    /// Debug session for breakpoints and stepping.
    debug_session: DebugSession,

    /// Timeline for time travel debugging.
    timeline: Timeline,

    /// Vocabulary registry for natural language parsing.
    vocabulary_registry: VocabularyRegistry,

    /// Action registry for command execution.
    action_registry: ActionRegistry,

    /// Compiled scopes for noun resolution.
    scopes: Vec<CompiledScope>,

    /// Full action declarations keyed by action name.
    /// Includes params, preconditions, and handlers.
    action_decls: HashMap<KeywordId, ActionDecl>,
}

impl Session {
    /// Creates a new session with an empty world.
    #[must_use]
    pub fn new() -> Self {
        Self {
            world: World::new(0),
            variables: HashMap::new(),
            entity_names: HashMap::new(),
            load_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            auto_commit: true,
            module_registry: ModuleRegistry::new(),
            namespace_context: NamespaceContext::new(),
            tracer: Tracer::disabled(),
            debug_session: DebugSession::new(),
            timeline: Timeline::new(),
            vocabulary_registry: VocabularyRegistry::default(),
            action_registry: ActionRegistry::new(),
            scopes: Vec::new(),
            action_decls: HashMap::new(),
        }
    }

    /// Creates a new session with the given world.
    #[must_use]
    pub fn with_world(world: World) -> Self {
        Self {
            world,
            variables: HashMap::new(),
            entity_names: HashMap::new(),
            load_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            auto_commit: true,
            module_registry: ModuleRegistry::new(),
            namespace_context: NamespaceContext::new(),
            tracer: Tracer::disabled(),
            debug_session: DebugSession::new(),
            timeline: Timeline::new(),
            vocabulary_registry: VocabularyRegistry::default(),
            action_registry: ActionRegistry::new(),
            scopes: Vec::new(),
            action_decls: HashMap::new(),
        }
    }

    /// Returns a reference to the current world.
    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }

    /// Returns a mutable reference to the current world.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Replaces the world (for auto-commit after mutations).
    pub fn set_world(&mut self, world: World) {
        self.world = world;
    }

    /// Gets a session variable by name.
    #[must_use]
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    /// Sets a session variable.
    pub fn set_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Returns all session variables.
    #[must_use]
    pub fn variables(&self) -> &HashMap<String, Value> {
        &self.variables
    }

    /// Gets a named entity by its symbolic name.
    #[must_use]
    pub fn get_entity(&self, name: &str) -> Option<EntityId> {
        self.entity_names.get(name).copied()
    }

    /// Registers a named entity.
    pub fn register_entity(&mut self, name: String, entity: EntityId) {
        self.entity_names.insert(name, entity);
    }

    /// Returns all named entities.
    #[must_use]
    pub fn entity_names(&self) -> &HashMap<String, EntityId> {
        &self.entity_names
    }

    /// Gets the current load path.
    #[must_use]
    pub fn load_path(&self) -> &PathBuf {
        &self.load_path
    }

    /// Sets the load path (used when loading files).
    pub fn set_load_path(&mut self, path: PathBuf) {
        self.load_path = path;
    }

    /// Returns whether auto-commit is enabled.
    #[must_use]
    pub const fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    /// Sets the auto-commit mode.
    pub fn set_auto_commit(&mut self, auto_commit: bool) {
        self.auto_commit = auto_commit;
    }

    /// Returns a reference to the module registry.
    #[must_use]
    pub fn module_registry(&self) -> &ModuleRegistry {
        &self.module_registry
    }

    /// Returns a mutable reference to the module registry.
    pub fn module_registry_mut(&mut self) -> &mut ModuleRegistry {
        &mut self.module_registry
    }

    /// Returns a reference to the namespace context.
    #[must_use]
    pub fn namespace_context(&self) -> &NamespaceContext {
        &self.namespace_context
    }

    /// Returns a mutable reference to the namespace context.
    pub fn namespace_context_mut(&mut self) -> &mut NamespaceContext {
        &mut self.namespace_context
    }

    /// Sets the namespace context.
    pub fn set_namespace_context(&mut self, context: NamespaceContext) {
        self.namespace_context = context;
    }

    /// Resolves a path relative to the current load path.
    #[must_use]
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let p = PathBuf::from(path);
        if p.is_absolute() {
            p
        } else {
            self.load_path.join(p)
        }
    }

    /// Returns a reference to the tracer.
    #[must_use]
    pub fn tracer(&self) -> &Tracer {
        &self.tracer
    }

    /// Returns a mutable reference to the tracer.
    pub fn tracer_mut(&mut self) -> &mut Tracer {
        &mut self.tracer
    }

    /// Returns a reference to the debug session.
    #[must_use]
    pub fn debug_session(&self) -> &DebugSession {
        &self.debug_session
    }

    /// Returns a mutable reference to the debug session.
    pub fn debug_session_mut(&mut self) -> &mut DebugSession {
        &mut self.debug_session
    }

    /// Returns a reference to the timeline.
    #[must_use]
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// Returns a mutable reference to the timeline.
    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    /// Returns a reference to the vocabulary registry.
    #[must_use]
    pub fn vocabulary_registry(&self) -> &VocabularyRegistry {
        &self.vocabulary_registry
    }

    /// Returns a mutable reference to the vocabulary registry.
    pub fn vocabulary_registry_mut(&mut self) -> &mut VocabularyRegistry {
        &mut self.vocabulary_registry
    }

    /// Returns a reference to the action registry.
    #[must_use]
    pub fn action_registry(&self) -> &ActionRegistry {
        &self.action_registry
    }

    /// Returns a mutable reference to the action registry.
    pub fn action_registry_mut(&mut self) -> &mut ActionRegistry {
        &mut self.action_registry
    }

    /// Returns a reference to the compiled scopes.
    #[must_use]
    pub fn scopes(&self) -> &[CompiledScope] {
        &self.scopes
    }

    /// Adds a compiled scope.
    pub fn add_scope(&mut self, scope: CompiledScope) {
        self.scopes.push(scope);
    }

    /// Registers a full action declaration.
    pub fn register_action_decl(&mut self, action_name: KeywordId, decl: ActionDecl) {
        self.action_decls.insert(action_name, decl);
    }

    /// Gets the full action declaration.
    #[must_use]
    pub fn get_action_decl(&self, action_name: KeywordId) -> Option<&ActionDecl> {
        self.action_decls.get(&action_name)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// SessionContext - RuntimeContext implementation for VM execution
// =============================================================================

/// A context that provides full runtime access for VM execution.
///
/// This wraps a mutable reference to [`Session`] and implements both
/// [`VmContext`] (for read-only World access) and [`RuntimeContext`]
/// (for registering schemas, vocabulary, and other machine configuration).
///
/// # Example
///
/// ```ignore
/// let mut session = Session::new();
/// let mut ctx = SessionContext::new(&mut session, &mut interner);
/// // VM can now execute with full runtime access
/// ```
pub struct SessionContext<'a> {
    /// Mutable reference to the session.
    session: &'a mut Session,
    /// Mutable reference to the string interner.
    interner: &'a mut Interner,
}

impl<'a> SessionContext<'a> {
    /// Creates a new `SessionContext` wrapping a session and interner.
    #[must_use]
    pub fn new(session: &'a mut Session, interner: &'a mut Interner) -> Self {
        Self { session, interner }
    }

    /// Returns a reference to the underlying session.
    #[must_use]
    pub fn session(&self) -> &Session {
        self.session
    }

    /// Returns a mutable reference to the underlying session.
    pub fn session_mut(&mut self) -> &mut Session {
        self.session
    }
}

// =============================================================================
// VmContext implementation for SessionContext
// =============================================================================

impl VmContext for SessionContext<'_> {
    fn get_component(&self, entity: EntityId, component: KeywordId) -> Result<Option<Value>> {
        self.session.world.get(entity, component)
    }

    fn get_field(
        &self,
        entity: EntityId,
        component: KeywordId,
        field: KeywordId,
    ) -> Result<Option<Value>> {
        self.session.world.get_field(entity, component, field)
    }

    fn exists(&self, entity: EntityId) -> bool {
        self.session.world.exists(entity)
    }

    fn has_component(&self, entity: EntityId, component: KeywordId) -> bool {
        self.session.world.has(entity, component)
    }

    fn resolve_keyword(&self, value: &Value) -> Option<KeywordId> {
        if let Value::Keyword(k) = value {
            Some(*k)
        } else {
            None
        }
    }

    fn with_component(&self, component: KeywordId) -> Vec<EntityId> {
        self.session.world.with_component(component).collect()
    }

    fn find_relationships(
        &self,
        rel_type: Option<KeywordId>,
        source: Option<EntityId>,
        target: Option<EntityId>,
    ) -> Vec<EntityId> {
        self.session
            .world
            .find_relationships(rel_type, source, target)
    }

    fn targets(&self, source: EntityId, rel_type: KeywordId) -> Vec<EntityId> {
        self.session.world.targets(source, rel_type).collect()
    }

    fn sources(&self, target: EntityId, rel_type: KeywordId) -> Vec<EntityId> {
        self.session.world.sources(target, rel_type).collect()
    }
}

// =============================================================================
// RuntimeContext implementation for SessionContext
// =============================================================================

impl RuntimeContext for SessionContext<'_> {
    fn register_component_schema(&mut self, schema: &Value) -> Result<()> {
        let component_schema = parse_component_schema(schema, self.interner)?;
        let new_world = self.session.world.register_component(component_schema)?;
        self.session.set_world(new_world);
        Ok(())
    }

    fn register_relationship_schema(&mut self, schema: &Value) -> Result<()> {
        let rel_schema = parse_relationship_schema(schema, self.interner)?;
        let new_world = self.session.world.register_relationship(rel_schema)?;
        self.session.set_world(new_world);
        Ok(())
    }

    fn register_verb(&mut self, data: &Value) -> Result<()> {
        let verb = parse_verb(data, self.interner)?;
        self.session.vocabulary_registry_mut().register_verb(verb);
        Ok(())
    }

    fn register_direction(&mut self, data: &Value) -> Result<()> {
        let direction = parse_direction(data, self.interner)?;
        self.session
            .vocabulary_registry_mut()
            .register_direction(direction);
        Ok(())
    }

    fn register_preposition(&mut self, data: &Value) -> Result<()> {
        let prep = parse_preposition(data)?;
        self.session
            .vocabulary_registry_mut()
            .register_preposition(prep);
        Ok(())
    }

    fn register_pronoun(&mut self, data: &Value) -> Result<()> {
        let pronoun = parse_pronoun(data, self.interner)?;
        self.session
            .vocabulary_registry_mut()
            .register_pronoun(pronoun);
        Ok(())
    }

    fn register_adverb(&mut self, data: &Value) -> Result<()> {
        let adverb = extract_keyword_field(data, "name")?;
        self.session
            .vocabulary_registry_mut()
            .register_adverb(adverb);
        Ok(())
    }

    fn register_type(&mut self, data: &Value) -> Result<()> {
        let noun_type = parse_noun_type(data)?;
        self.session
            .vocabulary_registry_mut()
            .register_type(noun_type);
        Ok(())
    }

    fn register_scope(&mut self, data: &Value) -> Result<()> {
        // Scopes require compilation - for now, store the raw data
        // Full implementation will compile the scope resolver
        let _name = extract_keyword_field(data, "name")?;
        // TODO: Compile scope and add to session.scopes
        Ok(())
    }

    fn register_command(&mut self, data: &Value) -> Result<()> {
        let cmd = parse_command_syntax(data)?;
        self.session.vocabulary_registry_mut().register_command(cmd);
        Ok(())
    }

    fn register_action(&mut self, data: &Value) -> Result<()> {
        // Actions have complex structure - extract name and store
        let _name = extract_keyword_field(data, "name")?;
        // TODO: Compile action and register in action_registry
        // For now, this is a placeholder
        Ok(())
    }

    fn register_rule(&mut self, data: &Value) -> Result<EntityId> {
        // Rules are entities with :meta/rule component
        // This will spawn an entity and compile the rule
        let _name = extract_keyword_field(data, "name")?;
        // TODO: Spawn rule entity, compile pattern/action, store in World
        // For now, return a placeholder entity ID
        Err(Error::new(ErrorKind::Internal(
            "rule registration not yet implemented".to_string(),
        )))
    }

    fn intern_keyword(&mut self, name: &str) -> KeywordId {
        self.interner.intern_keyword(name)
    }
}

// =============================================================================
// Helper functions for parsing Value maps into schema/vocabulary types
// =============================================================================

/// Extracts a keyword field from a Value map.
fn extract_keyword_field(value: &Value, field_name: &str) -> Result<KeywordId> {
    let map = value
        .as_map()
        .ok_or_else(|| Error::new(ErrorKind::Internal(format!("expected map, got {value:?}"))))?;

    // Look for the field by iterating and checking keyword names
    for (k, v) in map.iter() {
        if let Value::Keyword(kw) = k {
            // We need to check if this keyword matches the field name
            // Since we don't have the interner here, we check by the raw keyword
            // This is a limitation - the field name must match exactly
            if format!("{kw:?}").contains(field_name) {
                if let Value::Keyword(kw_val) = v {
                    return Ok(*kw_val);
                }
            }
        }
    }

    Err(Error::new(ErrorKind::Internal(format!(
        "missing required field: {field_name}"
    ))))
}

/// Extracts a keyword field from a Value map, returning None if not found.
fn extract_optional_keyword_field(value: &Value, field_name: &str) -> Option<KeywordId> {
    extract_keyword_field(value, field_name).ok()
}

/// Extracts a vector of keywords from a field.
fn extract_keyword_vec(value: &Value, field_name: &str) -> Vec<KeywordId> {
    let Some(map) = value.as_map() else {
        return Vec::new();
    };

    for (k, v) in map.iter() {
        if let Value::Keyword(kw) = k {
            if format!("{kw:?}").contains(field_name) {
                if let Value::Vec(vec) = v {
                    return vec
                        .iter()
                        .filter_map(|v| {
                            if let Value::Keyword(k) = v {
                                Some(*k)
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            }
        }
    }

    Vec::new()
}

/// Extracts an optional string field.
fn extract_string_field(value: &Value, field_name: &str) -> Option<String> {
    let map = value.as_map()?;

    for (k, v) in map.iter() {
        if let Value::Keyword(kw) = k {
            if format!("{kw:?}").contains(field_name) {
                if let Value::String(s) = v {
                    return Some(s.to_string());
                }
            }
        }
    }

    None
}

/// Extracts an optional integer field.
fn extract_int_field(value: &Value, field_name: &str) -> Option<i64> {
    let map = value.as_map()?;

    for (k, v) in map.iter() {
        if let Value::Keyword(kw) = k {
            if format!("{kw:?}").contains(field_name) {
                if let Value::Int(i) = v {
                    return Some(*i);
                }
            }
        }
    }

    None
}

/// Parses a component schema from a Value map.
fn parse_component_schema(value: &Value, interner: &mut Interner) -> Result<ComponentSchema> {
    let name = extract_keyword_field(value, "name")?;

    // Check if it's a tag component
    let storage = extract_string_field(value, "storage");
    if storage.as_deref() == Some("tag") {
        return Ok(ComponentSchema::tag(name));
    }

    let mut schema = ComponentSchema::new(name);

    // Parse fields
    if let Some(map) = value.as_map() {
        for (k, v) in map.iter() {
            if let Value::Keyword(kw) = k {
                if format!("{kw:?}").contains("fields") {
                    if let Value::Vec(fields) = v {
                        for field_val in fields.iter() {
                            let field = parse_field_schema(field_val, interner)?;
                            schema = schema.with_field(field);
                        }
                    }
                }
            }
        }
    }

    Ok(schema)
}

/// Parses a field schema from a Value map.
fn parse_field_schema(value: &Value, _interner: &mut Interner) -> Result<FieldSchema> {
    let name = extract_keyword_field(value, "name")?;
    let type_str = extract_string_field(value, "type").unwrap_or_else(|| "any".to_string());

    let ty = match type_str.as_str() {
        "int" => Type::Int,
        "float" => Type::Float,
        "bool" => Type::Bool,
        "string" => Type::String,
        "keyword" => Type::Keyword,
        "entity" => Type::EntityRef,
        "vec" | "vector" => Type::vec(Type::Any),
        "map" => Type::map(Type::Any, Type::Any),
        "set" => Type::set(Type::Any),
        _ => Type::Any,
    };

    // Check if required (default true)
    let required = extract_string_field(value, "required").is_none_or(|s| s != "false");

    if required {
        Ok(FieldSchema::required(name, ty))
    } else {
        Ok(FieldSchema::optional_nil(name, ty))
    }
}

/// Parses a relationship schema from a Value map.
fn parse_relationship_schema(
    value: &Value,
    _interner: &mut Interner,
) -> Result<RelationshipSchema> {
    let name = extract_keyword_field(value, "name")?;
    let mut schema = RelationshipSchema::new(name);

    // Parse cardinality
    if let Some(card_str) = extract_string_field(value, "cardinality") {
        let cardinality = match card_str.as_str() {
            "one-to-one" | "1:1" => Cardinality::OneToOne,
            "one-to-many" | "1:n" | "1:N" => Cardinality::OneToMany,
            "many-to-one" | "n:1" | "N:1" => Cardinality::ManyToOne,
            _ => Cardinality::ManyToMany,
        };
        schema = schema.with_cardinality(cardinality);
    }

    // Parse on-delete
    if let Some(on_delete_str) = extract_string_field(value, "on-delete") {
        let on_delete = match on_delete_str.as_str() {
            "cascade" => OnDelete::Cascade,
            "nullify" => OnDelete::Nullify,
            _ => OnDelete::Remove,
        };
        schema = schema.with_on_delete(on_delete);
    }

    Ok(schema)
}

/// Parses a verb from a Value map.
fn parse_verb(value: &Value, _interner: &mut Interner) -> Result<Verb> {
    let name = extract_keyword_field(value, "name")?;
    let synonyms: HashSet<KeywordId> = extract_keyword_vec(value, "synonyms").into_iter().collect();

    Ok(Verb { name, synonyms })
}

/// Parses a direction from a Value map.
fn parse_direction(value: &Value, _interner: &mut Interner) -> Result<Direction> {
    let name = extract_keyword_field(value, "name")?;
    let synonyms: HashSet<KeywordId> = extract_keyword_vec(value, "synonyms").into_iter().collect();
    let opposite = extract_optional_keyword_field(value, "opposite");

    Ok(Direction {
        name,
        synonyms,
        opposite,
    })
}

/// Parses a preposition from a Value map.
fn parse_preposition(value: &Value) -> Result<Preposition> {
    let name = extract_keyword_field(value, "name")?;
    let implies = extract_optional_keyword_field(value, "implies");

    Ok(Preposition { name, implies })
}

/// Parses a pronoun from a Value map.
fn parse_pronoun(value: &Value, _interner: &mut Interner) -> Result<Pronoun> {
    let name = extract_keyword_field(value, "name")?;

    // Parse gender
    let gender_str = extract_string_field(value, "gender").unwrap_or_else(|| "neuter".to_string());
    let gender = match gender_str.as_str() {
        "masculine" | "male" => PronounGender::Masculine,
        "feminine" | "female" => PronounGender::Feminine,
        _ => PronounGender::Neuter,
    };

    // Parse number
    let number_str =
        extract_string_field(value, "number").unwrap_or_else(|| "singular".to_string());
    let number = match number_str.as_str() {
        "plural" => PronounNumber::Plural,
        _ => PronounNumber::Singular,
    };

    Ok(Pronoun {
        name,
        gender,
        number,
    })
}

/// Parses a noun type from a Value map.
fn parse_noun_type(value: &Value) -> Result<NounType> {
    let name = extract_keyword_field(value, "name")?;
    let extends = extract_keyword_vec(value, "extends");
    let pattern_source = extract_string_field(value, "pattern").unwrap_or_default();

    Ok(NounType {
        name,
        extends,
        pattern_source,
    })
}

/// Parses a command syntax from a Value map.
fn parse_command_syntax(value: &Value) -> Result<CommandSyntax> {
    let name = extract_keyword_field(value, "name")?;
    let action = extract_keyword_field(value, "action")?;
    #[allow(clippy::cast_possible_truncation)]
    let priority = extract_int_field(value, "priority").unwrap_or(0) as i32;
    let syntax_source = extract_string_field(value, "syntax").unwrap_or_default();

    Ok(CommandSyntax {
        name,
        action,
        priority,
        syntax_source,
    })
}
