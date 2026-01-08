//! Pronoun tracking state.
//!
//! Tracks referents for pronouns like "it", "him", "her", "them".

use longtable_foundation::{EntityId, KeywordId};

/// State for pronoun resolution.
#[derive(Clone, Debug, Default)]
pub struct PronounState {
    /// "it" referent (neuter singular)
    it: Option<EntityId>,
    /// "him" referent (masculine singular)
    him: Option<EntityId>,
    /// "her" referent (feminine singular)
    her: Option<EntityId>,
    /// "them" referent (plural)
    them: Vec<EntityId>,
    /// Known pronoun keywords for resolution
    pronoun_keywords: Option<PronounKeywords>,
}

/// Known pronoun keywords for mapping to slots.
#[derive(Clone, Copy, Debug)]
pub struct PronounKeywords {
    /// KeywordId for "it"
    pub it: KeywordId,
    /// KeywordId for "him"
    pub him: KeywordId,
    /// KeywordId for "her"
    pub her: KeywordId,
    /// KeywordId for "them"
    pub them: KeywordId,
}

impl PronounState {
    /// Creates a new pronoun state with no referents.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new pronoun state with known pronoun keywords.
    #[must_use]
    pub fn with_keywords(keywords: PronounKeywords) -> Self {
        Self {
            pronoun_keywords: Some(keywords),
            ..Self::default()
        }
    }

    /// Sets the known pronoun keywords for resolution.
    pub fn set_keywords(&mut self, keywords: PronounKeywords) {
        self.pronoun_keywords = Some(keywords);
    }

    /// Sets the "it" referent.
    pub fn set_it(&mut self, entity: EntityId) {
        self.it = Some(entity);
    }

    /// Sets the "him" referent.
    pub fn set_him(&mut self, entity: EntityId) {
        self.him = Some(entity);
    }

    /// Sets the "her" referent.
    pub fn set_her(&mut self, entity: EntityId) {
        self.her = Some(entity);
    }

    /// Sets the "them" referent.
    pub fn set_them(&mut self, entities: Vec<EntityId>) {
        self.them = entities;
    }

    /// Resolves a pronoun keyword to its referent(s).
    ///
    /// Returns `None` if the pronoun is not recognized or has no referent set.
    #[must_use]
    pub fn resolve(&self, pronoun: KeywordId) -> Option<Vec<EntityId>> {
        let keywords = self.pronoun_keywords.as_ref()?;

        if pronoun == keywords.it {
            self.it.map(|e| vec![e])
        } else if pronoun == keywords.him {
            self.him.map(|e| vec![e])
        } else if pronoun == keywords.her {
            self.her.map(|e| vec![e])
        } else if pronoun == keywords.them {
            if self.them.is_empty() {
                None
            } else {
                Some(self.them.clone())
            }
        } else {
            None
        }
    }

    /// Gets the "it" referent.
    #[must_use]
    pub fn get_it(&self) -> Option<EntityId> {
        self.it
    }

    /// Gets the "him" referent.
    #[must_use]
    pub fn get_him(&self) -> Option<EntityId> {
        self.him
    }

    /// Gets the "her" referent.
    #[must_use]
    pub fn get_her(&self) -> Option<EntityId> {
        self.her
    }

    /// Gets the "them" referent.
    #[must_use]
    pub fn get_them(&self) -> &[EntityId] {
        &self.them
    }

    /// Clears all pronoun referents.
    pub fn clear(&mut self) {
        self.it = None;
        self.him = None;
        self.her = None;
        self.them.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_is_empty() {
        let state = PronounState::new();
        assert!(state.get_it().is_none());
        assert!(state.get_him().is_none());
        assert!(state.get_her().is_none());
        assert!(state.get_them().is_empty());
    }

    #[test]
    fn test_set_and_get_it() {
        let mut state = PronounState::new();
        let entity = EntityId::new(1, 0);
        state.set_it(entity);
        assert_eq!(state.get_it(), Some(entity));
    }

    #[test]
    fn test_resolve_without_keywords_returns_none() {
        let mut state = PronounState::new();
        let entity = EntityId::new(1, 0);
        state.set_it(entity);

        // Without keywords set, resolve should return None
        let fake_keyword = KeywordId::REL_TYPE; // Any keyword
        assert!(state.resolve(fake_keyword).is_none());
    }

    #[test]
    fn test_resolve_with_keywords() {
        use longtable_foundation::Interner;

        let mut interner = Interner::new();
        let it_kw = interner.intern_keyword("it");
        let him_kw = interner.intern_keyword("him");
        let her_kw = interner.intern_keyword("her");
        let them_kw = interner.intern_keyword("them");

        let keywords = PronounKeywords {
            it: it_kw,
            him: him_kw,
            her: her_kw,
            them: them_kw,
        };

        let mut state = PronounState::with_keywords(keywords);
        let entity1 = EntityId::new(1, 0);
        let entity2 = EntityId::new(2, 0);

        // Set referents
        state.set_it(entity1);
        state.set_them(vec![entity1, entity2]);

        // Resolve should return correct referents
        assert_eq!(state.resolve(it_kw), Some(vec![entity1]));
        assert_eq!(state.resolve(them_kw), Some(vec![entity1, entity2]));

        // Unset pronouns should return None
        assert!(state.resolve(him_kw).is_none());
        assert!(state.resolve(her_kw).is_none());
    }
}
