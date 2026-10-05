# lemonfiber on a NAS

Each release publishes an image of lemonfiber, `ghcr.io/lemonfiber/lemonfiber`, for
`linux/amd64` and `linux/arm64`. The release builds it from the release's own
binaries, after checking each one against its digest and its attestation,
and attests the image as it does the other release artefacts. The image holds the
lemonfiber binary, the Docker command line and its Compose plugin, which lemonfiber
runs to drive the stack, and nothing else. There is no shell and no package manager.

Each release also carries one template for each platform, naming that release's
image by digest:

| Platform | Release asset | Where it goes |
|---|---|---|
| Unraid | `lemonfiber-unraid.xml` | The flash share, as `config/plugins/dockerMan/templates-user/my-lemonfiber.xml`. Then choose it under Docker, Add Container, Template. |
| TrueNAS SCALE | `lemonfiber-truenas.yaml` | Apps, Discover Apps, Custom App, Install via YAML. |
| Synology | `lemonfiber-synology.yaml` | Container Manager, Project, Create. |
| Compose, Portainer and the rest | `lemonfiber-compose.yaml` | Beside a `.env` holding the settings it names. |

The templates live in this directory with `{{IMAGE}}` in place of the image. The
release writes the digest in.

## The socket

The container holds the host's Docker socket, `/var/run/docker.sock`. The socket is
control of the host's Docker: whoever holds it can start a privileged container that
mounts the host's root, which is control of the host. lemonfiber needs it because it
runs the stack through Docker. A native install has the same power through the
`docker` group, because that group is access to the same socket.

The image has no shell, so the socket is reachable through lemonfiber and nothing
else in the container. The container runs as an ordinary user that has joined the
socket's group, not as root. Each template takes that group as a setting, because
NAS systems number it differently:

| Platform | The socket's group in the template |
|---|---|
| Unraid | `--group-add 281` in Extra Parameters |
| TrueNAS SCALE | `999`. Read yours in System, Shell, with `stat -c %g /var/run/docker.sock` |
| Synology | `0`, because Container Manager's socket belongs to the root group. lemonfiber still runs as your user |
| Compose | `DOCKER_GID`, read with `stat -c %g /var/run/docker.sock` |

If the socket is not mounted, lemonfiber says it cannot reach Docker and starts
nothing.

## The network

The container shares the host's network, with no network of its own. lemonfiber
reaches the stack's services on the host's loopback, where they publish their ports,
and a container's own loopback is not the host's.

The web surface listens on the host's loopback, at the port the template names
(`7461` unless you change it). It is not offered to the network until a password is
set, and lemonfiber refuses to listen beyond loopback without one. To offer it to
your network:

1. Set a password: `docker exec -it lemonfiber lemonfiber ui --set-password --no-browser --port 7462`.
   Answer the prompt, then stop it with Ctrl-C.
2. Add `--lan` to the container's command and recreate the container.

## One path inside and out

Compose resolves every bind mount on the host, not inside the container that asked.
So lemonfiber's own directory, where it writes the stack, and the directory your
media lives under are each mounted at the same path inside the container as on the
host. The templates set `XDG_CONFIG_HOME` and `XDG_DATA_HOME` beneath lemonfiber's
directory, so its settings and its stack are kept there.

lemonfiber reads its own mounts from Docker before it starts a stack. If the stack's
directory or the data location is mounted from a different path, or not from the
host at all, it refuses and names both paths.

## The terminal interface

```
docker exec -it lemonfiber lemonfiber
```

## Updating

lemonfiber in the image never replaces its own binary. Where it would offer an
update, it gives the command to pull the new image, `docker pull
ghcr.io/lemonfiber/lemonfiber:<version>`, and says to recreate the container from
that release's template, which names the new image by digest. Settings and the stack
stay in the directories the container mounts.
