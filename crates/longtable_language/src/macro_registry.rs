//! Registry for macro definitions.
//!
//! Stores and retrieves macro definitions by name.

use crate::macro_def::MacroDef;
use std::collections::HashMap;

// =============================================================================
// MacroRegistry
// =============================================================================

/// Registry for storing and looking up macro definitions.
///
/// Macros are stored by their qualified name (`namespace/name`).
#[derive(Clone, Debug, Default)]
pub struct MacroRegistry {
    /// Map from qualified name to macro definition.
    macros: HashMap<String, MacroDef>,
    /// Current namespace for unqualified lookups.
    current_namespace: String,
}

impl MacroRegistry {
    /// Creates a new empty macro registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
            current_namespace: "user".to_string(),
        }
    }

    /// Creates a new macro registry with stdlib macros pre-registered.
    #[must_use]
    pub fn new_with_stdlib() -> Self {
        let mut registry = Self::new();
        crate::stdlib_macros::register_stdlib_macros(&mut registry);
        registry
    }

    /// Creates a new macro registry with the given current namespace.
    #[must_use]
    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        Self {
            macros: HashMap::new(),
            current_namespace: namespace.into(),
        }
    }

    /// Sets the current namespace.
    pub fn set_namespace(&mut self, namespace: impl Into<String>) {
        self.current_namespace = namespace.into();
    }

    /// Returns the current namespace.
    #[must_use]
    pub fn current_namespace(&self) -> &str {
        &self.current_namespace
    }

    /// Registers a macro definition.
    ///
    /// The macro is stored by its qualified name.
    pub fn register(&mut self, def: MacroDef) {
        let name = def.qualified_name();
        self.macros.insert(name, def);
    }

    /// Looks up a macro by its qualified name.
    #[must_use]
    pub fn get(&self, qualified_name: &str) -> Option<&MacroDef> {
        self.macros.get(qualified_name)
    }

    /// Looks up a macro by unqualified name in the current namespace.
    #[must_use]
    pub fn get_local(&self, name: &str) -> Option<&MacroDef> {
        let qualified = format!("{}/{}", self.current_namespace, name);
        self.macros.get(&qualified)
    }

    /// Resolves a macro name (qualified or unqualified).
    ///
    /// If the name contains `/`, it's treated as qualified.
    /// Otherwise, it's looked up in the current namespace first,
    /// then in the "core" namespace for stdlib macros.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&MacroDef> {
        if name.contains('/') {
            self.get(name)
        } else {
            // Try current namespace first
            self.get_local(name).or_else(|| {
                // Fall back to core namespace for stdlib macros
                let core_qualified = format!("core/{name}");
                self.macros.get(&core_qualified)
            })
        }
    }

    /// Checks if a macro is registered with the given qualified name.
    #[must_use]
    pub fn contains(&self, qualified_name: &str) -> bool {
        self.macros.contains_key(qualified_name)
    }

    /// Checks if a name refers to a registered macro.
    ///
    /// Works with both qualified and unqualified names.
    #[must_use]
    pub fn is_macro(&self, name: &str) -> bool {
        self.resolve(name).is_some()
    }

    /// Returns all registered macro names.
    #[must_use]
    pub fn macro_names(&self) -> Vec<&str> {
        self.macros.keys().map(String::as_str).collect()
    }

    /// Returns the number of registered macros.
    #[must_use]
    pub fn len(&self) -> usize {
        self.macros.len()
    }

    /// Returns true if no macros are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.macros.is_empty()
    }

    /// Clears all registered macros.
    pub fn clear(&mut self) {
        self.macros.clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macro_def::MacroParam;
    use crate::span::Span;

    fn test_macro(name: &str, namespace: &str) -> MacroDef {
        MacroDef::new(
            name.to_string(),
            namespace.to_string(),
            vec![MacroParam::Normal("x".to_string())],
            vec![],
            Span::default(),
        )
    }

    #[test]
    fn register_and_lookup() {
        let mut registry = MacroRegistry::new();
        let def = test_macro("when", "core");

        registry.register(def);

        assert!(registry.contains("core/when"));
        assert!(!registry.contains("core/unless"));

        let found = registry.get("core/when").unwrap();
        assert_eq!(found.name, "when");
    }

    #[test]
    fn lookup_local() {
        let mut registry = MacroRegistry::with_namespace("core");
        registry.register(test_macro("when", "core"));
        registry.register(test_macro("cond", "core"));

        let found = registry.get_local("when").unwrap();
        assert_eq!(found.qualified_name(), "core/when");

        assert!(registry.get_local("unknown").is_none());
    }

    #[test]
    fn resolve_qualified_and_unqualified() {
        let mut registry = MacroRegistry::with_namespace("core");
        registry.register(test_macro("when", "core"));
        registry.register(test_macro("special", "other"));

        // Unqualified lookup in current namespace
        assert!(registry.resolve("when").is_some());

        // Qualified lookup in different namespace
        assert!(registry.resolve("other/special").is_some());

        // Unqualified doesn't find other namespaces
        assert!(registry.resolve("special").is_none());
    }

    #[test]
    fn is_macro() {
        let mut registry = MacroRegistry::with_namespace("user");
        registry.register(test_macro("test-macro", "user"));

        assert!(registry.is_macro("test-macro"));
        assert!(registry.is_macro("user/test-macro"));
        assert!(!registry.is_macro("other/test-macro"));
        assert!(!registry.is_macro("unknown"));
    }

    #[test]
    fn macro_names() {
        let mut registry = MacroRegistry::new();
        registry.register(test_macro("a", "ns1"));
        registry.register(test_macro("b", "ns2"));

        let names = registry.macro_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"ns1/a"));
        assert!(names.contains(&"ns2/b"));
    }

    #[test]
    fn len_and_is_empty() {
        let mut registry = MacroRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(test_macro("test", "user"));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn clear() {
        let mut registry = MacroRegistry::new();
        registry.register(test_macro("test", "user"));
        assert!(!registry.is_empty());

        registry.clear();
        assert!(registry.is_empty());
    }
}
