# syntax=docker/dockerfile:1

# The worker image that `lucidity::fly` starts machines from: the `lunatic` runtime fork lucidity runs on, built from
# source.  Build it from the repo root: `docker build -f docker/Dockerfile -t twitchax/lunatic:<tag> .`.
FROM rust:1.99-bookworm AS build

# The fork's branch or commit to build.
ARG LUNATIC_REF=v0.15.0
ADD https://github.com/twitchax/lunatic.git#${LUNATIC_REF} /lunatic
WORKDIR /lunatic
RUN cargo build --release --locked

# The same distribution as the build, for the libraries the runtime links (OpenSSL, zlib, zstd).
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 zlib1g libzstd1 sqlite3 iproute2 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /lunatic/target/release/lunatic /lunatic

COPY ./docker/app.entrypoint.sh .
RUN chmod a+x app.entrypoint.sh

ENTRYPOINT [ "/app.entrypoint.sh" ]
