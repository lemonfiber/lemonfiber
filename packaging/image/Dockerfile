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
FROM docker:29.9.2-cli@sha256:5e6baf67325a79f64ed3bed6863954d5e05d0f3d4e57cbdb14809346814bbfdb AS docker

FROM gcr.io/distroless/static-debian13:nonroot@sha256:18f4cc26c9ffa7f20a359298be7deadac5c607c8e03a02b8119f1a5fc2f82c99

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
