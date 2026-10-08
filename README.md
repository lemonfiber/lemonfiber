<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/logo-on-ink.svg">
    <img alt="lemonfiber" src=".github/logo.svg" height="72">
  </picture>
</p>

<h1 align="center">Lemonfiber</h1>

<p align="center">
  Sets up a self-hosted media stack, runs the part of it you need, and checks
  that it is actually working.
</p>

<p align="center">
  <a href="https://github.com/lemonfiber/lemonfiber/actions/workflows/build.yml"><img alt="build" src="https://github.com/lemonfiber/lemonfiber/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://github.com/lemonfiber/lemonfiber/actions/workflows/codeql.yml"><img alt="codeql" src="https://github.com/lemonfiber/lemonfiber/actions/workflows/codeql.yml/badge.svg"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=lemonfiber_lemonfiber"><img alt="quality gate" src="https://sonarcloud.io/api/project_badges/measure?project=lemonfiber_lemonfiber&metric=alert_status"></a>
  <a href="https://sonarcloud.io/summary/new_code?id=lemonfiber_lemonfiber"><img alt="coverage" src="https://sonarcloud.io/api/project_badges/measure?project=lemonfiber_lemonfiber&metric=coverage"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/lemonfiber/lemonfiber"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/lemonfiber/lemonfiber/badge"></a>
</p>

---

`lemonfiber` is a command-line tool for running your own media server at home.
It installs and wires together open-source services such as Jellyfin, Sonarr,
Radarr, Prowlarr and Seerr, then keeps checking them: is the VPN really carrying
the torrent traffic, are imports hardlinking instead of copying?

It is for anyone comfortable running Docker who does not want to configure six
web interfaces by hand. Everyone else in the house never sees it: they ask for
things in Seerr and watch them in Jellyfin.

This repository holds the `lemonfiber` binary: the command line, a terminal
dashboard, and the local web API that the web console and phone app use. It is
written in Rust.

> **Status:** before 1.0.
> [What is built](https://docs.lemonfiber.app/project/whats-built/) lists what
> works today and what is still planned.

## Install

You need:

- macOS or Linux. There is no native Windows build.
- Docker with Compose v2.20 or newer, running.

Install the newest release with its installer. Every release is on the
[releases page](https://github.com/lemonfiber/lemonfiber/releases).

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/lemonfiber/lemonfiber/releases/latest/download/lemonfiber-installer.sh | sh
```

The installer puts `lemonfiber` in `~/.cargo/bin` (or `$CARGO_HOME/bin`). Check
it worked:

```console
$ lemonfiber --version
lemonfiber 0.16.0
```

Prebuilt archives with checksums, and building from source, are covered in
[Install lemonfiber](https://docs.lemonfiber.app/start/install/).

## First run

Run it with nothing configured and it offers to set itself up:

```sh
lemonfiber
```

Setup asks only what it cannot work out itself, tests each credential against
the live service, and writes nothing until you confirm a summary. Then:

```sh
lemonfiber up tv        # start one part of the stack: search, download, organise TV
lemonfiber ps           # what each service is really doing
lemonfiber doctor       # check the VPN, hardlinks, disk space and more
```

`tv` is a *form*: a named part of the stack. `lemonfiber forms` lists them all,
from `search` (just find things) to `full` (everything).
[Your first stack](https://docs.lemonfiber.app/start/your-first-stack/) walks
through setup step by step.

## Documentation

Everything about using lemonfiber is on
[docs.lemonfiber.app](https://docs.lemonfiber.app):

- [Running your stack](https://docs.lemonfiber.app/running/): forms, the services, quality, requests, backups
- [When something is wrong](https://docs.lemonfiber.app/fixing/): the doctor, and every error code
- [Command reference](https://docs.lemonfiber.app/commands/): every command and flag
- [The API and the SDKs](https://docs.lemonfiber.app/api/): build on the local web API

## Working on lemonfiber

```sh
git clone --recurse-submodules https://github.com/lemonfiber/lemonfiber.git
cd lemonfiber
cargo build --workspace
just ci                 # formatting, lints, tests, spelling; also installs the git hooks
```

[`.docs/`](.docs/00-index.md) explains how the code is laid out and why, starting
with the [module layout](.docs/architecture/module-layout.md). Every change cites
a requirement in the [specification](https://github.com/lemonfiber/spec); read
the [contributing guide](https://github.com/lemonfiber/spec/blob/main/50-governance/contributing.md)
and [AGENTS.md](AGENTS.md) before your first pull request.

To report a vulnerability, follow [SECURITY.md](SECURITY.md). Questions go to
[Discord](https://discord.nightworks.io).

## Licence

[Hippocratic License 3.0](LICENSE): source-available and ethical-source, and
deliberately not OSI-approved. The
[licence rationale](https://github.com/lemonfiber/spec/blob/main/90-appendix/license-rationale.md)
explains what that means for you. The services lemonfiber runs keep their own
open-source licences.

lemonfiber is made by [NightWorksIO](https://nightworks.io).

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
