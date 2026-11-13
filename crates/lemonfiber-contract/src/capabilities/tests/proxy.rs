//! `proxy.front` across its contract: one script run against a proxy in process and
//! through the contract, which must answer alike and be told alike.

use lemonfiber_ports::service::{Proxying, Route, Routing, Tls};

use super::{crosses_alike, front};

/// Every operation of the capability, each answer written down.
async fn script<P: Proxying>(proxy: &P) -> Vec<String> {
    let table = Routing {
        tls: Tls::Automatic,
        routes: vec![
            Route {
                name: "requests.home.local".to_owned(),
                host: "requests".to_owned(),
                port: 5055,
            },
            Route {
                name: "books.home.local".to_owned(),
                host: "books".to_owned(),
                port: 8083,
            },
        ],
    };
    vec![
        format!("{:?}", proxy.apply(&table).await),
        format!("{:?}", proxy.routing().await),
        format!("{:?}", proxy.reload().await),
    ]
}

#[tokio::test]
async fn a_proxy_answers_and_is_told_through_its_contract_as_it_is_in_process() {
    crosses_alike!(script, front::Adapter, ["apply", "routing", "reload"]);
}
