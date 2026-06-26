//! Render orchestration: the spec→storyboard→render [`pipeline`] and the spend
//! [`wallet`]. `server.rs` is HTTP routing + JSON shaping over this module;
//! every render and every spend decision rides through here.

pub mod pipeline;
pub mod wallet;
