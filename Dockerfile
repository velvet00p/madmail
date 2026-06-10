# Stage 1: admin-web — build the SvelteKit admin UI (embedded into madmail at compile time).
FROM oven/bun:1 AS admin-web-build
WORKDIR /app
COPY external/madmail-admin-web/package.json external/madmail-admin-web/bun.lock* ./
RUN bun install --frozen-lockfile
COPY external/madmail-admin-web/ .
RUN bun run build



# ------------------------------------------------------------
# Stage 2: build-env — compile madmail (Rust) and fetch iroh-relay for WebXDC realtime.
FROM rust:1-alpine AS build-env

ARG IROH_RELAY_VERSION=v0.35.0

RUN set -ex && \
    apk upgrade --no-cache --available && \
    apk add --no-cache musl-dev sqlite-dev pkgconfig perl build-base curl tar git

WORKDIR /madmail

COPY Cargo.toml Cargo.lock ./
COPY .cargo ./.cargo
COPY crates ./crates
COPY tests/Cargo.toml ./tests/Cargo.toml

COPY external/madmail-admin-web/package.json external/madmail-admin-web/bun.lock* ./external/madmail-admin-web/
COPY --from=admin-web-build /app/build ./external/madmail-admin-web/build

RUN set -eux; \
    arch="$(uname -m)"; \
    case "$arch" in \
      x86_64) iroh_arch=x86_64-unknown-linux-musl ;; \
      aarch64) iroh_arch=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported arch: $arch" >&2; exit 1 ;; \
    esac; \
    assets=crates/chatmail-iroh/assets; \
    mkdir -p "$assets"; \
    curl -fsSL "https://github.com/n0-computer/iroh/releases/download/${IROH_RELAY_VERSION}/iroh-relay-${IROH_RELAY_VERSION}-${iroh_arch}.tar.gz" \
      | tar -xz -C "$assets"; \
    find "$assets" -type f -name iroh-relay -exec mv {} "$assets/iroh-relay" \; ; \
    chmod +x "$assets/iroh-relay"; \
    printf '%s\n' "$IROH_RELAY_VERSION" > "$assets/VERSION"

ENV CHATMAIL_ADMIN_WEB_BUILD=/madmail/external/madmail-admin-web/build
RUN cargo build --release --locked --bin madmail


# ------------------------------------------------------------
# Stage 3: runtime — minimal Alpine image with madmail, iroh-relay, and default config.
FROM alpine:3.21.2
LABEL maintainer="Madmail <admin@madmail.chat>"
LABEL org.opencontainers.image.source=https://github.com/themadorg/madmail

RUN set -ex && \
    apk upgrade --no-cache --available && \
    apk --no-cache add ca-certificates tzdata && \
    mkdir -p /etc/madmail/certs /var/lib/madmail /run/madmail

COPY assets/madmail.conf.docker /etc/madmail/madmail.conf
COPY --from=build-env /madmail/target/release/madmail /bin/madmail
COPY --from=build-env /madmail/crates/chatmail-iroh/assets/iroh-relay /bin/iroh-relay

EXPOSE 25 143 993 587 465 8080
VOLUME ["/var/lib/madmail", "/etc/madmail", "/run/madmail"]
ENTRYPOINT ["/bin/madmail", "--config", "/etc/madmail/madmail.conf"]
CMD ["run", "--libexec", "/var/lib/madmail"]