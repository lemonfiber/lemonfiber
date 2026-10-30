//! What the core asks of whatever fills a capability, and how an adapter answers.
//!
//! A capability's contract is the ports the core calls for it, spoken over HTTP to an
//! adapter a plugin ships beside what it runs. Each capability is declared once, with the
//! crate's `contract!` macro, as the port methods it carries. From that one declaration
//! come the wire types each call crosses as, a client that implements every one of those
//! ports by asking an adapter, a dispatcher an adapter serves them through, and the
//! descriptors the published documents are generated from. So the core, an adapter and the
//! documents cannot disagree about a call: there is one place it is written.
//!
//! **Every answer is a stranger's.** An adapter is a plugin's code in a container of its
//! own. What it answers is decoded against the operation's types and nothing else: an
//! unknown field, a wrong type, a status the contract does not declare, a body past the
//! bound or an answer past the deadline is refused and never read ([`client`]).

pub mod adapter;
pub mod capabilities;
pub mod client;
pub mod conformance;
mod declare;
pub mod documents;

use std::future::Future;
use std::pin::Pin;

use declare::contract;
pub mod wire;

pub use adapter::Served;
pub use client::{Bounds, Contracted};
pub use lemonfiber_ports::service::Failure;
pub use wire::Refusal;

/// The path every operation sits under: `/lemonfiber/<capability>/v<major>/<operation>`.
#[must_use]
pub fn path(capability: &str, major: u32, operation: &str) -> String {
    format!("/lemonfiber/{capability}/v{major}/{operation}")
}

/// A contract as a service names it: `capability@major`.
#[must_use]
pub fn spoken(capability: &str, major: u32) -> String {
    format!("{capability}@{major}")
}

/// The capability a contract named `capability@major` is of.
#[must_use]
pub fn capability_of(spoken: &str) -> Option<&str> {
    spoken.split_once('@').map(|(capability, _)| capability)
}

/// An answer read as one operation's, by a client asked it: what the core makes of it.
type Reading = for<'c> fn(
    &'c Contracted,
    &'static str,
    u32,
    &'static str,
    Bounds,
) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + 'c>>;

/// One operation of one capability's contract: what it is called, and the shapes of what
/// is asked and what is answered.
#[derive(Debug, Clone)]
pub struct Operation {
    /// The capability it belongs to, as the vocabulary names it.
    pub capability: &'static str,
    /// The contract's major.
    pub major: u32,
    /// Its name, which is the port method's.
    pub name: &'static str,
    /// The schema of what is asked.
    pub asked: schemars::Schema,
    /// The schema of what is answered.
    pub answered: schemars::Schema,
    /// What its answer is held to.
    pub bounds: Bounds,
    /// Each place its answer holds a credential the upstream issues, as a JSON pointer,
    /// `""` where the whole answer is one.
    pub secret: &'static [&'static str],
    /// What its conformance case posts, its placeholders still to be filled.
    pub asks: serde_json::Value,
    /// For an operation bound to a kind of media, what its case for each media type posts
    /// instead, by media type; none for every other operation.
    pub filed: Vec<(&'static str, serde_json::Value)>,
    /// Whether it may answer that what it applied is in force once the upstream restarts.
    pub restarts: bool,
    /// The statuses its conformance case is to be refused with, where what the case asks
    /// is something the operation must refuse; nothing where the case is to be answered.
    pub refused: Option<&'static [u16]>,
    /// How its answer is read.
    reads: Reading,
}

impl Operation {
    /// The operation `name` of `capability`'s major, asked as `A` and answered as `R`.
    #[must_use]
    pub fn new<A, R>(capability: &'static str, major: u32, name: &'static str) -> Self
    where
        A: schemars::JsonSchema,
        R: schemars::JsonSchema + serde::de::DeserializeOwned + Send + 'static,
    {
        Self {
            capability,
            major,
            name,
            asked: schemars::schema_for!(A),
            answered: schemars::schema_for!(R),
            bounds: Bounds::STANDARD,
            secret: &[],
            asks: serde_json::Value::Null,
            filed: Vec::new(),
            restarts: std::any::TypeId::of::<R>()
                == std::any::TypeId::of::<lemonfiber_ports::service::Applied>(),
            refused: None,
            reads: read_as::<R>,
        }
    }

    /// The same operation, its conformance case posting `asks`.
    #[must_use]
    pub fn asking<A: serde::Serialize>(mut self, asks: &A) -> Self {
        self.asks = serde_json::to_value(asks).unwrap_or_default();
        self
    }

    /// What `client` answers this operation comes to, read as the core reads it: the
    /// answer's status, size and shape held to the contract.
    ///
    /// # Errors
    ///
    /// As [`Contracted::call`].
    pub async fn judged(&self, client: &Contracted) -> Result<(), Failure> {
        (self.reads)(client, self.capability, self.major, self.name, self.bounds).await
    }

    /// The same operation, its answer bounded at `largest` bytes.
    #[must_use]
    pub const fn within(mut self, largest: usize) -> Self {
        self.bounds.largest = largest;
        self
    }

    /// The same operation, its answer awaited for `deadline`.
    #[must_use]
    pub const fn awaited(mut self, deadline: std::time::Duration) -> Self {
        self.bounds.deadline = deadline;
        self
    }

    /// The same operation, its answer holding a credential at each pointer of `secret`.
    #[must_use]
    pub const fn holding(mut self, secret: &'static [&'static str]) -> Self {
        self.secret = secret;
        self
    }

    /// The same operation, its conformance case to be refused with one of `refused`
    /// where any is named.
    #[must_use]
    pub const fn refusing(mut self, refused: Option<&'static [u16]>) -> Self {
        self.refused = refused;
        self
    }

    /// Where it is asked.
    #[must_use]
    pub fn path(&self) -> String {
        path(self.capability, self.major, self.name)
    }
}

/// Ask `client` one operation and read its answer as `R`, keeping nothing of it.
fn read_as<'c, R: serde::de::DeserializeOwned + Send + 'static>(
    client: &'c Contracted,
    capability: &'static str,
    major: u32,
    name: &'static str,
    bounds: Bounds,
) -> Pin<Box<dyn Future<Output = Result<(), Failure>> + Send + 'c>> {
    Box::pin(async move {
        client
            .call::<(), R>(capability, major, name, bounds, &())
            .await
            .map(drop)
    })
}

/// One capability's contract: its name, its major and its operations.
#[derive(Debug, Clone)]
pub struct Capability {
    /// The capability, as the vocabulary names it.
    pub name: &'static str,
    /// The major this build speaks.
    pub major: u32,
    /// Every operation, in the order they are declared.
    pub operations: Vec<Operation>,
}
