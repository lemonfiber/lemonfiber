# `lemonfiber` — error codes

Rendered from `contract/codes.json`, which is generated from the registry every code is
declared in. Run `just codes` to rewrite both.

Every code lemonfiber can raise, and nothing else. A code is a family and a number,
it is never recycled, and it is the token to search for. What each one means, and
what to do about it, is in `contract/codes.json` and written for operators at
<https://docs.lemonfiber.app/fixing/every-error-by-code/>.

## `ACK` — answering a warning

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `ACK-1` | `NOT_WARNED` | error | 1 | 500 | 0.6.0 | Raised when an answer names something nothing is warning about. |

## `ADMIT` — who the web interface lets in

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `ADMIT-1` | `TOO_SHORT` | error | 1 | 500 | 0.10.0 | Raised when a password is too short to stand in front of this. |
| `ADMIT-2` | `NO_SALT` | error | 1 | 500 | 0.10.0 | Raised when this machine will not supply the salt a record is made with. |
| `ADMIT-3` | `MISTYPED` | error | 1 | 500 | 0.10.0 | Raised when the two answers were not the same word. |
| `ADMIT-4` | `NOT_ADMITTED` | error | 1 | 403 | 0.17.0 | Raised when a request carried no token, session or key this run admits. |
| `ADMIT-5` | `ELSEWHERE` | error | 1 | 403 | 0.17.0 | Raised when a request said it came from somewhere this server is not. |
| `ADMIT-6` | `NOT_YOURS` | warning | 1 | 403 | 0.17.0 | Raised when an account asked for something that is not its to ask for. |
| `ADMIT-7` | `UNCONFIRMED` | warning | 1 | 403 | 0.17.0 | Raised when the media server could not say whether an account is still one. |
| `ADMIT-8` | `NOT_THE_PASSWORD` | error | 1 | 401 | 0.17.0 | Raised when the password offered at the door was wrong, or none is set. |
| `ADMIT-9` | `TOO_MANY_ATTEMPTS` | warning | 1 | 429 | 0.17.0 | Raised when the door has been given too many wrong passwords lately. |
| `ADMIT-10` | `NOT_A_PASSWORD` | error | 1 | 400 | 0.17.0 | Raised when what was offered at the door is not a password. |
| `ADMIT-11` | `KEY_IN_THE_CLEAR` | error | 1 | 403 | 0.17.0 | Raised when a key arrived from another machine over a connection its pin does not verify. |
| `ADMIT-12` | `NOT_FOR_A_KEY` | warning | 1 | 403 | 0.17.0 | Raised when a key asked for something its scope does not reach. |

## `ASK` — putting a request to the web surface

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `ASK-1` | `NO_SUCH_ACTION` | error | 1 | 404 | 0.17.0 | Raised where no action goes by the name that was asked for. |
| `ASK-2` | `MISSING_ARGUMENT` | error | 1 | 400 | 0.17.0 | Raised where an action was not given an argument it needs. |
| `ASK-3` | `UNRECOGNISED_ARGUMENT` | error | 1 | 400 | 0.17.0 | Raised where an argument was given a value that names nothing. |
| `ASK-4` | `UNWANTED_ARGUMENT` | error | 1 | 400 | 0.17.0 | Raised where an action was given an argument its command has nowhere to put. |
| `ASK-5` | `ARGUMENTS_TOGETHER` | error | 1 | 400 | 0.17.0 | Raised where two arguments that each name a different request arrived together. |
| `ASK-6` | `NOT_ARGUMENTS` | error | 1 | 400 | 0.17.0 | Raised where the body of an action is not arguments it can read. |
| `ASK-7` | `NO_SUCH_JOB` | error | 1 | 404 | 0.17.0 | Raised where a job was asked about that this run did not start. |
| `ASK-8` | `NOT_AN_ANSWER` | error | 1 | 400 | 0.17.0 | Raised where the body of a setup step is not an answer it can read. |
| `ASK-9` | `NO_ENDPOINT` | error | 1 | 404 | 0.17.0 | Raised where a path under the endpoints is one no endpoint answers. |
| `ASK-10` | `WRONG_METHOD` | error | 1 | 405 | 0.17.0 | Raised where an endpoint was asked with a method it does not answer. |
| `ASK-11` | `NOT_A_KEY_REQUEST` | error | 1 | 400 | 0.17.0 | Raised where the body of a mint is not a key's name, scope, purpose and the password. |
| `ASK-12` | `NOT_AN_IDEMPOTENCY_KEY` | error | 1 | 400 | 0.18.0 | Raised where an action's `Idempotency-Key` is not one to 255 visible characters, or is given more than once. |
| `ASK-13` | `IDEMPOTENCY_KEY_REUSED` | error | 1 | 400 | 0.18.0 | Raised where an `Idempotency-Key` already sent with one action and its arguments is sent with another. |

## `BACKUP` — capturing your configuration

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `BACKUP-1` | `NO_ROOM` | error | 1 | 500 | 0.3.0 | Raised when there is not enough room to write a backup. |
| `BACKUP-2` | `NOT_WRITTEN` | error | 1 | 500 | 0.3.0 | Raised when the backup archive could not be written. |
| `BACKUP-3` | `NOT_MEASURED` | error | 1 | 500 | 0.3.0 | Raised when the room for a backup could not be measured. |
| `BACKUP-4` | `STILL_RUNNING` | error | 1 | 500 | 0.9.0 | Raised when a capture could not be shown that nothing is writing to a database. |
| `BACKUP-5` | `NOWHERE_TO_KEEP` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to keep an archive. |
| `BACKUP-6` | `NOWHERE_KEPT` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to look for archives. |
| `BACKUP-7` | `NOT_LISTED` | error | 1 | 500 | 0.9.0 | Raised when the directory the archives are kept in could not be read. |

## `BIND` — where the stack is actually listening

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `BIND-1` | `BEYOND_LOOPBACK` | error | 1 | 500 | 0.10.0 | Raised when a service the stack calls admin answers somewhere off this machine. |
| `BIND-2` | `AROUND_THE_FIREWALL` | warning | 1 | 500 | 0.10.0 | Raised where a published port is reached without the host's own firewall rules being consulted. |
| `BIND-3` | `DELIBERATE` | warning | 1 | 500 | 0.10.0 | Raised where an admin service answers off this machine and the operator has written down that they meant it to. |

## `BUNDLE` — the support bundle

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `BUNDLE-1` | `BUNDLE_LEAK` | critical | 1 | 500 | 0.7.0 | Raised when a bundle would still hold something that reads as a credential. |
| `BUNDLE-2` | `BUNDLE_NO_ROOM` | error | 1 | 500 | 0.7.0 | Raised when there is not enough room to write a bundle. |
| `BUNDLE-3` | `BUNDLE_UNWRITTEN` | error | 1 | 500 | 0.7.0 | Raised when the archive could not be written. |
| `BUNDLE-4` | `BUNDLE_UNCONFIRMED` | error | 1 | 500 | 0.7.0 | Raised when a setting was asked to be shown as it is without that being confirmed. |
| `BUNDLE-5` | `BUNDLE_NO_MARKS` | error | 1 | 500 | 0.7.0 | Raised when the machine can offer no randomness to derive stand-ins from. |
| `BUNDLE-6` | `NOWHERE_TO_KEEP` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to keep a bundle. |
| `BUNDLE-7` | `NOWHERE_HELD` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to look for a bundle it kept. |
| `BUNDLE-8` | `NOT_HELD` | error | 1 | 404 | 0.9.0 | Raised when a name does not name one of the bundles this run kept. |

## `CONFIG` — your settings

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `CONFIG-1` | `CONFIG_UNREADABLE` | error | 5 | 500 | 0.1.0 | Raised when configuration exists and cannot be read. |
| `CONFIG-2` | `CONFIG_NOT_WRITTEN` | error | 1 | 500 | 0.1.0 | Raised when configuration cannot be written. |
| `CONFIG-3` | `CONFIG_NOWHERE` | error | 1 | 500 | 0.1.0 | Raised when there is nowhere to keep configuration. |
| `CONFIG-4` | `CREDENTIALS_EXPOSED` | warning | 1 | 500 | 0.13.0 | Raised when a file holding a credential can be read by more than its owner. |
| `CONFIG-5` | `CONFIG_TOO_NEW` | error | 1 | 500 | 0.14.0 | Raised when configuration was written by a newer lemonfiber. |
| `CONFIG-6` | `CONFIG_SPANS_LINES` | error | 1 | 500 | 0.16.0 | Raised when a setting's key or value spans more than one line. |
| `CONFIG-7` | `AGGREGATOR_EXPOSED` | warning | 1 | 500 | 0.17.0 | Raised when the Usenet indexer aggregator answers a read of its whole configuration to a caller presenting nothing, or will not say whether it does. |

## `CRED` — credentials a service refuses

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `CRED-1` | `CREDENTIAL_REJECTED` | error | 1 | 500 | 0.2.0 | Raised when a service answers and refuses the credential it generated itself. |
| `CRED-2` | `INDEXER_REJECTED` | error | 1 | 500 | 0.2.0 | Raised when the indexer answers and refuses the key it was given. |
| `CRED-3` | `INDEXER_LIMITED` | warning | 1 | 500 | 0.2.0 | Raised when the indexer authenticates the key but cannot serve it right now. |

## `DECLINE` — the service that answers an invitation's decline

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `DECLINE-1` | `UNEXPLAINED` | warning | 1 | 500 | 0.17.0 | Raised when the decline service's key was used later than anything the service recorded doing with it. |

## `DIAG` — narrowing a diagnosis

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `DIAG-1` | `NO_SUCH_CHECK` | error | 1 | 500 | 0.9.0 | Raised when a run is narrowed to a check nothing in this stack reports. |

## `DOCKER` — talking to the engine

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `DOCKER-1` | `ENGINE_UNREACHABLE` | error | 3 | 500 | 0.1.0 | Raised when the container engine cannot be reached. |
| `DOCKER-2` | `NO_SUCH_CONTAINER` | warning | 1 | 500 | 0.1.0 | Raised when a container that should exist does not. |
| `DOCKER-3` | `HOST_UNRESOLVED` | error | 1 | 500 | 0.15.0 | Raised when the host an endpoint names cannot be found on the network. |
| `DOCKER-4` | `HOST_REFUSED` | error | 1 | 500 | 0.15.0 | Raised when the host is found and refuses the connection. |
| `DOCKER-5` | `LOGIN_REJECTED` | error | 1 | 500 | 0.15.0 | Raised when the host is reached and will not accept the SSH login. |
| `DOCKER-6` | `ENDPOINT_UNSUPPORTED` | error | 1 | 500 | 0.15.0 | Raised when an endpoint names a transport this build cannot drive. |
| `DOCKER-7` | `UNKNOWN_CONTEXT` | error | 1 | 500 | 0.15.0 | Raised when a named Docker context is not one this machine records. |
| `DOCKER-8` | `HOST_SILENT` | error | 1 | 500 | 0.15.0 | Raised when a remote host does not answer for a reason nothing here recognises. |

## `ENV` — the container engine

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `ENV-1` | `DOCKER_ABSENT` | error | 1 | 500 | 0.1.0 | Raised when the Docker client is not installed. |
| `ENV-2` | `DAEMON_DOWN` | error | 1 | 500 | 0.1.0 | Raised when the Docker client is present but its daemon is not answering. |
| `ENV-3` | `COMPOSE_UNUSABLE` | error | 1 | 500 | 0.1.0 | Raised when the Compose plugin is missing or too old to drive. |
| `ENV-4` | `ENGINE_NOT_AT_BOOT` | warning | 1 | 500 | 0.15.0 | Raised when the container engine is confirmed not to start with this machine. One code for both arrangements it can be. What is not set differs by platform — Docker Desktop's open-at-login setting on macOS and Windows, the daemon's own unit on native Linux — and what has gone wrong is the same thing either way: nothing brings the engine up, so nothing reads the restart policies that would bring the containers back. |
| `ENV-5` | `API_MISMATCH` | warning | 1 | 500 | 0.15.0 | Raised when this machine and the daemon speak different Docker API generations. |

## `FORM` — choosing what to run

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `FORM-1` | `NO_FORM_NAMED` | error | 1 | 400 | 0.1.0 | Raised when no form was named. |
| `FORM-2` | `NO_SUCH_FORM` | error | 1 | 404 | 0.1.0 | Raised when a named form is not declared by the stack. |
| `FORM-3` | `FORMS_CONFLICT` | error | 1 | 400 | 0.1.0 | Raised when forms that cannot be combined are named together. |
| `FORM-4` | `NOTHING_TO_RUN` | warning | 1 | 500 | 0.1.0 | Raised when narrowing leaves nothing to run. |

## `GATE` — the request gate

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `GATE-1` | `REFUSED` | warning | 1 | 500 | 0.17.0 | Raised when the request gate refused calls since the last diagnosis: the request service asked for something it has no use for. |
| `GATE-2` | `LOST` | warning | 1 | 500 | 0.17.0 | Raised when more entries reached the gate's record between two diagnoses than it keeps, so some could not be reported. |

## `GONE` — taking lemonfiber off this machine

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `GONE-1` | `NEEDS_AGREEING` | error | 1 | 500 | 0.13.0 | Raised when the tier that takes the library was confirmed without its own agreement. |
| `GONE-2` | `ANOTHER_READING` | error | 1 | 400 | 0.13.0 | Raised when an agreement names a reading of this machine that is not the one standing now. |
| `GONE-3` | `NOT_BACKED_UP` | error | 1 | 500 | 0.14.0 | Raised when the backup a destructive removal takes first could not be taken. |

## `HANDOFF` — pointing somebody's device at the stack

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `HANDOFF-1` | `NOBODY_NAMED` | error | 1 | 500 | 0.17.0 | Said where the hand-off is for nobody: the name is blank, or only spaces. |
| `HANDOFF-2` | `NO_MEDIA_SERVER` | error | 1 | 500 | 0.17.0 | Said where the stack holds no media server for a device to sign in to. |
| `HANDOFF-3` | `NOT_SET_UP` | error | 1 | 500 | 0.17.0 | Said where the media server's own account was never recorded. |
| `HANDOFF-4` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.17.0 | Said where the account named administers the media server. |

## `HOST` — keeping a command running without a terminal

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `HOST-1` | `NOTHING_TO_HOST_WITH` | warning | 1 | 500 | 0.12.0 | Raised where the platform has no service manager lemonfiber can configure. |
| `HOST-2` | `DEFINITION_UNWRITABLE` | error | 1 | 500 | 0.12.0 | Raised where a service definition could not be written. |
| `HOST-3` | `MANAGER_REFUSED` | error | 1 | 500 | 0.12.0 | Raised where the service manager refused what it was asked. |
| `HOST-4` | `NOWHERE_TO_WRITE` | error | 1 | 500 | 0.12.0 | Raised when this machine will not say where it keeps its own files. |
| `HOST-5` | `NO_PROGRAM` | error | 1 | 500 | 0.12.0 | Raised when this run cannot say where its own program is. |
| `HOST-6` | `NOTHING_NAMED_TO_GUARD` | error | 1 | 500 | 0.12.0 | Raised when the guard is to be hosted against nothing. |

## `INVITE` — offering somebody an account

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `INVITE-1` | `NO_MEDIA_SERVER` | error | 1 | 500 | 0.11.0 | Said where the stack holds no media server: there is nothing to make an account on. |
| `INVITE-2` | `NO_CREDENTIAL` | error | 1 | 500 | 0.11.0 | Said where the admin credential was never recorded: nothing can be asked of the server. |
| `INVITE-3` | `NOWHERE_TO_SEND` | error | 1 | 500 | 0.11.0 | Said where this machine has no address the household could arrive at. An invitation is an address somebody else types. Sending one built from a default would be sending a link that opens nothing, which is worse than saying there is none: the operator would learn it had failed from whoever they invited. |
| `INVITE-4` | `NOBODY_NAMED` | error | 1 | 500 | 0.11.0 | Said where the invitation is for nobody: the name is blank, or only spaces. The media server refuses this too, in its own words, which are `400` and a link to the specification of that status. The operator asked for something reasonable and mistyped it, and is owed a sentence about the name rather than about HTTP. |
| `INVITE-5` | `WOULD_NOT_RENEW` | error | 1 | 500 | 0.11.0 | Said where an invitation offered again could not be dated again, so its window is not real. The account is untouched and still theirs — what failed is the write that says when it was offered. Reported rather than glossed over because the message the operator is about to send promises a window, and this one would be counted from whenever the invitation was first made, which has already passed. |
| `INVITE-6` | `NO_LIBRARIES_READ` | error | 1 | 500 | 0.11.0 | Said where the media server would not say what libraries it holds. Refused rather than read as no libraries at all: a name matched against an empty list is a name that could not be found, and the operator would be told their library does not exist when what happened is that nobody could ask. |
| `INVITE-7` | `NO_SUCH_LIBRARY` | error | 1 | 500 | 0.11.0 | Said where no library goes by a name that was given. The ones there are, named: the fix is one word, and the words are already in hand. |
| `INVITE-8` | `WOULD_NOT_ALLOW` | error | 1 | 500 | 0.11.0 | Said where what the account may watch could not be written on it. A new account is taken back rather than left open, so the offer is refused whole; an existing one keeps what it already had. Said as which of the two is now true, because an operator told only that something failed would not know whether an open account is standing somewhere. |
| `INVITE-9` | `UNRECORDED` | error | 1 | 500 | 0.17.0 | Said where the offer could not be written down. An invitation runs out a set time after it is offered, and one this machine has no date for is taken back the next time anybody is invited — so an offer that could not be dated is not made. |
| `INVITE-10` | `UNGUARDED` | error | 1 | 500 | 0.17.0 | Said where the account could not be made one its person can claim: switched on, and bounded against guessing at its password. |
| `INVITE-11` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.17.0 | Said where the name given is the account that administers the media server. This is the account lemonfiber signs in as, and offering it would put a household member's limits on it. |

## `KEPT` — what lemonfiber keeps here

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `KEPT-1` | `NOWHERE_KNOWN` | error | 1 | 500 | 0.10.0 | Raised when this run does not know where lemonfiber's own files go. |

## `KEY` — keys another program reaches the stack with

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `KEY-1` | `BAD_NAME` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for under a word that cannot name one. |
| `KEY-2` | `NOT_A_SCOPE` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for with a scope that is none of the three. |
| `KEY-3` | `NOT_A_PURPOSE` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for with a purpose that is none of the three. |
| `KEY-4` | `UNREADABLE` | error | 1 | 500 | 0.17.0 | Raised when the record of keys is there and does not read as keys. |
| `KEY-5` | `NAME_TAKEN` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for under a name another key holds. |
| `KEY-6` | `NO_SUCH_MEMBER` | error | 1 | 404 | 0.17.0 | Raised when a member's key names an account the household does not hold. |
| `KEY-7` | `UNASKED` | error | 1 | 500 | 0.17.0 | Raised when the household could not be asked about a member's account. |
| `KEY-8` | `NOT_FOR_YOURSELF` | warning | 1 | 400 | 0.17.0 | Raised when a household member asks for a key, or a revoke, that is not theirs alone. |
| `KEY-9` | `NO_SECRET` | error | 1 | 500 | 0.17.0 | Raised when this machine would not supply the bytes a secret is made of. |
| `KEY-10` | `NO_SUCH_KEY` | error | 1 | 404 | 0.17.0 | Raised when no active key holds the name a revoke gave. |
| `KEY-11` | `MEMBERS_MAY_NOT_MINT` | warning | 1 | 409 | 0.17.0 | Raised when a household member asks to mint a key and the operator has not allowed members to. |

## `LIFE` — starting and stopping

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `LIFE-1` | `NEVER_SETTLED` | error | 4 | 500 | 0.1.0 | Raised when a service never reached a state that starting could accept. |
| `LIFE-2` | `STILL_NEEDED` | error | 1 | 500 | 0.8.0 | Raised when stopping would take a service out from under a form still running. |
| `LIFE-3` | `ALREADY_WORKING` | error | 1 | 409 | 0.8.0 | Another run is already working on this stack. |
| `LIFE-4` | `REGISTRY_REFUSED` | error | 1 | 400 | 0.10.0 | Fetching images is switched off, so there was nothing to fetch with. |
| `LIFE-5` | `NO_DATA_LOCATION` | error | 1 | 500 | 0.15.0 | Raised when a start was asked for over a data location that is not there. |
| `LIFE-6` | `ABSENT_THERE` | error | 1 | 500 | 0.15.0 | Raised when the stack's own location is not on the machine being operated. |
| `LIFE-7` | `ELSEWHERE_UNDERNEATH` | error | 1 | 500 | 0.17.0 | Raised when a path the stack mounts is not at the same path on the machine under the container lemonfiber runs in. |
| `LIFE-8` | `NO_ENGINE_IN_HERE` | error | 1 | 500 | 0.17.0 | Raised when lemonfiber runs in a container that cannot reach the engine. |
| `LIFE-9` | `NOT_ON_THIS_ENGINE` | error | 1 | 500 | 0.17.0 | Raised when the engine lemonfiber reaches from a container does not know that container. |

## `MIGRATE` — taking over a setup already here

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `MIGRATE-1` | `OFFER_MOVED` | warning | 1 | 400 | 0.17.0 | Raised when a replacement was agreed to for an offer that is not the one standing now. |

## `PAIR` — pairing a phone with the stack

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `PAIR-1` | `NOWHERE` | error | 1 | 500 | 0.17.0 | Raised when there is nowhere to keep what pairing a phone needs. |
| `PAIR-2` | `NOT_SERVED` | error | 1 | 500 | 0.17.0 | Raised when the web surface has not been served encrypted on the network. |
| `PAIR-3` | `NO_CERTIFICATE` | error | 1 | 500 | 0.17.0 | Raised when the certificate the surface presents cannot be read or made. |
| `PAIR-4` | `NO_ADDRESS` | error | 1 | 500 | 0.17.0 | Raised when this machine has no address a phone could reach it at. |
| `PAIR-5` | `UNNAMED` | error | 1 | 500 | 0.17.0 | Raised when the stack's identifier cannot be read or made. |

## `PLUGIN` — installing and running plugins

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `PLUGIN-1` | `CONTRIBUTED_FAILED` | error | 1 | 409 | 0.16.0 | A check a plugin contributed did not hold. One code for all of them rather than one per plugin, because a code is a stable thing an operator searches for and a plugin's own name is not this build's to mint one from. Which check and which plugin is on the finding, where it can be read. |
| `PLUGIN-2` | `UNREADABLE` | error | 1 | 404 | 0.16.0 | The source names no plugin this build can read. |
| `PLUGIN-3` | `REFUSED` | error | 1 | 400 | 0.16.0 | The manifest is read and this build refuses what it declares. |
| `PLUGIN-4` | `UNRECORDED` | error | 1 | 500 | 0.16.0 | The record of what is installed cannot be read. |
| `PLUGIN-5` | `ALREADY` | error | 1 | 400 | 0.16.0 | The plugin is installed already. |
| `PLUGIN-6` | `NOWHERE` | error | 1 | 500 | 0.16.0 | There is no stack on this machine to put a plugin's container in. |
| `PLUGIN-7` | `UNWRITABLE` | error | 1 | 500 | 0.16.0 | A directory or a document the install decided on would not land. |
| `PLUGIN-8` | `UNRECORDABLE` | error | 1 | 500 | 0.16.0 | The wiring went down and the record of what is installed did not. |
| `PLUGIN-9` | `UNPROVED` | error | 1 | 500 | 0.16.0 | The plugin's own service would not start, so nothing about it could be proved. |
| `PLUGIN-10` | `NOTHING_TO_REMOVE` | error | 1 | 404 | 0.16.0 | Nothing by that name is installed on this machine. |
| `PLUGIN-11` | `NOTHING_TO_UPDATE` | error | 1 | 404 | 0.16.0 | Nothing by that id is installed, so there is no version to replace. |
| `PLUGIN-12` | `STUCK` | error | 1 | 500 | 0.16.0 | The version installed would not come off, so nothing else was touched. |
| `PLUGIN-13` | `ANSWERED` | error | 1 | 400 | 0.16.0 | Raised when a plugin's service would answer on a label another plugin's already does. |
| `PLUGIN-14` | `TWO_SOURCES` | error | 1 | 400 | 0.17.0 | Raised when a plugin is installed from a source other than the one its name is already installed from. |
| `PLUGIN-15` | `SOURCE_OFF` | error | 1 | 400 | 0.17.0 | Raised when a plugin is named from a git source and fetching from one is switched off. |
| `PLUGIN-16` | `UNFETCHED` | error | 1 | 500 | 0.17.0 | Raised when a git source could not be reached or would not hand over a revision. |
| `PLUGIN-17` | `NO_REVISION` | error | 1 | 404 | 0.17.0 | Raised when a git source holds no branch, tag or commit by the name given. |
| `PLUGIN-18` | `CATALOGUE_OFF` | error | 1 | 400 | 0.17.0 | Raised when a plugin is installed by name and asking the catalogue is switched off. |
| `PLUGIN-19` | `CATALOGUE_UNREACHABLE` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index or its signature could not be fetched. |
| `PLUGIN-20` | `SIGNATURE_UNVERIFIED` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index has no signature, one that does not verify, or none this build carries a key to check. |
| `PLUGIN-21` | `CATALOGUE_UNREADABLE` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index verified and is not one this build reads. |
| `PLUGIN-22` | `NOT_CATALOGUED` | error | 1 | 404 | 0.17.0 | Raised when the catalogue holds no plugin by the name given. |
| `PLUGIN-23` | `NOT_AS_REVIEWED` | error | 1 | 500 | 0.17.0 | Raised when what the catalogue's origin served is not what the catalogue reviewed. |
| `PLUGIN-24` | `SPELLED_ALIKE` | error | 1 | 400 | 0.17.0 | Raised when a plugin's service would be named, where lemonfiber keeps what a service holds, as another installed plugin's service already is. |
| `PLUGIN-25` | `PLUGIN_OFFER_MOVED` | error | 1 | 400 | 0.17.0 | Raised when an install, an update or a removal answers an offer that was read against a plugin, a stack or a record that has since moved. |
| `PLUGIN-26` | `UNAPPROVED` | error | 1 | 400 | 0.17.0 | Raised when a value a recipe would carry to a destination was not approved as itself, or an approval names a pair the recipe does not carry. |
| `PLUGIN-27` | `ANOTHER_PLUGIN` | error | 1 | 400 | 0.17.0 | Raised when the source an update names holds a different plugin from the one it was asked to update. |
| `PLUGIN-28` | `OCCUPIED` | error | 1 | 400 | 0.17.0 | Raised when a plugin's service would take a name, a port or a label something already on this machine holds: a service of the stack or of the operator's overlay, another plugin's port, or a site in the proxy's live configuration. |
| `PLUGIN-29` | `CATALOGUE_REPLACED` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index verifies and is older than the newest one this machine has verified. |
| `PLUGIN-30` | `NEWEST_UNKEPT` | error | 1 | 500 | 0.17.0 | Raised when the record of the newest catalogue index this machine verified cannot be read or written. |
| `PLUGIN-31` | `SCHEME_REFUSED` | error | 1 | 400 | 0.17.0 | Raised when a git source is named over a transport other than https, before anything is asked of it. |
| `PLUGIN-32` | `ADDRESS_REFUSED` | error | 1 | 400 | 0.17.0 | Raised when a git source's host is, or stands for, an address on this machine or on a network of its own: loopback, private, link-local or unspecified. |
| `PLUGIN-33` | `HEADER_NAMED` | error | 1 | 400 | 0.17.0 | Raised when a recipe substitutes a value into a header's name, which is a fixed identifier of the protocol and written out; the manifest's every other fault is listed beside it. |
| `PLUGIN-34` | `INPUT_UNMATCHED` | error | 1 | 400 | 0.18.0 | Raised when a recipe of the act asks the operator for a value that was not given, or a value was given that no recipe of the act asks for. |
| `PLUGIN-35` | `CALL_REFUSED` | error | 1 | 500 | 0.18.0 | Raised when a recipe's call was not sent because its host stands for an address not out on the internet; the install or update was put back. |
| `PLUGIN-36` | `STEP_FAILED` | error | 1 | 500 | 0.18.0 | Raised when a recipe's step failed any other way — nothing answered, the answer was not the one it expects, a capture found nothing, or the answer was larger than a recipe reads; the install or update was put back. |
| `PLUGIN-37` | `PATH_NOT_PLAIN` | error | 1 | 400 | 0.18.0 | Raised when a recipe's call path is not a plain absolute path; the manifest's every other fault is listed beside it. |
| `PLUGIN-38` | `VALUE_WITHHELD` | error | 1 | 400 | 0.18.0 | Raised when a recipe's call was not sent because a value it carries may not go where it was going: not where its pairs say, not back to the service a credential belongs to, or outside without its approval; the install or update was put back. |

## `PROC` — the program underneath

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `PROC-1` | `MISSING_PROGRAM` | error | 3 | 500 | 0.1.0 | Raised when the program a subprocess needs is missing. |
| `PROC-2` | `UNUSABLE_PROGRAM` | error | 1 | 500 | 0.1.0 | Raised when a program exists but will not start. |

## `PROVIDER` — accounts and indexers

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `PROVIDER-1` | `PROVIDER_EMPTY` | error | 1 | 500 | 0.7.0 | Raised when an account has nothing left to serve. |
| `PROVIDER-2` | `PROVIDER_LOW` | warning | 1 | 500 | 0.7.0 | Raised when an account is running out, with time left to act. |
| `PROVIDER-3` | `PROVIDER_ENDING` | warning | 1 | 500 | 0.7.0 | Raised when the subscription behind an account ends soon. |
| `PROVIDER-4` | `INDEXER_RESTED` | warning | 1 | 500 | 0.7.0 | Raised when an indexer has been failing and its aggregator has rested it. |
| `PROVIDER-5` | `INDEXERS_ALL_FAILING` | error | 1 | 500 | 0.7.0 | Raised when every indexer is failing at once. |
| `PROVIDER-6` | `PROVIDER_REFUSED` | error | 1 | 500 | 0.7.0 | Raised when an account refuses the credential the client offers it. |
| `PROVIDER-7` | `PROVIDER_SILENT` | warning | 1 | 500 | 0.7.0 | Raised when an account has stopped answering the client entirely. |
| `PROVIDER-8` | `PROVIDER_CROWDED` | warning | 1 | 500 | 0.7.0 | Raised when the client is set to open more connections than an account allows. |
| `PROVIDER-9` | `INDEXER_CAPPED` | warning | 1 | 500 | 0.7.0 | Raised when an indexer has spent the allowance recorded against it. |

## `QUAL` — quality against what is available

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `QUAL-1` | `HEADROOM_LOW` | warning | 1 | 500 | 0.4.0 | Raised when the free space holds too little content at the chosen quality. |
| `QUAL-2` | `PRESET_UNMET` | warning | 1 | 500 | 0.4.0 | Raised when releases exist but the profile — the chosen quality included — wants none of them. |
| `QUAL-3` | `NONE_AVAILABLE` | warning | 1 | 500 | 0.4.0 | Raised when a clean search turns up nothing at all for wanted content. |

## `QUOTA` — what the household may ask for

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `QUOTA-1` | `UNREACHABLE` | error | 1 | 500 | 0.12.0 | Raised where the request service would not answer, so nothing was changed. |
| `QUOTA-2` | `NO_LIMIT` | error | 1 | 400 | 0.12.0 | Raised where a policy that lives inside a limit was chosen without one. |
| `QUOTA-4` | `NOT_WAITING` | error | 1 | 404 | 0.12.0 | Raised where the request named is not one that is waiting on anybody. |
| `QUOTA-5` | `NO_REASON` | error | 1 | 400 | 0.12.0 | Raised where a request was turned down and the reason said nothing. |
| `QUOTA-6` | `NOBODY` | error | 1 | 404 | 0.12.0 | Raised where nobody in the household goes by the name that was given. |
| `QUOTA-7` | `NEVER_HERE` | warning | 1 | 404 | 0.12.0 | Raised where the request service holds no account for somebody who has one here. |
| `QUOTA-8` | `NOTHING_AGREED` | error | 1 | 400 | 0.12.0 | Raised where a run was asked to close what has waited too long and the household has never said how long that is. |
| `QUOTA-9` | `TOO_SOON` | error | 1 | 400 | 0.12.0 | Raised where the period named would close a request nobody was ever reminded about. |

## `RATE` — holding the stack to a share of the line

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `RATE-1` | `NOTHING_MEASURED` | error | 1 | 400 | 0.12.0 | Raised when a limit is expressed as a share of a line nothing has measured. |
| `RATE-2` | `NO_ZONE` | error | 1 | 400 | 0.12.0 | Raised when a schedule is asked for and nothing says which zone the clients would read it in. |
| `RATE-3` | `UNREADABLE` | error | 1 | 400 | 0.12.0 | Raised when what was asked for could not be read as a limit, a window or a cap. |
| `RATE-4` | `NOTHING_TO_LIMIT` | error | 1 | 400 | 0.12.0 | Raised when there is no download client to limit. |
| `RATE-5` | `NOTHING_TO_PAUSE` | error | 1 | 400 | 0.17.0 | Raised when there is no download client to pause or resume. |

## `READ` — asking the web surface a question

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `READ-1` | `UNWANTED` | error | 1 | 400 | 0.9.0 | Raised where a read was given a parameter its answer has nowhere to put. |
| `READ-2` | `REPEATED` | error | 1 | 400 | 0.9.0 | Raised where a parameter carrying one value was given more than once. |
| `READ-3` | `NO_SUCH_READ` | error | 1 | 404 | 0.17.0 | Raised where no read goes by the name that was asked for. |
| `READ-4` | `NO_TERM` | error | 1 | 400 | 0.17.0 | Raised where a trace was asked for and named nothing to follow. |
| `READ-5` | `NOT_A_SEASON` | error | 1 | 400 | 0.17.0 | Raised where the season to narrow a trace to is not a number. |
| `READ-6` | `NO_SETTING` | error | 1 | 400 | 0.17.0 | Raised where a setting was asked for by an empty name. |
| `READ-7` | `NO_MEMBER` | error | 1 | 400 | 0.17.0 | Raised where a household member was asked for by an empty name. |
| `READ-8` | `NO_SHELF_WITHOUT_A_MEMBER` | error | 1 | 400 | 0.17.0 | Raised where a shelf was asked for and nobody was named whose it is. |
| `READ-9` | `NOT_A_COUNT` | error | 1 | 400 | 0.17.0 | Raised where how many holdings to answer with is not a whole number. |
| `READ-10` | `TOO_MANY_AT_ONCE` | error | 1 | 400 | 0.17.0 | Raised where more holdings were asked for than one read answers with. |
| `READ-11` | `NO_SUCH_GROUP` | error | 1 | 400 | 0.17.0 | Raised where a diagnosis was narrowed to a group or check that is not one. |
| `READ-12` | `NO_SUCH_REMOVAL` | error | 1 | 400 | 0.17.0 | Raised where a removal was named that is none of the four there are. |
| `READ-13` | `NO_UPDATE_OBJECT` | error | 1 | 400 | 0.17.0 | Raised where moving forward was asked about and neither stack nor self named. |
| `READ-14` | `NOT_A_LINE_COUNT` | error | 1 | 400 | 0.17.0 | Raised where how many log lines to begin with is not a number within the ceiling. |
| `READ-15` | `NOT_A_CHOICE` | error | 1 | 400 | 0.17.0 | Raised where a parameter that takes a yes or a no is neither true nor false. |
| `READ-16` | `MEMBER_AND_DEFAULTS` | error | 1 | 400 | 0.17.0 | Raised where a household read named a member and asked for the household's defaults as well. |

## `REHEARSE` — asking what a command would do

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `REHEARSE-1` | `CANNOT` | error | 1 | 400 | 0.15.0 | The flag cannot be honoured by this command, and never will be. |
| `REHEARSE-2` | `NOT_YET` | error | 1 | 400 | 0.15.0 | The flag is not honoured by this command yet. |

## `REISSUE` — letting somebody set a new password

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `REISSUE-1` | `UNREADABLE` | error | 1 | 500 | 0.11.0 | Said where the media server will not say who holds an account. |
| `REISSUE-2` | `NOBODY_HERE` | error | 1 | 500 | 0.11.0 | Said where nobody by that name is in the household. |
| `REISSUE-3` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.11.0 | Said where the account named administers the server. |
| `REISSUE-4` | `WOULD_NOT_REISSUE` | error | 1 | 500 | 0.11.0 | Said where the media server refused to make the account claimable again. |

## `REMOVE` — taking somebody out of the household

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `REMOVE-1` | `NOBODY_NAMED` | error | 1 | 500 | 0.11.0 | Said where the removal is for nobody: the name is blank, or only spaces. |
| `REMOVE-2` | `NO_MEDIA_SERVER` | error | 1 | 500 | 0.11.0 | Said where the stack holds no media server: there is no account to remove. |
| `REMOVE-3` | `UNREADABLE` | error | 1 | 500 | 0.11.0 | Said where the media server will not say who it holds. |
| `REMOVE-4` | `NOBODY_HERE` | error | 1 | 500 | 0.11.0 | Said where nobody by that name is in the household. |
| `REMOVE-5` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.11.0 | Said where the account named administers the server. |
| `REMOVE-6` | `WOULD_NOT_REMOVE` | error | 1 | 500 | 0.11.0 | Said where the media server refused to remove the account. |

## `REPAIR` — putting right what the doctor found

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `REPAIR-1` | `STALE` | warning | 1 | 400 | 0.9.0 | Raised when consent was given for an offer that no longer stands. |
| `REPAIR-2` | `NOWHERE_TO_LOOK` | error | 1 | 500 | 0.9.0 | Raised when a run cannot say where lemonfiber's own files are. |
| `REPAIR-3` | `OFFER_CANNOT_DISTURB` | error | 1 | 500 | 0.9.0 | Raised when a run that may not act was asked for the checks that disturb. |

## `RESTORE` — putting configuration back

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `RESTORE-1` | `CORRUPT` | error | 1 | 500 | 0.3.0 | Raised when a backup archive cannot be read to decide a restore. |
| `RESTORE-2` | `TOO_NEW` | error | 1 | 500 | 0.3.0 | Raised when an archive was written by a newer lemonfiber than this one. |
| `RESTORE-3` | `INCOMPATIBLE` | error | 1 | 500 | 0.3.0 | Raised when an archive's format cannot be restored by this build. |
| `RESTORE-4` | `UNSAFE` | critical | 1 | 500 | 0.3.0 | Raised when an archive holds a member that would be written outside its area. |
| `RESTORE-5` | `NEEDS_REPOINT` | warning | 1 | 500 | 0.3.0 | Raised when a restore onto a different data root awaits the operator's consent. |
| `RESTORE-6` | `NOT_RESTORED` | error | 1 | 500 | 0.3.0 | Raised when an archive could not be unpacked. |
| `RESTORE-7` | `STILL_RUNNING` | error | 1 | 500 | 0.9.0 | Raised when a restore could not be shown that nothing is writing to a database. |
| `RESTORE-8` | `NOT_KEPT_HERE` | error | 1 | 500 | 0.9.0 | Raised when a name does not name one of the backups this machine kept. |
| `RESTORE-9` | `NOWHERE_KEPT` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to look for an archive. |
| `RESTORE-10` | `NOT_REPOINTED` | error | 1 | 500 | 0.9.0 | Raised when the restored settings could not be pointed at this machine's data root. |
| `RESTORE-11` | `MOVED_ON` | warning | 1 | 400 | 0.9.0 | Raised when consent was given for a listing that no longer stands. |
| `RESTORE-12` | `NOT_OURS` | error | 1 | 500 | 0.14.0 | Raised when the archive holds trees lemonfiber does not manage. |

## `SEED` — wiring the services together

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `SEED-1` | `SERVICE_UNAVAILABLE` | warning | 1 | 500 | 0.1.0 | Raised when a service is not answering yet. |
| `SEED-2` | `SERVICE_UNAUTHORISED` | error | 1 | 500 | 0.1.0 | Raised when a service rejects the credential lemonfiber holds. |
| `SEED-3` | `SERVICE_REFUSED` | error | 1 | 500 | 0.1.0 | Raised when a service answers with something unusable. |
| `SEED-4` | `SERVICE_UNSUPPORTED` | error | 1 | 500 | 0.4.0 | Raised when a service does not serve the API version this build speaks. |

## `SERVE` — the web surface

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `SERVE-1` | `ADDRESS_TAKEN` | error | 1 | 500 | 0.9.0 | Raised when the address the surface was asked to serve on cannot be taken. |
| `SERVE-2` | `NO_TOKEN` | error | 1 | 500 | 0.9.0 | Raised when this machine will not supply the randomness a token is made of. |
| `SERVE-3` | `NO_PASSWORD` | error | 1 | 500 | 0.10.0 | Raised when the network was asked for and nothing here can say who is knocking. |
| `SERVE-4` | `UNSETTLED_PORT` | error | 1 | 500 | 0.17.0 | Raised when serving encrypted was asked for with no port that stays the same. |
| `SERVE-5` | `NO_CERTIFICATE` | error | 1 | 500 | 0.17.0 | Raised when the certificate to serve encrypted with cannot be read or made. |
| `SERVE-6` | `UNRENDERABLE` | error | 1 | 500 | 0.17.0 | Raised when an answer could not be rendered. |
| `SERVE-7` | `NO_JOB_NAME` | error | 1 | 500 | 0.17.0 | Raised when this machine will not supply the randomness a job is named with. |
| `SERVE-8` | `UNANSWERED` | error | 1 | 500 | 0.18.0 | Raised when an action's work ended before it had an answer to give. |

## `SETUP` — the first run

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `SETUP-1` | `NOT_REVIEWED` | error | 1 | 400 | 0.2.0 | Raised when apply is asked for before the answers have been reviewed. |
| `SETUP-2` | `DIR_NOT_MADE` | error | 1 | 500 | 0.2.0 | Raised when the operator's chosen data directory cannot be created. |
| `SETUP-3` | `NOT_REMOVED` | error | 1 | 500 | 0.2.0 | Raised when a directory from an interrupted apply could not be removed. |
| `SETUP-4` | `NEEDS_SERVICE` | error | 1 | 500 | 0.2.0 | Raised when reversing needs the service that made a change. |
| `SETUP-5` | `DOES_NOT_APPLY` | error | 1 | 400 | 0.2.0 | Raised when an answer is not meaningful on the platform setup is running on. |
| `SETUP-6` | `ALREADY_UNDERWAY` | error | 1 | 500 | 0.2.0 | Raised when setup is asked to gather answers for a wizard already past it. |
| `SETUP-7` | `ALREADY_SET_UP` | error | 1 | 400 | 0.9.0 | Raised when setup is answered on a machine that is already set up. |
| `SETUP-8` | `NOTHING_TO_RECOVER` | error | 1 | 400 | 0.9.0 | Raised when a recovery is asked for and no apply stopped part-way. |
| `SETUP-9` | `NOT_PUT_BACK` | warning | 1 | 500 | 0.9.0 | Raised when a reversal would write over a setting the operator has since chosen. |
| `SETUP-10` | `NOT_OPENED` | warning | 1 | 500 | 0.14.0 | Raised when a reversal meets a credential whose sealed record will not open. |
| `SETUP-11` | `STILL_HOLDING` | warning | 1 | 500 | 0.16.0 | Raised when a directory a reversal would remove still holds something else's files. |
| `SETUP-12` | `NOT_WITHDRAWN` | error | 1 | 500 | 0.16.0 | Raised when a region a reversal would take out of a stack file cannot be. |
| `SETUP-13` | `NOT_REWOUND` | error | 1 | 500 | 0.17.0 | Raised when a file a reversal would write back to what it held cannot be. |

## `SPACE` — the disk, and letting a download go

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `SPACE-1` | `HALTED` | critical | 1 | 500 | 0.12.0 | Raised when the volume is full and new acquisitions are therefore halted. |
| `SPACE-2` | `NOWHERE_TO_MEASURE` | error | 1 | 400 | 0.12.0 | Raised when there is no data location to measure. |
| `SPACE-3` | `WALK_REFUSED` | error | 1 | 500 | 0.12.0 | Raised when the data location is there and could not be read. |
| `SPACE-4` | `NOTHING_TO_ASK` | error | 1 | 400 | 0.12.0 | Raised when there is no torrent client here to be holding a completed download. |
| `SPACE-5` | `NOT_HELD` | error | 1 | 400 | 0.12.0 | Raised when the client answers and is holding nothing of the name given. |
| `SPACE-6` | `ANOTHER_OFFER` | error | 1 | 400 | 0.12.0 | Raised when an agreement names an offer that is not the one standing now. |
| `SPACE-7` | `STILL_HELD` | error | 1 | 500 | 0.12.0 | Raised when the client could not be reached, or would not let a download go. |

## `STACK` — the stack description

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `STACK-1` | `STACK_UNREADABLE` | error | 5 | 500 | 0.1.0 | Raised when a stack directory holds no readable manifest. |
| `STACK-2` | `STACK_UNUSABLE` | error | 1 | 500 | 0.1.0 | Raised when a manifest is readable and this build cannot use it. |
| `STACK-3` | `STACK_NOT_EMBEDDED` | critical | 1 | 500 | 0.1.0 | Raised when the embedded stack is not intact. |
| `STACK-4` | `STACK_NOT_SET_UP` | error | 1 | 500 | 0.1.0 | Raised when lemonfiber has nowhere to write the stack. |
| `STACK-5` | `STACK_NOT_WRITTEN` | error | 1 | 500 | 0.1.0 | Raised when the stack could not be written to disk. |
| `STACK-6` | `STACK_INVALID` | error | 5 | 500 | 0.1.0 | Raised when a manifest parses and breaks the contract. |
| `STACK-7` | `STACK_MALFORMED` | error | 5 | 500 | 0.14.0 | Raised when a manifest is not TOML at all. |
| `STACK-8` | `STACK_UNRECOGNISED` | error | 5 | 500 | 0.14.0 | Raised when a manifest declares names this build does not know. |
| `STACK-9` | `STACK_NEEDS_NEWER` | error | 1 | 500 | 0.17.0 | Raised when a stack names a newer lemonfiber than the one running. |
| `STACK-10` | `STACK_UNASSEMBLED` | error | 5 | 500 | 0.18.0 | Raised when a manifest's files are not laid out as the contract says. |

## `STORAGE` — the data location

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `STORAGE-1` | `COPY_ONLY` | warning | 1 | 500 | 0.2.0 | Raised when the data root cannot hardlink, so imports must copy. |
| `STORAGE-2` | `ROOT_UNWRITABLE` | error | 1 | 500 | 0.2.0 | Raised when the data root exists but cannot be written to. |
| `STORAGE-3` | `ROOT_ABSENT` | error | 1 | 500 | 0.2.0 | Raised when the data root is not there to test. |
| `STORAGE-4` | `SPACE_LOW` | warning | 1 | 500 | 0.2.0 | Raised when the volume holding the data root is nearly full. |
| `STORAGE-5` | `DEGRADED` | error | 1 | 500 | 0.2.0 | Raised when the data root used to hardlink and no longer does. |
| `STORAGE-6` | `SERVICE_DENIED` | error | 1 | 500 | 0.2.0 | Raised when the operator owns the data root but the services cannot write it. |
| `STORAGE-7` | `SPLIT_MOUNTS` | warning | 1 | 500 | 0.15.0 | Raised where a service would see more than one mount beneath the data location, so anything imported between them is copied rather than linked. |
| `STORAGE-8` | `COPYING_SINCE_DEGRADED` | warning | 1 | 500 | 0.18.0 | Raised where a location that stopped hardlinking leaves the stack copying in a mode it was not set up for. |

## `TELLING` — what the household is told about

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `TELLING-1` | `BEHIND` | warning | 1 | 500 | 0.11.0 | Raised when the household is told about less than lemonfiber now sets out to tell them, through no choice of the operator's. |

## `TUI` — the terminal interface

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `TUI-1` | `DRAWING` | error | 1 | 500 | 0.6.0 | A screen that could not be drawn, as a problem rather than a panic. |

## `UNDO` — putting a run back

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `UNDO-1` | `NO_SUCH_RUN` | error | 1 | 500 | 0.14.0 | Raised when no run carries the stamp a reversal was asked for. |
| `UNDO-2` | `MORE_THAN_ONE_RUN` | error | 1 | 500 | 0.14.0 | Raised when a stamp names more than one run, so which to put back is not settled. |
| `UNDO-3` | `CANNOT_SUCCEED` | error | 1 | 500 | 0.14.0 | Raised when a run cannot be put back, carrying the reason it cannot. |
| `UNDO-4` | `NOWHERE_TO_LOOK` | error | 1 | 500 | 0.14.0 | Raised when a run cannot say where lemonfiber's own files are. |

## `UPDATE` — moving the stack onto newer versions

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `UPDATE-1` | `NOT_CHECKED` | error | 1 | 500 | 0.14.0 | Raised when what this machine has pulled could not be read. |
| `UPDATE-2` | `NO_SUCH_SERVICE` | error | 1 | 500 | 0.14.0 | Raised when the service an update was narrowed to is not one the stack declares. |
| `UPDATE-3` | `STILL_TRANSFERRING` | warning | 1 | 500 | 0.14.0 | Raised when transfers are still in flight and the run was not asked to wait. |
| `UPDATE-4` | `CAPTURE_LEFT_IT_DOWN` | error | 1 | 500 | 0.14.0 | Raised when the stack came down for the capture and the capture would not write. |

## `VPN` — traffic leaving the tunnel

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `VPN-1` | `LEAKING` | critical | 1 | 500 | 0.1.0 | Raised when the download client's egress does not match the tunnel. |
| `VPN-2` | `VPN_CONTAINER_DOWN` | error | 1 | 500 | 0.1.0 | Raised when the VPN container that should carry traffic is not running. |
| `VPN-3` | `CLIENT_ISOLATED` | warning | 1 | 500 | 0.1.0 | Raised when the client cannot reach the internet through the tunnel. |
| `VPN-4` | `NO_FORWARDED_PORT` | warning | 1 | 500 | 0.2.0 | Raised when port forwarding was asked for but the provider granted no port. |
| `VPN-5` | `KILLSWITCH_LEAKS` | error | 1 | 500 | 0.4.0 | The code a stack whose traffic survives its tunnel earns. |
| `VPN-6` | `TUNNEL_NOT_RESTORED` | error | 1 | 500 | 0.4.0 | The code a stack whose tunnel could not be put back earns. |
| `VPN-7` | `PORT_MISMATCH` | warning | 1 | 500 | 0.6.0 | Raised when the client is listening somewhere other than the forwarded port. |
| `VPN-8` | `NO_TUNNEL` | warning | 1 | 500 | 0.6.0 | Raised when torrents are configured with nothing containing them. |

## `WATCH` — guarding the data location

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `WATCH-1` | `NOTHING_TO_WATCH` | error | 1 | 500 | 0.2.0 | Raised when a watch is asked for but no data location is configured to watch. |
| `WATCH-2` | `ALREADY_GONE` | error | 1 | 500 | 0.2.0 | Raised when the data location is already gone when the watch is asked to start. |

## `WIRE` — choosing what fills a capability

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `WIRE-1` | `NO_SUCH_FILLER` | error | 1 | 404 | 0.16.0 | A capability was named that no service in this stack provides. |
| `WIRE-2` | `CANNOT_FILL` | error | 1 | 400 | 0.16.0 | The service named cannot do the thing it was asked to fill. |
| `WIRE-3` | `NOTHING_ASKS` | warning | 1 | 400 | 0.16.0 | Nothing in this stack asks for the capability, so a choice would change nothing. |
| `WIRE-4` | `CHOICE_UNWRITABLE` | error | 1 | 500 | 0.16.0 | The setting recording the choice could not be written. |
| `WIRE-5` | `WIRING_MOVED` | error | 1 | 400 | 0.17.0 | Raised when a choice answers an offer that was read against a wiring that has since moved. |
| `WIRE-6` | `UNREASONABLE` | error | 1 | 400 | 0.17.0 | Raised when the reason given for a choice is longer than a reason may be, or holds a line break or another control character. |
| `WIRE-7` | `ALREADY_FILLS` | advisory | 1 | 400 | 0.18.0 | Raised where the service chosen already fills the capability, so there is nothing to change. |

## `WIRING` — drift between services

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `WIRING-1` | `DRIFTED` | warning | 1 | 500 | 0.7.0 | Raised when a download client no longer files where lemonfiber wired it. |

## `WORD` — the glossary

| Code | Name | Severity | Exit | Status | Since | Summary |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- |
| `WORD-1` | `UNRECOGNISED` | error | 1 | 404 | 0.9.0 | A word this product does not explain, as a refusal that says what it does. Through the error model rather than a bare line, so it carries a code and a way forward like every other refusal — and the way forward is the list itself, which is short enough to be the answer rather than a pointer at one. It lies in the naming: the word is the whole of what was asked for, and there is no entry for it. A surface that reported this as its own failure would be telling a caller to try again at something that will never work. |

## Retired

Published once, raised by nothing, and never given to another problem: `QUOTA-3`.
