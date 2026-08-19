# Hosting

What this program is, what that rules out, and how to put it on the internet.

## What it is

Measured on the build this was written against:

| | |
|---|---|
| Binary | 4.6 MB, one file, 27 crates compiled in |
| Memory resident | 6 MB, idle, with tables in it |
| Egress per player-hour | 13.0 MB before the long poll; a fraction of that after |
| First page load | 355 KB across 29 requests |
| State on disk | one small text file per finished game |

Four properties decide everything else.

**It now makes one outbound request.** Signing somebody in means a POST to
Google's token endpoint, which is the only thing this process asks of the outside
world and the reason `ureq` and a TLS stack are in the tree at all. Everything
else it serves comes out of itself.

**It is a long-lived process.** Lobbies, seats, ready marks, presence and what
has been said live in a `Mutex<Vec<Table>>` in process memory. Only finished
games reach the disk, and a lobby never does. Kill the process and every lobby
dies with it.

**It cannot be run twice.** Two instances have two lists of tables. A player
would reach whichever one the router picked, and half the time it would not know
about their game. One replica is the only correct number until that state moves
out of the process, and moving it is a rewrite rather than a setting.

**It must not sleep.** Scaling to zero is a restart by another name.

That rules out serverless outright: there is no process to hold the tables and
no socket to listen on. It also rules out a free tier that spins down on
inactivity, which is most of them.

## Railway

The choice, at **$5 a month**, on Hobby. Live at
`carranta-production.up.railway.app`.

Railway bills actual per-second consumption rather than a reserved size, and
this process consumes almost nothing: a few megabytes of memory and CPU only
while somebody is moving. So the plan fee is the bill, and the `$5` of usage it
includes is not going to be touched by a table of friends. A custom domain is
included, with the certificate issued and renewed automatically.

**The free plan is not the plan.** It was, when this document was first written,
and it has since become a `$1` monthly credit with one project, half a gigabyte
of memory and half a gigabyte of volume, which is a shape you would be fighting
rather than using. Hobby is `$5` a month with `$5` of usage included, five
gigabytes of volume, and the region choice below. Pro is `$20` and buys
collaboration features nothing here needs.

**Region: EU West Metal, Amsterdam**, `europe-west4-drams3a`, set in
`railway.toml`. It used to be a Pro-only choice and is not any more: the Metal
regions opened to Hobby, EU West among them. The one thing to confirm in the
dashboard rather than take on trust is that the **volume** can live in the same
region as the service, because Metal regions lacked volume support when they
first opened and a volume in another region is either refused or a latency
nobody planned.

What is in the repository for it:

- **`Dockerfile`**, two stages, ending in `debian:stable-slim` plus one binary.
  Everything the server serves is compiled into it.
- **`.dockerignore`**, so the build does not copy `target/` or anybody's games.
- **`railway.toml`**, pinning one replica in Amsterdam and naming the health
  check, with the reasons written down.

Four things to set in the dashboard, none of which belong in a file:

1. **A volume mounted at `/data`.** One gigabyte is thousands of games. Without
   it the history vanishes on every deploy, and so does the roster of who played
   them. The container writes to `/data/games`, which is what the `--games` flag
   in the `Dockerfile` points at. Confirm it is offered in the same region the
   service is in.
2. **`CARRANTA_BUILD`** as a build argument, set to `$RAILWAY_GIT_COMMIT_SHA`, so
   the hash in the header names the commit rather than saying `container`.
3. **`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` and `PUBLIC_ORIGIN`**, if you
   want people to be able to sign in. All three or none: without them the button
   is not shown and the routes are not there, which is a whole application that
   happens not to offer accounts rather than a broken one. `PUBLIC_ORIGIN` is
   `https://your.domain` with no trailing path, and the redirect Google must have
   registered is that plus `/signin/done`. See `accounts.md`.
4. **Install the Railway GitHub App on the repository, and then switch
   auto-deploy on.** Two steps, not one, and the first is silent about the
   second. Connecting Railway to GitHub through OAuth is enough to *read* a repo
   and build it on demand; webhooks need the app on the repository itself.

   The service says which of the two is missing. Before the app:
   `{ enabled: false, canEnable: false, reason: "NO_INSTALLATION" }`. After it:
   `{ enabled: false, canEnable: true }`, which is the app installed and the
   switch still off. Only the second state can be fixed from the dashboard, and
   it has to be: the deployment API can read this setting and cannot write it.
5. **Nothing else.** `PORT` is set by Railway, and the binary treats its presence
   as the signal to bind `0.0.0.0` rather than loopback.

**The toolchain pin has to match the manifest.** `rust-version` in the workspace
manifest and the `FROM rust:` tag in the `Dockerfile` are the same number said
twice, and they once disagreed with reality rather than with each other: both
said 1.87, the code had grown let chains, which are stable from 1.88, and nothing
built with 1.87 to notice. The container would have been the first thing to try,
on the first deploy, which is the worst place to learn it. If the local
toolchain is newer than the pin, `cargo +<pin> build --release` is the check, and
it is worth running before any release that touches dependencies.

### What the first deployment showed

It is up, in Amsterdam, and three things are worth writing down because they are
not what the configuration file appears to say.

**`railway.toml` is read, and the service's stored settings are not the whole
truth.** Asked for its configuration, the service answers `builder: RAILPACK`,
which is the default it was created with. The build log says otherwise:

```
[build 7/7] RUN cargo build --release -p carranta-ui --bin carranta-play
[stage-1 3/3] COPY --from=build /src/target/release/carranta-play ...
```

Config as code wins at build time. Believe the log rather than the settings.

**The region came from the file.** `europe-west4-drams3a` is stored as
`multiRegionConfig: {"ams": {"numReplicas": 1}}`, so the identifier is
normalised and the intent survives. Nothing had to be clicked for it.

**A build argument has to exist before the build that reads it.** Setting
`CARRANTA_BUILD` after the first deploy was triggered left that image stamped
`container`, which is the `ARG` default in the `Dockerfile` and is at least
honest about not knowing. Re-running a deployment does not fix it either, since
that reuses the existing image. Only another *build* does.

The opt-in matters and is easy to miss: Railway does not inject its variables
into a Dockerfile build, because "Docker isolates the build from the host
environment by design". A variable reaches the build only if the `Dockerfile`
declares an `ARG` of that name, which this one does.

And there is a second half to it. `CARRANTA_BUILD` is set to
`${{RAILWAY_GIT_COMMIT_SHA}}`, which is **not among the variables a service has
unless the deploy came from a GitHub trigger**. Listing them shows
`RAILWAY_PROJECT_ID`, `RAILWAY_SERVICE_NAME` and their like, and no git ones at
all. So until the app above is installed, that reference resolves to an empty
string, the empty string overrides the `ARG` default, and the page reads
`unknown`, which the build script means literally: nobody told it, and there is
no repository in the container to ask. The setting is right for the end state
and honest before it.

**A push is not a deploy until the GitHub App is installed.** This was assumed
here and was wrong: the first deploy worked, the next push was ignored, and the
service said why when asked, `reason: "NO_INSTALLATION"`. OAuth between Railway
and GitHub is enough to read a repository and build it on demand; webhooks need
the app on the repository itself.

**What the API cannot do, and therefore what is left to a person.** The
deployment tools cover projects, services, variables, domains, logs, status and
config. They do not cover **volumes**, and they say outright that **regions and
replicas** are not theirs either, which is exactly why those two live in
`railway.toml` and why the volume does not. So:

- **The volume is the one manual step.** Until it is mounted at `/data`, every
  deploy starts with an empty games directory and an empty roster: nothing
  breaks, and nothing is remembered.

## Cloudflare in front

Free, and it does three jobs this server deliberately does not do itself.

**Compression.** The server sends everything uncompressed, because compressing
it in-house would mean either another dependency or a hand-written DEFLATE
encoder, and neither is worth it for something a proxy does for free. Cloudflare
compresses on the way out: the board's markup is 252 KB and goes to about 81 KB.

**Caching the parts that never change.** The art, the fonts and the sounds are
compiled into the binary and are the same bytes for the life of a build, so they
are served `public, max-age=31536000, immutable` and a cache in front will hold
them. Everything else is `no-store`: a board is different every few seconds and
a cached one is a lie.

**Absorbing what a public address attracts.** This server has no rate limiting
of its own.

Point the domain at Cloudflare, proxy the record to the Railway host, and leave
compression on. Nothing in the program changes.

## The long poll

The page used to ask every three seconds and be told nothing had changed almost
every time: twelve hundred answers an hour per open page, each the whole board,
and a move reaching the other screens up to three seconds after it was made.

Now every answer carries a **mark**, and the next request hands it back as
`?since=`. The server holds that request until the table stops matching the
mark, up to twenty seconds, then answers anyway. So an answer means something
happened, and a move lands as fast as it can be sent.

- **The mark is opaque to the page**, which hands back what it was given. What
  counts as a change is the server's business, and it counts more than the
  version does: the version is moves alone, deliberately, because it is what
  makes a click against a stale board refuse and a remark is not a reason to
  refuse somebody's move. Sitting down, standing up, being ready, saying
  anything, the host changing a setting and the room closing all change the mark
  and none of them change the version.
- **The hold ticks the session**, every hundred milliseconds, inside the loop.
  It has to: the server only wakes when asked, and a held request is the only
  thing asking for as long as it is held, so without this a paced bot's move
  would wait for the hold to expire. It is also what runs the turn clock.
- **Twenty seconds** is under every proxy timeout worth worrying about. A held
  request that a proxy kills looks to the page like a network error rather than
  like nothing having happened.
- **An unfamiliar mark answers at once.** A page that reloads, or one whose
  server restarted, sends something the table has never had and is answered
  immediately rather than held.
- **A finished game stops the loop.** There is nothing left to ask about, and a
  tab left open on one would otherwise hold a connection for twenty seconds at a
  time for ever. Worse, it would put the game back on the table list on every
  request while it did, so the one thing that made memory bounded would be
  undone by anybody who forgot to close a tab.

## When a table is let go of

Three different clocks, because "nobody is here" means three different things.

| | After | What happens | What it costs |
|---|---|---|---|
| A seat in a game | 2 minutes | The house bot plays it | Your position, until you come back |
| A seat in a room | 2 minutes | The chair goes back to the table | The seat; the room can start without you |
| A room | 20 minutes | Closed | Nothing: no moves, no file |
| A game | 20 minutes | Off the table, still on disk | The conversation; the game comes back on request |

All four are measured from the last request about the thing, which for a seat is
that seat's own page and for a table is anybody's. An open page therefore holds
what it is looking at indefinitely, which is correct: somebody is there.

Two cases are deliberately not clocks. **A game where everybody has gone waits
for all of them** rather than playing itself out, because a game finished by
four house bots while the room was empty is a game destroyed. And **coming back
is always allowed**: a seat you were in is yours whatever has happened since,
including a restart. The rule is that you cannot take a *new* seat in a game
under way.

Above all of it sits the ceiling of sixteen tables, which evicts finished games
first and then the oldest. It is a bound rather than a policy: nothing that
falls off it is lost, and by the time it does any work the sweep has usually
done the work already.

## Concurrency

A thread per connection, capped at 512, with a small stack apiece.

It served one connection at a time until the long poll made that impossible, and
it was already wrong before then: with four people at a table, serialising them
means three of them wait on the fourth's network. One slow client blocked
everybody, which is a denial of service anybody could commit by accident.

Over the cap the connection is dropped rather than queued. Queueing behind busy
workers is how a server turns a load problem into a hang; refusing is the honest
answer and is what a proxy in front will report as a failure.

**Sockets have a ten second read and write timeout.** A client that opens a
connection and says nothing used to hold a thread for ever, which on one thread
meant everybody. Ten seconds bounds how long a request may dribble in, not how
long the answer may take, because a phone on a train is slow rather than
hostile.

## Still open

- **No rate limiting.** Anybody may deal tables in a loop. The sixteen-table
  ceiling and the twenty-minute sweep bound the damage, and Cloudflare bounds
  the rest, but there is nothing in the program.

  The roster is the one thing here with no ceiling of its own: a request with no
  cookie is a new visitor, a new visitor is a row, and the file is rewritten
  whole, so a flood of cookieless requests is quadratic work on a growing file.
  Art, fonts and sounds are exempt, which takes about twenty-nine of every thirty
  requests in a page load out of it. The rest is the same answer as everything
  else in this list, and worth a ceiling of its own the day this list gets
  shorter.
- **No authentication.** A cookie proves a player. It is enough to answer "show
  me mine" on one machine and nothing more, and the pages say so rather than
  implying an account. What the cookie carries is now a *device token* rather
  than the identity itself, and the identity it resolves to is what a game
  records: see `ui.md` §19. That split is what accounts attach to later, and it
  closed something worth naming on its own.

  **A game file used to contain a bearer token.** The cookie value was the
  identity, and the identity was written into every game as the chair's key, so
  anybody who could read the games directory could set that cookie and be that
  person. No page ever emitted one, so it was not a live hole, but it was one
  page away from being one and a backup away from being one on somebody else's
  disk. Principals go in files; tokens go in cookies and in the roster and
  nowhere else.
- **One process, one machine.** Everything above keeps that shape, and it stays
  right for a surprisingly long time. The determinism that already lets a game
  be rebuilt from its seed and its moves is most of what a distributed version
  would need, if it ever needs one.
