# Implementation status

What is built in **this repo** versus what the
[spec roadmap](https://github.com/lemonfiber/spec/blob/main/00-overview/roadmap.md)
still asks for. Read this before reconstructing state from the source — it exists
so a new contributor (human or agent) does not have to.

This tracks the `lemonfiber` binary's milestones, **M2–M6**. M0–M1 live in the
`spec` and `media-stack` repos and are recorded here only for context.

- Update this file **in the same PR** as the work it describes. A tracker in a
  separate change drifts; one that moves with the code cannot.
- Status is per *deliverable*, mirroring the roadmap's own tables. The landing
  column cites the PR or commit that made it true, so a claim here is checkable.

**Legend:** ✅ done · ◐ partial · ☐ not started

---

## M0 — Specification · ✅

In the [`spec`](https://github.com/lemonfiber/spec) repo. Recorded here for
context only.

## M0.5 — Governance in force · ✅

CI, templates, and the citation-gated workflow are in force (DCO, CODEOWNERS,
spec-references bot, labeler, SonarCloud gate, OpenSSF hardening).

## M1 — `media-stack` standalone · ✅

The stack ships embedded as a submodule under [`assets/`](assets/) and is read at
build time; the manifest and compose fragments live there. The stack's own
standalone CI lives in the `media-stack` repo.

---

## M2 — Core: manifest, compose driver, CLI · ✅

| Deliverable | Status | Landing |
|-------------|--------|---------|
| Workspace + `cargo-dist` scaffold | ✅ | `f8ee9d0` |
| `stack.toml` parser + validation (compile-time schema check) | ✅ | #18, `lemonfiber-manifest` |
| Embedded assets (`include_dir!` + `--stack-dir`) | ✅ | #13 |
| Platform detection (macOS / Linux / Linux-Desktop / WSL2) | ✅ | [`platform.rs`](crates/lemonfiber-core/src/platform.rs) |
| Compose command builder (pure, golden-tested) | ✅ | #14, [`stack/compose.rs`](crates/lemonfiber-core/src/stack/compose.rs) |
| Form closure + composition (`B1-R4`, `B1-R5`) | ✅ | #14, #15, [`stack/closure.rs`](crates/lemonfiber-core/src/stack/closure.rs) |
| `up` / `down` / `restart` / `ps` / `logs` / `pull` | ✅ | #17, #21 |
| `.env` read/write (comment- and order-preserving) | ✅ | #16, [`config/env.rs`](crates/lemonfiber-core/src/config/env.rs) |
| `config get`/`set`/`show` (with secret redaction) | ✅ | #19 |

---

## M3 — Setup wizard + doctor · ◐

The product-thesis milestone, partly built ahead of order: the diagnostics
harness and the first check landed before the wizard.

| Deliverable | Spec | Status | Landing / notes |
|-------------|------|--------|-----------------|
| `doctor` — Check trait, remedy per finding | `C1-R1..R12` | ✅ | #22, [`doctor.rs`](crates/lemonfiber-core/src/doctor.rs) |
| VPN leak test (`exec` gluetun + client, compare IPs) | `C2`, `VPN-1..3` | ✅ | #22, [`doctor/vpn`](crates/lemonfiber-core/src/doctor/vpn/mod.rs) |
| Preflight / Environment check (Docker present vs unreachable, Compose ≥ min) | `A2-R9`, `C1-R13` | ✅ | [`doctor/environment.rs`](crates/lemonfiber-core/src/doctor/environment.rs) |
| Empirical hardlink test (create / `stat` / inode + link-count) | `C5-R1`, `C5-R13`, `A2-R8` | ✅ | [`doctor/storage.rs`](crates/lemonfiber-core/src/doctor/storage.rs); `FileSystem` port + `Disk` adapter |
| Storage-mode detection (fs type, network mount, exFAT, WSL2) | `C5-R2`, `C5-R14` | ✅ | derived from the probe; type named via `sysinfo` |
| Free-space check on the data root | `C5-R6` | ◐ | `storage.space` finding warns on a floor; queue-based projection waits on the `service::Client` adapter |
| Degraded-link detection (was linking, now not) | `C5-R11` | ✅ | `storage.hardlinks` reports a regression against a baseline recorded in `storage-state.json` |
| Permission distinction (operator vs service PUID/PGID) | `C5-R10` | ✅ | `storage.permissions`; native-Linux only, ownership vs `PUID`/`PGID` (mapped away on Docker Desktop, so skipped there) |
| Data-root availability supervisor (stop on loss, no auto-restart) | `C5-R7`, `C5-R8`, `C5-R9` | ✅ | `lemonfiber watch`; the `Volume` port detects a vanished or swapped mount by device id and stops the forms |
| Credential validation against live services | `A3-R1..R4`, `A3-R10` | ◐ | The doctor check landed: [`doctor/credentials.rs`](crates/lemonfiber-core/src/doctor/credentials.rs) reads each Servarr-shape service's generated key from its config through the `FileSystem` port and runs [`servarr.rs`](crates/lemonfiber-core/src/servarr.rs)'s `identity()` over the HTTP transport ([`ports/http.rs`](crates/lemonfiber-core/src/ports/http.rs) + reqwest/rustls [`adapters/http.rs`](crates/lemonfiber-core/src/adapters/http.rs)), reporting the observed name and version on success (`A3-R3`) and keeping proven, rejected (`401/403`), unreachable (no answer) and unusable (the service's own words, verbatim) as distinct verdicts (`A3-R4`). An unwritten key is a service still starting, skipped and retried, not a fault (`D1-R1`). It participates in `lemonfiber doctor` under the `credentials` category (`A3-R10`); `diagnose` resolves the targets from the running stack's Servarr services, reaching each on its loopback port and reading its key from where Compose mounts its config. `http` is now a capability on the context. What remains for A3 is the other credential kinds — Usenet provider and indexer, torrent indexer — each proven against its own live service. |
| VPN port-forward validation + ProtonVPN NAT-PMP guidance | `A3-R8` | ✅ | `vpn.port-forward` finding: reads the granted port from Gluetun's status file; names ProtonVPN's NAT-PMP-at-generation trap on failure, generic for other forwarding providers, `unverified` for unknown ones, `not-applicable` where forwarding is off (`C2-R6`, `C2-R16`, `C2-R18`, `A3-R14`, `A3-R15`). The continuous re-push lifecycle (`C2-R4`/`C2-R5`/`C2-R19`) stays with M-later. |
| Prerequisites / account guidance (dependency map before credentials) | `A1-R1..R13` | ◐ | [`prerequisites.rs`](crates/lemonfiber-core/src/prerequisites.rs) derives the map, and setup now **renders it**: the moment protocols are chosen, `setup::run` shows the checklist those choices imply through the [`prompt`](crates/lemonfiber/src/prompt.rs) — the accounts needed, each with what it is, why, its cost band, and the criteria (no vendors) that decide it (`A1-R2`/`A1-R4`/`A1-R5`), the Usenet provider told from the indexer (`A1-R6`), the VPN's port-forwarding criterion and its consequence stated at the point of choosing (`A1-R12`/`A1-R13`), the torrents-without-a-VPN warning (`A1-R9`), and the library-only zero-account path stated plainly and first (`A1-R3`). Shown before the questions that follow, and resumable (`A1-R1`/`A1-R8`). The wizard's step order was corrected to ask protocols before the checklist derives from them (spec [a2](https://github.com/lemonfiber/spec/blob/main/10-functional/features/a-getting-started/a2-setup-wizard.md) fixed first). `A1-R7`/`A1-R10` (credential entry and validation) arrive with the Credentials step and A3. |
| Wizard state machine (resumable, review-before-write, non-interactive guard) | `A2-R1..R15` | ◐ | [`wizard.rs`](crates/lemonfiber-core/src/wizard.rs): the read-only-phase state machine — ordered steps, applicability/skip (container-user asked only where ownership is real; native Jellyfin only where offered), forward/back navigation, resumable `Progress` (serde, restored to the reached step), answer validation that enforces the platform gates, the non-interactive guard (`unanswered`), and the offer-setup gate (`A2-R1..R5`, `A2-R13`, `A2-R14`; `A2-R6/R7` via the platform predicates). Writes nothing itself (`A2-R2`). `Wizard::plan` renders the gathered answers into the exact environment settings apply will write (protocols, data root, container user, `JELLYFIN_MODE`) — the same value review shows (`A2-R3`); an unanswered question contributes no setting. The `failed-apply` recovery frame is now in ahead of the write it guards (`A2-R10`): `Progress` carries a lifecycle `Phase` (`in-progress`/`reviewing`/`applying`/`applied`, `#[serde(default)]` so older progress files still load), `Status::of` classifies what a later run finds — collapsing a persisted `applying` marker to `failed-apply`, since a live apply is its only writer — and `Recovery` reports exactly what the interrupted apply wrote (from the change [`journal`](crates/lemonfiber-core/src/journal.rs)) and resolves the operator's `resume` / `roll back` / `start over` choice — both roll back and start over reverse the recorded writes via the journal's `rewind` so nothing is stranded on disk, differing only in whether the answers survive. Apply's decision layer is now in, still pure: `Wizard::transition` walks the lifecycle only along the edges setup takes — review only once every question is answered, apply after review, applied after apply, and the one backward edge a rolled-back apply takes to return to review — refusing every skip or quiet downgrade, so a writing or written phase is unreachable without passing the gate the earlier one stands for. `Plan::changes` turns the reviewed settings into the exact journal `Set` changes the write both makes and records, each reading the value the environment file held before (`Some("")` for a present-but-empty key, `None` for a new one) so an interrupted apply unwinds to precisely what was there. The config-write half of the I/O executor is now in: [`app/apply.rs`](crates/lemonfiber-core/src/app/apply.rs)'s `apply` drives a reviewed wizard through the lifecycle and lands its settings — persisting `applying` to `setup-progress.json` before the first write, journalling each change to `journal.jsonl` before it is written, writing each setting through the config store, and persisting `applied` last; a stop anywhere leaves the marker and journal the recovery frame reads (an unreviewed wizard is refused, `SETUP-1`, having written nothing). The shared text writer lives in [`config::store::write`](crates/lemonfiber-core/src/config/store.rs), so progress, journal and settings all land — and report a `NotWritten` — the same way. Apply now also creates the data directory before the settings: the operator's chosen location is made where it does not already exist, journalled as a reversible `Made` first so a stop after removes it, and left untouched and unrecorded where it is already there (the operator's own library to adopt), so unwinding never removes it; a location that cannot be created stops the apply with its own `SETUP-2` and the marker left for recovery. Apply now also materialises the stack: the embedded compose files are written where Compose reads them (`paths.stack()`) through [`Source::materialise`](crates/lemonfiber-core/src/stack.rs). The stack is lemonfiber's own regenerable output — rewritten identically on every run — so unlike the settings and the data directory it is **not** journalled: its undo is simply the next apply rewriting it, a build artifact rather than stranded work, which keeps the journal's `Delete` a single "remove exactly this empty directory" for the operator-owned paths. An external stack (`--stack-dir`) is the operator's own, already on disk, left as it is. `apply` now takes the install [`Paths`](crates/lemonfiber-core/src/config/paths.rs) and the stack `Source`, and a `Fault` names each way a write can fail (a store file, a data directory, the stack) so one boxing site turns any into a problem. Step 12's write set is complete. The setup orchestration that drives the wizard is now in too: [`app/setup.rs`](crates/lemonfiber-core/src/app/setup.rs)'s `run` walks the wizard and a `Prompt` port together — asking each question that applies here and is unanswered, in order (a resumed or non-applicable one passed over), then applying once the operator confirms the plan (`A2-R3`); an answer the platform rejects stops it (`SETUP-5`) and a declined review applies nothing. The asking is a port, so the whole walk is driven in a test by a scripted prompt with no terminal. It is now **runnable end to end**: `lemonfiber setup` ([main.rs](crates/lemonfiber/src/main.rs)'s `run_setup` + the terminal [`prompt`](crates/lemonfiber/src/prompt.rs) adapter that reads and renders each question) drives the wizard on a machine with nothing configured, applies the answers, **and brings the stack up** — refreshing the settings it read at startup against the file it just wrote, then dispatching `up` on the `tv` form through the same settle-and-health path every start uses. It offers setup only where nothing is set up and points a configured machine at its settings (`A2-R14`), refuses a piped or scripted run that has no one to answer rather than blocking on stdin (`A2-R13`), and does not rehearse under `--dry-run`. It also **routes recovery** (`A2-R10`): before anything else it reads the saved progress and, on a `failed-apply`, shows the operator what the interrupted run wrote (loaded through the new `setup::progress_at` and `recover::journal_at`) and offers the three ways out the wizard keeps recoverable — **resume** (re-apply from the recorded answers via `setup::resume`, since apply persists them), **roll back** (`recover::undo` the writes, then apply again), or **start over** (undo and discard the progress and journal) — so a stopped setup is never mistaken by the configured-yet check for a finished one. Gathering now **saves progress after each answer** (`A2-R4`): quitting mid-question leaves a resumable file, and a later run reads it as `in-progress` and picks up where it left off — the wizard, restored, is asked only the questions it still lacks. What remains of A2 is per-image pull progress with an expected-duration statement (`A2-R11`), which needs a streaming process port. |
| Jellyfin native-mode + PUID/PGID offers (platform-aware) | `A2-R6`, `A2-R7` | ◐ | Platform-gated decision logic done: `Environment::ownership_is_real` gates the PUID/PGID ask (`A2-R6`, native Linux only), `Environment::offers_native_jellyfin` gates the native-Jellyfin offer (`A2-R7`, macOS/Windows only, per ADR-0007). The wizard that presents the offer arrives with A2. |

**Exit criteria:** a fresh machine reaches a running `tv` form in under 15
minutes with no service web UI opened; the leak test provably catches a
misconfigured VPN (the leak half is met).

---

## M4 — Seed · ◐

Wiring services to each other and recording it so it can be undone. The
`lemonfiber seed` command exists and wires the first edge — qBittorrent's web UI
password (`D1-R16`): it reads the temporary password from the container's log,
replaces it with a generated one through the client, and records the generated
one in `QBITTORRENT_PASSWORD` where the forwarded-port push reads it. It also
wires each media-filing \*arr's root folders — one per media type, under
`/data/media` — reading the application's key from its config and skipping an
application that has not written one yet. It now also registers each \*arr's
download clients: SABnzbd where its generated key is on disk, qBittorrent where
its password was minted this run — or, on a later run when nothing is minted,
where the recorded password is read back from `QBITTORRENT_PASSWORD`, so an \*arr
that came up after the first seed still learns about qBittorrent. It now also runs
Prowlarr's app sync in the other direction: each of those media-filing \*arrs is
registered back into Prowlarr as an application, so Prowlarr pushes it the shared
indexers. And it makes Jellyfin the identity source for Seerr: Jellyfin has no key
on disk, so lemonfiber mints its admin password by driving Jellyfin's own
first-run setup, records it, and signs Seerr in through Jellyfin — never
re-pointing an already-initialised Seerr, whose existing sign-ins are the
household's.

| Deliverable | Spec | Status | Landing / notes |
|-------------|------|--------|-----------------|
| `service::Client` port (Servarr shape) | `D1-*` | ✅ | [`ports/service.rs`](crates/lemonfiber-core/src/ports/service.rs), `SEED-1..3` |
| Servarr-shape adapter (`identity`, register client/root folder) | `D1-*` | ◐ | [`servarr.rs`](crates/lemonfiber-core/src/servarr.rs): one `Client` for all four Servarr apps, on the HTTP port. `identity` + `register_download_client`/`register_root_folder` post to the versioned API with the key; a transport failure is `Unavailable`, `401/403` is `Unauthorised`, other refusals carry the service's verbatim message. Fake-`Http`-tested. `root_folders` and now `download_clients` read the service's connections back — the latter decoding each client's endpoint (host and port) out of Servarr's `fields` array, so a connection can be matched by where it reaches rather than its label. The per-implementation download-client *write* schema is now filled in: `register_download_client` builds the registration document the [download-client contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/download-client.md) describes — the `implementation`/`configContract` and `fields` that differ between SABnzbd (Usenet, `apiKey`) and qBittorrent (torrent, `username`/`password`), with the category field named per target application. `DownloadClient` now carries the client kind, its credential and its category. |
| Seed orchestration (skip-if-absent, preserve operator edits) | `D1-*` | ◐ | [`seed.rs`](crates/lemonfiber-core/src/seed.rs): the pure policy (`intent`/`Report` — `D1-R2/R3/R5/R6`) plus the first execute-driver, `wire_root_folders`: observes the service through the port (`Client::root_folders`, now read-back via [`servarr.rs`](crates/lemonfiber-core/src/servarr.rs)), leaves a folder already present by path (`D1-R2`/`D1-R8`), skips every folder when the service is unavailable (`D1-R5`), and for each missing one registers it, **reads it back before calling it wired** (`D1-R4`) and journals the write. `wire_download_clients` now carries the same shape to download clients, with one difference that is the point of it: a client is matched by the endpoint it reaches — host and port — not by its label, so one the operator renamed is recognised as the same connection and not duplicated (`D1-R8`). `wire_applications` now carries the same shape to Prowlarr's applications, matched by the `baseUrl` Prowlarr reaches each \*arr on. The remaining graph edges follow the same shape. |
| Download-client credentials (read own key / generate) | `D1-R1`, `D1-R16` | ◐ | A download client is registered into a \*arr with the client's *own* credential. [`sabnzbd.rs`](crates/lemonfiber-core/src/sabnzbd.rs)'s `api_key` reads SABnzbd's generated key from its `sabnzbd.ini` — matched by the exact `api_key` entry so a neighbouring `nzb_key` is not mistaken for it, empty/absent = not-generated-yet (`D1-R1`), the same text-read shape as the Servarr key reader. qBittorrent has nothing durable to read, so lemonfiber generates, sets and records its WebUI password (`D1-R16`). The generation primitive is built: [`secret.rs`](crates/lemonfiber-core/src/secret.rs) renders bytes from a [`Random`](crates/lemonfiber-core/src/ports/random.rs) port (OS CSPRNG adapter) as a `.env`-safe hex secret, `None` rather than a weak fallback where randomness is unavailable. Setting it is built too: [`qbittorrent.rs`](crates/lemonfiber-core/src/qbittorrent.rs) reads the temporary password qBittorrent logs on start, authenticates with it, sets the generated one, and confirms by authenticating again with it — a cookie-session flow, unlike Servarr, so the `Web` adapter now keeps a host-scoped cookie store. The seed driver that runs the exchange is built too: [`seed.rs`](crates/lemonfiber-core/src/seed.rs)'s `wire_qbittorrent_password` mints the password from the `Random` port, sets it through the client, and hands the value back for the surface to record — no randomness means no set and nothing recorded, never a weak fallback. The `lemonfiber seed` surface now runs the whole exchange and records the result, and on a later run reads the recorded password back from `QBITTORRENT_PASSWORD` (an unreadable file reads the same as an empty one) so qBittorrent is still offered as a download client once its temporary password is gone. What remains is the rest of the graph: Prowlarr, Bindery, Jellyfin→Seerr. |
| Change journal (read-back + undo) | `E4-R1`, `E4-R2` | ◐ | [`journal.rs`](crates/lemonfiber-core/src/journal.rs): each `Change` records the four things a reversal and a readable history need — timestamp, originating operation, target, and the before/after values (`E4-R1`); `undo` yields the inverse `Action` per change (remove the created resource by the id the service returned, restore a value / remove it where there was none, or delete a path that was made — `E4-R2`), and `rewind` unwinds most-recent-first. Three change kinds now: a service resource `Created`, a config value `Set`, and a filesystem path `Made` — the last the reversible vocabulary apply's directory creation needs, undoing to a `Delete` of exactly the path lemonfiber created (never one that was already there). Pure data + serde for the jsonl log; the surface stamps the time and persists, and seed and apply write it. The undos it yields are now **carried out** by [`app/recover.rs`](crates/lemonfiber-core/src/app/recover.rs)'s `undo`: a `Restore` rewrites (or removes, via the new [`EnvFile::remove`](crates/lemonfiber-core/src/config/env.rs)/[`store::unset`](crates/lemonfiber-core/src/config/store.rs)) a setting, a `Delete` removes a made directory with a plain empty-only `remove_dir` (so an operator's populated location is never emptied, and a directory a stop left unmade counts as already undone), and a service-made `Remove` — which only the service that made it can undo — does not stop the reversible work: those are set aside and reported together at the end (`SETUP-4`), so a mixed journal still has its settings and directories fully reversed. A real I/O failure (a setting or directory that will not budge) does stop it, most-recent-first, leaving a sane re-runnable partial state. This closes the failed-apply loop in core: the recovery frame detects it, decides the undos, and this reverses them (the interactive surface that triggers it is still to come). |
| Download-client / root-folder / Prowlarr / Jellyfin→Seerr wiring | `D1-R7`, `D1-R16` | ◐ | Root-folder, download-client, **Prowlarr app-sync** and **Jellyfin→Seerr identity** drivers landed (see the orchestration row). Prowlarr app sync registers each media-filing \*arr as an application in Prowlarr through a dedicated [`prowlarr.rs`](crates/lemonfiber-core/src/prowlarr.rs) adapter and an [`AppSync`](crates/lemonfiber-core/src/ports/service.rs) port — Prowlarr speaks `/api/v1`, not the media \*arrs' `/api/v3`, so it is a client of its own — and `seed.rs`'s `wire_applications` observes, writes, reads back by `baseUrl` (not label) and journals, per the [Prowlarr-application contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/prowlarr-application.md). **Jellyfin→Seerr** (`D1-R7`, `D1-R16`): [`jellyfin.rs`](crates/lemonfiber-core/src/jellyfin.rs) drives Jellyfin's first-run `/Startup/*` setup to mint and set the admin password (recorded under `JELLYFIN_ADMIN_PASSWORD`, the qBittorrent shape), and [`seerr.rs`](crates/lemonfiber-core/src/seerr.rs) signs Seerr in through Jellyfin via `auth/jellyfin`; `wire_jellyfin_identity` reads back that Seerr reports itself initialised and never re-points one already set up (the household's own — `D1-R7`'s consent case), per the [Jellyfin→Seerr contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/jellyfin-seerr-identity.md). **Bindery via Torznab (`D1-R15`) is deferred** — its API (a niche fork) can't be pinned without live verification. |

**Exit criterion:** after wiping config, `lemonfiber up tv && lemonfiber seed`
restores full functionality in under 2 minutes, idempotently.

---

## M5 — TUI · ☐

Second surface over the same core (ratatui, per
[ADR-0003](https://github.com/lemonfiber/spec/blob/main/00-overview/decisions/0003-rust-ratatui-for-cli.md)).
Not started.

## M6 — Release engineering · ◐

CI hardening and the `cargo-dist` scaffold are in place; signed multi-platform
release automation is not yet exercised end-to-end.
