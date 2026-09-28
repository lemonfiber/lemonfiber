# `lemonfiber ui`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber ui`

```text
Serve the web interface, for as long as you leave it running.

Started when you ask for it and not before: nothing is installed, nothing keeps running afterwards, and stopping it leaves nothing behind. It listens on this machine only unless `--lan` says otherwise and serves plain text unless `--tls` does, which it says when it starts, with the address and this run's token.

Usage: lemonfiber ui [OPTIONS]

Options:
      --json
          Print machine-readable output

      --port <PORT>
          The port to listen on. Without it, whichever one is free is used and the whole address is printed

      --dry-run
          Say what would happen, and change nothing

      --no-browser
          Do not ask this desktop to open a browser

      --assets <PATH>
          Serve the interface from this directory rather than from the binary.

          No build carries a web app of its own yet, so this is the only way to serve one. A build asked without it says as much rather than answering with an empty page.

      --force
          Take the stack from a run that claimed it and did not give it back

      --lan
          Offer this to your network, rather than to this machine only.

          Refused unless a password has been set. This surface can start, stop and reconfigure everything and reaches every password the system holds, so it is not offered to a network with nothing in front of it.

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --set-password
          Set the password this surface asks for, before it starts.

          Asked for at the keyboard and never on this line: a password typed as an argument is a password in your shell's history and in the list of processes this machine is running.

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

      --tls
          Serve it encrypted, with a certificate this program made and keeps.

          Off unless asked for. A browser warns about a certificate nobody it trusts signed, and learning to click past that warning costs more than plain text on a network you trust; a paired phone is different, because it pins this certificate instead of asking anybody. Needs `--port`: a paired phone keeps the address it was given.

  -h, --help
          Print help (see a summary with '-h')
```
