//! `subtitles.fetch` across its contract: one script run against a subtitle finder in
//! process and through the contract, which must answer alike and be told alike.

use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, Subtitles, Watched, Watching};

use super::{contracted, fetch, Served, Upstream};

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

/// Every operation of the capability, each answer written down.
async fn script<S: Subtitles>(finder: &S) -> Vec<String> {
    let watched = Watched {
        which: Kind::Movies,
        host: "films".to_owned(),
        port: 7878,
        api_key: "films-key".to_owned(),
    };
    vec![
        format!("{:?}", finder.watching(Kind::Tv).await),
        format!("{:?}", finder.watching(Kind::Movies).await),
        format!("{:?}", finder.watch(&watched).await),
    ]
}

#[tokio::test]
async fn a_subtitle_finder_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    let in_process = Upstream::default();
    let local = script(&in_process).await;
    let served = Served::default();
    let reached = Arc::clone(&served.upstream);
    let crossed = script(&fetch::Adapter(contracted(served))).await;
    assert_eq!(crossed, local);
    assert_eq!(reached.told(), in_process.told());
    assert_eq!(in_process.told().len(), 1);
}
