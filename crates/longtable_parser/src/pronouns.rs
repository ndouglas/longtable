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
}

impl PronounState {
    /// Creates a new pronoun state with no referents.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    #[must_use]
    pub fn resolve(&self, _pronoun: KeywordId) -> Option<Vec<EntityId>> {
        // TODO: Map pronoun keyword to the appropriate slot
        // For now, just return None
        None
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
}
