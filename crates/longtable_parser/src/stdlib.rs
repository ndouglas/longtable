//! Standard library for adventure games.
//!
//! Contains default vocabulary definitions for common adventure game patterns.

use std::collections::HashSet;

use longtable_foundation::KeywordId;
use longtable_storage::World;

use crate::scope::{CompiledScope, ScopeEvaluator, ScopeKind};
use crate::syntax::{CompiledSyntax, CompiledSyntaxElement};
use crate::vocabulary::{
    Direction, NounType, Preposition, Pronoun, PronounGender, PronounNumber, Verb,
    VocabularyRegistry,
};

/// Standard directions DSL.
pub const DIRECTIONS_DSL: &str = r#"
;; Cardinal directions
(direction: north :synonyms [n] :opposite south)
(direction: south :synonyms [s] :opposite north)
(direction: east :synonyms [e] :opposite west)
(direction: west :synonyms [w] :opposite east)

;; Vertical directions
(direction: up :synonyms [u] :opposite down)
(direction: down :synonyms [d] :opposite up)

;; Diagonal directions
(direction: northeast :synonyms [ne] :opposite southwest)
(direction: northwest :synonyms [nw] :opposite southeast)
(direction: southeast :synonyms [se] :opposite northwest)
(direction: southwest :synonyms [sw] :opposite northeast)

;; Special directions
(direction: in :synonyms [enter inside] :opposite out)
(direction: out :synonyms [exit outside] :opposite in)
"#;

/// Standard verbs DSL.
pub const VERBS_DSL: &str = r#"
;; Movement
(verb: go :synonyms [walk move travel head proceed])
(verb: enter :synonyms [go-in go-into])
(verb: exit :synonyms [leave go-out])
(verb: climb :synonyms [scale ascend])

;; Looking
(verb: look :synonyms [l examine x inspect describe])
(verb: read :synonyms [peruse])

;; Manipulation
(verb: take :synonyms [get grab pick-up acquire obtain])
(verb: drop :synonyms [put-down release discard])
(verb: put :synonyms [place set insert])
(verb: give :synonyms [hand offer])
(verb: show :synonyms [display present])
(verb: throw :synonyms [toss hurl chuck])

;; Containers
(verb: open :synonyms [])
(verb: close :synonyms [shut])
(verb: lock :synonyms [])
(verb: unlock :synonyms [])

;; Combat
(verb: attack :synonyms [kill hit strike stab slash fight])
(verb: block :synonyms [parry deflect])

;; Social
(verb: say :synonyms [speak tell])
(verb: ask :synonyms [question query])
(verb: greet :synonyms [hello hi wave])

;; Interaction
(verb: use :synonyms [operate activate])
(verb: push :synonyms [press shove])
(verb: pull :synonyms [tug yank])
(verb: turn :synonyms [rotate twist])
(verb: touch :synonyms [feel poke])

;; Inventory
(verb: inventory :synonyms [i inv items])
(verb: wear :synonyms [don put-on])
(verb: remove :synonyms [take-off doff])

;; Meta
(verb: wait :synonyms [z])
(verb: save :synonyms [])
(verb: restore :synonyms [load])
(verb: quit :synonyms [q exit-game])
(verb: help :synonyms [?])
"#;

/// Standard prepositions DSL.
pub const PREPOSITIONS_DSL: &str = r#"
(preposition: in :implies location)
(preposition: on :implies surface)
(preposition: with :implies instrument)
(preposition: to :implies recipient)
(preposition: from :implies source)
(preposition: at :implies target)
(preposition: about :implies topic)
(preposition: through :implies passage)
(preposition: under :implies beneath)
(preposition: behind :implies obscured)
(preposition: into :implies destination)
(preposition: onto :implies surface)
"#;

/// Standard pronouns DSL.
pub const PRONOUNS_DSL: &str = r#"
(pronoun: it :gender :neuter :number :singular)
(pronoun: him :gender :masculine :number :singular)
(pronoun: her :gender :feminine :number :singular)
(pronoun: them :gender :neuter :number :plural)
"#;

/// Standard types DSL.
pub const TYPES_DSL: &str = r#"
;; Base type - anything that exists
(type: thing
  :where [[?obj :name _]])

;; Takeable things
(type: takeable
  :extends [thing]
  :where [[?obj :takeable true]])

;; Containers
(type: container
  :extends [thing]
  :where [[?obj :container/capacity _]])

;; Openable things (doors, containers)
(type: openable
  :extends [thing]
  :where [[?obj :openable true]])

;; Lockable things
(type: lockable
  :extends [openable]
  :where [[?obj :lockable true]])

;; Weapons
(type: weapon
  :extends [takeable]
  :where [[?obj :weapon/damage _]])

;; Living things
(type: living
  :extends [thing]
  :where [[?obj :health/current _]])

;; Persons (for him/her pronouns)
(type: person
  :extends [living]
  :where [[?obj :person/gender _]])

;; Rooms/locations
(type: room
  :where [[?obj :room/description _]])

;; Wearable things
(type: wearable
  :extends [takeable]
  :where [[?obj :wearable true]])

;; Readable things
(type: readable
  :extends [thing]
  :where [[?obj :text _]])
"#;

/// Standard scopes DSL.
pub const SCOPES_DSL: &str = r#"
;; Immediate scope - room contents and inventory
(scope: immediate
  :where [[?actor :location ?room]
          [?obj :location ?room]])

;; Visible scope - immediate + transparent containers
(scope: visible
  :extends [immediate]
  :where [[?obj :location/in ?container]
          [?container :transparent true]])

;; Reachable scope - visible + open containers
(scope: reachable
  :extends [visible]
  :where [[?obj :location/in ?container]
          [?container :container/open true]])
"#;

/// Standard commands DSL.
pub const COMMANDS_DSL: &str = r#"
;; Basic movement
(command: go
  :syntax [:verb [?dir direction]]
  :action go)

;; Looking
(command: look
  :syntax [:verb]
  :action look-around)

(command: look-at
  :syntax [:verb :at [?obj]]
  :action examine)

(command: examine
  :syntax [:verb [?obj]]
  :action examine)

;; Taking and dropping
(command: take
  :syntax [:verb [?obj takeable]]
  :action take)

(command: take-all
  :syntax [:verb all]
  :action take-all)

(command: drop
  :syntax [:verb [?obj]]
  :action drop)

;; Putting things
(command: put-in
  :syntax [:verb [?obj] :in [?dest container]]
  :action put-in)

(command: put-on
  :syntax [:verb [?obj] :on [?dest]]
  :action put-on)

;; Opening and closing
(command: open
  :syntax [:verb [?obj openable]]
  :action open)

(command: close
  :syntax [:verb [?obj openable]]
  :action close)

;; Combat
(command: attack
  :syntax [:verb [?target living]]
  :action attack)

(command: attack-with
  :syntax [:verb [?target living] :with [?weapon weapon]]
  :action attack-with
  :priority 1)

;; Inventory
(command: inventory
  :syntax [:verb]
  :action show-inventory)

;; Speech
(command: say
  :syntax [:verb [?text]]
  :action say)

(command: say-to
  :syntax [:verb [?text] :to [?target person]]
  :action say-to)

;; Waiting
(command: wait
  :syntax [:verb]
  :action wait)
"#;

/// Gets all standard library DSL as a combined string.
#[must_use]
pub fn all_stdlib_dsl() -> String {
    format!(
        "{DIRECTIONS_DSL}\n{VERBS_DSL}\n{PREPOSITIONS_DSL}\n{PRONOUNS_DSL}\n{TYPES_DSL}\n{SCOPES_DSL}\n{COMMANDS_DSL}"
    )
}

/// Standard library keywords for parser components.
#[derive(Clone, Debug)]
pub struct StdlibKeywords {
    // Directions
    pub north: KeywordId,
    pub south: KeywordId,
    pub east: KeywordId,
    pub west: KeywordId,
    pub up: KeywordId,
    pub down: KeywordId,
    pub northeast: KeywordId,
    pub northwest: KeywordId,
    pub southeast: KeywordId,
    pub southwest: KeywordId,
    pub dir_in: KeywordId,
    pub dir_out: KeywordId,

    // Verbs
    pub go: KeywordId,
    pub look: KeywordId,
    pub take: KeywordId,
    pub drop: KeywordId,
    pub put: KeywordId,
    pub open: KeywordId,
    pub close: KeywordId,
    pub attack: KeywordId,
    pub inventory: KeywordId,
    pub wait: KeywordId,

    // Prepositions
    pub prep_in: KeywordId,
    pub prep_on: KeywordId,
    pub prep_with: KeywordId,
    pub prep_to: KeywordId,
    pub prep_from: KeywordId,
    pub prep_at: KeywordId,

    // Pronouns
    pub it: KeywordId,
    pub him: KeywordId,
    pub her: KeywordId,
    pub them: KeywordId,

    // Actions
    pub action_go: KeywordId,
    pub action_look: KeywordId,
    pub action_examine: KeywordId,
    pub action_take: KeywordId,
    pub action_drop: KeywordId,
    pub action_open: KeywordId,
    pub action_close: KeywordId,
    pub action_attack: KeywordId,
    pub action_inventory: KeywordId,
    pub action_wait: KeywordId,

    // Scope keywords
    pub scope_immediate: KeywordId,
    pub scope_visible: KeywordId,
    pub scope_reachable: KeywordId,

    // Component keywords for scope evaluation
    pub location: KeywordId,
    pub inventory_rel: KeywordId,
    pub container_open: KeywordId,
    pub transparent: KeywordId,
    pub location_in: KeywordId,

    // Noun type keywords
    pub type_thing: KeywordId,
    pub type_takeable: KeywordId,
    pub type_container: KeywordId,
    pub type_openable: KeywordId,
    pub type_living: KeywordId,
    pub type_weapon: KeywordId,

    // Entity property keywords
    pub name: KeywordId,
    pub aliases: KeywordId,
    pub adjectives: KeywordId,
}

impl StdlibKeywords {
    /// Interns all standard library keywords in the world.
    #[must_use]
    pub fn intern(world: &mut World) -> Self {
        let interner = world.interner_mut();

        Self {
            // Directions
            north: interner.intern_keyword("direction/north"),
            south: interner.intern_keyword("direction/south"),
            east: interner.intern_keyword("direction/east"),
            west: interner.intern_keyword("direction/west"),
            up: interner.intern_keyword("direction/up"),
            down: interner.intern_keyword("direction/down"),
            northeast: interner.intern_keyword("direction/northeast"),
            northwest: interner.intern_keyword("direction/northwest"),
            southeast: interner.intern_keyword("direction/southeast"),
            southwest: interner.intern_keyword("direction/southwest"),
            dir_in: interner.intern_keyword("direction/in"),
            dir_out: interner.intern_keyword("direction/out"),

            // Verbs
            go: interner.intern_keyword("verb/go"),
            look: interner.intern_keyword("verb/look"),
            take: interner.intern_keyword("verb/take"),
            drop: interner.intern_keyword("verb/drop"),
            put: interner.intern_keyword("verb/put"),
            open: interner.intern_keyword("verb/open"),
            close: interner.intern_keyword("verb/close"),
            attack: interner.intern_keyword("verb/attack"),
            inventory: interner.intern_keyword("verb/inventory"),
            wait: interner.intern_keyword("verb/wait"),

            // Prepositions
            prep_in: interner.intern_keyword("prep/in"),
            prep_on: interner.intern_keyword("prep/on"),
            prep_with: interner.intern_keyword("prep/with"),
            prep_to: interner.intern_keyword("prep/to"),
            prep_from: interner.intern_keyword("prep/from"),
            prep_at: interner.intern_keyword("prep/at"),

            // Pronouns
            it: interner.intern_keyword("pronoun/it"),
            him: interner.intern_keyword("pronoun/him"),
            her: interner.intern_keyword("pronoun/her"),
            them: interner.intern_keyword("pronoun/them"),

            // Actions
            action_go: interner.intern_keyword("action/go"),
            action_look: interner.intern_keyword("action/look-around"),
            action_examine: interner.intern_keyword("action/examine"),
            action_take: interner.intern_keyword("action/take"),
            action_drop: interner.intern_keyword("action/drop"),
            action_open: interner.intern_keyword("action/open"),
            action_close: interner.intern_keyword("action/close"),
            action_attack: interner.intern_keyword("action/attack"),
            action_inventory: interner.intern_keyword("action/show-inventory"),
            action_wait: interner.intern_keyword("action/wait"),

            // Scope keywords
            scope_immediate: interner.intern_keyword("scope/immediate"),
            scope_visible: interner.intern_keyword("scope/visible"),
            scope_reachable: interner.intern_keyword("scope/reachable"),

            // Component keywords for scope evaluation
            location: interner.intern_keyword("component/location"),
            inventory_rel: interner.intern_keyword("rel/carrying"),
            container_open: interner.intern_keyword("container/open"),
            transparent: interner.intern_keyword("component/transparent"),
            location_in: interner.intern_keyword("rel/contains"),

            // Noun type keywords
            type_thing: interner.intern_keyword("type/thing"),
            type_takeable: interner.intern_keyword("type/takeable"),
            type_container: interner.intern_keyword("type/container"),
            type_openable: interner.intern_keyword("type/openable"),
            type_living: interner.intern_keyword("type/living"),
            type_weapon: interner.intern_keyword("type/weapon"),

            // Entity property keywords
            name: interner.intern_keyword("entity/name"),
            aliases: interner.intern_keyword("entity/aliases"),
            adjectives: interner.intern_keyword("entity/adjectives"),
        }
    }
}

/// Registers standard directions in the vocabulary.
pub fn register_directions(vocab: &mut VocabularyRegistry, kw: &StdlibKeywords) {
    // Cardinal directions
    vocab.register_direction(Direction {
        name: kw.north,
        synonyms: HashSet::new(),
        opposite: Some(kw.south),
    });
    vocab.register_direction(Direction {
        name: kw.south,
        synonyms: HashSet::new(),
        opposite: Some(kw.north),
    });
    vocab.register_direction(Direction {
        name: kw.east,
        synonyms: HashSet::new(),
        opposite: Some(kw.west),
    });
    vocab.register_direction(Direction {
        name: kw.west,
        synonyms: HashSet::new(),
        opposite: Some(kw.east),
    });

    // Vertical
    vocab.register_direction(Direction {
        name: kw.up,
        synonyms: HashSet::new(),
        opposite: Some(kw.down),
    });
    vocab.register_direction(Direction {
        name: kw.down,
        synonyms: HashSet::new(),
        opposite: Some(kw.up),
    });

    // Diagonal
    vocab.register_direction(Direction {
        name: kw.northeast,
        synonyms: HashSet::new(),
        opposite: Some(kw.southwest),
    });
    vocab.register_direction(Direction {
        name: kw.northwest,
        synonyms: HashSet::new(),
        opposite: Some(kw.southeast),
    });
    vocab.register_direction(Direction {
        name: kw.southeast,
        synonyms: HashSet::new(),
        opposite: Some(kw.northwest),
    });
    vocab.register_direction(Direction {
        name: kw.southwest,
        synonyms: HashSet::new(),
        opposite: Some(kw.northeast),
    });

    // Special
    vocab.register_direction(Direction {
        name: kw.dir_in,
        synonyms: HashSet::new(),
        opposite: Some(kw.dir_out),
    });
    vocab.register_direction(Direction {
        name: kw.dir_out,
        synonyms: HashSet::new(),
        opposite: Some(kw.dir_in),
    });
}

/// Registers standard verbs in the vocabulary.
pub fn register_verbs(vocab: &mut VocabularyRegistry, kw: &StdlibKeywords) {
    vocab.register_verb(Verb {
        name: kw.go,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.look,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.take,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.drop,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.put,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.open,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.close,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.attack,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.inventory,
        synonyms: HashSet::new(),
    });
    vocab.register_verb(Verb {
        name: kw.wait,
        synonyms: HashSet::new(),
    });
}

/// Registers standard prepositions in the vocabulary.
pub fn register_prepositions(vocab: &mut VocabularyRegistry, kw: &StdlibKeywords) {
    vocab.register_preposition(Preposition {
        name: kw.prep_in,
        implies: Some(kw.location),
    });
    vocab.register_preposition(Preposition {
        name: kw.prep_on,
        implies: None,
    });
    vocab.register_preposition(Preposition {
        name: kw.prep_with,
        implies: None,
    });
    vocab.register_preposition(Preposition {
        name: kw.prep_to,
        implies: None,
    });
    vocab.register_preposition(Preposition {
        name: kw.prep_from,
        implies: None,
    });
    vocab.register_preposition(Preposition {
        name: kw.prep_at,
        implies: None,
    });
}

/// Registers standard pronouns in the vocabulary.
pub fn register_pronouns(vocab: &mut VocabularyRegistry, kw: &StdlibKeywords) {
    vocab.register_pronoun(Pronoun {
        name: kw.it,
        gender: PronounGender::Neuter,
        number: PronounNumber::Singular,
    });
    vocab.register_pronoun(Pronoun {
        name: kw.him,
        gender: PronounGender::Masculine,
        number: PronounNumber::Singular,
    });
    vocab.register_pronoun(Pronoun {
        name: kw.her,
        gender: PronounGender::Feminine,
        number: PronounNumber::Singular,
    });
    vocab.register_pronoun(Pronoun {
        name: kw.them,
        gender: PronounGender::Neuter,
        number: PronounNumber::Plural,
    });
}

/// Registers standard noun types in the vocabulary.
pub fn register_types(vocab: &mut VocabularyRegistry, kw: &StdlibKeywords) {
    vocab.register_type(NounType {
        name: kw.type_thing,
        extends: Vec::new(),
        pattern_source: "[[?obj :name _]]".to_string(),
    });
    vocab.register_type(NounType {
        name: kw.type_takeable,
        extends: vec![kw.type_thing],
        pattern_source: "[[?obj :takeable true]]".to_string(),
    });
    vocab.register_type(NounType {
        name: kw.type_container,
        extends: vec![kw.type_thing],
        pattern_source: "[[?obj :container/capacity _]]".to_string(),
    });
    vocab.register_type(NounType {
        name: kw.type_openable,
        extends: vec![kw.type_thing],
        pattern_source: "[[?obj :openable true]]".to_string(),
    });
    vocab.register_type(NounType {
        name: kw.type_living,
        extends: vec![kw.type_thing],
        pattern_source: "[[?obj :health/current _]]".to_string(),
    });
    vocab.register_type(NounType {
        name: kw.type_weapon,
        extends: vec![kw.type_takeable],
        pattern_source: "[[?obj :weapon/damage _]]".to_string(),
    });
}

/// Creates standard scopes for the parser.
#[must_use]
pub fn create_scopes(kw: &StdlibKeywords) -> Vec<CompiledScope> {
    vec![
        // Immediate scope - room contents + inventory
        CompiledScope {
            name: kw.scope_immediate,
            parent: None,
            kind: ScopeKind::Union(vec![]),
        },
        // Visible scope - immediate + transparent containers
        CompiledScope {
            name: kw.scope_visible,
            parent: Some(kw.scope_immediate),
            kind: ScopeKind::ContainerContents {
                require_open: false,
                require_transparent: true,
            },
        },
        // Reachable scope - visible + open containers
        CompiledScope {
            name: kw.scope_reachable,
            parent: Some(kw.scope_visible),
            kind: ScopeKind::ContainerContents {
                require_open: true,
                require_transparent: false,
            },
        },
    ]
}

/// Creates a scope evaluator with standard keywords.
#[must_use]
pub fn create_scope_evaluator(kw: &StdlibKeywords) -> ScopeEvaluator {
    ScopeEvaluator::new(
        kw.location,
        kw.inventory_rel,
        kw.container_open,
        kw.transparent,
        kw.location_in,
    )
}

/// Creates standard command syntaxes.
#[must_use]
pub fn create_syntaxes(kw: &StdlibKeywords) -> Vec<CompiledSyntax> {
    vec![
        // look
        CompiledSyntax {
            command: kw.look,
            action: kw.action_look,
            elements: vec![CompiledSyntaxElement::Verb(kw.look)],
            priority: 0,
        },
        // look at [thing]
        CompiledSyntax {
            command: kw.look,
            action: kw.action_examine,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.look),
                CompiledSyntaxElement::Preposition(kw.prep_at),
                CompiledSyntaxElement::Noun {
                    var: "target".to_string(),
                    type_constraint: None,
                },
            ],
            priority: 1,
        },
        // take [takeable]
        CompiledSyntax {
            command: kw.take,
            action: kw.action_take,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.take),
                CompiledSyntaxElement::Noun {
                    var: "target".to_string(),
                    type_constraint: Some(kw.type_takeable),
                },
            ],
            priority: 0,
        },
        // drop [thing]
        CompiledSyntax {
            command: kw.drop,
            action: kw.action_drop,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.drop),
                CompiledSyntaxElement::Noun {
                    var: "target".to_string(),
                    type_constraint: None,
                },
            ],
            priority: 0,
        },
        // go [direction]
        CompiledSyntax {
            command: kw.go,
            action: kw.action_go,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.go),
                CompiledSyntaxElement::Direction {
                    var: "direction".to_string(),
                },
            ],
            priority: 0,
        },
        // open [openable]
        CompiledSyntax {
            command: kw.open,
            action: kw.action_open,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.open),
                CompiledSyntaxElement::Noun {
                    var: "target".to_string(),
                    type_constraint: Some(kw.type_openable),
                },
            ],
            priority: 0,
        },
        // close [openable]
        CompiledSyntax {
            command: kw.close,
            action: kw.action_close,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.close),
                CompiledSyntaxElement::Noun {
                    var: "target".to_string(),
                    type_constraint: Some(kw.type_openable),
                },
            ],
            priority: 0,
        },
        // attack [living]
        CompiledSyntax {
            command: kw.attack,
            action: kw.action_attack,
            elements: vec![
                CompiledSyntaxElement::Verb(kw.attack),
                CompiledSyntaxElement::Noun {
                    var: "target".to_string(),
                    type_constraint: Some(kw.type_living),
                },
            ],
            priority: 0,
        },
        // inventory
        CompiledSyntax {
            command: kw.inventory,
            action: kw.action_inventory,
            elements: vec![CompiledSyntaxElement::Verb(kw.inventory)],
            priority: 0,
        },
        // wait
        CompiledSyntax {
            command: kw.wait,
            action: kw.action_wait,
            elements: vec![CompiledSyntaxElement::Verb(kw.wait)],
            priority: 0,
        },
    ]
}

/// Registers all standard vocabulary in the registry.
pub fn register_all(vocab: &mut VocabularyRegistry, kw: &StdlibKeywords) {
    register_directions(vocab, kw);
    register_verbs(vocab, kw);
    register_prepositions(vocab, kw);
    register_pronouns(vocab, kw);
    register_types(vocab, kw);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_stdlib_dsl() {
        let dsl = all_stdlib_dsl();
        assert!(dsl.contains("direction: north"));
        assert!(dsl.contains("verb: look"));
        assert!(dsl.contains("preposition: in"));
        assert!(dsl.contains("pronoun: it"));
        assert!(dsl.contains("type: thing"));
        assert!(dsl.contains("scope: immediate"));
        assert!(dsl.contains("command: look"));
    }

    #[test]
    fn test_intern_keywords() {
        let mut world = World::new(42);
        let kw = StdlibKeywords::intern(&mut world);

        // Verify keywords are properly interned
        assert_ne!(kw.north, kw.south);
        assert_ne!(kw.look, kw.take);
        assert_ne!(kw.prep_in, kw.prep_on);
    }

    #[test]
    fn test_register_directions() {
        let mut world = World::new(42);
        let kw = StdlibKeywords::intern(&mut world);
        let mut vocab = VocabularyRegistry::new();

        register_directions(&mut vocab, &kw);

        assert!(vocab.lookup_direction(kw.north).is_some());
        assert!(vocab.lookup_direction(kw.south).is_some());
        assert!(vocab.lookup_direction(kw.up).is_some());
    }

    #[test]
    fn test_register_verbs() {
        let mut world = World::new(42);
        let kw = StdlibKeywords::intern(&mut world);
        let mut vocab = VocabularyRegistry::new();

        register_verbs(&mut vocab, &kw);

        assert!(vocab.lookup_verb(kw.look).is_some());
        assert!(vocab.lookup_verb(kw.take).is_some());
        assert!(vocab.lookup_verb(kw.attack).is_some());
    }

    #[test]
    fn test_create_scopes() {
        let mut world = World::new(42);
        let kw = StdlibKeywords::intern(&mut world);

        let scopes = create_scopes(&kw);

        assert_eq!(scopes.len(), 3);
        assert_eq!(scopes[0].name, kw.scope_immediate);
        assert_eq!(scopes[1].name, kw.scope_visible);
        assert_eq!(scopes[2].name, kw.scope_reachable);
    }

    #[test]
    fn test_create_syntaxes() {
        let mut world = World::new(42);
        let kw = StdlibKeywords::intern(&mut world);

        let syntaxes = create_syntaxes(&kw);

        assert!(!syntaxes.is_empty());

        // Find the look syntax
        let look_syntax = syntaxes.iter().find(|s| s.command == kw.look);
        assert!(look_syntax.is_some());
    }
}
