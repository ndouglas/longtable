//! Action registry and execution.
//!
//! Defines and executes game actions triggered by commands.

use std::collections::HashMap;

use longtable_foundation::KeywordId;

/// A compiled action definition.
#[derive(Clone, Debug)]
pub struct CompiledAction {
    /// Action name
    pub name: KeywordId,
    /// Parameter variable names
    pub params: Vec<String>,
    /// Salience for rule ordering
    pub salience: i32,
    // preconditions: Vec<CompiledPrecondition>, // Added later
    // handler: CompiledHandler, // Added later
}

/// Result of checking an action's preconditions.
#[derive(Clone, Debug)]
pub enum PreconditionResult {
    /// All preconditions passed
    Pass,
    /// A precondition failed with a message
    Fail {
        /// The failure message to display
        message: String,
    },
}

/// Registry of all defined actions.
#[derive(Clone, Debug, Default)]
pub struct ActionRegistry {
    actions: HashMap<KeywordId, CompiledAction>,
}

impl ActionRegistry {
    /// Creates a new empty action registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an action.
    pub fn register(&mut self, action: CompiledAction) {
        self.actions.insert(action.name, action);
    }

    /// Looks up an action by name.
    #[must_use]
    pub fn get(&self, name: KeywordId) -> Option<&CompiledAction> {
        self.actions.get(&name)
    }

    /// Generates rules for all registered actions.
    ///
    /// Actions generate high-salience rules that:
    /// 1. Match command entities by `:command/action`
    /// 2. Check preconditions
    /// 3. Execute handler
    /// 4. Mark command as processed
    pub fn generate_rules(&self) -> Vec<()> {
        // TODO: Return actual SpikeRule instances
        Vec::new()
    }
}
