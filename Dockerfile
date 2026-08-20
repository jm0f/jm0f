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

# Every champion the site can offer. Adding one is committing a `.net` file
# here (exported by `carranta-evolve --method neat --export`), so the network
# that played any rated game stays recoverable from the repository forever,
# which a file uploaded to a volume would not be.
#
# The directory is copied whole rather than named file by file, so a new
# champion needs no change here. An empty `bots/` is fine and means the house
# bot is the only player, which is what every deploy was before there was
# anything trained.
COPY bots /srv/bots

# Neither is a default the binary would pick on its own: on a laptop it binds
# loopback, which is right there and wrong in a container.
ENV HOST=0.0.0.0
ENV PORT=8080
EXPOSE 8080

# Not a shell form: with `exec` semantics the server is PID 1 and receives the
# platform's stop signal directly, so a deploy replaces it in a second rather
# than waiting out a ten second kill timer. The conditional this used to need
# is gone, because `--bots` on an empty or absent directory is not an error:
# the champions are offered, and a chair plays the house bot until a lobby
# asks for one of them.
CMD ["carranta-play", "--games", "/data/games", "--bots", "/srv/bots"]
