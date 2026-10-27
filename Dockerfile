# syntax=docker/dockerfile:1

# --- Build: statisches musl-Binary ---
FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p sofarr-server \
 && cp target/release/amigo-sofarr /amigo-sofarr

# --- Runtime: nichts außer dem Binary und CA-Zertifikaten ---
FROM scratch
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /amigo-sofarr /amigo-sofarr
ENV SOFARR_CONFIG=/config/sofarr.toml
EXPOSE 7676
USER 1000:1000
VOLUME ["/config"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD ["/amigo-sofarr", "healthcheck"]
ENTRYPOINT ["/amigo-sofarr"]
CMD ["serve"]
