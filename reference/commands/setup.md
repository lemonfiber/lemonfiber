# `lemonfiber setup`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber setup`

```text
Set up the stack by answering a few questions.

Interactive by default. Given the flags below, it runs unattended: each answers a question the wizard would otherwise ask, and `--yes` stands in for the confirmation. A non-interactive run missing a flag it needs is told which, rather than left waiting on input that will not come.

Usage: lemonfiber setup [OPTIONS]

Options:
      --json
          Print machine-readable output

      --status
          Report where setup stands and ask nothing. Takes precedence over the answers below, so a run that asks where it is never also answers

      --dry-run
          Say what would happen, and change nothing

      --yes
          Apply without a prompt to confirm — required for an unattended run

      --force
          Take the stack from a run that claimed it and did not give it back

      --protocols <PROTOCOLS>
          How to fetch content: `both`, `usenet`, `torrent`, or `none`

      --data-location <PATH>
          Where the library and downloads live

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --indexer-url <URL>
          An indexer's API base URL

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

      --indexer-key <KEY>
          The indexer's API key

      --usenet-host <HOST>
          The Usenet provider's hostname

      --usenet-port <PORT>
          The port the Usenet provider answers on (defaults to 563)

      --usenet-user <USER>
          The Usenet account username

      --usenet-pass <PASS>
          The Usenet account password

      --usenet-tls <BOOL>
          Whether the Usenet connection uses TLS (defaults to yes)

          [possible values: true, false]

      --library <MODE>
          How to serve the library: `docker`, `native`, or `none`

      --service-user <UID:GID>
          The container user, as `UID:GID`

      --vpn <BOOL>
          Whether a VPN carries the torrent traffic. Where torrents are chosen and this is absent, the run proceeds unprotected and records that it did

          [possible values: true, false]

      --household <BOOL>
          Whether others in the home will use it

          [possible values: true, false]

      --notifications <APPETITE>
          What to be told about: `problems`, `completions`, or `everything`

      --autostart <BOOL>
          Whether to start the stack when the machine boots

          [possible values: true, false]

  -h, --help
          Print help (see a summary with '-h')
```
