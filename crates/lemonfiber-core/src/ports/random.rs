//! A source of unpredictable bytes.
//!
//! Behind a port so the code that turns bytes into a secret is tested against a
//! fixed, known sequence — a generated secret is otherwise impossible to assert —
//! while the one place that reaches the operating system's randomness stays a
//! single adapter. Synchronous, because randomness is: there is nothing to await.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

/// A source of cryptographically unpredictable bytes, for the one secret
/// lemonfiber generates itself.
pub trait Random: Send + Sync {
    /// `n` unpredictable bytes, or `None` where the operating system could not
    /// provide them — which is not something to paper over with a weaker source,
    /// so the caller reports it rather than generating a guessable secret.
    fn bytes(&self, n: usize) -> Option<Vec<u8>>;
}
