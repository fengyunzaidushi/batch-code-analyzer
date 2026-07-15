//! Application-layer orchestration for Batch Code Analyzer.
//!
//! Use cases will be added in later milestones. Keeping this crate separate
//! makes the required Application -> Domain dependency direction explicit.

#![forbid(unsafe_code)]

pub use batch_code_analyzer_domain as domain;
