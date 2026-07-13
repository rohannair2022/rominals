# Anti-Vibe Coding Skill — Work Log & Research Queue

Use this file every time you code.

## Rule for each change

Before marking work as done, write:
- **What changed**
- **Why was it needed**
- **How sure you are** (High / Medium / Low)
- **What still feels uncertain**

If confidence is not **High**, add an item to the research queue.

## Work done log

| Date | Change summary | Files touched | Confidence | Uncertainty |
|---|---|---|---|---|
| 2026-07-12 | Split TUI into focused modules (controls/state/view/runtime) and kept CLI arg parsing in `main.rs`. | `backend_rominals/src/main.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs` | High | None |
| 2026-07-13 | Migrated terminal rendering to `ratatui` with structured panels (header/input/quote/status), colorized market change/error states, and table-based quote display. Kept `crossterm` for raw mode, screen lifecycle, and input events via `CrosstermBackend`. | `backend_rominals/Cargo.toml`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/view.rs`, `backend_rominals/src/tui/controls.rs` | High | None |

## Research queue (possible vibe-coded risk)

| Added on | Topic to research | Why this is a risk | Priority | Status |
|---|---|---|---|---|
| 2026-07-12 | Non-blocking quote fetch in TUI loop | `reqwest::blocking` runs in input loop, so network latency can freeze interaction. | High | Open |
| 2026-07-12 | Better TUI renderer abstraction | Full-screen clear every frame is simple but may cause flicker and poor extensibility as UI grows. | Medium | Closed (Migrated to `ratatui` on 2026-07-13) |
| 2026-07-12 | Error model cleanup | `Box<dyn Error>` is convenient but weak for explicit app-level error handling paths. | Medium | Open |

## Research outcomes

| Date | Topic | Decision | Follow-up change |
|---|---|---|---|
| 2026-07-13 | Better TUI renderer abstraction | Adopted `ratatui` for layout/widgets/styling while keeping `crossterm` backend for terminal and events. | Completed in `backend_rominals/src/tui/mod.rs` and `backend_rominals/src/tui/view.rs` |
