//! SCR-09 Agent Studio: a team of AI CLIs (claude, codex, agy, cursor) taking
//! a request from design to merged code, with the app as the orchestrator.

pub mod adapter;
pub mod engine;
pub mod git;
pub mod log;
pub mod process;
pub mod prompts;
pub mod workspace;

#[cfg(all(test, unix))]
mod engine_tests;
