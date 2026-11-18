//! What one run of the serving loop holds open, and the wait for the password it rests on.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use lemonfiber_api::events::live::Live;
use lemonfiber_core::app::Ctx;
use tokio::net::TcpListener;

use super::reach::Offered;
use super::{answering, Ending, Until};

/// What one run of the serving loop holds open.
pub(super) struct Held {
    /// Every socket that was taken, with the address it was taken on.
    pub(super) sockets: Vec<(TcpListener, SocketAddr)>,
    /// What is answered on them.
    pub(super) surface: Router,
    /// How far they were offered.
    pub(super) offered: Offered,
    /// What each connection is handed to first, where this run encrypts.
    pub(super) tls: Option<tokio_rustls::TlsAcceptor>,
}

/// Hold every socket that was taken until one of the two endings arrives.
///
/// Whatever ends it, the sockets are given up before this returns, so the next time
/// round the loop is not asking for a port this run is still holding. A fault from
/// accepting on a socket this process already holds means the process is going down
/// around it, and a second message about one event helps nobody.
pub(super) async fn holding(
    held: Held,
    live: &Live,
    ctx: &Arc<Ctx>,
    until: &mut Until,
    look: Duration,
) -> Ending {
    let Held {
        sockets,
        surface,
        offered,
        tls,
    } = held;
    let (stopping, stopped) = tokio::sync::watch::channel(false);
    let mut running = Vec::new();
    for (listener, _) in sockets {
        running.push(tokio::spawn(answering::answering(
            Box::new(listener),
            surface.clone(),
            stopped.clone(),
            answering::Limits::SERVED,
            tls.clone(),
        )));
    }
    let ending = tokio::select! {
        () = &mut *until => Ending::Stopped,
        () = revoked(Arc::clone(ctx), offered, look) => Ending::Revoked,
    };
    let _ = stopping.send(true);
    // Every stream ends with the binding it was opened on. A stream is a request that
    // never finishes, so without this the sockets would wait on it for ever, and a
    // browser on the household network would go on hearing what the operator is shown
    // after the password that let it in was taken away.
    live.end_every_stream();
    for server in running {
        let _ = tokio::time::timeout(answering::Limits::SERVED.let_go * 2, server).await;
    }
    ending
}

/// Wait until the password this binding rests on is gone.
///
/// Never, where it rests on none: a surface answering this machine alone is offered
/// to nobody a password would have kept out, so there is nothing for its going to
/// take away.
async fn revoked(ctx: Arc<Ctx>, offered: Offered, look: Duration) {
    match offered {
        Offered::Machine | Offered::Refused => std::future::pending().await,
        Offered::Network => {
            while super::password_kept(&ctx) {
                tokio::time::sleep(look).await;
            }
        }
    }
}
