//! Noun phrase representation and resolution.
//!
//! Handles parsing and resolving noun phrases like "the brass lamp" or "all swords".

use longtable_foundation::{EntityId, KeywordId, Value};
use longtable_storage::World;

use crate::vocabulary::VocabularyRegistry;

/// A parsed noun phrase.
#[derive(Clone, Debug, PartialEq)]
pub struct NounPhrase {
    /// Adjectives modifying the noun (e.g., "brass" in "brass lamp")
    pub adjectives: Vec<String>,
    /// The main noun word (e.g., "lamp")
    pub noun: String,
    /// Quantifier (the, a, all, etc.)
    pub quantifier: Quantifier,
    /// Ordinal selector (first, second, etc.)
    pub ordinal: Option<usize>,
}

impl NounPhrase {
    /// Creates a new noun phrase with just a noun.
    #[must_use]
    pub fn new(noun: impl Into<String>) -> Self {
        Self {
            adjectives: Vec::new(),
            noun: noun.into(),
            quantifier: Quantifier::Specific,
            ordinal: None,
        }
    }

    /// Adds an adjective to the noun phrase.
    #[must_use]
    pub fn with_adjective(mut self, adj: impl Into<String>) -> Self {
        self.adjectives.push(adj.into());
        self
    }

    /// Sets the quantifier.
    #[must_use]
    pub fn with_quantifier(mut self, quantifier: Quantifier) -> Self {
        self.quantifier = quantifier;
        self
    }
}

/// Quantifier for noun phrases.
#[derive(Clone, Debug, PartialEq)]
pub enum Quantifier {
    /// Specific item (the sword)
    Specific,
    /// Any matching item (a sword)
    Any,
    /// All matching items
    All,
    /// All except specified entities
    AllExcept(Vec<EntityId>),
}

/// Result of noun resolution.
#[derive(Clone, Debug)]
pub enum NounResolution {
    /// Uniquely resolved to one entity
    Unique(EntityId),
    /// Multiple matches - disambiguation needed
    Ambiguous(Vec<EntityId>),
    /// No matching entity found
    NotFound,
    /// Found entity but wrong type
    WrongType {
        /// The found entity
        found: EntityId,
        /// The expected type constraint name
        expected: String,
    },
    /// Resolved to multiple entities (for "all")
    Multiple(Vec<EntityId>),
}

/// Resolves noun phrases to entities in scope.
#[derive(Clone, Debug)]
pub struct NounResolver {
    /// Keyword for entity name component
    name_keyword: KeywordId,
    /// Keyword for aliases component
    aliases_keyword: KeywordId,
    /// Keyword for adjectives component
    adjectives_keyword: KeywordId,
}

impl NounResolver {
    /// Creates a new noun resolver with the given keywords.
    #[must_use]
    pub fn new(
        name_keyword: KeywordId,
        aliases_keyword: KeywordId,
        adjectives_keyword: KeywordId,
    ) -> Self {
        Self {
            name_keyword,
            aliases_keyword,
            adjectives_keyword,
        }
    }

    /// Resolves a noun phrase to entities in scope.
    ///
    /// Resolution order:
    /// 1. Exact name match
    /// 2. Alias match
    /// 3. Adjective + noun match
    /// 4. Partial match on name/alias
    pub fn resolve(
        &self,
        phrase: &NounPhrase,
        type_constraint: Option<KeywordId>,
        scope: &[EntityId],
        world: &World,
        vocab: &VocabularyRegistry,
    ) -> NounResolution {
        // Handle "all" quantifier
        if matches!(
            phrase.quantifier,
            Quantifier::All | Quantifier::AllExcept(_)
        ) {
            return self.resolve_all(phrase, type_constraint, scope, world, vocab);
        }

        // Find all matching entities
        let matches = self.find_matches(phrase, type_constraint, scope, world, vocab);

        match matches.len() {
            0 => NounResolution::NotFound,
            1 => NounResolution::Unique(matches[0]),
            _ => {
                // Handle ordinal (first, second, etc.)
                if let Some(ordinal) = phrase.ordinal {
                    if ordinal > 0 && ordinal <= matches.len() {
                        return NounResolution::Unique(matches[ordinal - 1]);
                    }
                }
                // Handle "any" quantifier
                if phrase.quantifier == Quantifier::Any {
                    return NounResolution::Unique(matches[0]);
                }
                NounResolution::Ambiguous(matches)
            }
        }
    }

    /// Resolves "all" or "all except" quantifiers.
    fn resolve_all(
        &self,
        phrase: &NounPhrase,
        type_constraint: Option<KeywordId>,
        scope: &[EntityId],
        world: &World,
        vocab: &VocabularyRegistry,
    ) -> NounResolution {
        let mut matches = self.find_matches(phrase, type_constraint, scope, world, vocab);

        // Handle "all except"
        if let Quantifier::AllExcept(exceptions) = &phrase.quantifier {
            matches.retain(|e| !exceptions.contains(e));
        }

        if matches.is_empty() {
            NounResolution::NotFound
        } else {
            NounResolution::Multiple(matches)
        }
    }

    /// Finds all entities matching the noun phrase.
    fn find_matches(
        &self,
        phrase: &NounPhrase,
        type_constraint: Option<KeywordId>,
        scope: &[EntityId],
        world: &World,
        vocab: &VocabularyRegistry,
    ) -> Vec<EntityId> {
        let mut matches = Vec::new();

        for &entity in scope {
            if self.entity_matches(entity, phrase, world) {
                // Check type constraint if any
                if let Some(type_kw) = type_constraint {
                    if !self.check_type(entity, type_kw, world, vocab) {
                        continue;
                    }
                }
                matches.push(entity);
            }
        }

        matches
    }

    /// Checks if an entity matches a noun phrase.
    fn entity_matches(&self, entity: EntityId, phrase: &NounPhrase, world: &World) -> bool {
        let noun_lower = phrase.noun.to_lowercase();

        // 1. Check exact name match
        // Use get_field since components store fields in a Map structure
        if let Ok(Some(Value::String(name))) =
            world.get_field(entity, self.name_keyword, self.name_keyword)
        {
            if name.to_lowercase() == noun_lower {
                return self.adjectives_match(entity, phrase, world);
            }
        }

        // 2. Check aliases
        if let Ok(Some(Value::Vec(aliases))) =
            world.get_field(entity, self.aliases_keyword, self.aliases_keyword)
        {
            for alias in aliases.iter() {
                if let Value::String(alias_str) = alias {
                    if alias_str.to_lowercase() == noun_lower {
                        return self.adjectives_match(entity, phrase, world);
                    }
                }
            }
        }

        // 3. Check partial name match (word appears in name)
        if let Ok(Some(Value::String(name))) =
            world.get_field(entity, self.name_keyword, self.name_keyword)
        {
            let name_lower = name.to_lowercase();
            if name_lower.contains(&noun_lower) || noun_lower.contains(&name_lower) {
                return self.adjectives_match(entity, phrase, world);
            }
        }

        false
    }

    /// Checks if the entity's adjectives match the noun phrase's adjectives.
    fn adjectives_match(&self, entity: EntityId, phrase: &NounPhrase, world: &World) -> bool {
        // If no adjectives required, match succeeds
        if phrase.adjectives.is_empty() {
            return true;
        }

        // Get entity's adjectives using get_field
        let entity_adjectives = if let Ok(Some(Value::Vec(adjs))) =
            world.get_field(entity, self.adjectives_keyword, self.adjectives_keyword)
        {
            adjs.iter()
                .filter_map(|v| {
                    if let Value::String(s) = v {
                        Some(s.to_lowercase())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        // All requested adjectives must be present
        phrase.adjectives.iter().all(|adj| {
            let adj_lower = adj.to_lowercase();
            entity_adjectives.contains(&adj_lower)
        })
    }

    /// Checks if an entity matches a type constraint.
    fn check_type(
        &self,
        entity: EntityId,
        type_kw: KeywordId,
        world: &World,
        vocab: &VocabularyRegistry,
    ) -> bool {
        // Look up the type definition in vocabulary
        if let Some(_noun_type) = vocab.lookup_type(type_kw) {
            // For now, check if entity has components matching the type pattern
            // Full pattern matching will integrate with the engine later
            // Simple check: see if entity exists (placeholder)
            world.exists(entity)
        } else {
            // No type definition found, default to true
            true
        }
    }

    /// Gets a description of an entity for disambiguation.
    pub fn describe(&self, entity: EntityId, world: &World) -> String {
        // Get adjectives and name using get_field
        let adjectives = if let Ok(Some(Value::Vec(adjs))) =
            world.get_field(entity, self.adjectives_keyword, self.adjectives_keyword)
        {
            adjs.iter()
                .filter_map(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            String::new()
        };

        let name = if let Ok(Some(Value::String(name))) =
            world.get_field(entity, self.name_keyword, self.name_keyword)
        {
            name.to_string()
        } else {
            "thing".to_string()
        };

        if adjectives.is_empty() {
            name
        } else {
            format!("{adjectives} {name}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noun_phrase_builder() {
        let np = NounPhrase::new("lamp")
            .with_adjective("brass")
            .with_quantifier(Quantifier::Specific);

        assert_eq!(np.noun, "lamp");
        assert_eq!(np.adjectives, vec!["brass"]);
        assert!(matches!(np.quantifier, Quantifier::Specific));
    }

    #[test]
    fn test_quantifier_variants() {
        assert!(matches!(Quantifier::Specific, Quantifier::Specific));
        assert!(matches!(Quantifier::Any, Quantifier::Any));
        assert!(matches!(Quantifier::All, Quantifier::All));
    }
}
