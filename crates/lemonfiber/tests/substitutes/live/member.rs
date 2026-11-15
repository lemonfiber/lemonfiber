//! A household member asking as a member asks: through the web surface this build serves,
//! with a key scoped to them, by way of the Rust SDK.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use lemonfiber_sdk::contract::actions::Ask;
use lemonfiber_sdk::contract::reads;
use lemonfiber_sdk::{Address, Client, Credential};

use super::core::{strings_under, Core};

/// How long the web surface may take to listen.
const LISTENING_WITHIN: Duration = Duration::from_secs(60);

/// How often a wait for it looks again.
const LOOKING: Duration = Duration::from_millis(500);

/// What the member's key is minted as being for.
const PURPOSE: &str = "other";

/// The web surface, served for as long as this is held.
pub struct Served {
    /// The process serving it.
    child: Child,
    /// The port it listens on, on loopback.
    port: u16,
}

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Served {
    /// The web surface of `core`, served on a free loopback port.
    ///
    /// # Errors
    ///
    /// Where no port is free, the build cannot be started, or it never listens.
    pub fn by(core: &Core) -> Result<Self, String> {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|bound| bound.local_addr())
            .map_err(|why| why.to_string())?
            .port();
        let child = Command::new(env!("CARGO_BIN_EXE_lemonfiber"))
            .args(core.dirs())
            .args(["ui", "--no-browser", "--port", &port.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|why| why.to_string())?;
        let served = Self { child, port };
        let listening = super::waited_every(LISTENING_WITHIN, LOOKING, || {
            std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
        });
        if listening {
            Ok(served)
        } else {
            Err(format!("the web surface never listened on {port}"))
        }
    }
}

/// `member`'s request for `asked`, sent with a key the operator minted for them alone.
///
/// # Errors
///
/// Where the key is not minted, or the web surface does not take the request.
pub fn asked(
    core: &Core,
    served: &Served,
    member: &str,
    asked: &super::Asked,
) -> Result<(), String> {
    let minted = core.run(
        &[
            "key",
            "mint",
            &format!("{member}-asks"),
            "--scope",
            &format!("member:{member}"),
            "--purpose",
            PURPOSE,
        ],
        "",
    );
    let secret = strings_under(&minted.said, "secret")
        .into_iter()
        .next()
        .ok_or_else(|| format!("no key minted:\n{}", minted.printed))?;
    let seasons = asked
        .seasons
        .iter()
        .map(|season| season.parse::<u32>().map_err(|why| why.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let address = Address::new(&format!("http://127.0.0.1:{}", served.port), None)
        .map_err(|why| why.to_string())?;
    let client = Client::new(
        address,
        Credential::new(secret).map_err(|why| why.to_string())?,
    )
    .map_err(|why| why.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|why| why.to_string())?;
    let (id, unit) = match asked.title {
        super::Title::Catalogued(id) => (id.to_owned(), None),
        super::Title::Found { unit, term, named } => (
            runtime.block_on(found(&client, member, asked.kind, unit, term, named))?,
            Some(unit.to_owned()),
        ),
    };
    let ask = Ask {
        id: Some(id),
        kind: Some(asked.kind.to_owned()),
        seasons: (!seasons.is_empty()).then_some(seasons),
        unit,
        ..Ask::default()
    };
    runtime
        .block_on(client.act(&ask))
        .map(|acted| println!("asked: {acted:?}"))
        .map_err(|why| format!("{why:?}"))
}

/// The id of the candidate called `named` that `member`'s search for `term`, as a `unit`
/// of `kind`, finds first.
///
/// # Errors
///
/// Where the search is refused, or finds nothing by that name.
async fn found(
    client: &Client,
    member: &str,
    kind: &str,
    unit: &str,
    term: &str,
    named: &str,
) -> Result<String, String> {
    let search = reads::Askable {
        member: Some(member.to_owned()),
        term: Some(term.to_owned()),
        kind: Some(kind.to_owned()),
        unit: Some(unit.to_owned()),
        ..reads::Askable::default()
    };
    let page = client
        .read(&search)
        .await
        .map_err(|why| format!("{why:?}"))?
        .data;
    page.titles
        .iter()
        .find(|one| one.title.contains(named))
        .map(|one| one.id.clone())
        .ok_or_else(|| format!("no {named} among what {term} finds: {page:?}"))
}
