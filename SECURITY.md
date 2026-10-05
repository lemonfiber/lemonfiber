# Security policy

## Reporting a vulnerability

**Please do not report a security vulnerability in a public issue, pull request or
discussion.**

Report it privately on GitHub: open this repository's **Security** tab and choose
**Report a vulnerability**. Only the maintainers see the report, and the fix is
worked on with you in a private security advisory.

If you cannot use GitHub, ask a maintainer for a private conversation on
[Discord](https://discord.nightworks.io), and keep the details out of public
channels.

Include what you can of:

- the version (`lemonfiber version`) and the platform;
- what an attacker needs: a device on the household network, a household member's
  account, the operator's session, or a container in the stack;
- the steps that reproduce it, and what happens;
- a fix, if you have one in mind.

## Supported versions

lemonfiber is pre-1.0. A security fix ships in the next release, and earlier
releases do not receive fixes.

## Scope

In scope: everything this repository builds — the `lemonfiber` binary, its command
line, terminal interface and local web API, the stack it embeds and how it
configures it, credential handling, the VPN isolation check, plugins as lemonfiber
installs and runs them, and the release pipeline and installer.

The threat model, including the threats it does not defend against, is in the spec:
[40-quality/security.md](https://github.com/lemonfiber/spec/blob/main/40-quality/security.md).
A flaw in a service the stack runs belongs to that service's project; where the way
lemonfiber configures it is the cause, it belongs here.

## Disclosure

A security fix may be merged ahead of its spec change under the maintainer
[override](https://github.com/lemonfiber/spec/blob/main/50-governance/overrides.md),
because a public spec change describing a vulnerability must not precede its patch.
The spec is corrected straight after.
