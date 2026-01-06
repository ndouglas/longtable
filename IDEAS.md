# Ideas for Later

This file captures ideas that are interesting but out of scope for the current phase. These are not commitments—just notes to revisit when the time is right.

---

## Persistent Command History

**Context**: REPL session history that persists across launches.

**Why deferred**: Introduces state outside the World (history file, session resumption, reset semantics). Adds complexity to what should be a simple "evaluate expressions against a world" model.

**When to revisit**: After Phase 5 (Interface) is stable.

**Considerations**:
- Where to store? (`~/.longtable_history`, project-local `.lt_history`?)
- Session reset/resume semantics
- History search (Ctrl+R style)
- Per-project vs global history

---

## Inline Documentation System

**Context**: Mini-man-pages for functions via macros/annotations on definitions.

**Why deferred**: Requires design work on how documentation attaches to functions (metadata? special syntax? macros?). The `(help ...)` function can exist without this—it just won't have much to show yet.

**When to revisit**: Phase 5 (Standard Library) or Phase 7 (Documentation).

**Possible approaches**:
- `(defn name "docstring" [args] body)` — docstring as second element
- `(meta name :doc "...")` — separate metadata attachment
- `#doc[...]` tagged literal on definitions
- Derive from source comments (`;; @doc ...`)

**Considerations**:
- Should documentation be queryable at runtime?
- Should it survive serialization?
- Integration with external doc generators (rustdoc style)

---

## Structured I/O for ML Integration

**Context**: Machine-readable input/output modes for integration with systems like OpenAI Gym, reinforcement learning environments, or automated testing.

**Why deferred**: Not a REPL concern. Better suited to a dedicated interface (socket server, stdin/stdout JSON mode, or embedded API).

**When to revisit**: After Phase 5, when considering embedding scenarios.

**Possible approaches**:
- Socket server with JSON/MessagePack protocol
- `--json` CLI flag for structured stdin/stdout
- Embedded `World` API for direct Rust integration
- WebSocket server for browser-based interfaces

**Considerations**:
- Latency requirements for RL training loops
- Observation/action space definition
- Determinism guarantees for reproducibility
