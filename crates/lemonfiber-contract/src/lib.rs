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
mod declare;
pub mod documents;

use declare::contract;
pub mod wire;

pub use adapter::Served;
pub use client::Contracted;
pub use lemonfiber_ports::service::Failure;
pub use wire::Refusal;

/// The path every operation sits under: `/lemonfiber/<capability>/v<major>/<operation>`.
#[must_use]
pub fn path(capability: &str, major: u32, operation: &str) -> String {
    format!("/lemonfiber/{capability}/v{major}/{operation}")
}

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
    /// The most its answer may carry, in bytes.
    pub largest: usize,
}

impl Operation {
    /// The operation `name` of `capability`'s major, asked as `A` and answered as `R`.
    #[must_use]
    pub fn new<A: schemars::JsonSchema, R: schemars::JsonSchema>(
        capability: &'static str,
        major: u32,
        name: &'static str,
    ) -> Self {
        Self {
            capability,
            major,
            name,
            asked: schemars::schema_for!(A),
            answered: schemars::schema_for!(R),
            largest: client::LARGEST,
        }
    }

    /// The same operation, its answer bounded at `largest` bytes.
    #[must_use]
    pub const fn within(mut self, largest: usize) -> Self {
        self.largest = largest;
        self
    }

    /// Where it is asked.
    #[must_use]
    pub fn path(&self) -> String {
        path(self.capability, self.major, self.name)
    }
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
