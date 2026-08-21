//! Who somebody is, kept apart from how they prove it.
//!
//! Three concepts that are easy to conflate, and the scoping doc (§8.2) names
//! all three because conflating them is what makes accounts impossible to add
//! later:
//!
//! - a **principal**, the durable identity, ours rather than any auth
//!   provider's, and the thing a game file records;
//! - a **credential**, how a principal proves it is itself. Today there is one
//!   kind, a device token in a cookie. Later there is a password and a Google
//!   account, and one principal may have several;
//! - a **seat**, a position in one game, which lives in `server.rs` and is
//!   nobody's identity at all.
//!
//! This file existed before any account does on purpose. §13 of the scoping doc
//! puts it plainly: identity cannot be retrofitted onto immutable logs. A game
//! file is written once and never edited, so whatever it records as the player
//! is the player for ever, and claiming an account later has to be an *alias*
//! resolved when the analytics read, never a rewrite of what was written.
//!
//! ## The split this fixes
//!
//! The cookie value was the identity. It was written into every game file as the
//! chair's key, which means each finished game carried a bearer token: anybody
//! who could read the directory could set that cookie and be that person. It
//! never reached a page, so it was not a live hole, but it was one page away
//! from being one, and a backup away from being one on somebody else's disk.
//!
//! A principal id is public. It goes in files, it is compared, it identifies. A
//! device token is a secret and lives in the cookie and in this table and
//! nowhere else. They are different strings for anybody arriving from now on.
//!
//! ## What it is not
//!
//! Not a login. There is no password here, no email, no session, and no way yet
//! to attach a second device to a principal. What there is, is the shape those
//! things attach to, and the alias table they will write into.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// What this build writes.
///
/// Version 1 is the first: people, their names, their device tokens and the
/// claim aliases. The same promise the game format makes, for the same reason:
/// a file written by a newer build says so rather than being half-read.
const VERSION: u32 = 1;

/// One durable identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    /// Public. This is what a game file records and what a rating is about.
    pub id: String,
    /// Unix milliseconds when we first saw them.
    pub created: u64,
    /// What they call themselves, or empty.
    ///
    /// On the person rather than on the seat, so somebody who has typed their
    /// name once has typed it for every table they will ever sit at. It used to
    /// live in a map beside the server and die with the process.
    pub name: String,
    /// Whether they have declared themselves old enough (P-11, sixteen).
    ///
    /// The table settings this person deals by default, as the query string
    /// [`deal`] already parses, or empty for the server's own defaults. The
    /// roster writes down what it was told and the server owns what the words
    /// mean, which is the same stance the game files take about chairs.
    pub table: String,
    /// Recorded here and asked nowhere yet: the declaration is a flow with a
    /// screen, and chat is its trigger rather than the account. The field is
    /// here so that the flow has somewhere to write when it is built, and so
    /// that "we never asked" is a value in the table rather than an absence
    /// somebody has to remember the meaning of.
    pub declared_adult: bool,
}

/// The table.
///
/// One lock over the whole thing. It is a handful of entries, every operation
/// is a map lookup, and the file is rewritten whole on any change: a reader
/// that has to reason about two locks to answer "who is this" is worse than a
/// lock nobody contends for.
pub struct People {
    path: PathBuf,
    book: Mutex<Book>,
    /// Whether there was a table here before this process opened it.
    ///
    /// The migration runs once, on a directory that has games in it and no
    /// table beside them. A server that has run this build before has a table
    /// and nothing to migrate, whatever is in the games.
    existed: bool,
}

#[derive(Default)]
struct Book {
    people: HashMap<String, Person>,
    /// Device token to principal. The only credential kind there is today.
    devices: HashMap<String, String>,
    /// Principals from before this table existed that no device has claimed.
    ///
    /// The migration, and the only circumstance in which presenting a principal
    /// gets you that principal. Every game already on disk records the old
    /// cookie value as its chair's key, so those visitors are known to history
    /// under a string their browser is still carrying: refusing it would orphan
    /// every game they have played, and accepting *any* unknown string would
    /// make a principal read off disk into a credential, which is the exact hole
    /// this file exists to close.
    ///
    /// So: only strings that already appear in a game file, and each one only
    /// once. The first browser to present it binds it to a fresh token and it
    /// leaves this set for ever. The window it opens is real and is written down
    /// here rather than in a commit message: for as long as an entry is
    /// unclaimed, somebody who can read the games directory can present that key
    /// and become that person. It closes the first time the owner visits, and
    /// there is nothing to migrate on a server that starts empty.
    pending: Vec<String>,
    /// Guest principal to account principal (P-1).
    ///
    /// Claiming an account does not move anybody's history, it points at it.
    /// The alternative, rewriting the games a guest played to carry the new id,
    /// breaks every checksum over them and destroys the append-only property
    /// that makes a replay worth trusting.
    aliases: HashMap<String, String>,
    /// A credential from somewhere else, to the principal it proves.
    ///
    /// Keyed by provider and subject together, `google:1234…`, so two providers
    /// that happen to number their users the same way are still two credentials.
    /// A principal may have several, which is what makes "sign in with Apple as
    /// well" a row rather than a redesign.
    ///
    /// What is stored is the subject and nothing else. Google will offer an
    /// email address, a name and a picture; the subject is the only one of them
    /// that is stable, because an address can change hands and a subject cannot,
    /// and taking only it means this table holds no personal data at all. There
    /// is nothing here to leak, and nothing to send anything to.
    credentials: HashMap<String, String>,
}

impl People {
    /// Open the table beside the games, reading it if it is there.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        // Its own extension, not the games'. The store lists a directory for
        // `*.carranta` and decodes what it finds, and this file being skipped
        // because it fails that decode is luck rather than design: a format that
        // cannot be mistaken for a game is better than one that is only ever
        // rejected.
        let path = dir.join("people.roster");
        let read = std::fs::read_to_string(&path).ok();
        let existed = read.is_some();
        let book = read.and_then(|t| decode(&t)).unwrap_or_default();
        People {
            path,
            book: Mutex::new(book),
            existed,
        }
    }

    /// Whether this table is being written for the first time.
    ///
    /// Only worth asking to avoid the work of gathering what [`Self::adopt_the_games`]
    /// would be handed: it reads every game on disk, and a server that has run
    /// this build before has nothing to migrate.
    pub fn is_new(&self) -> bool {
        !self.existed
    }

    /// Take the keys the games already name as people, once.
    ///
    /// Called with whatever the store says played, and does nothing at all
    /// unless this directory had games in it and no table beside them. See
    /// [`Book::pending`] for what it opens and how long for; the short of it is
    /// that a string is adoptable only if a game file already names it, and only
    /// until one browser presents it.
    pub fn adopt_the_games(&self, chairs: &[(String, String)]) {
        if self.existed {
            return;
        }
        let mut book = self.book.lock().unwrap();
        if !book.people.is_empty() {
            return;
        }
        for (key, name) in chairs {
            if !is_key(key) {
                continue;
            }
            if !book.people.contains_key(key) {
                book.pending.push(key.clone());
                book.people.insert(
                    key.clone(),
                    Person {
                        id: key.clone(),
                        // Not now: they were here before this table was. Nothing
                        // reads it as a fact about when they first played, and a
                        // made-up date that looks precise is worse than nought.
                        created: 0,
                        name: String::new(),
                        declared_adult: false,
                        table: String::new(),
                    },
                );
            }
            // The caller hands these over newest game first, so the first name a
            // person is seen under is the last one they chose. Somebody who has
            // been Marta for six games and typed something else once comes back
            // as Marta.
            if let Some(person) = book.people.get_mut(key)
                && person.name.is_empty()
            {
                person.name = name.clone();
            }
        }
        write(&self.path, &book);
    }

    /// Who this device belongs to, enrolling it if it is new.
    ///
    /// Answers with a principal in every case rather than an `Option` the caller
    /// has to invent a person for. Called on every request that needs to know
    /// who is asking, which is not every request: the art, the fonts and the
    /// sounds are the same bytes for everybody and enrol nobody.
    ///
    /// A token this table does not know is one of two things:
    ///
    /// - **A cookie from before this table existed**, which is exactly the set
    ///   [`Self::adopt_the_games`] wrote down and no more. That value is adopted
    ///   as the principal, so every game they have played is still theirs, and a
    ///   fresh token is minted so the secret and the identifier stop being the
    ///   same string. It leaves the pending list as it goes, so the next browser
    ///   to present it is a stranger.
    /// - **Anything else**, including a principal somebody read off disk and a
    ///   browser with no cookie at all: a new person, principal and token minted
    ///   independently.
    ///
    /// A caller with no cookie passes an empty string, which is the second case.
    pub fn arrive(&self, token: &str) -> Arrival {
        let mut book = self.book.lock().unwrap();
        if let Some(id) = book.devices.get(token) {
            return Arrival {
                principal: id.clone(),
                token: token.to_string(),
                fresh_token: false,
            };
        }
        // A cookie from before this table existed, and only one that a game file
        // already names: the value it carried is the name history knows them by,
        // so it becomes the principal and the cookie is given a new secret.
        // Taken out of the pending list as it is claimed, so the string is inert
        // from here on and a second browser presenting it is a stranger.
        //
        // Anything else, including a principal somebody read off disk, is a new
        // person. That is the whole of the split: a principal identifies and a
        // token proves, and only one of them is a secret.
        let adopting = book.pending.iter().any(|k| k == token);
        let principal = if adopting {
            book.pending.retain(|k| k != token);
            token.to_string()
        } else {
            mint(&book.people)
        };
        let fresh = mint_token(&book.devices);
        book.devices.insert(fresh.clone(), principal.clone());
        book.people.entry(principal.clone()).or_insert(Person {
            id: principal.clone(),
            created: now(),
            name: String::new(),
            declared_adult: false,
            table: String::new(),
        });
        let out = Arrival {
            principal,
            token: fresh,
            fresh_token: true,
        };
        write(&self.path, &book);
        out
    }

    /// What somebody calls themselves.
    pub fn name(&self, principal: &str) -> String {
        self.book
            .lock()
            .unwrap()
            .people
            .get(principal)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }

    /// The table settings this person deals by, or empty for the server's.
    pub fn table_defaults(&self, principal: &str) -> String {
        self.book
            .lock()
            .unwrap()
            .people
            .get(principal)
            .map(|p| p.table.clone())
            .unwrap_or_default()
    }

    /// Set the table settings this person deals by. Empty clears them.
    pub fn set_table_defaults(&self, principal: &str, table: &str) {
        let mut book = self.book.lock().unwrap();
        let Some(person) = book.people.get_mut(principal) else {
            return;
        };
        if person.table != table {
            person.table = table.to_string();
            write(&self.path, &book);
        }
    }

    /// Set what somebody calls themselves, if it has changed.
    pub fn rename(&self, principal: &str, name: &str) {
        let mut book = self.book.lock().unwrap();
        let Some(person) = book.people.get_mut(principal) else {
            return;
        };
        if person.name == name {
            return;
        }
        person.name = name.to_string();
        write(&self.path, &book);
    }

    /// Record that somebody has declared themselves old enough (P-11).
    pub fn declare_adult(&self, principal: &str) {
        let mut book = self.book.lock().unwrap();
        let Some(person) = book.people.get_mut(principal) else {
            return;
        };
        if person.declared_adult {
            return;
        }
        person.declared_adult = true;
        write(&self.path, &book);
    }

    /// Whether somebody has declared themselves old enough (P-11).
    pub fn declared_adult(&self, principal: &str) -> bool {
        self.book
            .lock()
            .unwrap()
            .people
            .get(principal)
            .is_some_and(|p| p.declared_adult)
    }

    /// Somebody has proved a credential. Work out who they are now.
    ///
    /// Four cases, and the interesting one is the third.
    ///
    /// 1. **The credential is already this browser's principal.** Nothing to do
    ///    but hand back a fresh token, which signing in should do anyway.
    /// 2. **The credential is new and this browser has no account.** The guest
    ///    they already are *becomes* the account: the credential is attached to
    ///    the principal they have been playing under. No alias is needed and
    ///    nothing is claimed, because their history was never anybody else's.
    ///    This is the ordinary path and it is the reason guests are worth having.
    /// 3. **The credential belongs to somebody else and this browser is a guest
    ///    with something to keep.** They played here before signing in, on this
    ///    machine, as a guest; the account is theirs on another. That is exactly
    ///    P-1, so the guest is aliased to the account and their games follow. The
    ///    caller decides what "something to keep" means, because whether a
    ///    principal has ever played is a question about games and this file knows
    ///    nothing about games.
    /// 4. **The credential is new and this browser is already signed in as
    ///    somebody else.** Two accounts, one machine. A fresh principal takes the
    ///    credential and nothing is aliased: the person sitting here has said
    ///    they are somebody different, not that two accounts are one.
    ///
    /// Always ends with a *new* device token. A session that survives signing in
    /// is a session that survived whatever came before it.
    pub fn sign_in(&self, credential: &str, browser: &str, claimable: bool) -> SignedIn {
        let mut book = self.book.lock().unwrap();
        let known = book.credentials.get(credential).cloned();
        let browser_has_account = book.credentials.values().any(|p| p == browser);
        let (principal, claimed) = match known {
            // Cases 1 and 3.
            Some(theirs) => {
                let claim = theirs != browser && claimable && !browser_has_account;
                if claim {
                    book.aliases.insert(browser.to_string(), theirs.clone());
                }
                (theirs, claim.then(|| browser.to_string()))
            }
            // Case 2.
            None if !browser_has_account && book.people.contains_key(browser) => {
                book.credentials
                    .insert(credential.to_string(), browser.to_string());
                (browser.to_string(), None)
            }
            // Case 4, and a browser this table has never heard of.
            None => {
                let fresh = mint(&book.people);
                book.people.insert(
                    fresh.clone(),
                    Person {
                        id: fresh.clone(),
                        created: now(),
                        name: String::new(),
                        declared_adult: false,
                        table: String::new(),
                    },
                );
                book.credentials
                    .insert(credential.to_string(), fresh.clone());
                (fresh, None)
            }
        };
        let token = mint_token(&book.devices);
        book.devices.insert(token.clone(), principal.clone());
        let out = SignedIn {
            principal,
            token,
            claimed,
        };
        write(&self.path, &book);
        out
    }

    /// Whether this principal has signed in with anything.
    pub fn has_account(&self, principal: &str) -> bool {
        self.book
            .lock()
            .unwrap()
            .credentials
            .values()
            .any(|p| p == principal)
    }

    /// Stop this token proving anybody.
    ///
    /// Removed rather than merely forgotten by the browser: a cookie that is
    /// cleared on one machine and still valid on the server is a session that
    /// somebody who copied it still holds. Signing out should end the session,
    /// not the browser's memory of it.
    pub fn sign_out(&self, token: &str) {
        let mut book = self.book.lock().unwrap();
        if book.devices.remove(token).is_some() {
            write(&self.path, &book);
        }
    }

    /// Point a guest's history at an account (P-1).
    ///
    /// Both must exist and must not be the same person, and the account must not
    /// itself be pointed somewhere: an alias of an alias is a chain somebody has
    /// to walk, and a chain is a cycle waiting to be written by hand.
    pub fn claim(&self, guest: &str, account: &str) -> Result<(), &'static str> {
        let mut book = self.book.lock().unwrap();
        if guest == account {
            return Err("that is the same person");
        }
        if !book.people.contains_key(guest) || !book.people.contains_key(account) {
            return Err("no such person");
        }
        if book.aliases.contains_key(account) {
            return Err("that account is itself claimed");
        }
        if book.aliases.contains_key(guest) {
            return Err("that guest is already claimed");
        }
        book.aliases.insert(guest.to_string(), account.to_string());
        write(&self.path, &book);
        Ok(())
    }

    /// Say that this token proves this principal.
    ///
    /// Tests only, and deliberately not a way in from anywhere else: choosing
    /// your own credential is the thing the whole file is arranged to prevent.
    /// What it buys is tests that can drive the server over HTTP as a named
    /// person, which is how they were written when a cookie was an identity.
    #[cfg(test)]
    pub fn bind(&self, token: &str, principal: &str) {
        let mut book = self.book.lock().unwrap();
        book.devices
            .insert(token.to_string(), principal.to_string());
        book.people.entry(principal.to_string()).or_insert(Person {
            id: principal.to_string(),
            created: now(),
            name: String::new(),
            declared_adult: false,
            table: String::new(),
        });
        write(&self.path, &book);
    }

    /// Everybody in the table, oldest first. For tests and for a future page.
    pub fn all(&self) -> Vec<Person> {
        let mut out: Vec<Person> = self.book.lock().unwrap().people.values().cloned().collect();
        out.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
        out
    }
}

/// What a request's cookie resolved to.
pub struct Arrival {
    /// Who they are. This is what goes into a game file.
    pub principal: String,
    /// What their cookie should say. The same as it did, unless it is new.
    pub token: String,
    /// Whether the caller has to set the cookie.
    pub fresh_token: bool,
}

/// What signing in settled.
pub struct SignedIn {
    /// Who they are now.
    pub principal: String,
    /// A new cookie for this browser, always.
    pub token: String,
    /// The guest principal whose history was pointed at this account, if any.
    ///
    /// Worth telling them about: "your games from this browser are now on your
    /// account" is the whole point of the flow, and a silent claim looks like a
    /// silent loss.
    pub claimed: Option<String>,
}

/// Following a claim, at the moment somebody reads rather than at the moment
/// somebody played (P-1, §8.2).
///
/// A trait so that the analytics can be handed one without depending on this
/// file, and so that the tests, which have no table, can be handed nothing.
pub trait Aliases {
    /// Whose record this player's games belong to now.
    fn resolve(&self, principal: &str) -> String;
}

impl Aliases for People {
    fn resolve(&self, principal: &str) -> String {
        let book = self.book.lock().unwrap();
        let mut at = principal;
        // Chains are refused when they are written, so one step is the whole of
        // it. The loop is a belt on that brace and is bounded, because a table
        // somebody edited by hand is a table that can name a cycle.
        for _ in 0..8 {
            match book.aliases.get(at) {
                Some(next) if next != at => at = next,
                _ => break,
            }
        }
        at.to_string()
    }
}

/// Nobody has claimed anything, which is what a corpus with no table beside it
/// means and what every test means.
pub struct NoAliases;

impl Aliases for NoAliases {
    fn resolve(&self, principal: &str) -> String {
        principal.to_string()
    }
}

/// Length of an id or a token. The same as the old visitor key, so an adopted
/// cookie is a well-formed principal.
const KEY_LEN: usize = 16;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Sixteen characters of real randomness, not already taken.
///
/// From the operating system, and this is not a nicety. A device token is a
/// bearer credential: whoever holds it is that person, with no second factor and
/// nothing else to check. These used to come from the clock and the process id
/// run through a mixer, which is fine for naming a browser and useless the
/// moment the name became the proof. Two visitors in the same millisecond got
/// adjacent values, and anybody who could guess when somebody first arrived
/// could enumerate a small space around it.
///
/// Sixteen of thirty-six characters is a little over eighty-two bits, drawn
/// uniformly, which is not a space anybody walks through.
pub(crate) fn secret() -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    // Rejection sampling rather than a modulus. Thirty-six does not divide two
    // hundred and fifty-six, so folding a byte would make the first sixteen
    // letters likelier than the rest, and a biased alphabet is a smaller space
    // than it looks.
    let mut out = String::with_capacity(KEY_LEN);
    let mut buf = [0u8; 64];
    while out.len() < KEY_LEN {
        if getrandom::fill(&mut buf).is_err() {
            // The system refused to give us randomness, which is not a thing to
            // paper over with a clock: every value this returns is somebody's
            // credential. Better to take the process down than to hand out a
            // guessable one and never know.
            panic!("no system randomness available, refusing to mint a credential");
        }
        for b in buf {
            if out.len() == KEY_LEN {
                break;
            }
            if (b as usize) < 252 {
                out.push(ALPHABET[(b % 36) as usize] as char);
            }
        }
    }
    out
}

/// A new string of the shape everything here uses, not already taken.
fn fresh<T>(taken: &HashMap<String, T>) -> String {
    for _ in 0..64 {
        let out = secret();
        if !taken.contains_key(&out) {
            return out;
        }
    }
    // Sixty-four collisions in a row against eighty-two bits does not happen,
    // and if it somehow did it would mean the randomness is broken, which is the
    // one case where carrying on regardless is worse than stopping.
    panic!("cannot find an unused key, which means the randomness is not random");
}

fn mint(taken: &HashMap<String, Person>) -> String {
    fresh(taken)
}

fn mint_token(taken: &HashMap<String, String>) -> String {
    fresh(taken)
}

/// Whether a string is one of ours, which is the check a cookie gets.
pub fn is_key(s: &str) -> bool {
    s.len() == KEY_LEN
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// The whole table, as a file somebody can open.
///
/// Rewritten whole on every change rather than appended to. The table is small
/// and an append log would need compaction, which is a second thing to get
/// right for a file that fits in a packet.
fn encode(book: &Book) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "carranta-people {VERSION}");
    let mut people: Vec<&Person> = book.people.values().collect();
    people.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.id.cmp(&b.id)));
    for p in people {
        let _ = writeln!(
            out,
            "person {} {} {}",
            p.id,
            p.created,
            if p.declared_adult { "adult" } else { "unasked" }
        );
        // A name is somebody else's text and runs to the end of its line, so it
        // gets a line of its own and there is nothing to escape. Omitted when
        // there is none: an empty name is not a name.
        if !p.name.is_empty() {
            let _ = writeln!(out, "name {} {}", p.id, p.name);
        }
        // A query string holds no spaces, so it fits the line format the way
        // an id does; empty means the server's defaults and is not written.
        if !p.table.is_empty() {
            let _ = writeln!(out, "table {} {}", p.id, p.table);
        }
    }
    let mut devices: Vec<(&String, &String)> = book.devices.iter().collect();
    devices.sort();
    for (token, principal) in devices {
        let _ = writeln!(out, "device {token} {principal}");
    }
    let mut aliases: Vec<(&String, &String)> = book.aliases.iter().collect();
    aliases.sort();
    for (guest, account) in aliases {
        let _ = writeln!(out, "claimed {guest} {account}");
    }
    // Written down so a restart does not reopen the migration for keys that have
    // already been claimed, and does not close it for keys that have not.
    let mut pending = book.pending.clone();
    pending.sort();
    for key in pending {
        let _ = writeln!(out, "awaiting {key}");
    }
    let mut credentials: Vec<(&String, &String)> = book.credentials.iter().collect();
    credentials.sort();
    for (credential, principal) in credentials {
        let _ = writeln!(out, "proves {credential} {principal}");
    }
    out
}

fn decode(text: &str) -> Option<Book> {
    let mut book = Book::default();
    let mut version = None;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (head, rest) = line.split_once(' ').unwrap_or((line, ""));
        match head {
            "carranta-people" => version = rest.parse::<u32>().ok(),
            "person" => {
                let mut t = rest.split_whitespace();
                let id = t.next()?.to_string();
                let created = t.next()?.parse().ok()?;
                let declared_adult = t.next() == Some("adult");
                book.people.insert(
                    id.clone(),
                    Person {
                        id,
                        created,
                        name: String::new(),
                        declared_adult,
                        table: String::new(),
                    },
                );
            }
            "name" => {
                let (id, name) = rest.split_once(' ')?;
                if let Some(p) = book.people.get_mut(id) {
                    p.name = name.to_string();
                }
            }
            "table" => {
                let (id, table) = rest.split_once(' ')?;
                if let Some(p) = book.people.get_mut(id) {
                    p.table = table.to_string();
                }
            }
            "device" => {
                let (token, principal) = rest.split_once(' ')?;
                book.devices
                    .insert(token.to_string(), principal.to_string());
            }
            "claimed" => {
                let (guest, account) = rest.split_once(' ')?;
                book.aliases.insert(guest.to_string(), account.to_string());
            }
            "awaiting" => book.pending.push(rest.to_string()),
            "proves" => {
                let (credential, principal) = rest.split_once(' ')?;
                book.credentials
                    .insert(credential.to_string(), principal.to_string());
            }
            // An unknown line is a newer build's, and the version check below is
            // what decides whether to trust any of it.
            _ => {}
        }
    }
    version
        .is_some_and(|v| (1..=VERSION).contains(&v))
        .then_some(book)
}

fn write(path: &Path, book: &Book) {
    let _ = std::fs::write(path, encode(book));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("carranta-people-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_new_browser_gets_an_identity_and_a_secret_that_are_not_the_same_string() {
        // The split this file exists for. The cookie value used to be the
        // identity and was written into every game file, so each finished game
        // carried a bearer token: anybody who could read the directory could
        // set that cookie and be that person.
        let d = dir("new");
        let people = People::open(&d);
        let a = people.arrive("");
        assert!(a.fresh_token, "a browser with no cookie is given one");
        assert!(is_key(&a.principal) && is_key(&a.token));
        assert_ne!(
            a.principal, a.token,
            "the name history knows them by is not the secret they prove it with"
        );
        // And the token is what a cookie carries back, so the same browser is
        // the same person on its next visit.
        let again = people.arrive(&a.token);
        assert_eq!(again.principal, a.principal);
        assert!(!again.fresh_token, "and needs no new cookie");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_cookie_from_before_this_table_keeps_its_history_exactly_once() {
        // Every game already on disk records the old cookie value as the chair's
        // key. Minting a fresh principal for those visitors would orphan every
        // game they have played, so the value they are already known by is
        // adopted, once, and the cookie is given a new secret.
        let d = dir("old");
        let was = "sd2v5zlwmnmgxdfw";
        let people = People::open(&d);
        people.adopt_the_games(&[(was.to_string(), "Egon".to_string())]);
        let a = people.arrive(was);
        assert_eq!(a.principal, was, "still the person their games name");
        assert!(a.fresh_token, "under a new secret");
        assert_ne!(a.token, was);
        assert_eq!(
            people.name(was),
            "Egon",
            "and under the name their games call them, rather than having to \
             type it again"
        );

        // And it is claimed. A second browser presenting the same string is a
        // stranger, which is what closes the window the migration opens.
        let replay = people.arrive(was);
        assert_ne!(
            replay.principal, was,
            "the string is inert once its owner has been here"
        );
        // A restart neither reopens it nor closes an unclaimed one.
        let other = "otherkey00000000";
        let people = People::open(&d);
        assert!(!people.arrive(was).principal.eq(was), "still claimed");
        people.adopt_the_games(&[(other.to_string(), String::new())]);
        assert_ne!(
            people.arrive(other).principal,
            other,
            "and a table that already exists has nothing to migrate, whatever \
             is in the games"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_principal_off_disk_is_not_a_way_in() {
        // The hole this whole file is about. A key that no game names was never
        // adoptable; a key that a game names stops being adoptable the moment
        // its owner turns up. Neither is ever a credential.
        let d = dir("thief");
        let people = People::open(&d);
        let mine = people.arrive("").principal;
        assert_ne!(
            people.arrive(&mine).principal,
            mine,
            "presenting somebody's principal buys nothing"
        );
        assert_ne!(
            people.arrive("nobodyatall00000").principal,
            "nobodyatall00000",
            "nor does presenting a string nobody has heard of"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_table_survives_a_restart() {
        let d = dir("restart");
        let (principal, token) = {
            let people = People::open(&d);
            let a = people.arrive("");
            people.rename(&a.principal, "Egon of the Long Name, and a comma");
            people.declare_adult(&a.principal);
            (a.principal, a.token)
        };
        let people = People::open(&d);
        let back = people.arrive(&token);
        assert_eq!(back.principal, principal);
        assert_eq!(
            people.name(&principal),
            "Egon of the Long Name, and a comma"
        );
        assert!(people.declared_adult(&principal));
        // A name is where the person is, not where the seat is: somebody who
        // typed it once has typed it for every table they will sit at. It used
        // to live in a map beside the server and die with the process.
        assert_eq!(people.all().len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn claiming_an_account_points_at_history_rather_than_moving_it() {
        // P-1. Logs are immutable, so a guest who signs up cannot have their
        // games rewritten to carry the new id: the claim is an alias and the
        // analytics resolve through it when they read.
        let d = dir("claim");
        let people = People::open(&d);
        let guest = people.arrive("").principal;
        let account = people.arrive("").principal;
        assert_eq!(people.resolve(&guest), guest, "nothing claimed yet");
        assert!(people.claim(&guest, &account).is_ok());
        assert_eq!(people.resolve(&guest), account, "their games are theirs");
        assert_eq!(people.resolve(&account), account);
        // The refusals, all of which are ways to write a chain or a cycle by
        // hand and then have to walk one.
        assert!(people.claim(&guest, &guest).is_err());
        assert!(people.claim(&guest, &account).is_err(), "already claimed");
        assert!(people.claim(&account, &guest).is_err(), "would be a cycle");
        assert!(people.claim(&guest, "nobodyatall00000").is_err());
        // And it survives the trip through the file.
        let people = People::open(&d);
        assert_eq!(people.resolve(&guest), account);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn signing_in_settles_which_of_four_things_just_happened() {
        let d = dir("signin");
        let people = People::open(&d);

        // 2. A credential nobody has used, on a browser with no account: the
        //    guest they already are becomes the account. Nothing is claimed,
        //    because their history was never anybody else's.
        let egon = people.arrive("").principal;
        let first = people.sign_in("g:egon", &egon, true);
        assert_eq!(first.principal, egon, "the guest became the account");
        assert_eq!(first.claimed, None, "and had nothing to claim");
        assert!(people.has_account(&egon));

        // 1. The same person again on the same browser: nothing to settle, but
        //    a new token all the same.
        let again = people.sign_in("g:egon", &egon, true);
        assert_eq!(again.principal, egon);
        assert_eq!(again.claimed, None);
        assert_ne!(again.token, first.token, "a fresh session either way");
        assert_eq!(people.arrive(&again.token).principal, egon);

        // 3. Their account, reached from a second machine where they have been
        //    playing as a guest. That guest's games follow them (P-1).
        let elsewhere = people.arrive("").principal;
        let moved = people.sign_in("g:egon", &elsewhere, true);
        assert_eq!(moved.principal, egon, "they are who the credential says");
        assert_eq!(moved.claimed.as_deref(), Some(elsewhere.as_str()));
        assert_eq!(people.resolve(&elsewhere), egon, "and so are their games");

        // The same, from a guest who has never played: nothing to point at, so
        // nothing is pointed. An alias per idle visitor is a table of rows that
        // mean nothing.
        let idle = people.arrive("").principal;
        assert_eq!(people.sign_in("g:egon", &idle, false).claimed, None);
        assert_eq!(people.resolve(&idle), idle, "left alone");

        // 4. A different person signing in on a machine that is already
        //    somebody's. Two accounts, one browser: a new principal, and the
        //    first one's history stays the first one's.
        let marta = people.sign_in("g:marta", &egon, true);
        assert_ne!(marta.principal, egon, "not the same person");
        assert_eq!(marta.claimed, None, "and not a claim on Egon's games");
        assert_eq!(people.resolve(&egon), egon, "Egon is still Egon");
        assert_eq!(people.arrive(&marta.token).principal, marta.principal);

        // All of it survives the trip through the file.
        let people = People::open(&d);
        assert_eq!(people.arrive(&moved.token).principal, egon);
        assert_eq!(people.resolve(&elsewhere), egon);
        assert!(people.has_account(&marta.principal));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn signing_out_ends_the_session_rather_than_the_browser_s_memory_of_it() {
        // A cookie cleared on one machine and still good on the server is a
        // session that whoever copied it still holds.
        let d = dir("signout");
        let people = People::open(&d);
        let a = people.arrive("");
        people.sign_out(&a.token);
        assert_ne!(
            people.arrive(&a.token).principal,
            a.principal,
            "that token proves nobody now"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_token_is_not_something_anybody_can_guess() {
        // These used to come from the clock and the process id, which is fine
        // for naming a browser and useless the moment the name became the
        // proof: two visitors in the same millisecond got adjacent values.
        let d = dir("entropy");
        let people = People::open(&d);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..500 {
            let a = people.arrive("");
            assert!(is_key(&a.token), "the shape the reader accepts");
            assert!(seen.insert(a.token), "and never the same one twice");
            assert!(seen.insert(a.principal));
        }
        // Every letter of the alphabet turns up across a thousand draws, which
        // a biased or truncated encoding would not manage.
        let letters: std::collections::HashSet<char> =
            seen.iter().flat_map(|s| s.chars()).collect();
        assert_eq!(letters.len(), 36, "the whole alphabet is in play");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_table_from_another_version_is_not_read_as_this_one() {
        let d = dir("version");
        let people = People::open(&d);
        let a = people.arrive("");
        let text = std::fs::read_to_string(d.join("people.roster")).expect("written");
        assert!(text.starts_with(&format!("carranta-people {VERSION}")));
        assert!(decode(&text).is_some(), "this build reads its own");
        let future = text.replace(
            &format!("carranta-people {VERSION}"),
            &format!("carranta-people {}", VERSION + 1),
        );
        assert!(decode(&future).is_none(), "and not a newer build's");
        assert!(
            decode("nothing here").is_none(),
            "nor a file with no version"
        );
        // A newer build's extra lines are ignored rather than fatal, so the
        // version check is the only thing that decides.
        let extra = format!("{text}whatever 1 2 3\n");
        assert_eq!(
            decode(&extra).map(|b| b.devices.len()),
            Some(1),
            "an unknown line is not a broken file"
        );
        assert_eq!(people.all()[0].id, a.principal);
        let _ = std::fs::remove_dir_all(&d);
    }
}
