//! Signing in with somebody else's identity provider.
//!
//! The server-side authorization code flow, and nothing more of OAuth than that.
//! The browser is sent to Google, comes back with a code, and this trades the
//! code for a subject over a connection made directly to Google. Three requests
//! in total, one of them ours.
//!
//! ## Why there is no signature check here
//!
//! Google's response carries an ID token, which is a signed JWT, and this does
//! not verify the signature. That is what Google's own documentation says to do
//! in this flow: *"since you are communicating directly with Google over an
//! intermediary-free HTTPS channel and using your client secret to authenticate
//! yourself to Google, you can be confident that the token you receive really
//! comes from Google and is valid."* The caveat attached to it is that any
//! component the token is *passed* to must validate it, and it is passed
//! nowhere: the subject is read out here and the token is dropped.
//!
//! Trust rests on TLS and the client secret rather than on a signature, which is
//! a smaller thing to get right and a much smaller thing to depend on. The
//! alternative, the browser-side flow, needs a script from `accounts.google.com`
//! on a page that has none, a JWT library, and Google's rotating public keys.
//!
//! ## Why the exchange is a trait
//!
//! Because a test suite that needs the internet and Google's uptime is a test
//! suite that fails for reasons nobody changed. [`Exchange`] is the seam: the
//! real one makes an HTTPS request, and the tests hand over a fake that answers
//! from a table. Everything else in the flow, which is where the mistakes
//! actually live, is tested either way.
//!
//! ## What is stored
//!
//! The subject, and nothing else. Not the email address, not the name, not the
//! picture, all of which Google offers. See `people.rs`.

use std::sync::Mutex;

/// Trading an authorization code for a durable subject.
///
/// One method, because that is the whole of what this server needs from an
/// identity provider: not a session, not a profile, not an access token to call
/// anything with. Who is this, durably, so a second visit is the same person.
pub trait Exchange: Send + Sync {
    /// The provider's name, which is half of a credential key.
    fn provider(&self) -> &str;
    /// Where to send somebody to sign in, carrying this opaque state.
    fn away(&self, state: &str) -> String;
    /// Trade a code for that provider's stable subject.
    fn subject(&self, code: &str) -> Result<String, String>;
}

/// Google, over the server-side code flow.
pub struct Google {
    client_id: String,
    client_secret: String,
    /// Where Google sends them back. Registered with Google, per environment,
    /// and sent again here because Google checks the two agree.
    redirect: String,
    /// Overridden by the tests that exercise the request builder itself.
    token_url: String,
    auth_url: String,
}

impl Google {
    /// From the environment, or nothing at all.
    ///
    /// Absent configuration is not an error and must not be: the whole feature
    /// is optional, so that a checkout with no secrets in it runs, serves,
    /// plays and passes its tests, and simply does not offer a way to sign in.
    /// A server that refused to start without a Google client would make every
    /// contributor get one.
    pub fn from_env() -> Option<Self> {
        Self::configured(|k| std::env::var(k).ok())
    }

    /// The same, from wherever the caller keeps its settings.
    ///
    /// Split out so the tests can describe a configuration without setting
    /// process environment variables, which is a global mutation, shared with
    /// every other test in the binary, and `unsafe` since the 2024 edition for
    /// exactly that reason.
    pub fn configured(look: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let client_id = look("GOOGLE_CLIENT_ID")?;
        let client_secret = look("GOOGLE_CLIENT_SECRET")?;
        let origin = look("PUBLIC_ORIGIN")?;
        if client_id.is_empty() || client_secret.is_empty() || origin.is_empty() {
            return None;
        }
        Some(Google {
            client_id,
            client_secret,
            redirect: format!("{}/signin/done", origin.trim_end_matches('/')),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        })
    }
}

impl Exchange for Google {
    fn provider(&self) -> &str {
        "google"
    }

    fn away(&self, state: &str) -> String {
        // `openid` alone. Every other scope asks for something about a person
        // that this server has decided not to hold, and a consent screen listing
        // things nobody wanted is its own kind of lie.
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope=openid&state={}",
            self.auth_url,
            urlencode(&self.client_id),
            urlencode(&self.redirect),
            urlencode(state),
        )
    }

    fn subject(&self, code: &str) -> Result<String, String> {
        let body = format!(
            "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
            urlencode(code),
            urlencode(&self.client_id),
            urlencode(&self.client_secret),
            urlencode(&self.redirect),
        );
        let answer = ureq::post(&self.token_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .send(&body)
            .map_err(|e| format!("could not reach the sign-in service: {e}"))?
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("the sign-in service answered oddly: {e}"))?;
        let token = crate::json::field(&answer, "id_token")
            .ok_or_else(|| "the sign-in service sent no identity".to_string())?;
        subject_of(&token).ok_or_else(|| "that identity has no subject in it".to_string())
    }
}

/// The `sub` out of a JWT's payload, without verifying anything.
///
/// Safe only because of where this is called from: the token came back over a
/// connection this process opened to Google, authenticated with our client
/// secret, and goes nowhere else. See the module note.
///
/// Three segments separated by dots, the middle one base64url of some JSON.
pub fn subject_of(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let json = String::from_utf8(base64url(payload)?).ok()?;
    let sub = crate::json::field(&json, "sub")?;
    (!sub.is_empty()).then_some(sub)
}

/// Base64url, no padding, rejecting anything that is not.
fn base64url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// Percent-encoding for a query value, allowing only what never needs it.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// How long a sign-in that has been started but not finished is held.
///
/// Long enough to read a consent screen and find the right account, short enough
/// that a link somebody left open in a tab last week is not still a live way in.
const PENDING_LIMIT: u64 = 10 * 60 * 1000;

/// Sign-ins that have been started and not yet come back.
///
/// The `state` parameter, which is the whole of the defence against being
/// walked into somebody else's account: an attacker who can make your browser
/// follow a link cannot make it follow one carrying a value only your browser
/// was given. Held here rather than signed into the URL because the table also
/// has to say *which browser* started it, and a row that can be deleted is one
/// that cannot be replayed.
pub struct Pending {
    waiting: Mutex<Vec<Started>>,
}

struct Started {
    state: String,
    /// The device token the browser held when it set out, so the sign-in is
    /// attached to the person who started it rather than to whoever comes back.
    browser: String,
    when: u64,
}

impl Default for Pending {
    fn default() -> Self {
        Self::new()
    }
}

impl Pending {
    pub fn new() -> Self {
        Pending {
            waiting: Mutex::new(Vec::new()),
        }
    }

    /// Begin one, and answer with the state to send.
    pub fn begin(&self, browser: &str) -> String {
        let state = crate::people::secret();
        let mut waiting = self.waiting.lock().unwrap();
        let cutoff = now().saturating_sub(PENDING_LIMIT);
        waiting.retain(|s| s.when > cutoff);
        // A cap, because this is memory anybody can ask for by loading a page.
        // The oldest go first, which at worst makes somebody press the button
        // again.
        if waiting.len() >= 256 {
            waiting.remove(0);
        }
        waiting.push(Started {
            state: state.clone(),
            browser: browser.to_string(),
            when: now(),
        });
        state
    }

    /// Finish one, and answer with the browser that started it.
    ///
    /// Consumed whether or not it is used again, so a state is good exactly
    /// once: a callback that can be replayed is a sign-in that can be replayed.
    pub fn finish(&self, state: &str) -> Option<String> {
        let mut waiting = self.waiting.lock().unwrap();
        let cutoff = now().saturating_sub(PENDING_LIMIT);
        waiting.retain(|s| s.when > cutoff);
        let at = waiting.iter().position(|s| s.state == state)?;
        Some(waiting.remove(at).browser)
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subject_is_read_out_of_a_token_without_trusting_its_shape() {
        // The payload of a real Google token, with the parts this reads and
        // nothing that could be mistaken for a signature check.
        let payload = br#"{"iss":"https://accounts.google.com","sub":"110169484474386276334","email":"x@example.com"}"#;
        let jwt = format!("header.{}.signature", b64url(payload));
        assert_eq!(subject_of(&jwt).as_deref(), Some("110169484474386276334"));

        // And every way it can be wrong is `None` rather than a panic or a
        // half-read value, because all of this is somebody else's bytes.
        assert_eq!(subject_of(""), None);
        assert_eq!(subject_of("onlyonesegment"), None);
        assert_eq!(subject_of("a.b"), None, "no payload to read");
        assert_eq!(subject_of("a.!!!!.c"), None, "not base64");
        assert_eq!(
            subject_of(&format!("a.{}.c", b64url(b"{\"iss\":\"who\"}"))),
            None,
            "no subject in it"
        );
        assert_eq!(
            subject_of(&format!("a.{}.c", b64url(b"{\"sub\":\"\"}"))),
            None,
            "an empty subject is not a subject"
        );
        assert_eq!(subject_of("a.bm90IGpzb24.c"), None, "not json");
    }

    #[test]
    fn a_state_is_good_once_and_only_for_ten_minutes() {
        // The whole of the defence against being walked into somebody else's
        // account, so its two properties are worth stating outright.
        let pending = Pending::new();
        let state = pending.begin("browser-a");
        assert_eq!(pending.finish(&state).as_deref(), Some("browser-a"));
        assert_eq!(
            pending.finish(&state),
            None,
            "and not a second time: a callback that replays is a sign-in that \
             replays"
        );
        assert_eq!(pending.finish("made up"), None);

        // It is also unguessable, which is the other half. Not a clock.
        let a = pending.begin("browser-a");
        let b = pending.begin("browser-b");
        assert_ne!(a, b);
        assert_eq!(a.len(), 16);

        // And it expires.
        {
            let mut waiting = pending.waiting.lock().unwrap();
            for s in waiting.iter_mut() {
                s.when = now().saturating_sub(30 * 60 * 1000);
            }
        }
        assert_eq!(pending.finish(&a), None, "an hour later is too late");
    }

    #[test]
    fn the_pending_list_cannot_be_grown_without_limit() {
        // It is memory anybody can ask for by loading a page.
        let pending = Pending::new();
        let first = pending.begin("browser");
        for _ in 0..300 {
            pending.begin("browser");
        }
        assert!(pending.waiting.lock().unwrap().len() <= 256);
        assert_eq!(pending.finish(&first), None, "the oldest went");
    }

    #[test]
    fn nothing_is_offered_without_somewhere_to_send_people() {
        // The feature is optional and has to be, or every checkout needs a
        // Google client of its own before it can serve a page.
        assert!(Google::configured(|_| None).is_none(), "nothing set");
        let whole = |k: &str| {
            Some(
                match k {
                    "GOOGLE_CLIENT_ID" => "id-1",
                    "GOOGLE_CLIENT_SECRET" => "shh",
                    _ => "https://example.test",
                }
                .to_string(),
            )
        };
        assert!(Google::configured(whole).is_some(), "all three set");
        for missing in ["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET", "PUBLIC_ORIGIN"] {
            assert!(
                Google::configured(|k| (k != missing).then(|| whole(k)).flatten()).is_none(),
                "half a configuration is not a configuration: {missing} missing"
            );
            assert!(
                Google::configured(|k| if k == missing {
                    Some(String::new())
                } else {
                    whole(k)
                })
                .is_none(),
                "and an empty {missing} is the same as none"
            );
        }
        // The redirect is derived once, so there is one place it can disagree
        // with what Google was told.
        let google = Google::configured(|k| {
            Some(
                match k {
                    "GOOGLE_CLIENT_ID" => "id-1",
                    "GOOGLE_CLIENT_SECRET" => "shh",
                    _ => "https://example.test/",
                }
                .to_string(),
            )
        })
        .expect("configured");
        assert_eq!(google.redirect, "https://example.test/signin/done");
    }

    #[test]
    fn the_url_carries_what_google_checks_and_nothing_it_did_not_ask_for() {
        let google = Google {
            client_id: "id-1".to_string(),
            client_secret: "shh".to_string(),
            redirect: "https://example.test/signin/done".to_string(),
            token_url: "https://example.test/token".to_string(),
            auth_url: "https://example.test/auth".to_string(),
        };
        let away = google.away("state-1");
        assert!(away.contains("client_id=id-1"));
        assert!(away.contains("response_type=code"));
        assert!(away.contains("state=state-1"));
        assert!(
            away.contains("redirect_uri=https%3A%2F%2Fexample.test%2Fsignin%2Fdone"),
            "encoded, or Google reads it as a different address: {away}"
        );
        // Only `openid`. Every other scope asks for something about a person
        // this server has decided not to hold.
        assert!(away.contains("scope=openid"));
        for asked in ["email", "profile"] {
            assert!(!away.contains(asked), "{asked} is not ours to ask for");
        }
        // And the secret is never in a URL the browser is handed.
        assert!(!away.contains("shh"), "the client secret stays here");
    }

    fn b64url(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            let take = chunk.len() + 1;
            for i in 0..take {
                out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }
        out
    }
}
