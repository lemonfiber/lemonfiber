//! The capabilities of the parts lemonfiber configures rather than another service
//! queries, and of its own guards: each part fills one, so a substitute can stand in for
//! it.

use lemonfiber_manifest::container;

use super::super::{Capability, Constraint, Credential, Probe, Requirement};

/// Serving the household's names.
pub(super) const PROXY_FRONT: Capability = Capability {
    name: "proxy.front",
    summary: "Answers the household's friendly names, forwarding each to the service it \
              names.",
    contract: "Serves every household name in the routing table it is applied, forwarding each \
               to the upstream the table names and serving it over TLS with the certificate \
               the table gives it or one it obtains itself, and answers a name asked for over \
               plain HTTP by sending the caller to HTTPS at the same name. It holds nothing of \
               the household's beyond the table, so it answers anybody.",
    probes: &[Probe {
        id: "redirects",
        title: "A name asked for over plain HTTP is sent to HTTPS",
        asks: "a read of a name it serves, over plain HTTP, presenting nothing",
        why: "A redirect is, like a refusal, an answer no port proxy produces: only a server \
              that routes by name sends a caller elsewhere. It is also all a proxy serving \
              every name over TLS answers on plain HTTP, whatever the name and the path.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[301, 308],
            body: &[],
        },
    }],
    listens: true,
};

/// Showing the household's dashboard.
pub(super) const DASHBOARD_SHOW: Capability = Capability {
    name: "dashboard.show",
    summary: "Shows the household one page of the services it has, grouped, each linked.",
    contract: "Shows the panel list it is applied: groups of entries, each with a name, an \
               icon, a link and a description, and where given a reachability check and a \
               widget, beside the page widgets it is given. It answers the list it shows to \
               anybody on the household network, and no entry or widget on it reads a \
               credential, so the page holds nothing that administers a service.",
    probes: &[Probe {
        id: "shows",
        title: "It answers with the groups of entries it shows",
        asks: "a read of the panel list it shows, presenting nothing",
        why: "A dashboard is a page anybody in the household may open, so the list it shows is \
              answerable without a credential, and the shape of that answer is what tells a \
              dashboard from an empty port. A fresh one may show no entry yet, and its answer \
              still reads as a list.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[200],
            body: &[
                Constraint::Json,
                Constraint::JsonHasKeys,
                Constraint::JsonTypes,
                Constraint::JsonArrayMin,
            ],
        },
    }],
    listens: true,
};

/// Writing quality profiles into the curators.
pub(super) const QUALITY_SYNC: Capability = Capability {
    name: "quality.sync",
    summary: "Writes the quality profiles lemonfiber chooses into each curator's own \
              configuration.",
    contract: "Writes the quality profiles it is applied into each curator it is given, at \
               that curator's address and with its key, on a schedule of its own, and says \
               what it last applied. It listens on nothing: no request reaches it, so it is \
               demonstrated by running its own program.",
    probes: &[Probe {
        id: "runs",
        title: "Its own program runs and says what it is",
        asks: "a run of the image's own program asking which release it is",
        why: "Nothing can ask it anything over the network, because it listens on nothing. Its \
              own program, run once from the pinned image with no network, is what can be \
              asked, and a program that names itself has started as the service will.",
        credential: Credential::None,
        requires: Requirement::Run {
            exit: &[0],
            body: &[Constraint::StdoutStartsWith],
        },
    }],
    listens: false,
};

/// Extracting archived releases and handing them back.
pub(super) const ARCHIVE_EXTRACT: Capability = Capability {
    name: "archive.extract",
    summary: "Extracts archived releases where they land and hands each back to the curator \
              that asked for it.",
    contract: "Watches each curator's download folder it is given, extracts the archived \
               releases it finds there, and tells that curator, at the address and with the \
               key it was given for it, that the extraction is ready to import. It answers \
               what it is extracting now to anybody who asks from inside the stack, and that \
               answer names no key.",
    probes: &[Probe {
        id: "working",
        title: "It answers with what it is working on",
        asks: "a read of what it is extracting now, presenting nothing",
        why: "It holds a key to every curator it hands back to, and none of them is in this \
              answer; what is, is the work it has in hand, which a fresh one answers with \
              nothing in hand and an emptied container cannot answer at all.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[200],
            body: &[
                Constraint::Json,
                Constraint::JsonHasKeys,
                Constraint::JsonTypes,
                Constraint::ContentType,
                Constraint::BodyStartsWith,
            ],
        },
    }],
    listens: true,
};

/// Declining an invitation on the invitee's word.
pub(super) const INVITE_DECLINE: Capability = Capability {
    name: "invite.decline",
    summary: "Lets an invited person refuse an invitation, and switches their account off \
              when they do.",
    contract: "Holds the media server key it switches an invitation off with, and the \
               invitations it is applied, each a token's hash, an account and when it lapses. \
               It serves the page where an invitee declines, and on a decline of an \
               invitation it holds switches that one account off on the media server. It \
               answers which key it holds by its fingerprint, and never the key.",
    probes: &[Probe {
        id: "held",
        title: "It answers which key it holds, and not the key",
        asks: "a read of the key it holds, presenting nothing",
        why: "Whether a key is in place is what has to be known before an invitation can be \
              declined at all, and its fingerprint says so without handing anybody the key. A \
              fresh one holds none and says that.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[200],
            body: &[
                Constraint::Json,
                Constraint::JsonHasKeys,
                Constraint::JsonTypes,
            ],
        },
    }],
    listens: true,
};

/// Guarding the media server's bytes.
pub(super) const STREAM_GUARD: Capability = Capability {
    name: container::STREAM_GUARD,
    summary: "Guards the media server's bytes, passing only what the media server says a \
              session may have.",
    contract: "Stands in front of the media server and enforces the guard it is applied: the \
               media server filler's declaration verbatim, the one proxy address it trusts to \
               name a client, and where it asks the core about a live-stream pass. On each byte \
               path the declaration names it asks the media server the declared question with \
               the session the request presents, and passes the bytes only on a yes; it \
               refuses, without asking anything, every path the declaration refuses and every \
               request naming an item in a shape no declared question can be asked about. It \
               answers the guard it enforces, all of it, and presents the certificate it is \
               handed.",
    probes: &[Probe {
        id: "refuses",
        title: "A request it cannot ask about is refused without asking",
        asks: "a request for an item's bytes in a shape no declared question can be asked \
               about, or for a path the declaration refuses, presenting nothing",
        why: "A guard that passed what it could not ask about would hand out bytes nobody was \
              asked whether a session may have. Refused before anything is asked, the answer is \
              the guard's own, which a fresh guard with nothing behind it gives as surely as \
              one in front of a library.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[403],
            body: &[],
        },
    }],
    listens: true,
};

/// Admitting the request service to the curators and the media server.
pub(super) const REQUEST_GATE: Capability = Capability {
    name: "request.gate",
    summary: "Lets the request service reach the curators and the media server through the \
              routes it is applied, and nowhere else.",
    contract: "Admits a call to a curator or the media server only on a route it is applied, \
               only where the call is on that route's list and presents a token whose hash the \
               route holds, and presents the route's own credential upstream. Until it is \
               applied routes it admits nothing. It answers each route's credential by its \
               fingerprint and never its value, and is reached by the request service alone.",
    probes: &[Probe {
        id: "guarded",
        title: "A call presenting no admitted token is refused",
        asks: "a call to a curator or the media server through it, presenting no token",
        why: "The gate holds credentials that administer every service it routes to, and its \
              whole job is that a caller without an admitted token reaches none of them. \
              Before it is applied routes it admits nothing, which is the same refusal.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[401, 403],
            body: &[],
        },
    }],
    listens: true,
};
