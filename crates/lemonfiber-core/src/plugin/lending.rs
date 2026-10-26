//! A first-party plugin's adapter lent to a service lemonfiber does not run.
//!
//! The core speaks a service only over a contract, through an adapter. A service somebody
//! else runs has no adapter in front of it, so one is lent: the plugin's adapter started
//! from its pin as a throwaway container on a network of its own, pointed at the service,
//! and handed that service's own credential in its `upstream.json` and nothing else. It
//! is asked over its contract like any adapter, and taken away with its network and its
//! throwaway configuration whatever it came to.

use lemonfiber_contract::adapter::{ready_path, KEY_FILE};
use lemonfiber_contract::Contracted;
use lemonfiber_plugin::{Health, HealthKind};

use super::installed::{Installed, Placed};
use super::program::{Operator, Starting, Throwaway};
use crate::app::Ctx;
use crate::ports::serving::{Serve, Served};

/// The flag an adapter is told where its upstream answers by.
const UPSTREAM_FLAG: &str = "--upstream";

/// Where the loopback interface reaches what a container publishes.
const LOOPBACK: &str = super::container::LOOPBACK;

/// What an adapter is lent for: the plugin it is of, the adapter, where the service it is
/// lent to answers, and the credential that service answers to.
pub(crate) struct Borrowing<'a> {
    /// The installed plugin the adapter is of.
    pub(crate) installed: &'a Installed,
    /// The adapter, as installing the plugin places it.
    pub(crate) adapter: &'a Placed,
    /// The address the adapter reaches the service at.
    pub(crate) upstream: String,
    /// Each name the adapter's container resolves to this machine.
    pub(crate) reaching: Vec<String>,
    /// Each field of the adapter's `upstream.json`, with the service's own value for it.
    pub(crate) credentials: Vec<(String, String)>,
}

/// An adapter lent to a service, running until it is returned.
pub(crate) struct Lent {
    /// Its container.
    served: Served,
    /// Its throwaway configuration, removed when this is dropped.
    _held: Throwaway,
    /// The adapter, asked over its contracts.
    pub(crate) adapter: Contracted,
}

impl Lent {
    /// Take its container and its network off the engine.
    pub(crate) async fn returned(self, ctx: &Ctx) {
        ctx.seams.serving.removed(&self.served).await;
    }
}

/// The adapter `borrowing` names, started and ready to be asked; or why it could not be,
/// with nothing left on the engine.
///
/// # Errors
///
/// Where the plugin's starting files are not kept, its configuration or its key cannot be
/// written, the engine will not serve it, or it is not ready within its limit.
pub(crate) async fn lent(ctx: &Ctx, borrowing: &Borrowing<'_>) -> Result<Lent, String> {
    let adapter = borrowing.adapter;
    let root = crate::app::plugins::kept::root(ctx, &borrowing.installed.plugin)
        .ok_or_else(|| "this machine keeps no starting files for its plugin".to_owned())?;
    let held = Throwaway::of(
        Some(Starting {
            root: &root,
            service: &adapter.service,
        }),
        ctx.seams.random.as_ref(),
    )?;
    let key = crate::secret::generate(ctx.seams.random.as_ref())
        .ok_or_else(|| crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())?;
    held.hold(KEY_FILE, &key)?;
    super::recording::handed_over(&held, &borrowing.credentials)?;
    let mut operator = Operator::of(ctx);
    if let Some((uid, gid)) = held.owner() {
        operator.ids = (uid.to_string(), gid.to_string());
    }
    let serve = Serve {
        command: vec![UPSTREAM_FLAG.to_owned(), borrowing.upstream.clone()],
        reaching: borrowing.reaching.clone(),
        ..super::recording::settled_as(adapter, &held, None, &operator)
    };
    let served = ctx
        .seams
        .serving
        .serve(&serve)
        .await
        .map_err(|unran| unran.to_string())?;
    match ready(ctx, adapter, &served).await {
        Ok(base) => Ok(Lent {
            served,
            _held: held,
            adapter: Contracted::new(ctx.seams.http.clone(), base, &adapter.service, key),
        }),
        Err(why) => {
            ctx.seams.serving.removed(&served).await;
            Err(why)
        }
    }
}

/// Where the loopback interface reaches the adapter `served`, once it says its upstream
/// answers; or why it did not.
async fn ready(ctx: &Ctx, adapter: &Placed, served: &Served) -> Result<String, String> {
    let health = Health {
        kind: HealthKind::Http,
        path: Some(ready_path()),
        timeout_s: None,
    };
    super::recording::healthy(
        ctx.seams.serving.as_ref(),
        ctx.seams.http.as_ref(),
        Some(&health),
        served,
    )
    .await?;
    served
        .port
        .map(|port| format!("http://{LOOPBACK}:{port}"))
        .ok_or_else(|| {
            format!(
                "{} publishes no port, so nothing can ask it",
                adapter.called()
            )
        })
}

#[cfg(test)]
mod tests;
