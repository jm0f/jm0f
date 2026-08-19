# Accounts

What exists, what the scoping document asked for, and what it would cost to
build in Rust rather than beside it.

## What exists

`crates/carranta-ui/src/people.rs`, and no way to sign in.

The three concepts §8.2 names are separated, which is the part that cannot be
added afterwards. A **principal** is the durable identity and is what a game
file records. A **credential** proves a principal; there is one kind, a device
token in a cookie. A **seat** is a position in one game and is nobody's identity.

Alongside them: the `guest → account` alias table (P-1), which the analytics
already resolve through, so a claim moves a rating without moving a game; a
display name that belongs to the person rather than to a seat; and a column for
the P-11 age declaration that nothing writes to yet.

The full account of it is `ui.md` §19. The short version is that a sign-in has
somewhere to attach, and attaching one changes nothing else.

## What the scoping document asked for

P-15 settles on **Auth.js**, with our own principal table behind it. §8.6 spells
out three consequences, and §8.7 draws the conclusion: Auth.js runs inside a JS
meta-framework, so the client becomes a Next.js or SvelteKit application
rendering a `carranta-wasm` board.

That was written before there was a server. What got built is a 3 MB
dependency-free Rust binary that serves one HTML page, and the whole of its
`Cargo.lock` is six entries, all of them ours. Following P-15 literally means a
second service in a second language, and the no-dependency property survives
only on the Rust side of the seam.

## What it costs to stay in Rust

Measured, not estimated: a scratch crate per row, `cargo tree`, unique crates
including the named one.

| What it buys | Crates | Async runtime |
|---|---|---|
| *Nothing. Today.* | **0** | no |
| Signed tokens and real entropy (`hmac`, `sha2`, `getrandom`) | 14 | no |
| Password hashing (`argon2`, RustCrypto) | 14 | no |
| A TLS client (`rustls` + `webpki-roots`) | 21 | no |
| Verifying Google's ID tokens (`jsonwebtoken`) | 36 | no |
| Sending mail (`lettre`, SMTP, rustls, blocking) | 60 | no |
| OAuth2 (`oauth2` v5, default features) | 110 | **yes** |
| OAuth2 with `default-features = false` | 67 | no |

And the two bundles that are actual decisions rather than parts:

| Bundle | Crates | Async runtime |
|---|---|---|
| **A.** Emailed sign-in links and passwords | **74** | no |
| **B.** A, plus Google sign-in | **135** | **yes** |

Three things those numbers say that the individual rows do not.

**Password hashing is cheap and safe to take.** `argon2` is RustCrypto, pure
Rust, no async, no TLS, fourteen crates of hashing primitives. This is the one
piece §8.6 flags as security-sensitive code we would otherwise write, and it is
the piece with the smallest possible cost to not write. Hand-rolling Argon2 is
not a thing to do.

**Sending email is where the weight is, not authentication.** Sixty of bundle
A's seventy-four crates are `lettre` and the TLS stack under it. Any flow with
an email in it (a magic link, a password reset, a verification) pays that, and
none of the alternatives are cheaper: an HTTP mail API needs the same TLS client
plus JSON.

**Google sign-in brings tokio.** `oauth2` v5 defaults to a `reqwest` client,
which is `hyper` on `tokio` on `rustls`. Turning default features off drops it to
67 and hands us the HTTP calls to make ourselves, against a server that has no
HTTP client and no async anything. This is the row that changes the shape of the
program rather than its size.

## Recommendation

**Take `argon2`, `hmac`, `sha2` and `getrandom`. Leave the rest until there is a
reason.** That is fourteen to twenty crates, no async, no TLS, no network client,
and it is enough for a real account: an email address, a password hashed
properly, a signed session, and the claim flow that already exists.

What it defers is the two expensive halves, and both defer cleanly:

- **Verification email.** Without it, an address is unproven, which matters for
  password reset and not much else at this stage. It is a bounded piece of work
  to add, and it is the same work whenever it happens.
- **Google sign-in.** §8.6 leans on it precisely to keep password code off the
  critical path, and taking `argon2` removes that argument: the password path is
  a library call rather than security-sensitive code we wrote. Google becomes a
  convenience to add when somebody asks for it, and it is a *second credential
  kind on the same principal*, which is exactly the shape `people.rs` already
  has.

The one thing worth deciding early rather than late is whether a session is a
cookie in this server's own table or a signed token. It should be the table: a
row that can be deleted is a session that can be revoked, and the table is
already there.

## Where this leaves P-15

Overruled on the mechanism, kept on the principle. The principle was **own the
principal table**, and treat whatever proves an identity as a source rather than
the system of record. That is built, and it is the part §13 said could not be
retrofitted. Auth.js was the means, and its cost, a second service and a JS
framework, is out of proportion to what it buys a program whose entire
dependency list is currently empty.
