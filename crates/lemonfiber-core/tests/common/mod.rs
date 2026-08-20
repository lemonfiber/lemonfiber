//! What the integration tests share: the fakes that speak lemonfiber's own models
//! rather than a port, and where the stack they run against lives.
//!
//! The port fakes live in `lemonfiber-fixtures`, where the crate's own tests can reach them
//! too. These two cannot: the Servarr service answers a seeding run and the gateway answers
//! a diagnosis, so both need models that live above the boundary — and a crate holding them
//! would have to depend on `lemonfiber-core`, which is the cycle the fixtures crate exists
//! to avoid.

#![allow(dead_code)]

pub mod service;
pub mod stack;
pub mod tunnel;
