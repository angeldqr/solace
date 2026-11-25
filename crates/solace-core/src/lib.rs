//! Solace domain layer. Must stay free of IO, OS and UI dependencies;
//! CI fails if this crate ever depends on tauri, portable-pty or windows.

pub mod agents;
pub mod orchestration;
pub mod session;
pub mod spaces;
pub mod terminals;
