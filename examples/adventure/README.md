# The Dark Cave - Adventure Example

A text adventure demo showcasing Longtable's natural language parsing and rule-based simulation capabilities.

## Running

```bash
cargo run -p longtable_runtime --bin longtable -- --run examples/adventure/_.lt
```

## Sample Transcript

```
===============================================
             THE DARK CAVE
        A Text Adventure Demo
===============================================

Type 'help' for a list of commands.

Welcome to the cave entrance!

> look

Cave Entrance
You stand at the mouth of a dark cave. Sunlight streams in from behind you,
illuminating rough stone walls covered in moss. A cold breeze carries the
smell of damp earth from deeper within.
You can go south.

> l lantern

A sturdy brass lantern with a glass chimney. It provides a warm, steady light.

> s

Main Hall
A vast underground chamber stretches before you. Ancient stalactites hang
from the ceiling like stone fangs. Multiple passages lead off into darkness.
You can go east, west, or north.

> l chest

A heavy wooden chest bound with iron bands. It is locked.

> e

Crystal Cavern
The walls here are studded with glowing crystals that cast an eerie blue
light. The crystals hum faintly, resonating with some unknown energy.
You can go west or south.

> l sword

An old iron sword. Despite its rust, the edge is still sharp.

> w

Main Hall

> n

Cave Entrance
```

## Supported Commands

### Working

| Command | Description |
|---------|-------------|
| `look` / `l` | Examine current room |
| `l <object>` / `x <object>` / `examine <object>` | Examine an object |
| `n` / `s` / `e` / `w` / `north` / `south` / `east` / `west` | Move in a direction |
| `go <direction>` | Move in a direction |
| `inventory` / `i` | List carried items |

### Not Yet Implemented

| Command | Issue |
|---------|-------|
| `take` / `get` | Needs `retract` function |
| `drop` / `put` | Needs `retract` function |
| `open` / `close` | Preconditions using `(or ...)` not working |
| `unlock` | Preconditions using `(or ...)` not working |

## File Structure

| File | Purpose |
|------|---------|
| `_.lt` | Entry point - loads all other files |
| `vocabulary.lt` | Verbs, directions, prepositions |
| `commands.lt` | Command syntax patterns |
| `actions.lt` | Action handlers and preconditions |
| `data/schemas.lt` | Component and relationship schemas |
| `data/world.lt` | Rooms, items, and connections |
| `stdlib.lt` | Helper functions (say, describe-room, etc.) |

## Architecture

The adventure example demonstrates several Longtable features:

1. **Natural Language Parsing**: Player input like "examine brass lantern" is parsed into structured commands
2. **Vocabulary System**: Verbs, prepositions, and directions are registered and matched
3. **Syntax Patterns**: Command patterns like `[:verb/look ?target:thing]` define valid input forms
4. **Action System**: Actions have parameters, preconditions, and handlers
5. **Entity-Component Storage**: Rooms, items, and the player are entities with components
6. **Relationships**: Spatial relationships (`in-room`, `exit/north`) connect entities
