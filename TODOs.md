# TODOs

- The distinction between "session state" (compiler/VM) and "game state" (World) might deserve clearer separation.
- We should find common sequences/patterns of opcodes and investigate merging them for performance.
- Audit the REPL functionality; this should be a very, very thin wrapper around the rest of the system with essentially no meaningful functionality of its own, no more than 2-3 special forms, etc. Functionality should generally be in the compiler, language, parser, etc.
