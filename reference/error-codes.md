# `lemonfiber` — error codes

Rendered from `contract/codes.json`, which is generated from the registry every code is
declared in. Run `just codes` to rewrite both.

Every code lemonfiber can raise, and nothing else. A code is a family and a number,
it is never recycled, and it is the token to search for. What each one means, and
what to do about it, is in `contract/codes.json` and written for operators at
<https://docs.lemonfiber.app/fixing/every-error-by-code/>. The last column is what a
household member's session is told in its place: a sentence and a remedy of its own, or
that it is a fault of the stack.

## `ACK` — answering a warning

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `ACK-1` | `NOT_WARNED` | error | 1 | 500 | 0.6.0 | Raised when an answer names something nothing is warning about. | A fault of the stack: not working right now, with whoever runs the house told. |

## `ADMIT` — who the web interface lets in

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `ADMIT-1` | `TOO_SHORT` | error | 1 | 500 | 0.10.0 | Raised when a password is too short to stand in front of this. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ADMIT-2` | `NO_SALT` | error | 1 | 500 | 0.10.0 | Raised when this machine will not supply the salt a record is made with. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ADMIT-3` | `MISTYPED` | error | 1 | 500 | 0.10.0 | Raised when the two answers were not the same word. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ADMIT-4` | `NOT_ADMITTED` | error | 1 | 403 | 0.17.0 | Raised when a request carried no token, session or key this run admits. | You're not signed in to the house. Sign in again. |
| `ADMIT-5` | `ELSEWHERE` | error | 1 | 403 | 0.17.0 | Raised when a request said it came from somewhere this server is not. | The house didn't recognise where this came from. Open the house from the place you were given. |
| `ADMIT-6` | `NOT_YOURS` | warning | 1 | 403 | 0.17.0 | Raised when an account asked for something that is not its to ask for. | That isn't yours to open. Ask whoever runs the house if you need it. |
| `ADMIT-7` | `UNCONFIRMED` | warning | 1 | 403 | 0.17.0 | Raised when the media server could not say whether an account is still one. | Your library isn't working right now, and whoever runs the house has been told. You're still signed in. Try again a little later. |
| `ADMIT-8` | `NOT_THE_PASSWORD` | error | 1 | 401 | 0.17.0 | Raised when the password offered at the door was wrong, or none is set. | That name and password don't match. Check them and try again. |
| `ADMIT-9` | `TOO_MANY_ATTEMPTS` | warning | 1 | 429 | 0.17.0 | Raised when the door has been given too many wrong passwords lately. | Too many tries didn't match, so the house is waiting a while. Wait a little, then try once more. |
| `ADMIT-10` | `NOT_A_PASSWORD` | error | 1 | 400 | 0.17.0 | Raised when what was offered at the door is not a password. | That sign-in couldn't be read. Enter your name and password, and try again. |
| `ADMIT-11` | `KEY_IN_THE_CLEAR` | error | 1 | 403 | 0.17.0 | Raised when a key arrived from another machine over a connection its pin does not verify. | This app reached the house over a connection that isn't private, so nothing was sent. Connect again from the app you paired with the house. |
| `ADMIT-12` | `NOT_FOR_A_KEY` | warning | 1 | 403 | 0.17.0 | Raised when a key asked for something its scope does not reach. | That isn't something this app may do for you. Ask whoever runs the house if you need it. |
| `ADMIT-13` | `NOT_OPEN` | error | 1 | 401 | 0.18.0 | Raised when a claim names an invitation that is not open. | This invitation can't be used any more. Ask whoever invited you for a new one. |
| `ADMIT-14` | `SHORT_CHOICE` | error | 1 | 400 | 0.18.0 | Raised when the password chosen at a claim is shorter than the least this takes. | That password is too short. Choose one of at least twelve characters. |

## `ASK` — putting a request to the web surface

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `ASK-1` | `NO_SUCH_ACTION` | error | 1 | 404 | 0.17.0 | Raised where no action goes by the name that was asked for. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-2` | `MISSING_ARGUMENT` | error | 1 | 400 | 0.17.0 | Raised where an action was not given an argument it needs. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-3` | `UNRECOGNISED_ARGUMENT` | error | 1 | 400 | 0.17.0 | Raised where an argument was given a value that names nothing. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-4` | `UNWANTED_ARGUMENT` | error | 1 | 400 | 0.17.0 | Raised where an action was given an argument its command has nowhere to put. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-5` | `ARGUMENTS_TOGETHER` | error | 1 | 400 | 0.17.0 | Raised where two arguments that each name a different request arrived together. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-6` | `NOT_ARGUMENTS` | error | 1 | 400 | 0.17.0 | Raised where the body of an action is not arguments it can read. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-7` | `NO_SUCH_JOB` | error | 1 | 404 | 0.17.0 | Raised where a job was asked about that this run did not start. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-8` | `NOT_AN_ANSWER` | error | 1 | 400 | 0.17.0 | Raised where the body of a setup step is not an answer it can read. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-9` | `NO_ENDPOINT` | error | 1 | 404 | 0.17.0 | Raised where a path under the endpoints is one no endpoint answers. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-10` | `WRONG_METHOD` | error | 1 | 405 | 0.17.0 | Raised where an endpoint was asked with a method it does not answer. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-11` | `NOT_A_KEY_REQUEST` | error | 1 | 400 | 0.17.0 | Raised where the body of a mint is not a key's name, scope, purpose and the password, or the body of a collection is not a claim alone. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-12` | `NOT_AN_IDEMPOTENCY_KEY` | error | 1 | 400 | 0.18.0 | Raised where an action's `Idempotency-Key` is not one to 255 visible characters, or is given more than once. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `ASK-13` | `IDEMPOTENCY_KEY_REUSED` | error | 1 | 400 | 0.18.0 | Raised where an `Idempotency-Key` already sent with one action and its arguments is sent with another. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |

## `BACKUP` — capturing your configuration

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `BACKUP-1` | `NO_ROOM` | error | 1 | 500 | 0.3.0 | Raised when there is not enough room to write a backup. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BACKUP-2` | `NOT_WRITTEN` | error | 1 | 500 | 0.3.0 | Raised when the backup archive could not be written. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BACKUP-3` | `NOT_MEASURED` | error | 1 | 500 | 0.3.0 | Raised when the room for a backup could not be measured. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BACKUP-4` | `STILL_RUNNING` | error | 1 | 500 | 0.9.0 | Raised when a capture could not be shown that nothing is writing to a database. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BACKUP-5` | `NOWHERE_TO_KEEP` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to keep an archive. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BACKUP-6` | `NOWHERE_KEPT` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to look for archives. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BACKUP-7` | `NOT_LISTED` | error | 1 | 500 | 0.9.0 | Raised when the directory the archives are kept in could not be read. | A fault of the stack: not working right now, with whoever runs the house told. |

## `BIND` — where the stack is actually listening

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `BIND-1` | `BEYOND_LOOPBACK` | error | 1 | 500 | 0.10.0 | Raised when a service the stack calls admin answers somewhere off this machine. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BIND-2` | `AROUND_THE_FIREWALL` | warning | 1 | 500 | 0.10.0 | Raised where a published port is reached without the host's own firewall rules being consulted. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BIND-3` | `DELIBERATE` | warning | 1 | 500 | 0.10.0 | Raised where an admin service answers off this machine and the operator has written down that they meant it to. | A fault of the stack: not working right now, with whoever runs the house told. |

## `BUNDLE` — the support bundle

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `BUNDLE-1` | `BUNDLE_LEAK` | critical | 1 | 500 | 0.7.0 | Raised when a bundle would still hold something that reads as a credential. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-2` | `BUNDLE_NO_ROOM` | error | 1 | 500 | 0.7.0 | Raised when there is not enough room to write a bundle. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-3` | `BUNDLE_UNWRITTEN` | error | 1 | 500 | 0.7.0 | Raised when the archive could not be written. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-4` | `BUNDLE_UNCONFIRMED` | error | 1 | 500 | 0.7.0 | Raised when a setting was asked to be shown as it is without that being confirmed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-5` | `BUNDLE_NO_MARKS` | error | 1 | 500 | 0.7.0 | Raised when the machine can offer no randomness to derive stand-ins from. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-6` | `NOWHERE_TO_KEEP` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to keep a bundle. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-7` | `NOWHERE_HELD` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to look for a bundle it kept. | A fault of the stack: not working right now, with whoever runs the house told. |
| `BUNDLE-8` | `BUNDLE_NOT_HELD` | error | 1 | 404 | 0.9.0 | Raised when a name does not name one of the bundles this run kept. | A fault of the stack: not working right now, with whoever runs the house told. |

## `CONFIG` — your settings

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `CONFIG-1` | `CONFIG_UNREADABLE` | error | 5 | 500 | 0.1.0 | Raised when configuration exists and cannot be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-2` | `CONFIG_NOT_WRITTEN` | error | 1 | 500 | 0.1.0 | Raised when configuration cannot be written. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-3` | `CONFIG_NOWHERE` | error | 1 | 500 | 0.1.0 | Raised when there is nowhere to keep configuration. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-4` | `CREDENTIALS_EXPOSED` | warning | 1 | 500 | 0.13.0 | Raised when a file holding a credential can be read by more than its owner. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-5` | `CONFIG_TOO_NEW` | error | 1 | 500 | 0.14.0 | Raised when configuration was written by a newer lemonfiber. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-6` | `CONFIG_SPANS_LINES` | error | 1 | 500 | 0.16.0 | Raised when a setting's key or value spans more than one line. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-8` | `OVERLAY_UNVETTED` | warning | 1 | 500 | 0.18.0 | Raised where the operator's overlay changes services, merged over the project lemonfiber writes and held to nothing it vets. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-9` | `OVERLAY_RETIRED` | warning | 1 | 500 | 0.18.0 | Raised where a path is still recorded under the retired overlay setting, which is no longer read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-10` | `COMPOSE_RETIRED` | warning | 1 | 500 | 0.18.0 | Raised where the project directory still holds a Compose file of the stack's own that lemonfiber no longer reads. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CONFIG-11` | `PART_UNCLAIMED` | warning | 1 | 500 | 0.18.0 | Raised where a part that belongs to whoever claims it first is held by nobody, is held by somebody other than lemonfiber, or could not be asked who holds it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `CRED` — credentials a service refuses

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `CRED-1` | `CREDENTIAL_REJECTED` | error | 1 | 500 | 0.2.0 | Raised when a service answers and refuses the credential it generated itself. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CRED-2` | `INDEXER_REJECTED` | error | 1 | 500 | 0.2.0 | Raised when the indexer answers and refuses the key it was given. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CRED-3` | `INDEXER_LIMITED` | warning | 1 | 500 | 0.2.0 | Raised when the indexer authenticates the key but cannot serve it right now. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CRED-4` | `REPLACEMENT_MOVED` | warning | 1 | 400 | 0.18.0 | Raised where a replacement of the media server's internal certificate answers an offer that is not the one a fresh reading builds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `CRED-5` | `ASTRAY` | warning | 1 | 500 | 0.18.0 | Raised where the media server holds a key or account under one of the stack's services' names that lemonfiber did not make or no longer knows. | A fault of the stack: not working right now, with whoever runs the house told. |

## `DECLINE` — the service that answers an invitation's decline

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `DECLINE-1` | `UNEXPLAINED` | warning | 1 | 500 | 0.17.0 | Raised when the decline service's key was used later than anything the service recorded doing with it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `DEVICE` — the keys a phone proves the operator with

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `DEVICE-1` | `NOT_A_DEVICE_NAME` | error | 1 | 400 | 0.18.0 | Raised when a device key is asked for under a word that cannot name one. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-2` | `DEVICE_NAME_TAKEN` | error | 1 | 409 | 0.18.0 | Raised when a device key is registered under a name another device key holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-3` | `PUBLIC_HALF_TAKEN` | error | 1 | 409 | 0.18.0 | Raised when a device key is registered with a public half another device key holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-4` | `NOT_P256` | error | 1 | 400 | 0.18.0 | Raised when the public half given is not a P-256 key. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-5` | `NO_ATTESTATION` | error | 1 | 400 | 0.18.0 | Raised when a device key is registered with no attestation. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-6` | `ATTESTATION_UNVERIFIED` | error | 1 | 400 | 0.18.0 | Raised when an attestation does not verify, or does not cover the public half. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-7` | `IN_SOFTWARE` | error | 1 | 400 | 0.18.0 | Raised when an attestation places the key in software, or says it was not made in the hardware that keeps it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-8` | `UNTRUSTED_APP` | error | 1 | 403 | 0.18.0 | Raised when an attestation names an app this stack does not trust. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-9` | `NOT_REGISTERED` | error | 1 | 404 | 0.18.0 | Raised when a device key named is not registered, or was revoked. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-10` | `CHALLENGE_NOT_STANDING` | warning | 1 | 403 | 0.18.0 | Raised when a mint carries a challenge that does not stand. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-11` | `SIGNATURE_WRONG` | error | 1 | 401 | 0.18.0 | Raised when a mint's signature does not verify against the device key. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-12` | `NO_CHALLENGE` | error | 1 | 500 | 0.18.0 | Raised when this machine would not supply the bytes a challenge is made of. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DEVICE-13` | `ATTESTATION_REVOKED` | error | 1 | 403 | 0.18.0 | Raised when an attestation's chain carries a certificate Google lists as revoked or suspended. | A fault of the stack: not working right now, with whoever runs the house told. |

## `DIAG` — narrowing a diagnosis

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `DIAG-1` | `NO_SUCH_CHECK` | error | 1 | 500 | 0.9.0 | Raised when a run is narrowed to a check nothing in this stack reports. | A fault of the stack: not working right now, with whoever runs the house told. |

## `DOCKER` — talking to the engine

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `DOCKER-1` | `ENGINE_UNREACHABLE` | error | 3 | 500 | 0.1.0 | Raised when the container engine cannot be reached. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-2` | `NO_SUCH_CONTAINER` | warning | 1 | 500 | 0.1.0 | Raised when a container that should exist does not. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-3` | `HOST_UNRESOLVED` | error | 1 | 500 | 0.15.0 | Raised when the host an endpoint names cannot be found on the network. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-4` | `HOST_REFUSED` | error | 1 | 500 | 0.15.0 | Raised when the host is found and refuses the connection. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-5` | `LOGIN_REJECTED` | error | 1 | 500 | 0.15.0 | Raised when the host is reached and will not accept the SSH login. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-6` | `ENDPOINT_UNSUPPORTED` | error | 1 | 500 | 0.15.0 | Raised when an endpoint names a transport this build cannot drive. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-7` | `UNKNOWN_CONTEXT` | error | 1 | 500 | 0.15.0 | Raised when a named Docker context is not one this machine records. | A fault of the stack: not working right now, with whoever runs the house told. |
| `DOCKER-8` | `HOST_SILENT` | error | 1 | 500 | 0.15.0 | Raised when a remote host does not answer for a reason nothing here recognises. | A fault of the stack: not working right now, with whoever runs the house told. |

## `ENV` — the container engine

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `ENV-1` | `DOCKER_ABSENT` | error | 1 | 500 | 0.1.0 | Raised when the Docker client is not installed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ENV-2` | `DAEMON_DOWN` | error | 1 | 500 | 0.1.0 | Raised when the Docker client is present but its daemon is not answering. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ENV-3` | `COMPOSE_UNUSABLE` | error | 1 | 500 | 0.1.0 | Raised when the Compose plugin is missing or too old to drive. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ENV-4` | `ENGINE_NOT_AT_BOOT` | warning | 1 | 500 | 0.15.0 | Raised when the container engine is confirmed not to start with this machine. One code for both arrangements it can be. What is not set differs by platform — Docker Desktop's open-at-login setting on macOS and Windows, the daemon's own unit on native Linux — and what has gone wrong is the same thing either way: nothing brings the engine up, so nothing reads the restart policies that would bring the containers back. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ENV-5` | `API_MISMATCH` | warning | 1 | 500 | 0.15.0 | Raised when this machine and the daemon speak different Docker API generations. | A fault of the stack: not working right now, with whoever runs the house told. |
| `ENV-6` | `NETWORKS_MIRRORED` | warning | 1 | 500 | 0.18.0 | Raised when the stack's engine is `OrbStack` with its network bridge on. | A fault of the stack: not working right now, with whoever runs the house told. |

## `FORM` — choosing what to run

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `FORM-1` | `NO_FORM_NAMED` | error | 1 | 400 | 0.1.0 | Raised when no form was named. | A fault of the stack: not working right now, with whoever runs the house told. |
| `FORM-2` | `NO_SUCH_FORM` | error | 1 | 404 | 0.1.0 | Raised when a named form is not declared by the stack. | A fault of the stack: not working right now, with whoever runs the house told. |
| `FORM-3` | `FORMS_CONFLICT` | error | 1 | 400 | 0.1.0 | Raised when forms that cannot be combined are named together. | A fault of the stack: not working right now, with whoever runs the house told. |
| `FORM-4` | `NOTHING_TO_RUN` | warning | 1 | 500 | 0.1.0 | Raised when narrowing leaves nothing to run. | A fault of the stack: not working right now, with whoever runs the house told. |

## `GATE` — the request gate

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `GATE-1` | `REFUSED` | warning | 1 | 500 | 0.17.0 | Raised when the request gate refused calls since the last diagnosis: the request service asked for something it has no use for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `GATE-2` | `LOST` | warning | 1 | 500 | 0.17.0 | Raised when more entries reached the gate's record between two diagnoses than it keeps, so some could not be reported. | A fault of the stack: not working right now, with whoever runs the house told. |

## `GONE` — taking lemonfiber off this machine

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `GONE-1` | `NEEDS_AGREEING` | error | 1 | 500 | 0.13.0 | Raised when the tier that takes the library was confirmed without its own agreement. | A fault of the stack: not working right now, with whoever runs the house told. |
| `GONE-2` | `ANOTHER_READING` | error | 1 | 400 | 0.13.0 | Raised when an agreement names a reading of this machine that is not the one standing now. | A fault of the stack: not working right now, with whoever runs the house told. |
| `GONE-3` | `NOT_BACKED_UP` | error | 1 | 500 | 0.14.0 | Raised when the backup a destructive removal takes first could not be taken. | A fault of the stack: not working right now, with whoever runs the house told. |

## `HANDOFF` — pointing somebody's device at the stack

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `HANDOFF-1` | `NOBODY_NAMED` | error | 1 | 500 | 0.17.0 | Said where the hand-off is for nobody: the name is blank, or only spaces. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HANDOFF-2` | `NO_MEDIA_SERVER` | error | 1 | 500 | 0.17.0 | Said where the stack holds no media server for a device to sign in to. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HANDOFF-3` | `NOT_SET_UP` | error | 1 | 500 | 0.17.0 | Said where the media server's own account was never recorded. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HANDOFF-4` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.17.0 | Said where the account named administers the media server. | A fault of the stack: not working right now, with whoever runs the house told. |

## `HOST` — keeping a command running without a terminal

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `HOST-1` | `NOTHING_TO_HOST_WITH` | warning | 1 | 500 | 0.12.0 | Raised where the platform has no service manager lemonfiber can configure. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HOST-2` | `DEFINITION_UNWRITABLE` | error | 1 | 500 | 0.12.0 | Raised where a service definition could not be written. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HOST-3` | `MANAGER_REFUSED` | error | 1 | 500 | 0.12.0 | Raised where the service manager refused what it was asked. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HOST-4` | `NOWHERE_TO_WRITE` | error | 1 | 500 | 0.12.0 | Raised when this machine will not say where it keeps its own files. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HOST-5` | `NO_PROGRAM` | error | 1 | 500 | 0.12.0 | Raised when this run cannot say where its own program is. | A fault of the stack: not working right now, with whoever runs the house told. |
| `HOST-6` | `NOTHING_NAMED_TO_GUARD` | error | 1 | 500 | 0.12.0 | Raised when the guard is to be hosted against nothing. | A fault of the stack: not working right now, with whoever runs the house told. |

## `INVITE` — offering somebody an account

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `INVITE-1` | `NO_MEDIA_SERVER` | error | 1 | 500 | 0.11.0 | Said where the stack holds no media server: there is nothing to make an account on. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-2` | `NO_CREDENTIAL` | error | 1 | 500 | 0.11.0 | Said where the admin credential was never recorded: nothing can be asked of the server. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-3` | `NOWHERE_TO_SEND` | error | 1 | 500 | 0.11.0 | Said where this machine has no address the household could arrive at. An invitation is an address somebody else types. Sending one built from a default would be sending a link that opens nothing, which is worse than saying there is none: the operator would learn it had failed from whoever they invited. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-4` | `NOBODY_NAMED` | error | 1 | 500 | 0.11.0 | Said where the invitation is for nobody: the name is blank, or only spaces. The media server refuses this too, in its own words, which are `400` and a link to the specification of that status. The operator asked for something reasonable and mistyped it, and is owed a sentence about the name rather than about HTTP. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-5` | `WOULD_NOT_RENEW` | error | 1 | 500 | 0.11.0 | Said where an invitation offered again could not be dated again, so its window is not real. The account is untouched and still theirs — what failed is the write that says when it was offered. Reported rather than glossed over because the message the operator is about to send promises a window, and this one would be counted from whenever the invitation was first made, which has already passed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-6` | `NO_LIBRARIES_READ` | error | 1 | 500 | 0.11.0 | Said where the media server would not say what libraries it holds. Refused rather than read as no libraries at all: a name matched against an empty list is a name that could not be found, and the operator would be told their library does not exist when what happened is that nobody could ask. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-7` | `NO_SUCH_LIBRARY` | error | 1 | 500 | 0.11.0 | Said where no library goes by a name that was given. The ones there are, named: the fix is one word, and the words are already in hand. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-8` | `WOULD_NOT_ALLOW` | error | 1 | 500 | 0.11.0 | Said where what the account may watch could not be written on it. A new account is taken back rather than left open, so the offer is refused whole; an existing one keeps what it already had. Said as which of the two is now true, because an operator told only that something failed would not know whether an open account is standing somewhere. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-9` | `UNRECORDED` | error | 1 | 500 | 0.17.0 | Said where the offer could not be written down. An invitation runs out a set time after it is offered, and one this machine has no date for is taken back the next time anybody is invited — so an offer that could not be dated is not made. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-10` | `UNGUARDED` | error | 1 | 500 | 0.17.0 | Said where the account could not be made one its person can claim: switched on, and bounded against guessing at its password. | A fault of the stack: not working right now, with whoever runs the house told. |
| `INVITE-11` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.17.0 | Said where the name given is the account that administers the media server. This is the account lemonfiber signs in as, and offering it would put a household member's limits on it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `KEPT` — what lemonfiber keeps here

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `KEPT-1` | `NOWHERE_KNOWN` | error | 1 | 500 | 0.10.0 | Raised when this run does not know where lemonfiber's own files go. | A fault of the stack: not working right now, with whoever runs the house told. |

## `KEY` — keys another program reaches the stack with

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `KEY-1` | `BAD_NAME` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for under a word that cannot name one. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-2` | `NOT_A_SCOPE` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for with a scope that is none of the three. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-3` | `NOT_A_PURPOSE` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for with a purpose that is none of the three. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-4` | `UNREADABLE` | error | 1 | 500 | 0.17.0 | Raised when the record of keys is there and does not read as keys. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-5` | `NAME_TAKEN` | error | 1 | 400 | 0.17.0 | Raised when a key is asked for under a name another key holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-6` | `NO_SUCH_MEMBER` | error | 1 | 404 | 0.17.0 | Raised when a member's key names an account the household does not hold. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-7` | `UNASKED` | error | 1 | 500 | 0.17.0 | Raised when the household could not be asked about a member's account. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-8` | `NOT_FOR_YOURSELF` | warning | 1 | 400 | 0.17.0 | Raised when a household member asks for a key, or a revoke, that is not theirs alone. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-9` | `NO_SECRET` | error | 1 | 500 | 0.17.0 | Raised when this machine would not supply the bytes a secret is made of. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-10` | `NO_SUCH_KEY` | error | 1 | 404 | 0.17.0 | Raised when no active key holds the name a revoke gave. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-11` | `MEMBERS_MAY_NOT_MINT` | warning | 1 | 409 | 0.17.0 | Raised when a household member asks to mint a key and the operator has not allowed members to. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-12` | `CLAIM_NOT_OPEN` | warning | 1 | 401 | 0.18.0 | Raised when a key is asked to be collected with a claim that collects nothing. | A fault of the stack: not working right now, with whoever runs the house told. |
| `KEY-13` | `UNLINKABLE` | warning | 1 | 409 | 0.18.0 | Raised when a key is asked to be minted as a claim and no address another machine reaches can go in its link. | A fault of the stack: not working right now, with whoever runs the house told. |

## `LIBRARY` — what the media server holds

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `LIBRARY-1` | `UNSEASONED` | warning | 1 | 500 | 0.18.0 | Raised where the media server holds a series' episodes and answers no seasons for it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `LIFE` — starting and stopping

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `LIFE-1` | `NEVER_SETTLED` | error | 4 | 500 | 0.1.0 | Raised when a service never reached a state that starting could accept. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-2` | `STILL_NEEDED` | error | 1 | 500 | 0.8.0 | Raised when stopping would take a service out from under a form still running. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-3` | `ALREADY_WORKING` | error | 1 | 409 | 0.8.0 | Another run is already working on this stack. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-4` | `REGISTRY_REFUSED` | error | 1 | 400 | 0.10.0 | Fetching images is switched off, so there was nothing to fetch with. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-5` | `NO_DATA_LOCATION` | error | 1 | 500 | 0.15.0 | Raised when a start was asked for over a data location that is not there. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-6` | `ABSENT_THERE` | error | 1 | 500 | 0.15.0 | Raised when the stack's own location is not on the machine being operated. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-7` | `ELSEWHERE_UNDERNEATH` | error | 1 | 500 | 0.17.0 | Raised when a path the stack mounts is not at the same path on the machine under the container lemonfiber runs in. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-8` | `NO_ENGINE_IN_HERE` | error | 1 | 500 | 0.17.0 | Raised when lemonfiber runs in a container that cannot reach the engine. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-9` | `NOT_ON_THIS_ENGINE` | error | 1 | 500 | 0.17.0 | Raised when the engine lemonfiber reaches from a container does not know that container. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-10` | `RESTART_MOVED` | warning | 1 | 400 | 0.18.0 | Raised where a restart names an offer that is not the one a fresh look at the stack builds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-11` | `MOUNT_LINKED` | error | 1 | 409 | 0.18.0 | Raised when a start finds a service's mount source reached through a link. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-12` | `OFFLINE_INSTALL_MOVED` | warning | 1 | 400 | 0.18.0 | Raised where an `up` or a `switch` names an offer that is not the install a fresh look at the stack offers. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-13` | `OFFLINE_INSTALL_UNANSWERED` | warning | 1 | 409 | 0.18.0 | Raised where a start would install a part this build carries and nobody said yes. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-14` | `OFFLINE_INSTALL_UNHELD` | error | 1 | 409 | 0.18.0 | Raised where an install a start was agreed to did not hold. | A fault of the stack: not working right now, with whoever runs the house told. |
| `LIFE-15` | `OFFLINE_INSTALL_UNREAD` | warning | 1 | 409 | 0.18.0 | Raised where a start would install a part whose install could not be read. | A fault of the stack: not working right now, with whoever runs the house told. |

## `MIGRATE` — taking over a setup already here

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `MIGRATE-1` | `OFFER_MOVED` | warning | 1 | 400 | 0.17.0 | Raised when a replacement was agreed to for an offer that is not the one standing now. | A fault of the stack: not working right now, with whoever runs the house told. |

## `NOTIFY` — where alerts are delivered

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `NOTIFY-1` | `NOT_A_BACKEND_NAME` | error | 1 | 400 | 0.18.0 | Raised when a back-end is added under a word that cannot name one. | A fault of the stack: not working right now, with whoever runs the house told. |
| `NOTIFY-2` | `BACKEND_NAME_TAKEN` | error | 1 | 409 | 0.18.0 | Raised when a back-end is added under a name another holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `NOTIFY-3` | `NO_SUCH_BACKEND` | error | 1 | 404 | 0.18.0 | Raised when no back-end holds the name asked about. | That way of being told isn't one the house offers. Choose from the ones offered. |
| `NOTIFY-4` | `HOSTED_UNACKNOWLEDGED` | error | 1 | 400 | 0.18.0 | Raised when a back-end on a host that is not the household's own is added without saying so. | A fault of the stack: not working right now, with whoever runs the house told. |
| `NOTIFY-5` | `NOT_A_BACKEND_ADDRESS` | error | 1 | 400 | 0.18.0 | Raised when a back-end's address is not one it can be reached at. | A fault of the stack: not working right now, with whoever runs the house told. |
| `NOTIFY-6` | `BACKEND_INCOMPLETE` | error | 1 | 400 | 0.18.0 | Raised when a back-end is added without what its kind needs. | A fault of the stack: not working right now, with whoever runs the house told. |
| `NOTIFY-7` | `NOT_A_RETRY_SCHEDULE` | error | 1 | 400 | 0.18.0 | Raised when a back-end's attempts and delays do not agree. | A fault of the stack: not working right now, with whoever runs the house told. |
| `NOTIFY-8` | `NOT_OFFERED` | error | 1 | 400 | 0.18.0 | Raised when a member chooses a back-end that is not offered to members. | That way of being told isn't offered to everybody. Choose from the ones offered. |
| `NOTIFY-9` | `NO_SUCH_WAKE` | error | 1 | 404 | 0.18.0 | Raised when a phone confirms a wake nobody sent it. | The house sent this phone nothing to confirm. Nothing needs doing. |
| `NOTIFY-10` | `EVENTS_REFUSED` | error | 1 | 401 | 0.18.0 | Raised when an event arrives without the key the core gave its sender. | A fault of the stack: not working right now, with whoever runs the house told. |

## `PAIR` — pairing a phone with the stack

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `PAIR-1` | `NOWHERE` | error | 1 | 500 | 0.17.0 | Raised when there is nowhere to keep what pairing a phone needs. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PAIR-2` | `NOT_SERVED` | error | 1 | 500 | 0.17.0 | Raised when the web surface has not been served encrypted on the network. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PAIR-3` | `NO_CERTIFICATE` | error | 1 | 500 | 0.17.0 | Raised when the certificate the surface presents cannot be read or made. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PAIR-4` | `NO_ADDRESS` | error | 1 | 500 | 0.17.0 | Raised when this machine has no address a phone could reach it at. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PAIR-5` | `UNNAMED` | error | 1 | 500 | 0.17.0 | Raised when the stack's identifier cannot be read or made. | A fault of the stack: not working right now, with whoever runs the house told. |

## `PLAY` — playing what the household holds

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `PLAY-1` | `NOT_AN_ITEM` | error | 1 | 400 | 0.18.0 | Said where the title named is not something the media server could hold. | That isn't a title the house holds. Choose a title from your shelf. |
| `PLAY-2` | `NOT_ON_THEIR_SHELF` | error | 1 | 404 | 0.18.0 | Said where the title is not one this member may watch, or is not there at all. | That title isn't on your shelf. Choose a title from your shelf. |
| `PLAY-3` | `NOT_A_DEVICE` | error | 1 | 400 | 0.18.0 | Said where what names a member's device is not a device id. | The house didn't recognise this device. Try again, and tell whoever runs the house if it keeps happening. |
| `PLAY-4` | `NOTHING_TO_PLAY_FROM` | error | 1 | 503 | 0.18.0 | Said where there is no media server to play from, or it was never set up. | This house isn't set up for watching yet. Ask whoever runs the house about it. |
| `PLAY-5` | `SERVER_SILENT` | error | 1 | 502 | 0.18.0 | Said where the media server would not answer for a member. | Your library isn't working right now, and whoever runs the house has been told. Nothing was changed. Try again a little later. |
| `PLAY-6` | `NOBODY_NAMED` | error | 1 | 400 | 0.18.0 | Said where nobody is named for something only a member can be. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLAY-7` | `NOT_IN_THE_HOUSEHOLD` | error | 1 | 404 | 0.18.0 | Said where the member named is not somebody in the household. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLAY-8` | `SIGNS_NO_DEVICE_IN` | error | 1 | 503 | 0.18.0 | Said where the media server will not sign a device in by code. | This device can't be signed in with a code right now. Ask whoever runs the house about it. |
| `PLAY-9` | `NO_SUCH_PICTURE` | error | 1 | 404 | 0.18.0 | Said where a title on the member's shelf has no picture of the kind asked for. | That title has no picture to show. You can still choose it by its name. |
| `PLAY-10` | `MISNAMED` | warning | 1 | 502 | 0.18.0 | Said where the media server named an item by something that is not an item id. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLAY-11` | `UNDECLARED` | warning | 1 | 502 | 0.18.0 | Said where the media server would not say where it serves each title, or said it in a way the front door cannot hold it to. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLAY-12` | `DOOR_UNREADY` | warning | 1 | 503 | 0.18.0 | Said where the front door has no certificate to present: no stack directory, none made yet, or one that cannot be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLAY-13` | `NO_HOUSEHOLD_ADDRESS` | warning | 1 | 503 | 0.18.0 | Said where the address the household reaches this machine at is not known. | A fault of the stack: not working right now, with whoever runs the house told. |

## `PLUGIN` — installing and running plugins

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `PLUGIN-1` | `CONTRIBUTED_FAILED` | error | 1 | 409 | 0.16.0 | A check a plugin contributed did not hold. One code for all of them rather than one per plugin, because a code is a stable thing an operator searches for and a plugin's own name is not this build's to mint one from. Which check and which plugin is on the finding, where it can be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-2` | `UNREADABLE` | error | 1 | 404 | 0.16.0 | The source names no plugin this build can read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-3` | `REFUSED` | error | 1 | 400 | 0.16.0 | The manifest is read and this build refuses what it declares. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-4` | `UNRECORDED` | error | 1 | 500 | 0.16.0 | The record of what is installed cannot be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-5` | `ALREADY` | error | 1 | 400 | 0.16.0 | The plugin is installed already. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-6` | `NOWHERE` | error | 1 | 500 | 0.16.0 | There is no stack on this machine to put a plugin's container in. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-7` | `UNWRITABLE` | error | 1 | 500 | 0.16.0 | A directory or a document the install decided on would not land. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-8` | `UNRECORDABLE` | error | 1 | 500 | 0.16.0 | The wiring went down and the record of what is installed did not. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-9` | `UNPROVED` | error | 1 | 500 | 0.16.0 | The plugin's own service would not start, so nothing about it could be proved. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-10` | `NOTHING_TO_REMOVE` | error | 1 | 404 | 0.16.0 | Nothing by that name is installed on this machine. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-11` | `NOTHING_TO_UPDATE` | error | 1 | 404 | 0.16.0 | Nothing by that id is installed, so there is no version to replace. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-12` | `STUCK` | error | 1 | 500 | 0.16.0 | The version installed would not come off, so nothing else was touched. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-13` | `ANSWERED` | error | 1 | 400 | 0.16.0 | Raised when a plugin's service would answer on a label another plugin's already does. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-14` | `TWO_SOURCES` | error | 1 | 400 | 0.17.0 | Raised when a plugin is installed from a source other than the one its name is already installed from. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-15` | `SOURCE_OFF` | error | 1 | 400 | 0.17.0 | Raised when a plugin is named from a git source and fetching from one is switched off. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-16` | `UNFETCHED` | error | 1 | 500 | 0.17.0 | Raised when a git source could not be reached or would not hand over a revision. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-17` | `NO_REVISION` | error | 1 | 404 | 0.17.0 | Raised when a git source holds no branch, tag or commit by the name given. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-18` | `CATALOGUE_OFF` | error | 1 | 400 | 0.17.0 | Raised when a plugin is installed by name and asking the catalogue is switched off. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-19` | `CATALOGUE_UNREACHABLE` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index or its signature could not be fetched. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-20` | `SIGNATURE_UNVERIFIED` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index has no signature, one that does not verify, or none this build carries a key to check. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-21` | `CATALOGUE_UNREADABLE` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index verified and is not one this build reads. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-22` | `NOT_CATALOGUED` | error | 1 | 404 | 0.17.0 | Raised when the catalogue holds no plugin by the name given. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-23` | `NOT_AS_REVIEWED` | error | 1 | 500 | 0.17.0 | Raised when what the catalogue's origin served is not what the catalogue reviewed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-24` | `SPELLED_ALIKE` | error | 1 | 400 | 0.17.0 | Raised when a plugin's service would be named, where lemonfiber keeps what a service holds, as another installed plugin's service already is. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-25` | `PLUGIN_OFFER_MOVED` | error | 1 | 400 | 0.17.0 | Raised when an install, an update or a removal answers an offer that was read against a plugin, a stack or a record that has since moved. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-26` | `UNAPPROVED` | error | 1 | 400 | 0.17.0 | Raised when a value a recipe would carry to a destination, or the egress guard's shape a service would take, was not approved as itself, or an approval names something the reading does not list. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-27` | `ANOTHER_PLUGIN` | error | 1 | 400 | 0.17.0 | Raised when the source an update names holds a different plugin from the one it was asked to update. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-28` | `OCCUPIED` | error | 1 | 400 | 0.17.0 | Raised when a plugin's service would take a name, a port or a label something already on this machine holds: a service of the stack or of the operator's overlay, another plugin's port, or a site in the proxy's live configuration. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-29` | `CATALOGUE_REPLACED` | error | 1 | 500 | 0.17.0 | Raised when the catalogue's index verifies and is older than the newest one this machine has verified. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-30` | `NEWEST_UNKEPT` | error | 1 | 500 | 0.17.0 | Raised when the record of the newest catalogue index this machine verified cannot be read or written. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-31` | `SCHEME_REFUSED` | error | 1 | 400 | 0.17.0 | Raised when a git source is named over a transport other than https, before anything is asked of it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-32` | `ADDRESS_REFUSED` | error | 1 | 400 | 0.17.0 | Raised when a git source's host is, or stands for, an address on this machine or on a network of its own: loopback, private, link-local or unspecified. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-33` | `HEADER_NAMED` | error | 1 | 400 | 0.17.0 | Raised when a recipe substitutes a value into a header's name, which is a fixed identifier of the protocol and written out; the manifest's every other fault is listed beside it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-34` | `INPUT_UNMATCHED` | error | 1 | 400 | 0.18.0 | Raised when a recipe of the act asks the operator for a value that was not given, or a value was given that no recipe of the act asks for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-35` | `CALL_REFUSED` | error | 1 | 500 | 0.18.0 | Raised when a recipe's call was not sent because its host stands for an address not out on the internet; the install or update was put back. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-36` | `STEP_FAILED` | error | 1 | 500 | 0.18.0 | Raised when a recipe's step failed any other way — nothing answered, the answer was not the one it expects, a capture found nothing, or the answer was larger than a recipe reads; the install or update was put back. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-37` | `PATH_NOT_PLAIN` | error | 1 | 400 | 0.18.0 | Raised when a recipe's call path is not a plain absolute path; the manifest's every other fault is listed beside it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-38` | `VALUE_WITHHELD` | error | 1 | 400 | 0.18.0 | Raised when a recipe's call was not sent because a value it carries may not go where it was going: not where its pairs say, not back to the service a credential belongs to, or outside without its approval; the install or update was put back. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-39` | `NOTHING_TO_PROVE` | error | 1 | 404 | 0.18.0 | Nothing by that id is installed, so there is nothing to prove again. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-40` | `STRANGER_REPLACING` | error | 1 | 403 | 0.18.0 | Raised when a plugin that is not first-party declares a service replacing one the stack ships. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-41` | `PROOF_RUN_FAILED` | error | 1 | 409 | 0.18.0 | Raised when a proof an installed plugin declares, run as the image's own program, does not hold in the doctor. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-42` | `MOVE_OFFERED` | warning | 1 | 409 | 0.18.0 | Raised when a part this build installs as a plugin still runs as the stack's own service, so moving it in place is on offer. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-43` | `PROOF_ASKED_FAILED` | error | 1 | 409 | 0.18.0 | Raised when a proof an installed plugin declares, asked of its service, does not hold in the doctor. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-44` | `NOTHING_TO_RUN` | error | 1 | 404 | 0.18.0 | Raised when a run names a plugin that is not installed, or a demand recipe the plugin does not declare. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-45` | `MANIFEST_UNKEPT` | error | 1 | 409 | 0.18.0 | Raised when the manifest kept for an installed plugin is missing or is not the one that was installed, so its demand recipes cannot be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-46` | `UNAIMED_CROSSING` | error | 1 | 403 | 0.18.0 | Raised when a third-party plugin declares a secret of another service that names no capability of its own it is for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-47` | `NO_GRANT` | error | 1 | 404 | 0.18.0 | Raised when a grant asked to be revoked is not one the plugin holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-48` | `EARLIER_TAKEN` | error | 1 | 400 | 0.18.0 | Raised when a plugin's setting or secret names, as the variable an earlier release kept its value under, one another installed plugin's setting or secret names. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-49` | `ANOTHER_SOURCE` | error | 1 | 403 | 0.18.0 | Raised when a plugin carrying the id of a part this build pins comes from anywhere but that pin. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PLUGIN-50` | `UNCONFORMING` | error | 1 | 400 | 0.18.0 | Raised when a case of a contract a plugin's adapter speaks fails, or no recording the plugin ships answers it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `PROC` — the program underneath

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `PROC-1` | `MISSING_PROGRAM` | error | 3 | 500 | 0.1.0 | Raised when the program a subprocess needs is missing. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROC-2` | `UNUSABLE_PROGRAM` | error | 1 | 500 | 0.1.0 | Raised when a program exists but will not start. | A fault of the stack: not working right now, with whoever runs the house told. |

## `PROVIDER` — accounts and indexers

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `PROVIDER-1` | `PROVIDER_EMPTY` | error | 1 | 500 | 0.7.0 | Raised when an account has nothing left to serve. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-2` | `PROVIDER_LOW` | warning | 1 | 500 | 0.7.0 | Raised when an account is running out, with time left to act. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-3` | `PROVIDER_ENDING` | warning | 1 | 500 | 0.7.0 | Raised when the subscription behind an account ends soon. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-4` | `INDEXER_RESTED` | warning | 1 | 500 | 0.7.0 | Raised when an indexer has been failing and its aggregator has rested it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-5` | `INDEXERS_ALL_FAILING` | error | 1 | 500 | 0.7.0 | Raised when every indexer is failing at once. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-6` | `PROVIDER_REFUSED` | error | 1 | 500 | 0.7.0 | Raised when an account refuses the credential the client offers it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-7` | `PROVIDER_SILENT` | warning | 1 | 500 | 0.7.0 | Raised when an account has stopped answering the client entirely. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-8` | `PROVIDER_CROWDED` | warning | 1 | 500 | 0.7.0 | Raised when the client is set to open more connections than an account allows. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PROVIDER-9` | `INDEXER_CAPPED` | warning | 1 | 500 | 0.7.0 | Raised when an indexer has spent the allowance recorded against it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `PUSH` — waking a phone that asked to be woken

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `PUSH-1` | `NOT_A_PUSH_NAME` | error | 1 | 400 | 0.18.0 | Raised when a push endpoint is registered under a word that cannot name one. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PUSH-2` | `PUSH_NAME_TAKEN` | error | 1 | 409 | 0.18.0 | Raised when a push endpoint is registered under a name another holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PUSH-3` | `PUSH_NOT_HTTPS` | error | 1 | 400 | 0.18.0 | Raised when a push endpoint's address is not an `https` address. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PUSH-4` | `PUSH_KEYS_UNREADABLE` | error | 1 | 400 | 0.18.0 | Raised when the keys a push endpoint gave cannot be encrypted to. | A fault of the stack: not working right now, with whoever runs the house told. |
| `PUSH-5` | `PUSH_NOT_REGISTERED` | error | 1 | 404 | 0.18.0 | Raised when no push endpoint holds the name asked about. | The house doesn't know this phone. Turn notifications on again in the app. |

## `QUAL` — quality against what is available

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `QUAL-1` | `HEADROOM_LOW` | warning | 1 | 500 | 0.4.0 | Raised when the free space holds too little content at the chosen quality. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUAL-2` | `PRESET_UNMET` | warning | 1 | 500 | 0.4.0 | Raised when releases exist but the profile — the chosen quality included — wants none of them. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUAL-3` | `NONE_AVAILABLE` | warning | 1 | 500 | 0.4.0 | Raised when a clean search turns up nothing at all for wanted content. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUAL-4` | `UNREAPPLIED` | error | 1 | 502 | 0.18.0 | Raised where a quality sync speaking its contract would not show or put back the preset over a hand edit. | A fault of the stack: not working right now, with whoever runs the house told. |

## `QUOTA` — what the household may ask for

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `QUOTA-1` | `UNREACHABLE` | error | 1 | 500 | 0.12.0 | Raised where the request service would not answer, so nothing was changed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-2` | `NO_LIMIT` | error | 1 | 400 | 0.12.0 | Raised where a policy that lives inside a limit was chosen without one. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-4` | `NOT_WAITING` | error | 1 | 404 | 0.12.0 | Raised where the request named is not one that is waiting on anybody. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-5` | `NO_REASON` | error | 1 | 400 | 0.12.0 | Raised where a request was turned down and the reason said nothing. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-6` | `NOBODY` | error | 1 | 404 | 0.12.0 | Raised where nobody in the household goes by the name that was given. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-7` | `NEVER_HERE` | warning | 1 | 404 | 0.12.0 | Raised where the request service holds no account for somebody who has one here. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-8` | `NOTHING_AGREED` | error | 1 | 400 | 0.12.0 | Raised where a run was asked to close what has waited too long and the household has never said how long that is. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-9` | `TOO_SOON` | error | 1 | 400 | 0.12.0 | Raised where the period named would close a request nobody was ever reminded about. | A fault of the stack: not working right now, with whoever runs the house told. |
| `QUOTA-10` | `UNRECORDED` | warning | 1 | 500 | 0.18.0 | Said where what the disk is holding back could not be written down beside the settings. | A fault of the stack: not working right now, with whoever runs the house told. |

## `RATE` — holding the stack to a share of the line

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `RATE-1` | `NOTHING_MEASURED` | error | 1 | 400 | 0.12.0 | Raised when a limit is expressed as a share of a line nothing has measured. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RATE-2` | `NO_ZONE` | error | 1 | 400 | 0.12.0 | Raised when a schedule is asked for and nothing says which zone the clients would read it in. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RATE-3` | `LIMIT_UNREADABLE` | error | 1 | 400 | 0.12.0 | Raised when what was asked for could not be read as a limit, a window or a cap. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RATE-4` | `NOTHING_TO_LIMIT` | error | 1 | 400 | 0.12.0 | Raised when there is no download client to limit. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RATE-5` | `NOTHING_TO_PAUSE` | error | 1 | 400 | 0.17.0 | Raised when there is no download client to pause or resume. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RATE-6` | `PAUSING_MOVED` | warning | 1 | 400 | 0.18.0 | Raised where pausing or resuming the download clients names an offer that is not the one a fresh look at them builds. | A fault of the stack: not working right now, with whoever runs the house told. |

## `READ` — asking the web surface a question

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `READ-1` | `UNWANTED` | error | 1 | 400 | 0.9.0 | Raised where a read was given a parameter its answer has nowhere to put. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-2` | `REPEATED` | error | 1 | 400 | 0.9.0 | Raised where a parameter carrying one value was given more than once. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-3` | `NO_SUCH_READ` | error | 1 | 404 | 0.17.0 | Raised where no read goes by the name that was asked for. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-4` | `NO_TERM` | error | 1 | 400 | 0.17.0 | Raised where a trace was asked for and named nothing to follow. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-5` | `NOT_A_SEASON` | error | 1 | 400 | 0.17.0 | Raised where the season to narrow a trace to is not a number. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-6` | `NO_SETTING` | error | 1 | 400 | 0.17.0 | Raised where a setting was asked for by an empty name. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-7` | `NO_MEMBER` | error | 1 | 400 | 0.17.0 | Raised where a household member was asked for by an empty name. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-8` | `NO_SHELF_WITHOUT_A_MEMBER` | error | 1 | 400 | 0.17.0 | Raised where a shelf was asked for and nobody was named whose it is. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-9` | `NOT_A_COUNT` | error | 1 | 400 | 0.17.0 | Raised where how many holdings to answer with is not a whole number. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-10` | `TOO_MANY_AT_ONCE` | error | 1 | 400 | 0.17.0 | Raised where more holdings were asked for than one read answers with. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-11` | `NO_SUCH_GROUP` | error | 1 | 400 | 0.17.0 | Raised where a diagnosis was narrowed to a group or check that is not one. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-12` | `NO_SUCH_REMOVAL` | error | 1 | 400 | 0.17.0 | Raised where a removal was named that is none of the four there are. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-13` | `NO_UPDATE_OBJECT` | error | 1 | 400 | 0.17.0 | Raised where moving forward was asked about and neither stack nor self named. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-14` | `NOT_A_LINE_COUNT` | error | 1 | 400 | 0.17.0 | Raised where how many log lines to begin with is not a number within the ceiling. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-15` | `NOT_A_CHOICE` | error | 1 | 400 | 0.17.0 | Raised where a parameter that takes a yes or a no is neither true nor false. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |
| `READ-16` | `MEMBER_AND_DEFAULTS` | error | 1 | 400 | 0.17.0 | Raised where a household read named a member and asked for the household's defaults as well. | The house didn't understand what was asked. Try again, and tell whoever runs the house if it keeps happening. |

## `REHEARSE` — asking what a command would do

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `REHEARSE-1` | `CANNOT` | error | 1 | 400 | 0.15.0 | The flag cannot be honoured by this command, and never will be. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REHEARSE-2` | `NOT_YET` | error | 1 | 400 | 0.15.0 | The flag is not honoured by this command yet. | A fault of the stack: not working right now, with whoever runs the house told. |

## `REISSUE` — letting somebody set a new password

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `REISSUE-1` | `UNREADABLE` | error | 1 | 500 | 0.11.0 | Said where the media server will not say who holds an account. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REISSUE-2` | `NOBODY_HERE` | error | 1 | 500 | 0.11.0 | Said where nobody by that name is in the household. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REISSUE-3` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.11.0 | Said where the account named administers the server. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REISSUE-4` | `WOULD_NOT_REISSUE` | error | 1 | 500 | 0.11.0 | Said where the media server refused to make the account claimable again. | A fault of the stack: not working right now, with whoever runs the house told. |

## `REMOVE` — taking somebody out of the household

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `REMOVE-1` | `NOBODY_NAMED` | error | 1 | 500 | 0.11.0 | Said where the removal is for nobody: the name is blank, or only spaces. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REMOVE-2` | `NO_MEDIA_SERVER` | error | 1 | 500 | 0.11.0 | Said where the stack holds no media server: there is no account to remove. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REMOVE-3` | `UNREADABLE` | error | 1 | 500 | 0.11.0 | Said where the media server will not say who it holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REMOVE-4` | `NOBODY_HERE` | error | 1 | 500 | 0.11.0 | Said where nobody by that name is in the household. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REMOVE-5` | `RUNS_THE_SERVER` | error | 1 | 500 | 0.11.0 | Said where the account named administers the server. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REMOVE-6` | `WOULD_NOT_REMOVE` | error | 1 | 500 | 0.11.0 | Said where the media server refused to remove the account. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REMOVE-7` | `REMOVAL_MOVED` | error | 1 | 400 | 0.18.0 | Said where the offer answered is not the one a reading of the removal makes now. | A fault of the stack: not working right now, with whoever runs the house told. |

## `REPAIR` — putting right what the doctor found

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `REPAIR-1` | `STALE` | warning | 1 | 400 | 0.9.0 | Raised when consent was given for an offer that no longer stands. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REPAIR-2` | `NOWHERE_TO_LOOK` | error | 1 | 500 | 0.9.0 | Raised when a run cannot say where lemonfiber's own files are. | A fault of the stack: not working right now, with whoever runs the house told. |
| `REPAIR-3` | `OFFER_CANNOT_DISTURB` | error | 1 | 500 | 0.9.0 | Raised when a run that may not act was asked for the checks that disturb. | A fault of the stack: not working right now, with whoever runs the house told. |

## `RESTORE` — putting configuration back

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `RESTORE-1` | `CORRUPT` | error | 1 | 500 | 0.3.0 | Raised when a backup archive cannot be read to decide a restore. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-2` | `TOO_NEW` | error | 1 | 500 | 0.3.0 | Raised when an archive was written by a newer lemonfiber than this one. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-3` | `INCOMPATIBLE` | error | 1 | 500 | 0.3.0 | Raised when an archive's format cannot be restored by this build. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-4` | `UNSAFE` | critical | 1 | 500 | 0.3.0 | Raised when an archive holds a member that would be written outside its area. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-5` | `NEEDS_REPOINT` | warning | 1 | 500 | 0.3.0 | Raised when a restore onto a different data root awaits the operator's consent. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-6` | `NOT_RESTORED` | error | 1 | 500 | 0.3.0 | Raised when an archive could not be unpacked. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-7` | `STILL_RUNNING` | error | 1 | 500 | 0.9.0 | Raised when a restore could not be shown that nothing is writing to a database. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-8` | `NOT_KEPT_HERE` | error | 1 | 500 | 0.9.0 | Raised when a name does not name one of the backups this machine kept. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-9` | `NOWHERE_KEPT` | error | 1 | 500 | 0.9.0 | Raised when this run has nowhere it knows to look for an archive. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-10` | `NOT_REPOINTED` | error | 1 | 500 | 0.9.0 | Raised when the restored settings could not be pointed at this machine's data root. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-11` | `MOVED_ON` | warning | 1 | 400 | 0.9.0 | Raised when consent was given for a listing that no longer stands. | A fault of the stack: not working right now, with whoever runs the house told. |
| `RESTORE-12` | `NOT_OURS` | error | 1 | 500 | 0.14.0 | Raised when the archive holds trees lemonfiber does not manage. | A fault of the stack: not working right now, with whoever runs the house told. |

## `SEED` — wiring the services together

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `SEED-1` | `SERVICE_UNAVAILABLE` | warning | 1 | 500 | 0.1.0 | Raised when a service is not answering yet. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SEED-2` | `SERVICE_UNAUTHORISED` | error | 1 | 500 | 0.1.0 | Raised when a service rejects the credential lemonfiber holds. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SEED-3` | `SERVICE_REFUSED` | error | 1 | 500 | 0.1.0 | Raised when a service answers with something unusable. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SEED-4` | `SERVICE_UNSUPPORTED` | error | 1 | 500 | 0.4.0 | Raised when a service answers that it does not offer what was asked. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SEED-5` | `SERVICE_DISALLOWED` | error | 1 | 500 | 0.18.0 | Raised when a service refuses what was asked because it never does what was asked. | A fault of the stack: not working right now, with whoever runs the house told. |

## `SERVE` — the web surface

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `SERVE-1` | `ADDRESS_TAKEN` | error | 1 | 500 | 0.9.0 | Raised when the address the surface was asked to serve on cannot be taken. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-2` | `NO_TOKEN` | error | 1 | 500 | 0.9.0 | Raised when this machine will not supply the randomness a token is made of. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-3` | `NO_PASSWORD` | error | 1 | 500 | 0.10.0 | Raised when the network was asked for and nothing here can say who is knocking. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-4` | `UNSETTLED_PORT` | error | 1 | 500 | 0.17.0 | Raised when serving encrypted was asked for with no port that stays the same. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-5` | `NO_CERTIFICATE` | error | 1 | 500 | 0.17.0 | Raised when the certificate to serve encrypted with cannot be read or made. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-6` | `UNRENDERABLE` | error | 1 | 500 | 0.17.0 | Raised when an answer could not be rendered. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-7` | `NO_JOB_NAME` | error | 1 | 500 | 0.17.0 | Raised when this machine will not supply the randomness a job is named with. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SERVE-8` | `UNANSWERED` | error | 1 | 500 | 0.18.0 | Raised when an action's work ended before it had an answer to give. | A fault of the stack: not working right now, with whoever runs the house told. |

## `SETTING` — the settings a service reads

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `SETTING-1` | `NO_SUCH_SETTING` | error | 5 | 404 | 0.18.0 | Raised where nothing in this stack reads a setting by the name given. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-2` | `A_SECRET` | error | 5 | 400 | 0.18.0 | Raised where the name given is a secret's, which this command never writes. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-3` | `TAKEN_ELSEWHERE` | error | 5 | 400 | 0.18.0 | Raised where the name given belongs to the other of the two commands that write settings. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-4` | `UNWRITABLE_VALUE` | error | 5 | 400 | 0.18.0 | Raised where a value cannot be written as the setting asked for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-5` | `SETTINGS_MOVED` | error | 1 | 400 | 0.18.0 | Raised when a change answers an offer read against settings that have since moved. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-6` | `ALREADY_HOLDS` | advisory | 1 | 400 | 0.18.0 | Raised where the setting already holds what the change asks for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-7` | `OUTSIDE_ITS_CHOICES` | error | 5 | 400 | 0.18.0 | Raised where a change would leave a setting holding a value outside its choices. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-8` | `HELD_BACK` | warning | 1 | 409 | 0.18.0 | Raised where a service is held back until the operator gives what it needs to start. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-9` | `OWNER_SET_UP` | error | 5 | 409 | 0.18.0 | Raised where `PLEX_LINK_MODE` would enter or leave `owner` for a request service already set up. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETTING-10` | `OWNER_HELD` | warning | 1 | 409 | 0.18.0 | Raised on every diagnosis while `PLEX_LINK_MODE` is `owner`, as a standing notice. | A fault of the stack: not working right now, with whoever runs the house told. |

## `SETUP` — the first run

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `SETUP-1` | `NOT_REVIEWED` | error | 1 | 400 | 0.2.0 | Raised when apply is asked for before the answers have been reviewed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-2` | `DIR_NOT_MADE` | error | 1 | 500 | 0.2.0 | Raised when the operator's chosen data directory cannot be created. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-3` | `NOT_REMOVED` | error | 1 | 500 | 0.2.0 | Raised when a directory from an interrupted apply could not be removed. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-4` | `NEEDS_SERVICE` | error | 1 | 500 | 0.2.0 | Raised when reversing needs the service that made a change. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-5` | `DOES_NOT_APPLY` | error | 1 | 400 | 0.2.0 | Raised when an answer is not meaningful on the platform setup is running on. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-6` | `ALREADY_UNDERWAY` | error | 1 | 500 | 0.2.0 | Raised when setup is asked to gather answers for a wizard already past it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-7` | `ALREADY_SET_UP` | error | 1 | 400 | 0.9.0 | Raised when setup is answered on a machine that is already set up. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-8` | `NOTHING_TO_RECOVER` | error | 1 | 400 | 0.9.0 | Raised when a recovery is asked for and no apply stopped part-way. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-9` | `NOT_PUT_BACK` | warning | 1 | 500 | 0.9.0 | Raised when a reversal would write over a setting the operator has since chosen. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-10` | `NOT_OPENED` | warning | 1 | 500 | 0.14.0 | Raised when a reversal meets a setting whose sealed record will not open. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-11` | `STILL_HOLDING` | warning | 1 | 500 | 0.16.0 | Raised when a directory a reversal would remove still holds something else's files. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-12` | `NOT_WITHDRAWN` | error | 1 | 500 | 0.16.0 | Raised when a region a reversal would take out of a stack file cannot be. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-13` | `NOT_REWOUND` | error | 1 | 500 | 0.17.0 | Raised when a file a reversal would write back to what it held cannot be. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-14` | `REPLACE_AT_THE_TERMINAL` | warning | 1 | 500 | 0.18.0 | Raised when a reversal meets a secret it cannot put back, which must be replaced at the terminal. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-15` | `NOT_MOVED_BACK` | error | 1 | 500 | 0.18.0 | Raised when a service's state lemonfiber moved could not be moved back. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SETUP-16` | `GUARD_UNANSWERED` | error | 1 | 400 | 0.18.0 | Raised when a VPN answer leaves the egress guard without what it needs to start. | A fault of the stack: not working right now, with whoever runs the house told. |

## `SPACE` — the disk, and letting a download go

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `SPACE-1` | `HALTED` | critical | 1 | 500 | 0.12.0 | Raised when the volume is full and new acquisitions are therefore halted. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-2` | `NOWHERE_TO_MEASURE` | error | 1 | 400 | 0.12.0 | Raised when there is no data location to measure. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-3` | `WALK_REFUSED` | error | 1 | 500 | 0.12.0 | Raised when the data location is there and could not be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-4` | `NOTHING_TO_ASK` | error | 1 | 400 | 0.12.0 | Raised when there is no torrent client here to be holding a completed download. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-5` | `NOT_HELD` | error | 1 | 400 | 0.12.0 | Raised when the client answers and is holding nothing of the name given. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-6` | `ANOTHER_OFFER` | error | 1 | 400 | 0.12.0 | Raised when an agreement names an offer that is not the one standing now. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-7` | `STILL_HELD` | error | 1 | 500 | 0.12.0 | Raised when the client could not be reached, or would not let a download go. | A fault of the stack: not working right now, with whoever runs the house told. |
| `SPACE-8` | `NOT_ON_OFFER` | error | 1 | 400 | 0.18.0 | Raised when an answer narrows to a category its offer does not hold as costing nothing. | A fault of the stack: not working right now, with whoever runs the house told. |

## `STACK` — the stack description

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `STACK-1` | `STACK_UNREADABLE` | error | 5 | 500 | 0.1.0 | Raised when a stack directory holds no readable manifest. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-2` | `STACK_UNUSABLE` | error | 1 | 500 | 0.1.0 | Raised when a manifest is readable and this build cannot use it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-4` | `STACK_NOT_SET_UP` | error | 1 | 500 | 0.1.0 | Raised when lemonfiber has nowhere to write the stack. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-5` | `STACK_NOT_WRITTEN` | error | 1 | 500 | 0.1.0 | Raised when the stack could not be written to disk. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-6` | `STACK_INVALID` | error | 5 | 500 | 0.1.0 | Raised when a manifest parses and breaks the contract. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-7` | `STACK_MALFORMED` | error | 5 | 500 | 0.14.0 | Raised when a manifest is not TOML at all. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-8` | `STACK_UNRECOGNISED` | error | 5 | 500 | 0.14.0 | Raised when a manifest declares names this build does not know. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-9` | `STACK_NEEDS_NEWER` | error | 1 | 500 | 0.17.0 | Raised when a stack names a newer lemonfiber than the one running. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-10` | `STACK_UNASSEMBLED` | error | 5 | 500 | 0.18.0 | Raised when a manifest's files are not laid out as the contract says. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-11` | `PROJECT_EDITED` | error | 1 | 409 | 0.18.0 | Raised when a change would write over a generated project the operator has edited. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-12` | `VOLUME_NOT_COPIED` | error | 1 | 500 | 0.18.0 | Raised when a service's state kept in a volume could not be copied into its mount tree before the service first starts from there. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-13` | `RESET_MOVED` | error | 1 | 400 | 0.18.0 | Raised where the offer a reset answers is not the one a reading of it makes now. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-14` | `CERTIFICATE_NOT_MADE` | error | 1 | 500 | 0.18.0 | Raised where the certificate a service declares could not be made or read before it starts. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STACK-15` | `SERVICE_UNFENCED` | error | 1 | 409 | 0.18.0 | Raised where a start names a fenced service no fence can be written for. | A fault of the stack: not working right now, with whoever runs the house told. |

## `STORAGE` — the data location

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `STORAGE-1` | `COPY_ONLY` | warning | 1 | 500 | 0.2.0 | Raised when the data root cannot hardlink, so imports must copy. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-2` | `ROOT_UNWRITABLE` | error | 1 | 500 | 0.2.0 | Raised when the data root exists but cannot be written to. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-3` | `ROOT_ABSENT` | error | 1 | 500 | 0.2.0 | Raised when the data root is not there to test. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-4` | `SPACE_LOW` | warning | 1 | 500 | 0.2.0 | Raised when the volume holding the data root is nearly full. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-5` | `DEGRADED` | error | 1 | 500 | 0.2.0 | Raised when the data root used to hardlink and no longer does. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-6` | `SERVICE_DENIED` | error | 1 | 500 | 0.2.0 | Raised when the operator owns the data root but the services cannot write it. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-7` | `SPLIT_MOUNTS` | warning | 1 | 500 | 0.15.0 | Raised where a service would see more than one mount beneath the data location, so anything imported between them is copied rather than linked. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-8` | `COPYING_SINCE_DEGRADED` | warning | 1 | 500 | 0.18.0 | Raised where a location that stopped hardlinking leaves the stack copying in a mode it was not set up for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `STORAGE-9` | `ROOT_REFUSED` | error | 1 | 409 | 0.18.0 | Raised where `DATA_ROOT` names an NFS export lemonfiber cannot write into the mount options, so no project is written from it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `TELLING` — what the household is told about

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `TELLING-1` | `BEHIND` | warning | 1 | 500 | 0.11.0 | Raised when the household is told about less than lemonfiber now sets out to tell them, through no choice of the operator's. | A fault of the stack: not working right now, with whoever runs the house told. |

## `TUI` — the terminal interface

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `TUI-1` | `DRAWING` | error | 1 | 500 | 0.6.0 | A screen that could not be drawn, as a problem rather than a panic. | A fault of the stack: not working right now, with whoever runs the house told. |

## `UNDO` — putting a run back

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `UNDO-1` | `NO_SUCH_RUN` | error | 1 | 500 | 0.14.0 | Raised when no run carries the stamp a reversal was asked for. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UNDO-2` | `MORE_THAN_ONE_RUN` | error | 1 | 500 | 0.14.0 | Raised when a stamp names more than one run, so which to put back is not settled. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UNDO-3` | `CANNOT_SUCCEED` | error | 1 | 500 | 0.14.0 | Raised when a run cannot be put back, carrying the reason it cannot. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UNDO-4` | `NOWHERE_TO_LOOK` | error | 1 | 500 | 0.14.0 | Raised when a run cannot say where lemonfiber's own files are. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UNDO-5` | `MOVED_FOR_GOOD` | error | 1 | 409 | 0.18.0 | Raised when a run moved one of lemonfiber's own guards onto its plugin, which no undo or removal puts back. | A fault of the stack: not working right now, with whoever runs the house told. |

## `UPDATE` — moving the stack onto newer versions

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `UPDATE-1` | `NOT_CHECKED` | error | 1 | 500 | 0.14.0 | Raised when what this machine has pulled could not be read. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UPDATE-2` | `NO_SUCH_SERVICE` | error | 1 | 500 | 0.14.0 | Raised when the service an update was narrowed to is not one the stack declares. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UPDATE-3` | `STILL_TRANSFERRING` | warning | 1 | 500 | 0.14.0 | Raised when transfers are still in flight and the run was not asked to wait. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UPDATE-4` | `CAPTURE_LEFT_IT_DOWN` | error | 1 | 500 | 0.14.0 | Raised when the stack came down for the capture and the capture would not write. | A fault of the stack: not working right now, with whoever runs the house told. |
| `UPDATE-5` | `UPDATE_MOVED` | warning | 1 | 400 | 0.18.0 | Raised where an update names an offer that is not the one a fresh look at the releases builds. | A fault of the stack: not working right now, with whoever runs the house told. |

## `VPN` — traffic leaving the tunnel

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `VPN-1` | `LEAKING` | critical | 1 | 500 | 0.1.0 | Raised when the download client's egress does not match the tunnel. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-2` | `VPN_CONTAINER_DOWN` | error | 1 | 500 | 0.1.0 | Raised when the VPN container that should carry traffic is not running. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-3` | `CLIENT_ISOLATED` | warning | 1 | 500 | 0.1.0 | Raised when the client cannot reach the internet through the tunnel. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-4` | `NO_FORWARDED_PORT` | warning | 1 | 500 | 0.2.0 | Raised when port forwarding was asked for but the provider granted no port. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-5` | `KILLSWITCH_LEAKS` | error | 1 | 500 | 0.4.0 | The code a stack whose traffic survives its tunnel earns. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-6` | `TUNNEL_NOT_RESTORED` | error | 1 | 500 | 0.4.0 | The code a stack whose tunnel could not be put back earns. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-7` | `PORT_MISMATCH` | warning | 1 | 500 | 0.6.0 | Raised when the client is listening somewhere other than the forwarded port. | A fault of the stack: not working right now, with whoever runs the house told. |
| `VPN-8` | `NO_TUNNEL` | warning | 1 | 500 | 0.6.0 | Raised when torrents are configured with nothing containing them. | A fault of the stack: not working right now, with whoever runs the house told. |

## `WATCH` — guarding the data location

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `WATCH-1` | `NOTHING_TO_WATCH` | error | 1 | 500 | 0.2.0 | Raised when a watch is asked for but no data location is configured to watch. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WATCH-2` | `ALREADY_GONE` | error | 1 | 500 | 0.2.0 | Raised when the data location is already gone when the watch is asked to start. | A fault of the stack: not working right now, with whoever runs the house told. |

## `WIRE` — choosing what fills a capability

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `WIRE-1` | `NO_SUCH_FILLER` | error | 1 | 404 | 0.16.0 | A capability was named that no service in this stack provides. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WIRE-2` | `CANNOT_FILL` | error | 1 | 400 | 0.16.0 | The service named cannot do the thing it was asked to fill. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WIRE-3` | `NOTHING_ASKS` | warning | 1 | 400 | 0.16.0 | Nothing in this stack asks for the capability, so a choice would change nothing. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WIRE-4` | `CHOICE_UNWRITABLE` | error | 1 | 500 | 0.16.0 | The setting recording the choice could not be written. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WIRE-5` | `WIRING_MOVED` | error | 1 | 400 | 0.17.0 | Raised when a choice answers an offer that was read against a wiring that has since moved. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WIRE-6` | `UNREASONABLE` | error | 1 | 400 | 0.17.0 | Raised when the reason given for a choice is longer than a reason may be, or holds a line break or another control character. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WIRE-7` | `ALREADY_FILLS` | advisory | 1 | 400 | 0.18.0 | Raised where the service chosen already fills the capability, so there is nothing to change. | A fault of the stack: not working right now, with whoever runs the house told. |

## `WIRING` — drift between services

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `WIRING-1` | `DRIFTED` | warning | 1 | 500 | 0.7.0 | Raised when a download client no longer files where lemonfiber wired it. | A fault of the stack: not working right now, with whoever runs the house told. |

## `WISH` — a member searching for a title and asking for it

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `WISH-1` | `NOT_A_SEARCH` | error | 1 | 400 | 0.18.0 | Said where the words searched for are too few or too many to search with. | A search needs at least two letters, and takes at most a hundred. Type at least two letters of the title, and no more than a hundred. |
| `WISH-2` | `NOT_A_PLACE` | error | 1 | 400 | 0.18.0 | Said where the place a search was asked to go on from is not one a page answered. | That search can't go on from there. Search again from the start. |
| `WISH-3` | `NOT_ASKABLE` | error | 1 | 404 | 0.18.0 | Said where no title this member may ask for goes by the id and kind named. | There's no such title to ask for. Search for it, and ask for a title the search shows. |
| `WISH-4` | `CANNOT_FETCH` | error | 1 | 409 | 0.18.0 | Said where the house cannot fetch that kind of title, or anything, right now. | The house can't fetch that kind of title right now. Ask whoever runs the house about it. |
| `WISH-5` | `ALREADY_HERE` | error | 1 | 409 | 0.18.0 | Said where the title asked for is already in the house. | It's already in the house, so there's nothing to ask for. Play it from your shelf. |
| `WISH-6` | `NOTHING_LEFT` | error | 1 | 409 | 0.18.0 | Said where the member has asked for all their allowance lets them, until it frees up. | You've asked for all you can for now. Your allowance frees up as earlier asks drop out of the count. Ask again once your allowance frees up. |
| `WISH-7` | `NO_ROOM` | error | 1 | 503 | 0.18.0 | Said where the house has no room for new things, which is the house's and nobody's limit. | The house has no room for new things right now. That's the house, not your allowance, and whoever runs the house has been told. Ask again later. |
| `WISH-8` | `NOT_WORKING` | error | 1 | 503 | 0.18.0 | Said where what answers the household's asks did not answer, so nothing was asked. | Asking isn't working right now, and whoever runs the house has been told. Nothing was searched or asked for. Try again a little later. |
| `WISH-9` | `NOT_SET_UP` | error | 1 | 503 | 0.18.0 | Said where the house has nothing to search or ask with at all. | This house isn't set up for asking for new things. Ask whoever runs the house about it. |
| `WISH-10` | `NOT_ITS_SEASON` | error | 1 | 400 | 0.18.0 | Said where a season named is not one the title has, or seasons were named for a film. | That isn't a season this title has. Choose from the seasons the title shows. |
| `WISH-11` | `NOBODY_ASKING` | error | 1 | 400 | 0.18.0 | Said where nobody is named for something only a member can do. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WISH-12` | `NOT_A_MEMBER` | error | 1 | 404 | 0.18.0 | Said where the member named is not somebody in the household. | A fault of the stack: not working right now, with whoever runs the house told. |
| `WISH-13` | `NO_PICTURE` | error | 1 | 404 | 0.18.0 | Said where a title a member may ask for has no picture to show them. | That title has no picture to show. You can still ask for it by its name. |

## `WORD` — the glossary

| Code | Name | Severity | Exit | Status | Since | Summary | For the household |
| ---- | ---- | -------- | ---- | ------ | ----- | ------- | ----------------- |
| `WORD-1` | `UNRECOGNISED` | error | 1 | 404 | 0.9.0 | A word this product does not explain, as a refusal that says what it does. Through the error model rather than a bare line, so it carries a code and a way forward like every other refusal — and the way forward is the list itself, which is short enough to be the answer rather than a pointer at one. It lies in the naming: the word is the whole of what was asked for, and there is no entry for it. A surface that reported this as its own failure would be telling a caller to try again at something that will never work. | A fault of the stack: not working right now, with whoever runs the house told. |

## Retired

Published once, raised by nothing, and never given to another problem: `CONFIG-7`, `QUOTA-3`, `STACK-3`.
