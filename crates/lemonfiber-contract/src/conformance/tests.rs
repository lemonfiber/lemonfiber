use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Failure;

use super::{cases, Expect};
use crate::Contracted;

#[test]
fn a_capability_opens_with_an_unkeyed_case_and_has_one_per_operation() {
    let serve = crate::capabilities::media::serve::capability();
    let listed = cases(&serve);

    assert_eq!(listed.len(), serve.operations.len() + 1);
    assert!(listed
        .first()
        .is_some_and(|first| first.case == "refuses-without-the-key"
            && !first.keyed
            && first.live
            && first.expect == Expect::Status(&[401])));
    assert!(listed.iter().skip(1).all(|case| case.keyed
        && !case.live
        && case.expect == Expect::Conforms
        && case.case == format!("{}-answers", case.operation.name)));
}

#[tokio::test]
async fn every_operation_reads_an_answer_as_the_client_does() {
    for capability in crate::capabilities::all() {
        for operation in &capability.operations {
            let refusing = Contracted::new(
                Fake::always(Answer::reply(418, "")),
                "http://adapter",
                "an-adapter",
                "k3y",
            );
            assert!(
                matches!(
                    operation.judged(&refusing).await,
                    Err(Failure::Refused { .. })
                ),
                "{}.{}",
                capability.name,
                operation.name
            );
        }
    }
    let serve = crate::capabilities::media::serve::capability();
    let signing_out = serve
        .operations
        .iter()
        .find(|operation| operation.name == "sign_out");
    let answering = Contracted::new(
        Fake::always(Answer::reply(204, "")),
        "http://adapter",
        "an-adapter",
        "k3y",
    );
    match signing_out {
        Some(operation) => assert!(operation.judged(&answering).await.is_ok()),
        None => unreachable!("media.serve signs a device out"),
    }
}
