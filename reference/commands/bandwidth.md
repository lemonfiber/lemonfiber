# `lemonfiber bandwidth`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber bandwidth`

```text
Account for the line: what it carries, what the stack takes, what that costs.

Asked nothing it reports — the limits in force, which side of your day the clients say they are on, and whether they are actually keeping to what they were given. Asked for a limit it declares one and hands it to every download client, then reads back what each says, because a client that accepts a setting and does not apply it looks exactly like one that did.

Nothing here touches anybody watching from your own library, and nothing here shapes this machine's traffic. It sets limits inside lemonfiber's own download clients and nowhere else.

Usage: lemonfiber bandwidth [OPTIONS]

Options:
      --down <SHARE OR RATE>
          How much of the line downloads may take: a share, a rate, or `unlimited`.

          A share is written with a per-cent sign — `50%` — and is measured against what the line was last seen to carry, which is shown beside it. A rate is written as a size a second, as in `2MiB`.

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --up <SHARE OR RATE>
          How much of it uploads may take.

          Set apart from the download and left alone unless you say otherwise. Home connections are lopsided and a saturated uplink slows everything down, downloads included, so this defaults to something more careful than the figure you give above.

      --active <FROM-TO>
          The hours the house is awake, when the limits apply: `07:00-23:00`.

          Outside them the stack has the line. The hours are kept by the download clients themselves, on your zone's clock, so they follow the wall clock through the daylight-saving changes. `none` takes the schedule away and leaves the limits standing around the clock.

      --force
          Take the stack from a run that claimed it and did not give it back

      --line <DOWN/UP>
          What this line carries, if you know: `60MiB/6MiB`, download first.

          Only needed to set a share before lemonfiber has seen the line at full speed. It measures as it goes otherwise, from what the downloads achieve when nothing is holding them back.

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --cap <SIZE>
          A monthly allowance for what this stack itself moves: `1TiB`.

          Counts only lemonfiber's own download clients. Everything else in the house is on the same line and is not counted, so your provider's meter will read higher. `none` takes the cap away.

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

      --when-exceeded <WHAT>
          What happens when that cap is reached: `pause`, `throttle` or `continue`.

          Chosen now rather than asked at the time, which arrives at two in the morning on a stack nobody is watching.

      --unrestricted-for <MINUTES>
          Lift the limits for this many minutes, then put them back.

          For the evening you want something now and know nobody else is affected. Time-boxed on purpose, so it cannot be left on.

  -h, --help
          Print help (see a summary with '-h')
```
