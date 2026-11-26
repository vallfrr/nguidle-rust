//! NGUIdleRustEngine: the PC application.
//!
//! The first launch (or any launch where the game files are not complete) shows a
//! setup window drawn by the same graphics layer as the game, from the interface
//! files embedded in the program (`assets/ui`). Later launches play the game directly.

pub mod art;
pub mod browse;
pub mod canvas;
pub mod dialog;
pub mod gpu;
pub mod install;
pub mod play;
pub mod preview;
pub mod setup;
pub mod sizes;

/// Name used for the program, its folders and its window.
pub const NAME: &str = "NGUIdleRustEngine";
