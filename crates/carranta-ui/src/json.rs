//! Just enough JSON to answer a browser.
//!
//! Written out rather than pulled in, like the statistics in
//! `carranta-analytics`: the workspace is dependency-free, and what a read-only
//! API needs is a writer, not a schema system. There is no parser here at all, //! the one thing the client sends back is a pair of integers, and a full parser
//! for that would be a liability rather than a convenience.

use std::fmt::Write as _;

/// Builds a JSON document.
#[derive(Default)]
pub struct Json {
    out: String,
    /// Whether the value about to be written needs a comma before it.
    fresh: bool,
}

impl Json {
    pub fn object() -> Self {
        Json {
            out: String::from("{"),
            fresh: true,
        }
    }

    fn sep(&mut self) {
        if !self.fresh {
            self.out.push(',');
        }
        self.fresh = false;
    }

    fn key(&mut self, k: &str) {
        self.sep();
        let _ = write!(self.out, "\"{k}\":");
    }

    pub fn num(&mut self, k: &str, v: impl Into<f64>) -> &mut Self {
        self.key(k);
        let v: f64 = v.into();
        // Non-finite values have no JSON spelling; null is the honest answer.
        if v.is_finite() {
            let _ = write!(self.out, "{v}");
        } else {
            self.out.push_str("null");
        }
        self
    }

    pub fn int(&mut self, k: &str, v: i64) -> &mut Self {
        self.key(k);
        let _ = write!(self.out, "{v}");
        self
    }

    pub fn bool(&mut self, k: &str, v: bool) -> &mut Self {
        self.key(k);
        self.out.push_str(if v { "true" } else { "false" });
        self
    }

    pub fn str(&mut self, k: &str, v: &str) -> &mut Self {
        self.key(k);
        escape(&mut self.out, v);
        self
    }

    /// A key whose value is `null` when absent.
    pub fn opt_int(&mut self, k: &str, v: Option<i64>) -> &mut Self {
        match v {
            Some(n) => self.int(k, n),
            None => {
                self.key(k);
                self.out.push_str("null");
                self
            }
        }
    }

    pub fn ints(&mut self, k: &str, v: impl IntoIterator<Item = i64>) -> &mut Self {
        self.key(k);
        self.out.push('[');
        let mut first = true;
        for n in v {
            if !first {
                self.out.push(',');
            }
            first = false;
            let _ = write!(self.out, "{n}");
        }
        self.out.push(']');
        self
    }

    /// An array of strings, each escaped the way `str` escapes one.
    pub fn strs<'a>(&mut self, k: &str, v: impl IntoIterator<Item = &'a str>) -> &mut Self {
        self.key(k);
        self.out.push('[');
        let mut first = true;
        for t in v {
            if !first {
                self.out.push(',');
            }
            first = false;
            escape(&mut self.out, t);
        }
        self.out.push(']');
        self
    }

    /// An array whose elements are written by a closure, one call per element.
    pub fn array<T>(
        &mut self,
        k: &str,
        items: impl IntoIterator<Item = T>,
        mut each: impl FnMut(&mut Json, T),
    ) -> &mut Self {
        self.key(k);
        self.out.push('[');
        let mut first = true;
        for item in items {
            if !first {
                self.out.push(',');
            }
            first = false;
            let mut inner = Json::object();
            each(&mut inner, item);
            self.out.push_str(&inner.finish());
        }
        self.out.push(']');
        self
    }

    /// An array of arrays of numbers.
    pub fn rows(&mut self, k: &str, rows: impl IntoIterator<Item = Vec<i64>>) -> &mut Self {
        self.key(k);
        self.out.push('[');
        let mut first = true;
        for row in rows {
            if !first {
                self.out.push(',');
            }
            first = false;
            self.out.push('[');
            for (i, n) in row.iter().enumerate() {
                if i > 0 {
                    self.out.push(',');
                }
                let _ = write!(self.out, "{n}");
            }
            self.out.push(']');
        }
        self.out.push(']');
        self
    }

    pub fn finish(mut self) -> String {
        self.out.push('}');
        self.out
    }
}

/// Write a JSON string literal, escaping what must be escaped.
fn escape(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Control characters must be escaped; everything else, including
            // non-ASCII, is valid UTF-8 in a JSON string as it stands.
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Read one unsigned integer field from a small JSON object.
///
/// The client sends `{"action":3,"version":12}` and nothing else, so this looks
/// for `"key"` followed by digits rather than parsing a document. Anything it
/// does not understand is `None`, which the caller turns into a rejected
/// request, a malformed body must never be guessed at.
pub fn read_u64(body: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let start = body.find(&needle)? + needle.len();
    let rest = body[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Read a five-number array field, as the trade composer sends it.
///
/// Same posture as [`read_u64`]: it recognises exactly the shape the page
/// sends, `"give":[1,0,2,0,0]`, and refuses anything else rather than
/// salvaging part of it. Quantities above 255 are refused too. The engine
/// counts cards in `u8`, and silently truncating would turn a nonsense offer
/// into a plausible one.
pub fn read_u8_array(body: &str, key: &str, n: usize) -> Option<Vec<u8>> {
    let needle = format!("\"{key}\"");
    let start = body.find(&needle)? + needle.len();
    let rest = body[start..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('[')?;
    let end = rest.find(']')?;
    let mut out = Vec::with_capacity(n);
    for field in rest[..end].split(',') {
        out.push(field.trim().parse::<u8>().ok()?);
    }
    (out.len() == n).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_round_trips_through_shape() {
        let mut j = Json::object();
        j.int("a", 1).bool("b", true).str("c", "hi");
        assert_eq!(j.finish(), r#"{"a":1,"b":true,"c":"hi"}"#);
    }

    #[test]
    fn strings_are_escaped() {
        let mut j = Json::object();
        j.str("s", "a\"b\\c\nd\te");
        assert_eq!(j.finish(), r#"{"s":"a\"b\\c\nd\te"}"#);

        let mut j = Json::object();
        // A control character has no literal spelling in JSON.
        j.str("s", "\u{1}");
        assert_eq!(j.finish(), r#"{"s":"\u0001"}"#);

        // Non-ASCII passes through: the response is UTF-8.
        let mut j = Json::object();
        j.str("s", "brück");
        assert_eq!(j.finish(), "{\"s\":\"brück\"}");
    }

    #[test]
    fn non_finite_numbers_become_null_rather_than_invalid_json() {
        let mut j = Json::object();
        j.num("a", f64::NAN).num("b", f64::INFINITY).num("c", 1.5);
        assert_eq!(j.finish(), r#"{"a":null,"b":null,"c":1.5}"#);
    }

    #[test]
    fn arrays_nest() {
        let mut j = Json::object();
        j.ints("xs", [1, 2, 3]);
        j.rows("rs", [vec![1, 2], vec![3]]);
        j.array("os", [7, 8], |o, n| {
            o.int("n", n);
        });
        assert_eq!(
            j.finish(),
            r#"{"xs":[1,2,3],"rs":[[1,2],[3]],"os":[{"n":7},{"n":8}]}"#
        );
    }

    #[test]
    fn empty_collections_are_still_valid() {
        let mut j = Json::object();
        j.ints("xs", std::iter::empty());
        j.array("os", Vec::<i32>::new(), |o, n| {
            o.int("n", n as i64);
        });
        assert_eq!(j.finish(), r#"{"xs":[],"os":[]}"#);
        assert_eq!(Json::object().finish(), "{}");
    }

    #[test]
    fn optional_numbers_become_null() {
        let mut j = Json::object();
        j.opt_int("a", Some(4)).opt_int("b", None);
        assert_eq!(j.finish(), r#"{"a":4,"b":null}"#);
    }

    #[test]
    fn reading_a_field_accepts_what_a_browser_sends() {
        assert_eq!(read_u64(r#"{"action":3,"version":12}"#, "action"), Some(3));
        assert_eq!(
            read_u64(r#"{"action":3,"version":12}"#, "version"),
            Some(12)
        );
        assert_eq!(read_u64(r#"{ "action" : 42 }"#, "action"), Some(42));
        assert_eq!(read_u64(r#"{"action":0}"#, "action"), Some(0));
    }

    #[test]
    fn an_array_field_is_read_whole_or_not_at_all() {
        let body = r#"{"give":[1,0,2,0,0],"want":[0,3,0,0,0],"version":7}"#;
        assert_eq!(read_u8_array(body, "give", 5), Some(vec![1, 0, 2, 0, 0]));
        assert_eq!(read_u8_array(body, "want", 5), Some(vec![0, 3, 0, 0, 0]));
        assert_eq!(read_u64(body, "version"), Some(7));

        // Wrong length, missing, malformed, or out of range: refused whole.
        assert_eq!(read_u8_array(r#"{"give":[1,2]}"#, "give", 5), None);
        assert_eq!(read_u8_array(r#"{"give":[]}"#, "give", 5), None);
        assert_eq!(read_u8_array("{}", "give", 5), None);
        assert_eq!(read_u8_array(r#"{"give":[1,0,0,0,x]}"#, "give", 5), None);
        assert_eq!(read_u8_array(r#"{"give":[1,0,0,0,-1]}"#, "give", 5), None);
        // 256 cards is not 0 cards: truncation would turn nonsense into a
        // plausible offer.
        assert_eq!(read_u8_array(r#"{"give":[256,0,0,0,0]}"#, "give", 5), None);
        assert_eq!(read_u8_array(r#"{"give":[1,0,0,0,0"#, "give", 5), None);
    }

    #[test]
    fn a_body_it_does_not_understand_is_refused_rather_than_guessed() {
        assert_eq!(read_u64("{}", "action"), None);
        assert_eq!(read_u64(r#"{"action":"x"}"#, "action"), None);
        assert_eq!(read_u64(r#"{"action":-1}"#, "action"), None);
        assert_eq!(read_u64(r#"{"action":}"#, "action"), None);
        assert_eq!(read_u64("", "action"), None);
        // A key that only appears as a prefix of another must not match.
        assert_eq!(read_u64(r#"{"actionable":5}"#, "action"), None);
    }
}
