<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/logo-on-ink.svg">
    <img alt="lemonfiber" src=".github/logo.svg" height="72">
  </picture>
</p>

<h1 align="center">Lemonfiber</h1>

<p align="center">
  The <code>lemonfiber</code> binary: one tool that sets up your media stack,
  runs it in slices, and proves it's working.<br>
  CLI and TUI over one core, and the API the web surface draws. Rust.
</p>

<p align="center">
  <a href="https://github.com/lemonfiber/lemonfiber/actions/workflows/build.yml"><img alt="build" src="https://github.com/lemonfiber/lemonfiber/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://github.com/lemonfiber/lemonfiber/actions/workflows/codeql.yml"><img alt="codeql" src="https://github.com/lemonfiber/lemonfiber/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=lemonfiber_lemonfiber"><img alt="quality gate" src="https://sonarcloud.io/api/project_badges/measure?project=lemonfiber_lemonfiber&metric=alert_status"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=lemonfiber_lemonfiber"><img alt="coverage" src="https://sonarcloud.io/api/project_badges/measure?project=lemonfiber_lemonfiber&metric=coverage"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/lemonfiber/lemonfiber"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/lemonfiber/lemonfiber/badge"></a>
</p>

---

> **Status: shipping (`0.10.0`).** The core, compose driver, CLI, setup wizard,
> trust checks, seed and lifecycle work are built and released. Every request
> reaches all three surfaces: the command line, a terminal dashboard that acts
> rather than only watching, and a web API a browser is served from.
> See [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) for
> built-vs-roadmap and the
> [roadmap](https://github.com/lemonfiber/spec/blob/main/00-overview/roadmap.md)
> (this repo is milestones **M2–M10**).

## What it is

`lemonfiber` orchestrates a fully open-source media stack — the *arr ecosystem,
Jellyfin, Seerr — and does the parts that are usually manual: guided setup, wiring
services together, and **verifying** things work rather than assuming they do
(is the VPN actually isolating traffic? are imports hardlinking?).

Run it two ways, one core behind both:

```
lemonfiber up tv        # scriptable
lemonfiber              # a terminal dashboard
```

A third surface, the web UI, is its own repository
([ADR-0011](https://github.com/lemonfiber/spec/blob/main/00-overview/decisions/0011-web-surface-as-a-fifth-repo.md)):
a single-page app that speaks to a local HTTP API this binary serves.

## The one load-bearing property

**`lemonfiber-core` cannot render.** It has no UI dependency of any kind — no
terminal, no HTTP server. A surface (CLI, TUI, web) is a *rendering*, never a
capability, which is what lets the web surface live in another repository at all. This is enforced by the crate graph, not by review. See spec
[`ARCH-R11`](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).

## Layout

```
crates/
├── lemonfiber/          bin — the only crate that draws a terminal (CLI, TUI)
├── lemonfiber-core/     lib — all logic, no UI
├── lemonfiber-api/      lib — the HTTP endpoints, and serving the web app
├── lemonfiber-ports/    lib — the boundary and its vocabulary
├── lemonfiber-manifest/ lib — parses stack.toml
└── lemonfiber-fixtures/ lib — shared test fixtures
.docs/                   repo-local technical docs (Rust-specific HOW)
```

## Building

```
cargo build --workspace     # or: just build
just ci                     # everything CI runs
```

`just ci` also turns on this repository's pre-push hook, which refuses a push
that would leave a branch carrying no commit `origin/main` does not — what
pushing the trunk over a feature branch looks like. It is `git config
core.hooksPath .githooks`, per clone, and `just hooks` does only that. A clone
where neither has run has no hook: git cannot read `.githooks/` on its own.

## Contributing

This project's spec is **canonical**: every change cites a spec identifier that
already exists. Before your first PR, read
[AGENTS.md](AGENTS.md) and the
[contributing guide](https://github.com/lemonfiber/spec/blob/main/50-governance/contributing.md).

## Licence

[Hippocratic License 3.0](LICENSE) — ethical-source, source-available,
deliberately not OSI-approved. See the
[rationale](https://github.com/lemonfiber/spec/blob/main/90-appendix/license-rationale.md).

---

<p align="center">
  <a href="https://nightworks.io">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset=".github/nightworks-white.png">
      <img alt="NightWorks.io" src=".github/nightworks-dark.png" height="20">
    </picture>
  </a>
  &nbsp;&middot;&nbsp;<a href="https://discord.nightworks.io"><img alt="Discord" src=".github/discord.svg" height="20"></a>
</p>
