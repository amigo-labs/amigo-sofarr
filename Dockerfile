# syntax=docker/dockerfile:1

# --- UI: statische SvelteKit-Seite (wird ins Binary eingebettet) ---
FROM oven/bun:1.4.3-alpine AS ui
WORKDIR /ui
COPY ui/package.json ui/bun.lock ./
RUN bun install --frozen-lockfile
COPY ui/ ./
RUN bun run build

# --- Build: statisches musl-Binary ---
FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
COPY --from=ui /ui/build ui/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p sofarr-server \
 && cp target/release/amigo-sofarr /amigo-sofarr

# --- Runtime: Alpine (gepinnt) mit yt-dlp und ffmpeg für die Trailer (D18) ---
# Neuere yt-dlp-Versionen kommen mit einem neu gebauten Image, nicht im laufenden Container.
FROM alpine:3.24@sha256:f59abe656b692d901e184be3445e084786602108599d2a51d229c0f5a48c71bf
RUN apk add --no-cache ca-certificates ffmpeg yt-dlp
COPY --from=builder /amigo-sofarr /amigo-sofarr
ENV SOFARR_CONFIG=/config/sofarr.toml
EXPOSE 7676
USER 1000:1000
VOLUME ["/config"]
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD ["/amigo-sofarr", "healthcheck"]
ENTRYPOINT ["/amigo-sofarr"]
CMD ["serve"]
