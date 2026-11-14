//! `subtitles.fetch` across its contract: one script run against a subtitle finder in
//! process and through the contract, which must answer alike and be told alike.

use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Subtitles, Watched};

use super::{crosses_alike, fetch};

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
    crosses_alike!(script, fetch::Adapter, ["watch"]);
}
