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
  CLI, TUI and a local web UI over one core. Rust.
</p>

<p align="center">
  <a href="https://github.com/lemonfiber/lemonfiber/actions/workflows/build.yml"><img alt="build" src="https://github.com/lemonfiber/lemonfiber/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://github.com/lemonfiber/lemonfiber/actions/workflows/codeql.yml"><img alt="codeql" src="https://github.com/lemonfiber/lemonfiber/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=lemonfiber_lemonfiber"><img alt="quality gate" src="https://sonarcloud.io/api/project_badges/measure?project=lemonfiber_lemonfiber&metric=alert_status"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=lemonfiber_lemonfiber"><img alt="coverage" src="https://sonarcloud.io/api/project_badges/measure?project=lemonfiber_lemonfiber&metric=coverage"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/lemonfiber/lemonfiber"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/lemonfiber/lemonfiber/badge"></a>
</p>

---

> **Status: pre-implementation.** This is a scaffold. The specification is
> complete; the code is not written yet. See the
> [spec](https://github.com/lemonfiber/spec) and the
> [roadmap](https://github.com/lemonfiber/spec/blob/main/00-overview/roadmap.md)
> (this repo is milestones **M2–M6**).

## What it is

`lemonfiber` orchestrates a fully open-source media stack — the *arr ecosystem,
Jellyfin, Seerr — and does the parts that are usually manual: guided setup, wiring
services together, and **verifying** things work rather than assuming they do
(is the VPN actually isolating traffic? are imports hardlinking?).

Run it three ways, one core behind all of them:

```
lemonfiber up tv        # scriptable
lemonfiber              # a terminal dashboard
lemonfiber ui           # a local web page
```

## The one load-bearing property

**`lemonfiber-core` cannot render.** It has no UI dependency of any kind — no
terminal, no HTTP server. A surface (CLI, TUI, web) is a *rendering*, never a
capability. This is enforced by the crate graph, not by review. See spec
[`ARCH-R11`](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).

## Layout

```
crates/
├── lemonfiber/         bin — the only crate that renders (CLI, TUI, web)
├── lemonfiber-core/    lib — all logic, no UI
└── lemonfiber-manifest/ lib — parses stack.toml
.docs/                  repo-local technical docs (Rust-specific HOW)
```

## Building

```
cargo build --workspace     # or: just build
just ci                     # everything CI runs
```

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
  &nbsp;&middot;&nbsp;<a href="https://discord.gg/daQmY23ym"><img alt="Discord" src=".github/discord.svg" height="20"></a>
</p>
