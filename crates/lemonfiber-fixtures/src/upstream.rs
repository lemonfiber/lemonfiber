//! The upstream a contract's tests run one script against, in process and through the
//! contract.
//!
//! It writes down every argument it is handed and answers from them, so an argument lost or
//! altered on the way shows in what it was told and in what it said. Each capability's ports
//! are implemented for it in a module of their own.

use std::sync::Mutex;

mod archive;
pub mod collecting;
mod dashboard;
pub mod downloading;
pub mod gate;
mod identity;
mod indexer;
mod invite;
pub mod library;
pub mod media;
pub mod network;
mod owner;
mod proxy;
mod quality;
pub mod request;
mod status;
pub mod stream;
pub mod subtitles;

/// The service the core knows every adapter as, and names its failures after.
pub const SERVICE: &str = "adapter";

/// An upstream that writes down every argument it is handed and answers from them.
#[derive(Default)]
pub struct Upstream {
    told: Mutex<Vec<String>>,
}

impl Upstream {
    /// Write down one thing it was told.
    fn tell(&self, what: String) {
        crate::noted(&self.told, what);
    }

    /// Everything it was told, in order.
    #[must_use]
    pub fn told(&self) -> Vec<String> {
        self.told
            .lock()
            .map(|told| told.clone())
            .unwrap_or_default()
    }

    /// The operations it was told about, each by the word its entry starts with.
    #[must_use]
    pub fn operations(&self) -> Vec<String> {
        self.told()
            .iter()
            .map(|told| told.split(' ').next().unwrap_or_default().to_owned())
            .collect()
    }
}

/// A value as it crosses, credentials and all.
fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
