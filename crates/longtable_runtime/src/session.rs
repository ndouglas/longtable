//! Session state for the REPL.
//!
//! The session holds the current world state and session-local variables.

use longtable_foundation::Value;
use longtable_storage::World;
use std::collections::HashMap;
use std::path::PathBuf;

/// Session state for an interactive REPL session.
#[derive(Debug)]
pub struct Session {
    /// The current world state.
    world: World,

    /// Session-local variable bindings (from `def`).
    variables: HashMap<String, Value>,

    /// Current load path for relative file resolution.
    load_path: PathBuf,

    /// Whether to auto-commit world mutations.
    auto_commit: bool,
}

impl Session {
    /// Creates a new session with an empty world.
    #[must_use]
    pub fn new() -> Self {
        Self {
            world: World::new(0),
            variables: HashMap::new(),
            load_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            auto_commit: true,
        }
    }

    /// Creates a new session with the given world.
    #[must_use]
    pub fn with_world(world: World) -> Self {
        Self {
            world,
            variables: HashMap::new(),
            load_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            auto_commit: true,
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
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
