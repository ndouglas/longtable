//! Module registry for tracking loaded namespaces and files.
//!
//! The `ModuleRegistry` tracks:
//! - Which files have been loaded
//! - Which namespaces are defined
//! - The loading stack for cycle detection

use crate::namespace::NamespaceDecl;
use longtable_foundation::{Error, ErrorKind, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// =============================================================================
// NamespaceInfo
// =============================================================================

/// Information about a loaded namespace.
#[derive(Clone, Debug)]
pub struct NamespaceInfo {
    /// The namespace declaration.
    pub decl: NamespaceDecl,
    /// Symbols exported by this namespace.
    pub exports: HashSet<String>,
    /// Source file path (canonical).
    pub source_path: PathBuf,
}

impl NamespaceInfo {
    /// Creates new namespace info.
    #[must_use]
    pub fn new(decl: NamespaceDecl, source_path: PathBuf) -> Self {
        Self {
            decl,
            exports: HashSet::new(),
            source_path,
        }
    }

    /// Adds an exported symbol.
    pub fn add_export(&mut self, symbol: String) {
        self.exports.insert(symbol);
    }

    /// Checks if a symbol is exported.
    #[must_use]
    pub fn exports_symbol(&self, symbol: &str) -> bool {
        self.exports.contains(symbol)
    }
}

// =============================================================================
// ModuleRegistry
// =============================================================================

/// Registry for tracking loaded modules and namespaces.
///
/// Provides:
/// - Namespace lookup by name
/// - File-to-namespace mapping
/// - Cycle detection during loading
#[derive(Debug, Default)]
pub struct ModuleRegistry {
    /// Map from namespace name to info.
    namespaces: HashMap<String, NamespaceInfo>,
    /// Map from canonical file path to namespace name.
    file_to_namespace: HashMap<PathBuf, String>,
    /// Set of files currently being loaded (for cycle detection).
    loading_stack: Vec<PathBuf>,
}

impl ModuleRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a file is currently being loaded (cycle detection).
    #[must_use]
    pub fn is_loading(&self, path: &Path) -> bool {
        self.loading_stack.iter().any(|p| p == path)
    }

    /// Push a file onto the loading stack.
    ///
    /// Returns an error if the file is already being loaded (cycle detected).
    pub fn begin_loading(&mut self, path: PathBuf) -> Result<()> {
        if self.is_loading(&path) {
            // Build the cycle description
            let cycle: Vec<_> = self
                .loading_stack
                .iter()
                .skip_while(|p| *p != &path)
                .map(|p| p.display().to_string())
                .collect();
            let cycle_str = cycle.join(" -> ");

            return Err(Error::new(ErrorKind::Internal(format!(
                "cyclic load detected: {} -> {}",
                cycle_str,
                path.display()
            ))));
        }

        self.loading_stack.push(path);
        Ok(())
    }

    /// Pop a file from the loading stack.
    pub fn finish_loading(&mut self, path: &Path) {
        if let Some(pos) = self.loading_stack.iter().position(|p| p == path) {
            self.loading_stack.remove(pos);
        }
    }

    /// Returns the current loading stack (for debugging).
    #[must_use]
    pub fn loading_stack(&self) -> &[PathBuf] {
        &self.loading_stack
    }

    /// Register a namespace.
    pub fn register_namespace(&mut self, info: NamespaceInfo) {
        let name = info.decl.name.full_name();
        let path = info.source_path.clone();

        self.file_to_namespace.insert(path, name.clone());
        self.namespaces.insert(name, info);
    }

    /// Lookup a namespace by name.
    #[must_use]
    pub fn get_namespace(&self, name: &str) -> Option<&NamespaceInfo> {
        self.namespaces.get(name)
    }

    /// Lookup a namespace by file path.
    #[must_use]
    pub fn get_namespace_for_file(&self, path: &Path) -> Option<&NamespaceInfo> {
        self.file_to_namespace
            .get(path)
            .and_then(|name| self.namespaces.get(name))
    }

    /// Check if a namespace is loaded.
    #[must_use]
    pub fn has_namespace(&self, name: &str) -> bool {
        self.namespaces.contains_key(name)
    }

    /// Check if a file has been loaded.
    #[must_use]
    pub fn has_file(&self, path: &Path) -> bool {
        self.file_to_namespace.contains_key(path)
    }

    /// Returns all loaded namespace names.
    #[must_use]
    pub fn namespace_names(&self) -> Vec<&str> {
        self.namespaces.keys().map(String::as_str).collect()
    }

    /// Resolve a qualified symbol to check if it exists.
    ///
    /// Returns `Some(qualified_name)` if the namespace exists and exports the symbol.
    /// For now, this is permissive and returns `Some` if the namespace exists.
    #[must_use]
    pub fn resolve_qualified(&self, namespace: &str, symbol: &str) -> Option<String> {
        if self.namespaces.contains_key(namespace) {
            // For now, we're permissive - if the namespace exists, allow the reference
            // In the future, we could check exports
            Some(format!("{namespace}/{symbol}"))
        } else {
            None
        }
    }

    /// Clear the registry (for testing).
    pub fn clear(&mut self) {
        self.namespaces.clear();
        self.file_to_namespace.clear();
        self.loading_stack.clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::namespace::NamespaceName;
    use crate::span::Span;

    fn test_decl(name: &str) -> NamespaceDecl {
        NamespaceDecl::new(NamespaceName::parse(name), Span::default())
    }

    fn test_info(name: &str, path: &str) -> NamespaceInfo {
        NamespaceInfo::new(test_decl(name), PathBuf::from(path))
    }

    #[test]
    fn register_and_lookup_namespace() {
        let mut registry = ModuleRegistry::new();
        let info = test_info("game.core", "/src/game/core.lt");

        registry.register_namespace(info);

        assert!(registry.has_namespace("game.core"));
        assert!(!registry.has_namespace("game.combat"));

        let found = registry.get_namespace("game.core").unwrap();
        assert_eq!(found.decl.name.full_name(), "game.core");
    }

    #[test]
    fn lookup_by_file() {
        let mut registry = ModuleRegistry::new();
        let info = test_info("game.core", "/src/game/core.lt");

        registry.register_namespace(info);

        assert!(registry.has_file(Path::new("/src/game/core.lt")));
        assert!(!registry.has_file(Path::new("/src/game/combat.lt")));

        let found = registry
            .get_namespace_for_file(Path::new("/src/game/core.lt"))
            .unwrap();
        assert_eq!(found.decl.name.full_name(), "game.core");
    }

    #[test]
    fn cycle_detection_allows_first_load() {
        let mut registry = ModuleRegistry::new();
        let path = PathBuf::from("/src/a.lt");

        assert!(registry.begin_loading(path.clone()).is_ok());
        assert!(registry.is_loading(&path));
    }

    #[test]
    fn cycle_detection_catches_cycle() {
        let mut registry = ModuleRegistry::new();

        registry.begin_loading(PathBuf::from("/src/a.lt")).unwrap();
        registry.begin_loading(PathBuf::from("/src/b.lt")).unwrap();

        // Trying to load a.lt again should fail
        let result = registry.begin_loading(PathBuf::from("/src/a.lt"));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("cyclic load detected"));
    }

    #[test]
    fn finish_loading_removes_from_stack() {
        let mut registry = ModuleRegistry::new();
        let path = PathBuf::from("/src/a.lt");

        registry.begin_loading(path.clone()).unwrap();
        assert!(registry.is_loading(&path));

        registry.finish_loading(&path);
        assert!(!registry.is_loading(&path));
    }

    #[test]
    fn nested_loading_works() {
        let mut registry = ModuleRegistry::new();

        registry.begin_loading(PathBuf::from("/src/a.lt")).unwrap();
        registry.begin_loading(PathBuf::from("/src/b.lt")).unwrap();
        registry.begin_loading(PathBuf::from("/src/c.lt")).unwrap();

        assert_eq!(registry.loading_stack().len(), 3);

        // Finish in reverse order
        registry.finish_loading(Path::new("/src/c.lt"));
        assert_eq!(registry.loading_stack().len(), 2);

        registry.finish_loading(Path::new("/src/b.lt"));
        assert_eq!(registry.loading_stack().len(), 1);

        registry.finish_loading(Path::new("/src/a.lt"));
        assert_eq!(registry.loading_stack().len(), 0);
    }

    #[test]
    fn resolve_qualified_existing_namespace() {
        let mut registry = ModuleRegistry::new();
        registry.register_namespace(test_info("game.core", "/src/game/core.lt"));

        assert_eq!(
            registry.resolve_qualified("game.core", "foo"),
            Some("game.core/foo".to_string())
        );
        assert_eq!(registry.resolve_qualified("game.unknown", "foo"), None);
    }

    #[test]
    fn namespace_names() {
        let mut registry = ModuleRegistry::new();
        registry.register_namespace(test_info("game.core", "/src/game/core.lt"));
        registry.register_namespace(test_info("game.combat", "/src/game/combat.lt"));

        let names = registry.namespace_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"game.core"));
        assert!(names.contains(&"game.combat"));
    }

    #[test]
    fn clear_registry() {
        let mut registry = ModuleRegistry::new();
        registry.register_namespace(test_info("game.core", "/src/game/core.lt"));
        registry
            .begin_loading(PathBuf::from("/src/test.lt"))
            .unwrap();

        registry.clear();

        assert!(!registry.has_namespace("game.core"));
        assert!(registry.loading_stack().is_empty());
    }

    #[test]
    fn namespace_info_exports() {
        let mut info = test_info("game.core", "/src/game/core.lt");

        info.add_export("foo".to_string());
        info.add_export("bar".to_string());

        assert!(info.exports_symbol("foo"));
        assert!(info.exports_symbol("bar"));
        assert!(!info.exports_symbol("baz"));
    }
}
