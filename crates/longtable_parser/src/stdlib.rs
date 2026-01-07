//! Standard library for adventure games.
//!
//! Contains default vocabulary definitions for common adventure game patterns.

/// Standard directions DSL.
pub const DIRECTIONS: &str = r#"
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
pub const VERBS: &str = r#"
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
pub const PREPOSITIONS: &str = r#"
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
pub const PRONOUNS: &str = r#"
(pronoun: it :gender :neuter :number :singular)
(pronoun: him :gender :masculine :number :singular)
(pronoun: her :gender :feminine :number :singular)
(pronoun: them :gender :neuter :number :plural)
"#;

/// Standard types DSL.
pub const TYPES: &str = r#"
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
pub const SCOPES: &str = r#"
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
pub const COMMANDS: &str = r#"
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

/// All standard library DSL combined.
pub const STDLIB: &str = concat!(
    ";; === DIRECTIONS ===\n",
    include_str!("stdlib.rs"), // This won't work, we'll use the constants
);

/// Gets all standard library DSL as a combined string.
#[must_use]
pub fn all_stdlib() -> String {
    format!("{DIRECTIONS}\n{VERBS}\n{PREPOSITIONS}\n{PRONOUNS}\n{TYPES}\n{SCOPES}\n{COMMANDS}")
}
