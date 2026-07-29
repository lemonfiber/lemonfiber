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
| Prerequisites / account guidance (dependency map before credentials) | `A1-R1..R13` | ◐ | [`prerequisites.rs`](crates/lemonfiber-core/src/prerequisites.rs) derives the map; the wizard renders it (A1-R7/R8/R10 arrive with A2/A3) |
| Wizard state machine (resumable, review-before-write, non-interactive guard) | `A2-R1..R15` | ◐ | [`wizard.rs`](crates/lemonfiber-core/src/wizard.rs): the read-only-phase state machine — ordered steps, applicability/skip (container-user asked only where ownership is real; native Jellyfin only where offered), forward/back navigation, resumable `Progress` (serde, restored to the reached step), answer validation that enforces the platform gates, the non-interactive guard (`unanswered`), and the offer-setup gate (`A2-R1..R5`, `A2-R13`, `A2-R14`; `A2-R6/R7` via the platform predicates). Writes nothing itself (`A2-R2`). `Wizard::plan` renders the gathered answers into the exact environment settings apply will write (protocols, data root, container user, `JELLYFIN_MODE`) — the same value review shows (`A2-R3`); an unanswered question contributes no setting. The `failed-apply` recovery frame is now in ahead of the write it guards (`A2-R10`): `Progress` carries a lifecycle `Phase` (`in-progress`/`reviewing`/`applying`/`applied`, `#[serde(default)]` so older progress files still load), `Status::of` classifies what a later run finds — collapsing a persisted `applying` marker to `failed-apply`, since a live apply is its only writer — and `Recovery` reports exactly what the interrupted apply wrote (from the change [`journal`](crates/lemonfiber-core/src/journal.rs)) and resolves the operator's `resume` / `roll back` / `start over` choice — both roll back and start over reverse the recorded writes via the journal's `rewind` so nothing is stranded on disk, differing only in whether the answers survive. The disk write, directory creation and stack materialisation, image pull and start, and the interactive surface land with those features — each now writing into an already-recoverable machine. |
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
| Change journal (read-back + undo) | `E4-R1`, `E4-R2` | ◐ | [`journal.rs`](crates/lemonfiber-core/src/journal.rs): each `Change` records the four things a reversal and a readable history need — timestamp, originating operation, target, and the before/after values (`E4-R1`); `undo` yields the inverse `Action` per change (remove the created resource by the id the service returned, or restore a value / remove it where there was none — `E4-R2`), and `rewind` unwinds most-recent-first. Pure data + serde for the jsonl log; the surface stamps the time and persists, and seed writes/reads it. |
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
