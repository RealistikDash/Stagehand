//! Domain logic for stagehand. No toolkit, bus or media framework dependencies: everything here
//! runs in the daemon, the CLI and in tests alike.

pub mod config;
pub mod model;
pub mod ports;
pub mod project;
pub mod protocol;
pub mod service;
pub mod steam;
pub mod still;
