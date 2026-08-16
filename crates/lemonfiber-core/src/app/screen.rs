//! The channel that is the screen itself.
//!
//! Every other channel is somewhere an alert has to be *sent* — a webhook, a chat
//! service, something that can refuse. This one is not. An alert is written to the
//! outbox before any channel is tried, and the dashboard reads the outbox, so by
//! the time delivery is attempted the operator can already see it.
//!
//! Which is why it cannot fail, and why it needs no configuration: there is
//! nothing to reach and nothing to get wrong. A stack with no channels set up
//! still tells its operator what happened, on the screen they are already looking
//! at — and a channel that must be configured before anything is reported is a
//! stack that stays silent for whoever never got round to it.

use async_trait::async_trait;

use crate::alert::Digest;
use crate::ports::notify::{Channel, Undelivered};

/// The screen, as a channel.
pub(crate) struct Screen;

#[async_trait]
impl Channel for Screen {
    fn name(&self) -> &'static str {
        "screen"
    }

    async fn deliver(&self, _digest: &Digest) -> Result<(), Undelivered> {
        // Nothing to do and nothing to fail: what makes an alert visible here is
        // that it was written down, which happened before this was called.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Screen;
    use crate::alert::Digest;
    use crate::ports::notify::Channel;

    #[tokio::test]
    async fn the_screen_never_refuses_an_alert() {
        // The one channel that cannot be down. An operator who configured nothing
        // is still told what happened.
        assert!(Screen.deliver(&Digest::default()).await.is_ok());
        assert_eq!(Screen.name(), "screen");
    }
}
