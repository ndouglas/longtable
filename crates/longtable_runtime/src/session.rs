//! Session state for the REPL.
//!
//! The session holds the current world state and session-local variables.

use longtable_debug::{DebugSession, Timeline, Tracer};
use longtable_foundation::{EntityId, Value};
use longtable_language::{ModuleRegistry, NamespaceContext};
use longtable_parser::VocabularyRegistry;
use longtable_storage::World;
use std::collections::HashMap;
use std::path::PathBuf;

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
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
