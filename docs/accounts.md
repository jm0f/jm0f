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
| Password hashing (`argon2`, RustCrypto) | 14 | no |
| Signed tokens and real entropy (`hmac`, `sha2`, `getrandom`) | 14 | no |
| A blocking HTTPS client (`ureq` 3, rustls) | **26** | no |
| A TLS client on its own (`rustls` + `webpki-roots`) | 21 | no |
| Verifying Google's ID tokens (`jsonwebtoken`) | 36 | no |
| Sending mail (`lettre`, SMTP, rustls, blocking) | 60 | no |
| Passkeys (`webauthn-rs`) | 99 | no |
| `oauth2` v5, `default-features = false` | 67 | no |
| `oauth2` v5, default features | 110 | **yes** |
| `google-oauth`, `features = ["blocking"]` | 120 | **yes** |
| `google-oauth`, default | 133 | **yes** |

And the bundles that are actual decisions rather than parts:

| Bundle | Crates | Async | Sends email |
|---|---|---|---|
| **A.** Google sign-in, code flow, `ureq` | **26** | no | **no** |
| **B.** Passwords, no email at all | 14 | no | no, and no recovery either |
| **C.** Passwords with reset and verification | 74 | no | yes |
| **D.** C plus Google via `google-oauth` | 180 | **yes** | yes |

## What the numbers say

**Passwords are what create the email.** This is the point the first draft of
this document missed, and it inverts its recommendation. Hashing a password is
cheap and safe (`argon2`, fourteen crates of arithmetic). What is *not* cheap is
everything a password drags behind it: somebody will forget one, so there is a
reset flow, so there is an address to send it to, so there is an address to
verify, so there is `lettre` and a TLS stack and sixty crates and a deliverability
problem and a mailbox somebody has to own. Bundle C is bundle B plus an outbox.

**Delegating sign-in removes the outbox rather than adding to it.** Nobody resets
a password we never held; Google owns recovery, and owning recovery is the
expensive part of owning identity. Bundle A is the cheapest row in the table
*and* the one with the least security-sensitive code we wrote.

**The Google client is not the way to talk to Google.** `google-oauth` is
maintained and points the right way, and it costs 120 crates and an async runtime
even with `features = ["blocking"]`, because that feature is `reqwest`'s blocking
wrapper and spins a tokio runtime inside itself:

```
tokio → hyper → hyper-util → reqwest → google-oauth
```

**The server-side code flow needs no signature check at all.** Google's own
documentation says so: "since you are communicating directly with Google over an
intermediary-free HTTPS channel and using your client secret to authenticate
yourself to Google, you can be confident that the token you receive really comes
from Google and is valid." The caveat is that any component the token is *passed*
to must validate it, and it is passed nowhere. So there is no JWT crate, no OAuth
crate, and no key fetching: one POST, one JSON body, one base64 payload, and
`json.rs` already exists.

That is also why the *browser-side* variant is worse on two counts. Verifying an
ID token means the page loaded Google Identity Services from `accounts.google.com`,
which puts a third-party script on a page that is currently self-contained, and
it is the variant that needs `jsonwebtoken` and Google's rotating keys.

## Recommendation

**Google sign-in over the server-side code flow, `ureq` and nothing else, and no
passwords.** Twenty-six crates, no async runtime, no outbox, and the guest path
stays exactly as it is.

Four properties worth having on purpose rather than by accident:

- **Guests are the default and stay the default.** Signing in is not how you play,
  it is how you carry your history to a second machine. Somebody with no Google
  account is not shut out of anything, which is the only thing that makes a
  single-provider decision defensible.
- **Store `sub` and nothing else.** Google will offer an email address, a name and
  a picture. The stable identifier is `sub`, which is what a credential should key
  on anyway because an email address can change hands and `sub` cannot. Taking
  only `sub` means the account system holds no personal data at all: H-8 retention
  becomes trivially satisfiable, and there is no address to leak or to send
  anything to even if somebody later wants to.
- **Keep the exchange behind a seam.** "Trade this code for a subject" must be a
  trait with a fake, or the test suite starts needing the internet and Google's
  uptime. Cheap to do first, tedious to retrofit.
- **A second provider is the same code again.** `people.rs` already holds several
  credentials per principal, so Apple or GitHub later is another redirect and
  another POST, not another design.

Two costs, both accepted deliberately:

- **`ring` enters the tree**, through rustls, because any outbound HTTPS needs a
  TLS stack. It is assembly and C, and it is the first dependency here that is not
  pure Rust. `unsafe_code = "forbid"` governs our crates rather than our
  dependencies, so nothing fails; this is a decision rather than an accident.
- **A client secret and a registered redirect**, per environment, which is the
  first secret this program has ever needed and one more thing to set in Railway
  beside the volume.

**Passkeys were the other email-free route and are not worth it here.** No
passwords, no email, no third party, and ninety-nine crates. Worse, the recovery
story is that losing your device loses your account unless you offer a fallback,
and the usual fallback is email, which is the thing being avoided.

## Where this leaves P-15

Overruled on the mechanism, kept on the principle. The principle was **own the
principal table**, and treat whatever proves an identity as a source rather than
the system of record. That is built, and it is the part §13 said could not be
retrofitted. Auth.js was the means, and its cost, a second service and a JS
framework, is out of proportion to what it buys a program whose entire
dependency list is currently empty.
