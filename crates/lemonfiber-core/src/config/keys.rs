//! Every name lemonfiber writes into the environment file, and the list of them.
//!
//! Apart from the rest of `config` because they are a different kind of thing: the
//! module around them decides what a recorded value comes to, and these decide
//! nothing at all — they are the vocabulary that reading and writing both quote.
//! Holding them here is also what keeps that module inside one sitting as the stack
//! gains settings, which is the one thing about it certain to keep happening.
//!
//! The keys the reachability switches own live beside those switches and are
//! imported here, because the list below has to name every setting lemonfiber has —
//! and a list that names all but seven is one nothing can be held to.

use super::reaching::{
    OFFLINE_KEY, REACH_GUIDES_KEY, REACH_HOUSEHOLD_KEY, REACH_INDEXER_KEY, REACH_REGISTRY_KEY,
    REACH_UPDATES_KEY, REACH_USENET_KEY,
};

/// The setting recording that a Usenet provider is configured.
///
/// lemonfiber's own answers live in the same file as the stack's settings,
/// under a prefix of its own. Compose is handed the file and will pass these to
/// containers that ask for them, which nothing does — the alternative, a second
/// configuration file, would mean an operator keeping two things in step.
pub const USENET_KEY: &str = "LEMONFIBER_USENET";

/// The setting recording that a VPN and torrent client are configured.
pub const TORRENT_KEY: &str = "LEMONFIBER_TORRENT";

/// The IP-echo service the leak check asks each container for its public
/// address.
///
/// A plain endpoint that answers with the caller's address and nothing else, so
/// the check runs `wget` against it from inside the containers rather than
/// lemonfiber reaching the network on their behalf.
pub const DEFAULT_IP_ECHO: &str = "https://ifconfig.me";

/// A second, independent source asked alongside the first.
///
/// Two rather than one because the entire leak verdict is a comparison against
/// what these report: a single source that is misconfigured, cached behind a
/// proxy, or simply wrong returns a plausible address, and the check says `pass`
/// while traffic leaves in the clear. Two that disagree cannot both be trusted,
/// and saying so is the only honest answer available.
pub const SECOND_IP_ECHO: &str = "https://icanhazip.com";

/// The setting naming the IP-echo service, or switching leak detection off.
pub const IP_ECHO_KEY: &str = "LEMONFIBER_IP_ECHO";

/// The setting that switches the plain-language explanations off.
///
/// On unless it is explicitly turned off, which is the right way round: somebody
/// meeting this vocabulary for the first time does not know there is a setting to
/// look for, and somebody who finds the explanations patronising knows exactly what
/// they want to stop.
pub const EXPLANATIONS_KEY: &str = "LEMONFIBER_EXPLANATIONS";

/// The setting that lets a start at a login happen while this machine is on its
/// battery.
///
/// Off unless it is explicitly turned on, which is the opposite way round from the
/// explanations and for the opposite reason: a media stack started on a battery
/// empties one in an afternoon, and an operator who wants that has a reason for it
/// while an operator who gets it by default has an afternoon ruined. A desktop with
/// no battery to read is unaffected either way — nothing here holds back a machine
/// that could not say where its power comes from.
pub const AUTOSTART_ON_BATTERY_KEY: &str = "LEMONFIBER_AUTOSTART_ON_BATTERY";

/// The hours the operator does not want waking for, as `HH:MM-HH:MM`.
///
/// Read in the zone `TZ` names, which is the same zone the stack hands every container,
/// so the window lands on the household's own evening rather than on UTC's.
pub const QUIET_HOURS_KEY: &str = "LEMONFIBER_QUIET_HOURS";

/// The zone the stack runs in.
///
/// Not lemonfiber's own setting — it is the stack's, handed to every container by the
/// compose file, and read here so a window means the same hour inside and out.
pub const ZONE_KEY: &str = "TZ";

/// The zone the stack's compose file falls back to when nothing names one.
///
/// Kept the same as `_common.yml`'s `${TZ:-Europe/Amsterdam}` deliberately: a window
/// read in a different zone from the containers it is about would be quiet at the
/// wrong hour and agree with nothing.
pub const DEFAULT_ZONE: &str = "Europe/Amsterdam";

/// A Compose file layered over the stack's own.
///
/// Written by standing lemonfiber beside a setup already here: it maps each service to
/// a port nothing else is using, so a second copy can be evaluated without moving the
/// first out of the way.
pub const OVERLAY_KEY: &str = "LEMONFIBER_OVERLAY";

/// The Compose project lemonfiber manages.
///
/// Its own by default. Set only by adopting a setup that was already here, which is
/// what makes lemonfiber a control surface over somebody else's stack rather than a
/// second stack beside it.
pub const PROJECT_KEY: &str = "LEMONFIBER_PROJECT";

/// The admin services the operator has said out loud they meant to expose.
///
/// The diagnosis offers to stop reporting an exposed admin surface "if you meant
/// it", and until this existed there was nowhere to say so — a remedy offering an
/// action nobody could take. This is where it is taken.
///
/// A name and a reason, because a name on its own records that somebody clicked
/// past a warning and nothing about why. The reason is what a person reading this
/// file in a year, or reading a support bundle, is actually served by, and it is the
/// same standard the displayed-settings register is held to.
///
/// `sonarr=it is behind the reverse proxy I already run,radarr=the same`
pub const EXPOSED_KEY: &str = "LEMONFIBER_EXPOSED";

/// The areas the operator has told lemonfiber to leave alone.
///
/// Pairs of `area=reason`, comma-separated, the same shape the register of
/// deliberately exposed services takes: it is a decision, and a decision recorded
/// without why is a note that somebody once wanted something and nothing about what
/// they knew. Both halves have to be there; how good the reason is is not judged,
/// which is where the two registers part company and why.
///
/// What an area covers, and what a declaration does and does not stop, is written in
/// [`crate::unmanaged`] — including the one write it deliberately does not reach.
///
/// `config/recyclarr=my own profiles live in here,sonarr=I tune this one by hand`
pub const UNMANAGED_KEY: &str = "LEMONFIBER_UNMANAGED";

/// The setting naming where downloads and the library are kept.
///
/// The one location the storage contract rests on: the compose driver mounts it
/// and the storage checks probe it. It is the stack's own setting rather than
/// one of lemonfiber's, so it carries no prefix.
pub const DATA_ROOT_KEY: &str = "DATA_ROOT";

/// The user id the service containers run as.
pub const PUID_KEY: &str = "PUID";

/// The group id the service containers run as.
pub const PGID_KEY: &str = "PGID";

/// The VPN provider the tunnel connects through.
///
/// Read by the port-forward check for one purpose: to name a provider's known
/// trap when port forwarding was asked for but no port arrived. It never decides
/// whether the tunnel itself works.
pub const VPN_PROVIDER_KEY: &str = "VPN_PROVIDER";

/// The setting recording whether server-side port forwarding was asked for.
///
/// Only some providers offer it, so a stack on one that does not leaves this off,
/// and the port-forward check reads that as "does not apply" rather than a fault.
pub const VPN_PORT_FORWARDING_KEY: &str = "VPN_PORT_FORWARDING";

/// The setting selecting how Jellyfin is served: in a container or on the host.
///
/// A single switch is the whole of the difference between the two modes — the
/// compose stack drops one service and the URLs change, nothing more. Absent
/// where the operator runs no media server at all.
pub const JELLYFIN_MODE_KEY: &str = "JELLYFIN_MODE";

/// The address the household's own links are pointed at.
///
/// The stack's, and the one thing in it that says where this machine is reached
/// from another device in the house. It ships pointed at this machine and nowhere
/// else, which is the right default for a machine nobody has told where it is and
/// the wrong address to hand anybody.
pub const HOUSEHOLD_HOST_KEY: &str = "HOMEPAGE_VAR_LAN_HOST";

/// The service the operator chose to send the household to, by the id the stack
/// declares it under.
///
/// lemonfiber's own answer rather than the stack's, so it carries lemonfiber's
/// prefix. What a name here may be is [`crate::door`]'s to decide and not this
/// module's: the question is which tier a service is published on, and the answer
/// belongs where the rest of that reasoning already lives.
pub const FRONT_DOOR_KEY: &str = "LEMONFIBER_FRONT_DOOR";

/// The base URL of the indexer the operator gave at setup.
pub const INDEXER_URL_KEY: &str = "INDEXER_URL";

/// The indexer's API key. A secret, held here the way the stack holds its others.
pub const INDEXER_APIKEY_KEY: &str = "INDEXER_APIKEY";

/// Whether the indexer credential was proven against the live service before it
/// was kept.
///
/// Off records a credential the operator chose to proceed with unverified, so a
/// later diagnosis can point at it rather than trusting it silently.
pub const INDEXER_VALIDATED_KEY: &str = "INDEXER_VALIDATED";

/// The Usenet provider's hostname.
pub const PROVIDER_HOST_KEY: &str = "USENET_HOST";

/// The port the Usenet provider answers NNTP on.
pub const PROVIDER_PORT_KEY: &str = "USENET_PORT";

/// The Usenet account username.
pub const PROVIDER_USER_KEY: &str = "USENET_USER";

/// The Usenet account password. A secret, held the way the stack holds its others.
pub const PROVIDER_PASS_KEY: &str = "USENET_PASS";

/// Whether the Usenet connection uses TLS.
pub const PROVIDER_TLS_KEY: &str = "USENET_TLS";

/// Whether the Usenet login was proven before it was kept — off records one the
/// operator chose to proceed with unverified.
pub const PROVIDER_VALIDATED_KEY: &str = "USENET_VALIDATED";

/// The environment key holding qBittorrent's web UI password.
///
/// Seeding generates this password and records it here, because the
/// forwarded-port push authenticates to qBittorrent with it — the one credential
/// lemonfiber mints and writes rather than reads from a service.
pub const QBITTORRENT_PASSWORD_KEY: &str = "QBITTORRENT_PASSWORD";

/// The environment key holding the Jellyfin administrator password.
///
/// Jellyfin generates no key on disk and asks for an account to be created, so
/// seeding mints this password, sets it by driving Jellyfin's own first-run
/// setup, and records it here — the same shape as qBittorrent's, and the
/// credential the Seerr identity wiring reads back. The account name is
/// [`JELLYFIN_ADMIN_USER`].
pub const JELLYFIN_ADMIN_PASSWORD_KEY: &str = "JELLYFIN_ADMIN_PASSWORD";

/// The environment key holding the listening server's first-account password.
///
/// The same shape as Jellyfin's and for the same reason: the service starts with no
/// account and writes no key, so lemonfiber mints this, creates the account with it,
/// and keeps it. The token its dashboard panel uses is derived from this on demand
/// rather than recorded beside it — the service hands back the same one every
/// sign-in, so a second record would be a second copy of the same secret.
pub const AUDIOBOOKSHELF_PASSWORD_KEY: &str = "AUDIOBOOKSHELF_PASSWORD";

/// The environment key holding the book \*arr's API key.
///
/// The one credential in the stack that lemonfiber mints and the *service* adopts,
/// rather than one it mints and keeps: given this in its environment the service takes
/// it verbatim instead of generating its own, which is what lets both sides know it
/// without reading the database it would otherwise keep it in.
pub const BINDERY_API_KEY: &str = "BINDERY_API_KEY";

/// The name of the listening server's first account.
pub const AUDIOBOOKSHELF_USER: &str = "admin";

/// The name of the Jellyfin administrator account lemonfiber creates at setup — the
/// household's own account, one source of truth for the name so the first-run driver
/// creates it, the Seerr identity wiring signs in with it, and a trace's library read
/// authenticates as it, all under the same name.
pub const JELLYFIN_ADMIN_USER: &str = "admin";

/// The account name qBittorrent's web UI is reached under.
///
/// One source of truth for the name, so the client that logs in, the download-client
/// registration that hands it to an \*arr, and the dashboard's own widget all present
/// the same one. Separate from Jellyfin's although both spell it `admin`: they are two
/// services, and either may change without the other.
pub const QBITTORRENT_USER: &str = "admin";

/// The environment key holding the account name qBittorrent is reached under.
///
/// The dashboard reads both halves of the credential from the environment and has no
/// default for this one, so a name that is never written leaves its widget unable to
/// authenticate.
pub const QBITTORRENT_USERNAME_KEY: &str = "QBITTORRENT_USERNAME";

/// Every setting lemonfiber names.
///
/// Declared rather than discovered. The guard that checks nothing is displayed with its
/// value without a reason written down needs to know which settings exist, and until this
/// list it knew only the ones the embedded stack declares — which is every namespace
/// except lemonfiber's own, the one where setup collects a Usenet password and an indexer
/// key. A guard that cannot see the settings that matter most is a guard that reports on
/// somebody else's file.
///
/// Reading them back out of this module's source would answer the same question and go
/// stale the first time somebody writes a name inline, so the list is the declaration and
/// a test holds the writer to it: what [`crate::wizard::Wizard::plan`] produces may only
/// be named here.
///
/// Seeding is not held to this list. What it records is partly computed — a key is
/// published under the id of the service it was read from — so the names cannot be
/// declared ahead of knowing the stack.
pub const SETTINGS: &[&str] = &[
    USENET_KEY,
    TORRENT_KEY,
    IP_ECHO_KEY,
    OFFLINE_KEY,
    REACH_REGISTRY_KEY,
    REACH_GUIDES_KEY,
    REACH_INDEXER_KEY,
    REACH_USENET_KEY,
    REACH_HOUSEHOLD_KEY,
    REACH_UPDATES_KEY,
    EXPLANATIONS_KEY,
    AUTOSTART_ON_BATTERY_KEY,
    PROJECT_KEY,
    OVERLAY_KEY,
    QUIET_HOURS_KEY,
    EXPOSED_KEY,
    UNMANAGED_KEY,
    DATA_ROOT_KEY,
    PUID_KEY,
    PGID_KEY,
    VPN_PROVIDER_KEY,
    VPN_PORT_FORWARDING_KEY,
    JELLYFIN_MODE_KEY,
    INDEXER_URL_KEY,
    INDEXER_APIKEY_KEY,
    INDEXER_VALIDATED_KEY,
    PROVIDER_HOST_KEY,
    PROVIDER_PORT_KEY,
    PROVIDER_USER_KEY,
    PROVIDER_PASS_KEY,
    PROVIDER_TLS_KEY,
    PROVIDER_VALIDATED_KEY,
    QBITTORRENT_PASSWORD_KEY,
    JELLYFIN_ADMIN_PASSWORD_KEY,
    AUDIOBOOKSHELF_PASSWORD_KEY,
    BINDERY_API_KEY,
    FRONT_DOOR_KEY,
];
