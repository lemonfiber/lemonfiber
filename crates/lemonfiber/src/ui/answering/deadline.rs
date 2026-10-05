//! A request body that has to have arrived by a deadline.
//!
//! The header timeout bounds how long a request takes to say what it is; nothing
//! bounded how long it took to send what it carries. A device that names a large body
//! and sends a byte a minute holds its connection for as long as it likes, so the body
//! is given a deadline from the moment its headers arrive, and reading it fails once
//! the deadline passes. Each route then answers a body that never arrived the way it
//! answers one that could not be read.
//!
//! Only what a request sends is bounded. What it is sent back is not, so a stream a
//! client holds open is as long-lived as before.

use std::future::Future as _;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Bytes;
use http_body::{Body, Frame, SizeHint};
use tokio::time::Sleep;

/// Why a body stopped being read.
#[derive(Debug)]
pub(super) struct TooSlow;

impl std::fmt::Display for TooSlow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the request body did not arrive in time")
    }
}

impl std::error::Error for TooSlow {}

/// A body read until it ends or its deadline passes, whichever comes first.
pub(super) struct Deadlined<B> {
    /// What is being read.
    inner: B,
    /// When reading it stops, where there is anything left to read.
    until: Option<Pin<Box<Sleep>>>,
}

impl<B: Body> Deadlined<B> {
    /// `inner`, given `within` from now to arrive.
    ///
    /// A body that is already over is given no deadline at all, so a request carrying
    /// nothing costs no timer.
    pub(super) fn within(inner: B, within: Duration) -> Self {
        let until = (!inner.is_end_stream()).then(|| Box::pin(tokio::time::sleep(within)));
        Self { inner, until }
    }
}

impl<B> Body for Deadlined<B>
where
    B: Body<Data = Bytes> + Unpin,
    B::Error: Into<axum::BoxError>,
{
    type Data = Bytes;
    type Error = axum::BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        if let Poll::Ready(frame) = Pin::new(&mut self.inner).poll_frame(cx) {
            return Poll::Ready(frame.map(|read| read.map_err(Into::into)));
        }
        let passed = self
            .until
            .as_mut()
            .is_some_and(|until| until.as_mut().poll(cx).is_ready());
        if passed {
            return Poll::Ready(Some(Err(TooSlow.into())));
        }
        Poll::Pending
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}
