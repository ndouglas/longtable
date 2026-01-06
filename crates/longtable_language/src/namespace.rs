//! Namespace and module types for the Longtable DSL.
//!
//! This module provides types for namespace declarations, require specifications,
//! and namespace context for symbol resolution.
//!
//! # Example
//!
//! ```clojure
//! (namespace game.combat
//!   (:require [game.core :as core]
//!             [game.utils :refer [distance clamp]]
//!             [game.items]))
//! ```

use crate::span::Span;
use std::collections::HashMap;
use std::fmt;

// =============================================================================
// NamespaceName
// =============================================================================

/// A qualified namespace name like "game.combat".
///
/// Stored as path segments for easy manipulation and comparison.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NamespaceName {
    /// Path segments (e.g., `["game", "combat"]` for "game.combat").
    pub segments: Vec<String>,
}

impl NamespaceName {
    /// Creates a new namespace name from segments.
    #[must_use]
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    /// Creates a namespace name from a dotted string like "game.combat".
    #[must_use]
    pub fn parse(s: &str) -> Self {
        Self {
            segments: s.split('.').map(String::from).collect(),
        }
    }

    /// Returns the full qualified name as a dotted string.
    #[must_use]
    pub fn full_name(&self) -> String {
        self.segments.join(".")
    }

    /// Returns the simple name (last segment).
    #[must_use]
    pub fn simple_name(&self) -> &str {
        self.segments.last().map_or("", String::as_str)
    }

    /// Returns true if this namespace is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for NamespaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl Default for NamespaceName {
    fn default() -> Self {
        Self::parse("user")
    }
}

// =============================================================================
// RequireSpec
// =============================================================================

/// A require specification from a `(:require ...)` clause.
///
/// Supports three forms:
/// - `[game.core :as core]` - alias the entire namespace
/// - `[game.utils :refer [distance clamp]]` - import specific symbols
/// - `[game.items]` - just ensure the namespace is loaded
#[derive(Clone, Debug, PartialEq)]
pub enum RequireSpec {
    /// `[game.core :as core]` - alias the entire namespace.
    Alias {
        /// The namespace to require.
        namespace: NamespaceName,
        /// The alias to use for this namespace.
        alias: String,
    },
    /// `[game.utils :refer [distance clamp]]` - import specific symbols.
    Refer {
        /// The namespace to require.
        namespace: NamespaceName,
        /// The symbols to import.
        symbols: Vec<String>,
    },
    /// `[game.items]` - just ensure the namespace is loaded.
    Use {
        /// The namespace to require.
        namespace: NamespaceName,
    },
}

impl RequireSpec {
    /// Returns the namespace being required.
    #[must_use]
    pub fn namespace(&self) -> &NamespaceName {
        match self {
            Self::Alias { namespace, .. }
            | Self::Refer { namespace, .. }
            | Self::Use { namespace } => namespace,
        }
    }
}

// =============================================================================
// NamespaceDecl
// =============================================================================

/// A namespace declaration.
///
/// Corresponds to:
/// ```clojure
/// (namespace game.combat
///   (:require [game.core :as core]
///             [game.utils :refer [distance clamp]]))
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct NamespaceDecl {
    /// The namespace name (e.g., "game.combat").
    pub name: NamespaceName,
    /// Required namespaces.
    pub requires: Vec<RequireSpec>,
    /// Source span.
    pub span: Span,
}

impl NamespaceDecl {
    /// Creates a new namespace declaration with the given name.
    #[must_use]
    pub fn new(name: NamespaceName, span: Span) -> Self {
        Self {
            name,
            requires: Vec::new(),
            span,
        }
    }

    /// Creates a namespace declaration from a string name.
    #[must_use]
    pub fn with_name(name: &str, span: Span) -> Self {
        Self::new(NamespaceName::parse(name), span)
    }
}

// =============================================================================
// LoadDecl
// =============================================================================

/// A load directive.
///
/// Corresponds to `(load "path/to/file.lt")`.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadDecl {
    /// The path to load (relative or absolute).
    pub path: String,
    /// Source span.
    pub span: Span,
}

impl LoadDecl {
    /// Creates a new load declaration.
    #[must_use]
    pub fn new(path: String, span: Span) -> Self {
        Self { path, span }
    }
}

// =============================================================================
// NamespaceContext
// =============================================================================

/// Context for symbol resolution within a namespace.
///
/// Tracks the current namespace and all imported symbols/aliases
/// for resolving qualified and unqualified references.
#[derive(Clone, Debug, Default)]
pub struct NamespaceContext {
    /// Current namespace name (None for top-level/user namespace).
    pub current: Option<NamespaceName>,
    /// Aliases: `alias_name` -> `full_namespace_name`.
    pub aliases: HashMap<String, String>,
    /// Referred symbols: `local_name` -> `qualified_name` (namespace/symbol).
    pub refers: HashMap<String, String>,
}

impl NamespaceContext {
    /// Creates a new empty namespace context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a namespace context from a namespace declaration.
    #[must_use]
    pub fn from_decl(decl: &NamespaceDecl) -> Self {
        let mut ctx = Self {
            current: Some(decl.name.clone()),
            aliases: HashMap::new(),
            refers: HashMap::new(),
        };

        for req in &decl.requires {
            ctx.add_require(req);
        }

        ctx
    }

    /// Adds a require specification to this context.
    pub fn add_require(&mut self, req: &RequireSpec) {
        match req {
            RequireSpec::Alias { namespace, alias } => {
                self.aliases.insert(alias.clone(), namespace.full_name());
            }
            RequireSpec::Refer { namespace, symbols } => {
                let ns_name = namespace.full_name();
                for sym in symbols {
                    self.refers.insert(sym.clone(), format!("{ns_name}/{sym}"));
                }
            }
            RequireSpec::Use { .. } => {
                // Use just loads the namespace, doesn't add any bindings
            }
        }
    }

    /// Adds an alias to this context.
    pub fn add_alias(&mut self, alias: &str, namespace: &str) {
        self.aliases
            .insert(alias.to_string(), namespace.to_string());
    }

    /// Adds a referred symbol to this context.
    pub fn add_refer(&mut self, local_name: &str, qualified_name: &str) {
        self.refers
            .insert(local_name.to_string(), qualified_name.to_string());
    }

    /// Resolve an aliased qualified symbol (e.g., "core/foo" -> "game.core/foo").
    ///
    /// Returns `Some(qualified)` if the alias exists, `None` otherwise.
    #[must_use]
    pub fn resolve_alias(&self, alias: &str, symbol: &str) -> Option<String> {
        self.aliases.get(alias).map(|ns| format!("{ns}/{symbol}"))
    }

    /// Resolve a referred symbol (e.g., "distance" -> "game.utils/distance").
    ///
    /// Returns `Some(qualified)` if the symbol was referred, `None` otherwise.
    #[must_use]
    pub fn resolve_referred(&self, name: &str) -> Option<String> {
        self.refers.get(name).cloned()
    }

    /// Returns the current namespace name as a string, or "user" if none.
    #[must_use]
    pub fn current_namespace_str(&self) -> String {
        self.current
            .as_ref()
            .map_or_else(|| "user".to_string(), NamespaceName::full_name)
    }

    /// Qualify a symbol to the current namespace.
    ///
    /// Returns `namespace/symbol` for the current namespace.
    #[must_use]
    pub fn qualify(&self, symbol: &str) -> String {
        format!("{}/{}", self.current_namespace_str(), symbol)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_name_from_str() {
        let ns = NamespaceName::parse("game.combat.ai");
        assert_eq!(ns.segments, vec!["game", "combat", "ai"]);
        assert_eq!(ns.full_name(), "game.combat.ai");
        assert_eq!(ns.simple_name(), "ai");
    }

    #[test]
    fn namespace_name_single_segment() {
        let ns = NamespaceName::parse("core");
        assert_eq!(ns.segments, vec!["core"]);
        assert_eq!(ns.full_name(), "core");
        assert_eq!(ns.simple_name(), "core");
    }

    #[test]
    fn namespace_name_display() {
        let ns = NamespaceName::parse("game.core");
        assert_eq!(format!("{ns}"), "game.core");
    }

    #[test]
    fn namespace_context_alias_resolution() {
        let mut ctx = NamespaceContext::new();
        ctx.add_alias("core", "game.core");
        ctx.add_alias("utils", "game.utils");

        assert_eq!(
            ctx.resolve_alias("core", "foo"),
            Some("game.core/foo".to_string())
        );
        assert_eq!(
            ctx.resolve_alias("utils", "bar"),
            Some("game.utils/bar".to_string())
        );
        assert_eq!(ctx.resolve_alias("unknown", "foo"), None);
    }

    #[test]
    fn namespace_context_refer_resolution() {
        let mut ctx = NamespaceContext::new();
        ctx.add_refer("distance", "game.utils/distance");
        ctx.add_refer("clamp", "game.utils/clamp");

        assert_eq!(
            ctx.resolve_referred("distance"),
            Some("game.utils/distance".to_string())
        );
        assert_eq!(
            ctx.resolve_referred("clamp"),
            Some("game.utils/clamp".to_string())
        );
        assert_eq!(ctx.resolve_referred("unknown"), None);
    }

    #[test]
    fn namespace_context_from_decl() {
        let mut decl = NamespaceDecl::with_name("game.combat", Span::default());
        decl.requires.push(RequireSpec::Alias {
            namespace: NamespaceName::parse("game.core"),
            alias: "core".to_string(),
        });
        decl.requires.push(RequireSpec::Refer {
            namespace: NamespaceName::parse("game.utils"),
            symbols: vec!["distance".to_string(), "clamp".to_string()],
        });

        let ctx = NamespaceContext::from_decl(&decl);

        assert_eq!(ctx.current_namespace_str(), "game.combat");
        assert_eq!(
            ctx.resolve_alias("core", "foo"),
            Some("game.core/foo".to_string())
        );
        assert_eq!(
            ctx.resolve_referred("distance"),
            Some("game.utils/distance".to_string())
        );
    }

    #[test]
    fn namespace_context_qualify() {
        let decl = NamespaceDecl::with_name("game.combat", Span::default());
        let ctx = NamespaceContext::from_decl(&decl);

        assert_eq!(ctx.qualify("attack"), "game.combat/attack");
    }

    #[test]
    fn namespace_context_default_user() {
        let ctx = NamespaceContext::new();
        assert_eq!(ctx.current_namespace_str(), "user");
        assert_eq!(ctx.qualify("foo"), "user/foo");
    }

    #[test]
    fn require_spec_namespace() {
        let alias = RequireSpec::Alias {
            namespace: NamespaceName::parse("game.core"),
            alias: "core".to_string(),
        };
        assert_eq!(alias.namespace().full_name(), "game.core");

        let refer = RequireSpec::Refer {
            namespace: NamespaceName::parse("game.utils"),
            symbols: vec!["foo".to_string()],
        };
        assert_eq!(refer.namespace().full_name(), "game.utils");

        let use_spec = RequireSpec::Use {
            namespace: NamespaceName::parse("game.items"),
        };
        assert_eq!(use_spec.namespace().full_name(), "game.items");
    }
}
