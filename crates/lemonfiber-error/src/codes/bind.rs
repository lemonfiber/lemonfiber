codes! {
    /// Raised when a service the stack calls admin answers somewhere off this machine.
    BEYOND_LOOPBACK = "BIND-1" {
        severity: Error,
        status: 500,
        since: "0.10.0",
        meaning: "A service the stack calls an admin service is reachable from your network. It \
            is meant to answer this machine and nothing else: it can change how your stack \
            works, and most services like it have weak or no password of their own, so anything \
            on your network reaching it is a way in.",
        remedy: "Publish it on this machine only, and apply the change. The message names the \
            address to use. Or, if you meant to expose it, say so once and say why, with \
            `LEMONFIBER_EXPOSED=<service>=<why>` in your settings.",
    }
    /// Raised where a published port is reached without the host's own firewall rules
    /// being consulted.
    AROUND_THE_FIREWALL = "BIND-2" {
        severity: Warning,
        status: 500,
        since: "0.10.0",
        meaning: "A firewall rule on this machine may not apply to the ports the stack \
            publishes. The container engine runs directly on this machine here, and it writes \
            its own forwarding rules ahead of the ones you add, so a port you believe is shut \
            may still answer. This is how publishing is meant to work and nothing is broken.",
        remedy: "Narrow what the household tier is published on, which is what does decide: \
            `LAN_BIND` in your settings, set to this machine's own address on your network. Or \
            leave it, if every device on this network is one you would let in anyway.",
    }
    /// Raised where an admin service answers off this machine and the operator has
    /// written down that they meant it to.
    DELIBERATE = "BIND-3" {
        severity: Warning,
        status: 500,
        since: "0.10.0",
        meaning: "An admin service is reachable from your network and you have written down that \
            you meant it to. It is still reported, because the exposure is real either way; what \
            changed is whose decision it is. The message quotes your reason back.",
        remedy: "Nothing, if that is still true. Or take it out of the exposed list and publish \
            it on this machine only.",
    }
}
