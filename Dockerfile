# Carranta, as a container.
#
# Two stages, because the toolchain is three hundred megabytes and the thing it
# produces is three. Everything the server needs at runtime is inside the binary
# already: the page, the fonts, the art and the sounds are all `include_str!` and
# `include_bytes!`, so the runtime layer is a base image and one file.
#
# Pinned to a Rust version rather than `latest`: the workspace declares an MSRV
# and a build that silently moves under it is a build that fails on a Tuesday for
# no reason anybody changed.
#
# It has to match `rust-version` in the workspace manifest, and once did not:
# both said 1.87, the code had since grown let chains, which are stable from
# 1.88, and nothing built with 1.87 to notice. This image would have been the
# first thing to try, on the first deploy.
FROM rust:1.88-slim-bookworm AS build
WORKDIR /src

# The manifests first, so a change to the source does not re-fetch the registry.
# There was nothing to fetch until sign-in needed an HTTPS client; there is very
# little now, and the split is what keeps a source change from re-fetching it.
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

# The trained champion, when the repository carries one. Deploying a champion
# is committing a `champion.net` (exported by `carranta-evolve --method neat`)
# at the repository root; without one the house heuristic plays, which is what
# every deploy did before there was anything trained.
#
# The glob matches one file or none, and Docker refuses a COPY whose sources
# match nothing, so the manifest rides along as a chaperone that always
# matches. It is a file the image was never going to read either way.
COPY Cargo.toml champion.ne[t] /srv/

# Neither is a default the binary would pick on its own: on a laptop it binds
# loopback, which is right there and wrong in a container.
ENV HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080

# A shell only to ask one question, `exec` so it then gets out of the way: the
# server replaces the shell and is PID 1, receiving the platform's stop signal
# directly, so a deploy replaces it in a second rather than waiting out a ten
# second kill timer. The question cannot be asked anywhere else: whether a
# champion was committed is decided per build, and `--trained` on a file that
# is not there is a refusal to start, which is right on a laptop and wrong as
# the default state of every image built before the first champion existed.
CMD ["/bin/sh", "-c", "if [ -f /srv/champion.net ]; then exec carranta-play --games /data/games --trained /srv/champion.net; else exec carranta-play --games /data/games; fi"]
