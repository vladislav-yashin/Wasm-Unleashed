# syntax=docker/dockerfile:1
# The browser relay image. It is deliberately separate from the nginx image: it holds no game
# assets, runs as an unprivileged user and only needs outbound UDP and the internal HTTP port.
FROM rust:1.100-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY tools ./tools
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p nfs-relay \
    && cp target/release/nfs-relay /tmp/nfs-relay

FROM alpine:3.22
LABEL org.opencontainers.image.title="Wasm Unleashed relay"
LABEL org.opencontainers.image.source="https://github.com/vladislav-yashin/Wasm-Unleashed"
RUN addgroup -S -g 10001 relay \
    && adduser -S -D -H -u 10001 -G relay relay
COPY --from=build /tmp/nfs-relay /usr/local/bin/nfs-relay
USER 10001:10001
EXPOSE 8090
HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q -T 2 -O /dev/null http://127.0.0.1:8090/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/nfs-relay"]
CMD ["--listen", "0.0.0.0:8090"]
