# The image a NAS installs lemonfiber from. `release-image.yml` builds it from the
# binaries a release published, once it has checked them, and the context holds
# nothing but those:
#
#   bin/amd64/lemonfiber   from lemonfiber-x86_64-unknown-linux-musl.tar.xz
#   bin/arm64/lemonfiber   from lemonfiber-aarch64-unknown-linux-musl.tar.xz
#
# No instruction here runs anything. Every layer is a copy, so building for a second
# platform needs no emulator, and the image holds no shell for anybody to open beside
# a process that holds the host's Docker socket.

# The Docker command line and its Compose plugin, which lemonfiber runs to drive the
# stack. Both are static binaries; nothing else is taken from this image.
FROM docker:29.8.2-cli@sha256:b1805116a6a86cc591b5d5f60a910a0715cdcc9d18d866ad68b1457ead25c35c AS docker

FROM gcr.io/distroless/static-debian13:nonroot@sha256:e2e927ec666bae08560abb3c55d0659eceabb657f56b6782ab500a9fc7f555e3

ARG TARGETARCH

COPY --from=docker /usr/local/bin/docker /usr/local/bin/docker
COPY --from=docker /usr/local/libexec/docker/cli-plugins/docker-compose /usr/local/libexec/docker/cli-plugins/docker-compose
COPY --chmod=0755 bin/${TARGETARCH}/lemonfiber /usr/local/bin/lemonfiber

LABEL org.opencontainers.image.title="lemonfiber" \
      org.opencontainers.image.description="lemonfiber for a NAS. The container holds the host's Docker socket, which is control of the host's Docker: whoever holds it can start a privileged container that mounts the host's root." \
      org.opencontainers.image.source="https://github.com/lemonfiber/lemonfiber" \
      org.opencontainers.image.documentation="https://github.com/lemonfiber/lemonfiber/blob/main/packaging/README.md" \
      org.opencontainers.image.vendor="NightWorksIO" \
      org.opencontainers.image.licenses="LicenseRef-Hippocratic-3.0"

ENTRYPOINT ["/usr/local/bin/lemonfiber"]
CMD ["ui", "--no-browser"]
