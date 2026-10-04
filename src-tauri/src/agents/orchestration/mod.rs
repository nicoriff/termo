//! Orchestration: the user describes a piece of work and Termo designs a
//! team for it — how many agents, each one's specialty, engine and model —
//! shows the plan with what it's expected to cost, and, once approved, runs
//! it under an orchestrator that reads every result and can change the plan
//! as it goes.
//!
//! The model proposes (planner.rs, supervisor.rs); the rules about what a
//! plan may contain, what can start and what needs the user's approval are
//! plain code (plan.rs); executor.rs runs the tasks through the same runner
//! every agent uses.

pub mod executor;
pub mod plan;
pub mod planner;
pub mod pricing;
pub mod store;
pub mod supervisor;
