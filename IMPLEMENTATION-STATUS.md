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
| VPN leak test (`exec` gluetun + client, compare IPs) | `C2`, `VPN-1..3` | ✅ | #22, [`doctor/vpn.rs`](crates/lemonfiber-core/src/doctor/vpn.rs) |
| Preflight / Environment check (Docker present vs unreachable, Compose ≥ min) | `A2-R9`, `C1-R13` | ✅ | [`doctor/environment.rs`](crates/lemonfiber-core/src/doctor/environment.rs) |
| Empirical hardlink test (create / `stat` / inode + link-count) | `C5-R1`, `C5-R13`, `A2-R8` | ✅ | [`doctor/storage.rs`](crates/lemonfiber-core/src/doctor/storage.rs); `FileSystem` port + `Disk` adapter |
| Storage-mode detection (fs type, network mount, exFAT, WSL2) | `C5-R2`, `C5-R14` | ✅ | derived from the probe; type named via `sysinfo` |
| Free-space check on the data root | `C5-R6` | ◐ | `storage.space` finding warns on a floor; queue-based projection waits on the `service::Client` adapter |
| Degraded-link detection (was linking, now not) | `C5-R11` | ✅ | `storage.hardlinks` reports a regression against a baseline recorded in `storage-state.json` |
| Permission distinction (operator vs service PUID/PGID) | `C5-R10` | ✅ | `storage.permissions`; native-Linux only, ownership vs `PUID`/`PGID` (mapped away on Docker Desktop, so skipped there) |
| Data-root availability supervisor (stop on loss, no auto-restart) | `C5-R7`, `C5-R8`, `C5-R9` | ✅ | `lemonfiber watch`; the `Volume` port detects a vanished or swapped mount by device id and stops the forms |
| Credential validation against live services | `A3-R1..R4`, `A3-R10` | ☐ | `service::Client` port defined; no adapter/check. HTTP transport now in place: [`ports/http.rs`](crates/lemonfiber-core/src/ports/http.rs) + the reqwest/rustls [`adapters/http.rs`](crates/lemonfiber-core/src/adapters/http.rs) (a refusal is a response, only a no-answer is a failure) — the seam the Servarr `Client` and the credentials check will build on. |
| VPN port-forward validation + ProtonVPN NAT-PMP guidance | `A3-R8` | ✅ | `vpn.port-forward` finding: reads the granted port from Gluetun's status file; names ProtonVPN's NAT-PMP-at-generation trap on failure, generic for other forwarding providers, `unverified` for unknown ones, `not-applicable` where forwarding is off (`C2-R6`, `C2-R16`, `C2-R18`, `A3-R14`, `A3-R15`). The continuous re-push lifecycle (`C2-R4`/`C2-R5`/`C2-R19`) stays with M-later. |
| Prerequisites / account guidance (dependency map before credentials) | `A1-R1..R13` | ◐ | [`prerequisites.rs`](crates/lemonfiber-core/src/prerequisites.rs) derives the map; the wizard renders it (A1-R7/R8/R10 arrive with A2/A3) |
| Wizard state machine (resumable, review-before-write, non-interactive guard) | `A2-R1..R15` | ◐ | [`wizard.rs`](crates/lemonfiber-core/src/wizard.rs): the read-only-phase state machine — ordered steps, applicability/skip (container-user asked only where ownership is real; native Jellyfin only where offered), forward/back navigation, resumable `Progress` (serde, restored to the reached step), answer validation that enforces the platform gates, the non-interactive guard (`unanswered`), and the offer-setup gate (`A2-R1..R5`, `A2-R13`, `A2-R14`; `A2-R6/R7` via the platform predicates). Writes nothing itself (`A2-R2`). `Wizard::plan` renders the gathered answers into the exact environment settings apply will write (protocols, data root, container user, `JELLYFIN_MODE`) — the same value review shows (`A2-R3`); an unanswered question contributes no setting. The disk write, directory creation and stack materialisation, image pull and start, `failed-apply` recovery (`A2-R10`), and the interactive surface land with those features. |
| Jellyfin native-mode + PUID/PGID offers (platform-aware) | `A2-R6`, `A2-R7` | ◐ | Platform-gated decision logic done: `Environment::ownership_is_real` gates the PUID/PGID ask (`A2-R6`, native Linux only), `Environment::offers_native_jellyfin` gates the native-Jellyfin offer (`A2-R7`, macOS/Windows only, per ADR-0007). The wizard that presents the offer arrives with A2. |

**Exit criteria:** a fresh machine reaches a running `tv` form in under 15
minutes with no service web UI opened; the leak test provably catches a
misconfigured VPN (the leak half is met).

---

## M4 — Seed · ☐

Wiring services to each other and recording it so it can be undone.

| Deliverable | Spec | Status | Landing / notes |
|-------------|------|--------|-----------------|
| `service::Client` port (Servarr shape) | `D1-*` | ✅ | [`ports/service.rs`](crates/lemonfiber-core/src/ports/service.rs), `SEED-1..3` |
| Servarr-shape adapter (`identity`, register client/root folder) | `D1-*` | ☐ | no adapter yet |
| Seed orchestration (skip-if-absent, preserve operator edits) | `D1-*` | ☐ | [`seed.rs`](crates/lemonfiber-core/src/seed.rs) is a doc stub |
| Change journal (read-back + undo) | — | ☐ | [`journal.rs`](crates/lemonfiber-core/src/journal.rs) is a doc stub |
| Download-client / root-folder / Prowlarr / Bindery / Jellyfin→Seerr wiring | `D1-R7`, `D1-R15` | ☐ | — |

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
