# Carranta, as a container.
#
# Two stages, because the toolchain is three hundred megabytes and the thing it
# produces is three. Everything the server needs at runtime is inside the binary
# already: the page, the fonts, the art and the sounds are all `include_str!` and
# `include_bytes!`, so the runtime layer is a base image and one file.
#
# Pinned to a Rust version rather than `latest`: the workspace declares an MSRV
# of 1.87 and a build that silently moves under it is a build that fails on a
# Tuesday for no reason anybody changed.
FROM rust:1.87-slim-bookworm AS build
WORKDIR /src

# The manifests first, so a change to the source does not re-fetch the registry.
# There is nothing to fetch, as it happens: this workspace has no third-party
# dependencies at all. The split costs nothing and keeps that true by accident
# rather than by luck.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY art ./art
COPY audio ./audio

# The build stamps the commit into the binary, and a container build has no git
# history to read it from. Passed in instead, so the header on every page says
# which build is serving it.
# Railway exposes the commit as RAILWAY_GIT_COMMIT_SHA; pass it through with
#   --build-arg CARRANTA_BUILD=$RAILWAY_GIT_COMMIT_SHA
# or leave it and every page will say "container", which is at least honest
# about not knowing.
ARG CARRANTA_BUILD=container
ENV CARRANTA_BUILD=${CARRANTA_BUILD}
RUN cargo build --release -p carranta-ui --bin carranta-play

# `slim` rather than `scratch`: the binary is dynamically linked against glibc,
# and static linking would mean musl and a slower allocator for no gain here.
FROM debian:stable-slim
# Games are written here, and this is where the volume is mounted. Created in
# the image so an unmounted run still works rather than failing on the first
# finished game.
RUN mkdir -p /data/games
COPY --from=build /src/target/release/carranta-play /usr/local/bin/carranta-play

# Neither is a default the binary would pick on its own: on a laptop it binds
# loopback, which is right there and wrong in a container.
ENV HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080

# Not a shell form: with `exec` the server is PID 1 and receives the platform's
# stop signal directly, so a deploy replaces it in a second rather than waiting
# out a ten second kill timer.
CMD ["carranta-play", "--games", "/data/games"]
