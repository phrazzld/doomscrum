//! Render orchestration: the spec→storyboard→render [`pipeline`], the spend
//! [`wallet`], and the durable cost [`ledger`]. `server.rs` is HTTP routing +
//! JSON shaping over this module; every render and every spend decision rides
//! through here.

pub mod ledger;
pub mod pipeline;
pub mod wallet;
