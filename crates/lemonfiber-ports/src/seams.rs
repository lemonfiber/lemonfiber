//! Everything a run reaches the world through, in one place.
//!
//! The implementations of these live above the core, in a crate the core does not
//! depend on — which is what stops anything holding a context from manufacturing a
//! socket, and what keeps a container runtime and a TLS stack out of a crate that must
//! not be able to reach the network at all.
//!
//! A bundle rather than a longer argument list. There are nine of them, they are always
//! supplied together, and a surface that had to name each one at every call would be a
//! surface where adding a tenth is a change in sixty places.
//!
//! It holds no policy. Which implementation each field carries is the surface's answer,
//! and the decisions taken over them — retrying a request, refusing one the operator's
//! settings forbid — are the core's, made above this.

use std::sync::Arc;

use crate::docker::Images;
use crate::filesystem::{Eraser, Volume};
use crate::hosting::Host;
use crate::http::Http;
use crate::nntp::Nntp;
use crate::occupancy::Occupancy;
use crate::random::Random;
use crate::FileSystem;

/// The seams a run reaches the outside world through.
pub struct Seams {
    /// How files are read, written and removed.
    pub filesystem: Arc<dyn FileSystem>,
    /// How requests leave this machine.
    pub http: Arc<dyn Http>,
    /// How the engine is asked what it has pulled.
    pub images: Arc<dyn Images>,
    /// How a drive is asked what it holds.
    pub volume: Arc<dyn Volume>,
    /// How what this machine keeps is removed.
    pub eraser: Arc<dyn Eraser>,
    /// How the disk is asked where it went.
    pub occupancy: Arc<dyn Occupancy>,
    /// How this machine is asked to keep something running.
    pub hosting: Arc<dyn Host>,
    /// Where unpredictable values come from.
    pub random: Arc<dyn Random>,
    /// How a Usenet provider is reached, which is a connection rather than a request.
    pub nntp: Arc<dyn Nntp>,
}
