//! `proxy.front` across its contract: one script run against a proxy in process and
//! through the contract, which must answer alike and be told alike.

use async_trait::async_trait;
use lemonfiber_ports::service::{Applied, Failure, Proxying, Route, Routing, Tls};

use super::{crosses_alike, front, Upstream};

#[async_trait]
impl Proxying for Upstream {
    async fn apply(&self, table: &Routing) -> Result<Applied, Failure> {
        self.tell(format!("apply {table:?}"));
        Ok(Applied::OnRestart)
    }
    async fn routing(&self) -> Result<Routing, Failure> {
        self.tell("routing".to_owned());
        Ok(Routing {
            tls: Tls::Files {
                certificate: "certificate.pem".to_owned(),
                key: "key.pem".to_owned(),
            },
            routes: vec![Route {
                name: "watch.home.local".to_owned(),
                host: "guard".to_owned(),
                port: 8096,
            }],
        })
    }
    async fn reload(&self) -> Result<Applied, Failure> {
        self.tell("reload".to_owned());
        Ok(Applied::Now)
    }
}

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
