codes! {
    /// Raised when the download client's egress does not match the tunnel.
    LEAKING = "VPN-1" {
        severity: Critical,
        status: 500,
        since: "0.1.0",
        meaning: "The download client's traffic is not going through the VPN: its public address \
            does not match the tunnel's, or it has connectivity the tunnel does not. Peers in \
            every swarm can see your home address. This is the one failure whose consequences \
            reach outside your machine.",
        remedy: "Stop torrent transfers now, then confirm the client shares the VPN's network — \
            `network_mode: service:<gateway>` in the stack.",
    }
    /// Raised when the VPN container that should carry traffic is not running.
    VPN_CONTAINER_DOWN = "VPN-2" {
        severity: Error,
        status: 500,
        since: "0.1.0",
        meaning: "The VPN container that should carry traffic is not running. Nothing routes \
            through a tunnel that is not up, so torrents cannot transfer — though nothing is \
            leaking while it is down.",
        remedy: "Start the form that includes it, then read its logs: `lemonfiber logs \
            <gateway>`.",
    }
    /// Raised when the client cannot reach the internet through the tunnel.
    CLIENT_ISOLATED = "VPN-3" {
        severity: Warning,
        status: 500,
        since: "0.1.0",
        meaning: "The tunnel is up and the client could not reach the internet through it. \
            Nothing is leaking, and torrents will not transfer until it can.",
        remedy: "Confirm the client uses the VPN container's network.",
    }
    /// Raised when port forwarding was asked for but the provider granted no port.
    NO_FORWARDED_PORT = "VPN-4" {
        severity: Warning,
        status: 500,
        since: "0.2.0",
        meaning: "The tunnel is up and no port was forwarded, so peers cannot open connections \
            to your client: download connectivity and seeding are both reduced. It cannot be \
            fixed while the stack is running.",
        remedy: "Regenerate the VPN credentials with port forwarding enabled. On ProtonVPN that \
            means enabling NAT-PMP and picking a P2P server when the WireGuard configuration is \
            generated.",
    }
    /// The code a stack whose traffic survives its tunnel earns.
    KILLSWITCH_LEAKS = "VPN-5" {
        severity: Error,
        status: 500,
        since: "0.4.0",
        meaning: "The tunnel was dropped on purpose and the download client still reached the \
            internet. Every torrent would continue in the open the moment the VPN fails, and a \
            VPN that never fails is not a thing.",
        remedy: "Enable the tunnel container's own killswitch. For gluetun that is \
            `FIREWALL=on`, which is its default.",
    }
    /// The code a stack whose tunnel could not be put back earns.
    TUNNEL_NOT_RESTORED = "VPN-6" {
        severity: Error,
        status: 500,
        since: "0.4.0",
        meaning: "The killswitch test dropped the tunnel and could not confirm putting it back. \
            The stack is left without one, and whether traffic is flowing outside it is exactly \
            what is now unknown.",
        remedy: "Restart the tunnel container now.",
    }
    /// Raised when the client is listening somewhere other than the forwarded port.
    PORT_MISMATCH = "VPN-7" {
        severity: Warning,
        status: 500,
        since: "0.6.0",
        meaning: "The provider forwards one port and the download client is listening on \
            another. Downloads still arrive, so nothing looks wrong — but no peer can reach the \
            client, so it cannot seed and connects to fewer sources.",
        remedy: "Run `lemonfiber up` to move the client onto the forwarded port.",
    }
    /// Raised when torrents are configured with nothing containing them.
    NO_TUNNEL = "VPN-8" {
        severity: Warning,
        status: 500,
        since: "0.6.0",
        meaning: "The stack declares torrents and no VPN-contained client to run them through, \
            so every torrent is visible under this connection's own address — to the network it \
            is on, and to every peer in the swarm.",
        remedy: "Put the torrent client behind a VPN container in the stack. Where this is \
            deliberate, answer it once with `lemonfiber doctor --accept vpn.unprotected`.",
    }
}
