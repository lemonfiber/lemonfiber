//! The upstream as a subtitle finder, and a finder watching nothing.

use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, Subtitles, Watched, Watching};

use super::Upstream;

#[async_trait]
impl Subtitles for Upstream {
    async fn watching(&self, which: Kind) -> Result<Watching, Failure> {
        Ok(Watching {
            enabled: which == Kind::Tv,
            host: which.media_type().to_owned(),
            port: 8989,
            keyed: true,
        })
    }
    async fn watch(&self, watched: &Watched) -> Result<(), Failure> {
        self.tell(format!("watch {watched:?}"));
        Ok(())
    }
}

/// A subtitle finder watching nothing.
pub struct WatchingNothing;

#[async_trait]
impl Subtitles for WatchingNothing {
    async fn watching(&self, _which: Kind) -> Result<Watching, Failure> {
        Ok(Watching {
            enabled: false,
            host: String::new(),
            port: 0,
            keyed: false,
        })
    }
    async fn watch(&self, _watched: &Watched) -> Result<(), Failure> {
        Ok(())
    }
}
