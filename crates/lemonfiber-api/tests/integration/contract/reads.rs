//! The reads the contract lists, held to what asking each one answers with.

use lemonfiber_api::contract::Contract;
use lemonfiber_core::model::kind;

use crate::reading::{asked, running, stack, world};

/// Each read asked as plainly as it can be, and once for each way it forks.
const ASKING: &[&str] = &[
    "",
    "?form=library",
    "?key=DATA_ROOT",
    "?word=seeding",
    "?member=someone",
    "?term=Sintel",
    "?what=stack",
    "?what=self",
];

/// The kind of each envelope an answer carries: one, or one a line where a read answers
/// with a document per line.
fn kinds_in(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|said| said.get("kind")?.as_str().map(str::to_owned))
        .collect()
}

/// Every read answers under a kind the contract lists it under, or with a refusal.
///
/// Asked for rather than read out of the source, so what is proven is what a client
/// parses and not that the table and the list agree with each other.
#[tokio::test]
async fn every_read_answers_under_a_kind_it_is_listed_under() {
    let mut heard = 0_usize;
    for read in Contract::describe().reads.iter().filter(|read| !read.file) {
        for query in ASKING {
            let path = format!("{}{query}", read.path);
            let Some((status, body)) = asked(world(running(), stack()), &path).await else {
                continue;
            };
            for said in kinds_in(&body) {
                if said == kind::ERROR.as_str() {
                    continue;
                }
                assert!(
                    status.is_success() && read.kinds.contains(&said.as_str()),
                    "{path} answered under {said}, and is listed under {:?}",
                    read.kinds
                );
                heard += 1;
            }
        }
    }
    assert!(
        heard > 30,
        "too few reads answered to say anything: {heard}"
    );
}
