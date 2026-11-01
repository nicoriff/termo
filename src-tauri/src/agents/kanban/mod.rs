//! Kanban boards: the user writes tasks — what each is about, when it's
//! done, what it waits for — and orders them; when the board is on, an agent
//! per column takes them: one does the work, another reviews it and sends it
//! back or approves it, an optional third writes how it went. Every task
//! keeps a log of who did what.
//!
//! The rules are plain code (model.rs); driver.rs runs the tasks through the
//! same runner every agent uses; tools.rs is what the column agents get in
//! Termo's MCP server, and ops.rs what the user and the consoles do.

pub mod driver;
pub mod model;
pub mod ops;
pub mod store;
pub mod tools;
pub mod worktree;
