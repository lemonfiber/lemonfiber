//! Whether the declaration an adapter speaking `media.serve` gives holds at install.

use lemonfiber_contract::capabilities::media::serve;

use crate::ports::service::Declaring as _;

/// Why the declaration an adapter speaking `media.serve` gives does not hold, where it
/// does not: one that admits a byte path without a question, or whose template host is
/// not its base, is refused at install rather than first met by the guard. One offering
/// no declaration serves nobody through a guard, so holds nothing back.
pub(super) async fn undeclarable(
    adapter: &lemonfiber_contract::Contracted,
    placed: &crate::plugin::Placed,
) -> Option<String> {
    let spoken = lemonfiber_contract::spoken(serve::CAPABILITY, serve::MAJOR);
    if !placed.speaks.contains(&spoken) {
        return None;
    }
    match serve::Adapter(adapter.clone()).declared().await {
        Ok(declared) => declared
            .checked()
            .err()
            .map(|refused| format!("{spoken}: its declaration was refused: {refused}")),
        Err(crate::ports::service::Failure::Unsupported { .. }) => None,
        Err(failure) => Some(format!("{spoken}: declared: {failure}")),
    }
}
