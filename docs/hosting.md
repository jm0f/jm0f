# Hosting

What this program is, what that rules out, and how to put it on the internet.

## What it is

Measured on the build this was written against:

| | |
|---|---|
| Binary | 3.0 MB, static, no third-party dependencies |
| Memory resident | 6 MB, idle, with tables in it |
| Egress per player-hour | 13.0 MB before the long poll; a fraction of that after |
| First page load | 355 KB across 29 requests |
| State on disk | one small text file per finished game |

Three properties decide everything else.

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

The choice, on the free plan while it fits, at a few dollars a month after.

Railway bills actual per-second consumption rather than a reserved size, and
this process consumes almost nothing: about six megabytes of memory and CPU only
while somebody is moving. The free plan's `$1` monthly credit covers a table of
friends indefinitely. A custom domain is included, one per service on the free
plan, with the certificate issued and renewed automatically.

What is in the repository for it:

- **`Dockerfile`**, two stages, ending in `debian:stable-slim` plus one binary.
  Everything the server serves is compiled into it.
- **`.dockerignore`**, so the build does not copy `target/` or anybody's games.
- **`railway.toml`**, pinning one replica, with the reason written down.

Three things to set in the dashboard, none of which belong in a file:

1. **A volume mounted at `/data`.** One gigabyte is thousands of games. Without
   it the history vanishes on every deploy. The container writes to
   `/data/games`, which is what the `--games` flag in the `Dockerfile` points at.
2. **`CARRANTA_BUILD`** as a build argument, set to `$RAILWAY_GIT_COMMIT_SHA`, so
   the hash in the header names the commit rather than saying `container`.
3. **Nothing else.** `PORT` is set by Railway, and the binary treats its presence
   as the signal to bind `0.0.0.0` rather than loopback.

## Cloudflare in front

Free, and it does three jobs this server deliberately does not do itself.

**Compression.** The server sends everything uncompressed, because compressing
it in-house would mean either a third-party crate or a hand-written DEFLATE
encoder, and this workspace has no third-party crates. Cloudflare compresses on
the way out: the board's markup is 252 KB and goes to about 81 KB.

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
- **No authentication.** A player is a cookie. It is enough to answer "show me
  mine" on one machine and nothing more, and the pages say so rather than
  implying an account.
- **One process, one machine.** Everything above keeps that shape, and it stays
  right for a surprisingly long time. The determinism that already lets a game
  be rebuilt from its seed and its moves is most of what a distributed version
  would need, if it ever needs one.
