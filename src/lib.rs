//! Library behind `skillctl`: load Agent Skills, validate them, route tasks
//! across model tiers, and export skills to other agents' instruction formats.
//! `contract` and `runlog` are the data formats later runtime work builds on.

pub mod contract;
pub mod export;
pub mod install;
pub mod routing;
pub mod runlog;
pub mod skill;
pub mod validate;
