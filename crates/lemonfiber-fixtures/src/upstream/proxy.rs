//! The upstream as a proxy.

use async_trait::async_trait;
use lemonfiber_ports::service::{Applied, Failure, Proxying, Route, Routing, Tls};

use super::Upstream;

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
