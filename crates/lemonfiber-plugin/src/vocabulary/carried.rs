//! The capabilities this generation carries, and the names it has dropped.
//!
//! Authored here and nowhere else. The prose is what a plugin author is held to,
//! so it is deliberate rather than descriptive: a contract that reads as a summary
//! of an implementation is one nobody can hold a second implementation to.
//!
//! Split from the module above it because a list of ten contracts, each with its own
//! probes, outgrew what one file may hold — not because the two are separate
//! concerns. Nothing here decides anything; it is the table the module reads.

use super::{Capability, Constraint, Credential, Probe, Removed, Requirement};

/// Every capability this generation carries, in the order the artefact lists them.
pub(super) const CARRIED: &[Capability] = &[
    Capability {
        name: "indexer.search",
        summary: "Runs a search across the indexers it holds and answers with what they \
                  returned.",
        contract: "Holds the operator's indexer accounts, accepts a search on their behalf, and \
                   answers with the releases those indexers returned. The accounts it holds, and \
                   the list of indexers they belong to, are not readable without the operator's \
                   credential.",
        probes: &[
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the indexers it holds",
                asks: "a read of the indexers configured on this service, presenting nothing",
                why: "An indexer account is an account somebody paid for. A service that would \
                      list them to the network has stopped satisfying this whether or not it \
                      still searches.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
            Probe {
                id: "indexers",
                title: "It answers with the indexers it holds",
                asks: "the same read, with the credential the operator holds",
                why: "Searching is what this is for, and the indexers it would search are the \
                      part that can be read without asking anything of a third party.",
                credential: Credential::Operator,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonAtLeast,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
        ],
    },
    Capability {
        name: "indexer.proxy",
        summary: "Fetches a page from an indexer that answers a browser rather than a client.",
        contract: "Fetches a page on another service's behalf when the site it comes from \
                   challenges a plain client. It holds nothing of the operator's and is reached \
                   only from inside the stack, so it answers an anonymous caller with its own \
                   name and version.",
        probes: &[Probe {
            id: "identifies",
            title: "It answers with its own name and version",
            asks: "a read of the service's own state, presenting nothing",
            why: "This is the one capability whose claimant deliberately holds no \
                      credential, so a refusal would be the wrong evidence. What it must show \
                      instead is that the thing behind the port is this and not an empty \
                      container.",
            credential: Credential::None,
            requires: Requirement {
                status: &[200],
                body: &[
                    Constraint::Json,
                    Constraint::JsonHasKeys,
                    Constraint::JsonTypes,
                    Constraint::JsonArrayMin,
                ],
            },
        }],
    },
    Capability {
        name: "download.usenet",
        summary: "Takes an NZB, fetches the articles it names, and reports where the finished \
                  files are.",
        contract: "Accepts an NZB from whatever asked for it, downloads what it names, and \
                   reports the path the finished files are at so a filer can move them. The \
                   queue and the provider account behind it are not readable without the \
                   operator's credential.",
        probes: &[
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the queue",
                asks: "a read of the download queue, presenting nothing",
                why: "The queue names what the household is downloading and the account paying \
                      for it. A client that would answer that to the network has stopped \
                      satisfying this.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
            Probe {
                id: "queue",
                title: "It answers with its queue",
                asks: "the same read, with the credential the operator holds",
                why: "Reporting where a finished download is requires having a queue to report \
                      from, and the queue is the part that can be read without starting one.",
                credential: Credential::Operator,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonAtLeast,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
        ],
    },
    Capability {
        name: "download.torrent",
        summary: "Takes a torrent or a magnet, exchanges with peers, and reports where the \
                  finished files are.",
        contract: "Accepts a torrent or a magnet from whatever asked for it, exchanges data with \
                   that torrent's peers, and reports the path the finished files are at. The \
                   queue is not readable without the operator's credential, and what it \
                   exchanges leaves through whatever carries its traffic rather than by a route \
                   of its own.",
        probes: &[
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the queue",
                asks: "a read of the download queue, presenting nothing",
                why: "A torrent client's queue is the one list in this stack whose disclosure \
                      has consequences outside the household, and the client is the only thing \
                      that can refuse it.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
            Probe {
                id: "queue",
                title: "It answers with its queue",
                asks: "the same read, with the credential the operator holds",
                why: "The same reason the Usenet client's queue is the readable half: it is what \
                      the service has to report from, and reading it starts nothing.",
                credential: Credential::Operator,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonAtLeast,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
        ],
    },
    Capability {
        name: "network.egress-guard",
        summary: "Carries another service's traffic out through a tunnel and stops it when the \
                  tunnel drops.",
        contract: "Carries the traffic of whatever is placed inside its network, out through a \
                   tunnel it dials, and stops carrying it the moment the tunnel is not up. It \
                   answers, from inside the stack, whether the tunnel is up — because a guard \
                   that cannot be asked is one nothing can prove is guarding.",
        probes: &[Probe {
            id: "identifies",
            title: "It answers whether the tunnel is up",
            asks: "a read of the tunnel's state, from inside the stack",
            why: "This is the one service whose failure has consequences outside the \
                      machine, and the failure it has is silent: a tunnel that dropped looks \
                      exactly like one that is up unless something asks.",
            credential: Credential::None,
            requires: Requirement {
                status: &[200],
                body: &[
                    Constraint::Json,
                    Constraint::JsonHasKeys,
                    Constraint::JsonTypes,
                    Constraint::JsonArrayMin,
                ],
            },
        }],
    },
    Capability {
        name: "library.curate",
        summary: "Holds a wanted list for one kind of media, asks for what is missing, and files \
                  what arrives.",
        contract: "Holds a list of what the household wants of one kind of media, asks an \
                   indexer for what is missing, hands the result to a download client, and files \
                   what arrives under the library's own naming. The wanted list is not readable \
                   without the operator's credential.",
        probes: &[
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the wanted list",
                asks: "a read of the wanted list, presenting nothing",
                why: "A curating service holds every key the stack was given and can be made to \
                      download anything. Its API answering the network at all is the failure; \
                      whether the list is interesting is beside the point.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
            Probe {
                id: "wanted",
                title: "It answers with what it is holding",
                asks: "the same read, with the credential the operator holds",
                why: "Holding a wanted list is the half of this that can be demonstrated without \
                      waiting for something to be released.",
                credential: Credential::Operator,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonAtLeast,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
        ],
    },
    Capability {
        name: "subtitles.fetch",
        summary: "Finds subtitles for what is already filed and puts them beside it.",
        contract: "Reads what the library already holds, finds subtitles for it in the languages \
                   the operator asked for, and writes them beside the media. Its own \
                   configuration and the provider accounts in it are not readable without the \
                   operator's credential.",
        probes: &[
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused its configuration",
                asks: "a read of the service's own state, presenting nothing",
                why: "It holds subtitle-provider accounts and a path into the library. Neither \
                      is something the network may read.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
            Probe {
                id: "status",
                title: "It answers with its own state",
                asks: "the same read, with the credential the operator holds",
                why: "What it is configured to fetch, and for what, is the part that can be read \
                      without a provider being asked anything.",
                credential: Credential::Operator,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
        ],
    },
    Capability {
        name: "media.serve",
        summary: "Serves the filed library to a player, with its own catalogue of what it holds.",
        contract: "Holds a catalogue of what is in the library and serves it to a player over \
                   HTTP. The catalogue is not readable without a credential — a library server \
                   that answers an anonymous caller is publishing the household's collection to \
                   every device on the network.",
        probes: &[
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the catalogue",
                asks: "a read of the catalogue of what it holds, presenting nothing",
                why: "This is the probe the whole vocabulary is for. A media server bound to the \
                      household network that stops refusing this keeps answering its health \
                      probe, keeps appearing green, and has published somebody's library. \
                      Nothing else would notice.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
            Probe {
                id: "catalogue",
                title: "It answers with the catalogue of what it holds",
                asks: "the same read, with the credential the operator holds",
                why: "Serving a library means having a catalogue of it. A service that refuses \
                      everybody including the operator is not serving anything.",
                credential: Credential::Operator,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonAtLeast,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
        ],
    },
    Capability {
        name: "identity.source",
        summary: "Answers, for the services that ask, whether a person is who they say they are.",
        contract: "Holds the household's accounts and answers another service's question about \
                   whether a given person is one of them. It tells an anonymous caller enough to \
                   begin signing in — which server this is — and nothing about who has an \
                   account here.",
        probes: &[
            Probe {
                id: "identifies",
                title: "It tells an anonymous caller which server this is",
                asks: "a read of the public server information a client needs before it can sign \
                       in",
                why: "Something has to be answerable before authentication, or nothing could \
                      ever authenticate. What that is is the server's identity, and it is the \
                      part with nothing of the household's in it.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the accounts",
                asks: "a read of the accounts it holds, presenting nothing",
                why: "The line between the two probes is the whole of this capability: which \
                      server, to anybody; who is on it, to nobody.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
        ],
    },
    Capability {
        name: "request.intake",
        summary: "Takes a household member's request for something not yet held and hands it on.",
        contract: "Lets a member of the household ask for something the library does not have, \
                   decides whether they may, and hands the request to whatever acquires it. Its \
                   own state is answerable to anybody; what has been requested, and by whom, is \
                   not.",
        probes: &[
            Probe {
                id: "identifies",
                title: "It answers with its own state",
                asks: "a read of the service's own version and state, presenting nothing",
                why: "This is the one service a household member reaches without being an \
                      operator, so it has a front door that answers before anybody signs in.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[200],
                    body: &[
                        Constraint::Json,
                        Constraint::JsonHasKeys,
                        Constraint::JsonTypes,
                        Constraint::JsonArrayMin,
                    ],
                },
            },
            Probe {
                id: "guarded",
                title: "An anonymous caller is refused the requests",
                asks: "a read of what has been requested, presenting nothing",
                why: "What a household asked for is the household's. A request list readable by \
                      the network is a list of what everybody in the house is waiting to watch.",
                credential: Credential::None,
                requires: Requirement {
                    status: &[401, 403],
                    body: &[],
                },
            },
        ],
    },
];

/// Every name a published generation carried and this one does not.
///
/// Empty, and it has to be able to be: nothing has been removed yet. A name that
/// goes is recorded here with the generation it went in, so a manifest naming it is
/// told *it was removed in generation N* rather than *no such capability*.
pub(super) const REMOVED: &[Removed] = &[];
