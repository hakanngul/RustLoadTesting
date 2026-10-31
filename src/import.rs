//! Builds requests from browser "Copy as cURL (bash)" commands, HAR files and
//! OpenAPI/Swagger JSON or YAML documents. Only request data is imported; HAR
//! responses are never read or stored.
use crate::core::{Auth, AuthKind, Body, Field, Request, sensitive};
use serde::Deserialize;

/// Same as the plan's request limit.
pub const IMPORT_LIMIT: usize = 500;
pub const HAR_SIZE_LIMIT: u64 = 50 * 1024 * 1024;

/// Reads a HAR or OpenAPI file chosen in the import dialog, with messages
/// that name the path (an empty path asks for one first).
pub fn read_import_file(path: &str) -> Result<Vec<u8>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Choose a file path first".into());
    }
    let cannot = |e: std::io::Error| format!("Cannot read {path}: {e}");
    let meta = std::fs::metadata(path).map_err(cannot)?;
    if meta.is_dir() {
        return Err(format!("{path} is a folder; choose a file"));
    }
    if meta.len() > HAR_SIZE_LIMIT {
        return Err(format!("{path} exceeds 50 MiB"));
    }
    std::fs::read(path).map_err(cannot)
}

/// Headers that describe the connection or are set by the HTTP client.
/// `accept-encoding` is dropped because responses are not decompressed.
const DROPPED_HEADERS: [&str; 10] = [
    "host",
    "content-length",
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
    "te",
    "trailer",
    "accept-encoding",
];

#[derive(Debug, Default)]
pub struct Imported {
    pub requests: Vec<Request>,
    pub warnings: Vec<String>,
    /// Plan variables the requests refer to (OpenAPI path parameters and
    /// credentials); added to the plan unless a variable of that name exists.
    pub variables: Vec<Field>,
}

/// Request data before it is turned into a [`Request`].
#[derive(Default)]
struct Raw {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: String,
    basic: Option<String>,
    /// Extra query pairs, e.g. from `curl -G -d`.
    query: Vec<(String, String)>,
}

#[derive(Default)]
struct Notes {
    dropped_encoding: bool,
    warnings: Vec<String>,
}
impl Notes {
    fn finish(mut self, requests: Vec<Request>) -> Imported {
        if self.dropped_encoding {
            self.warnings.push(
                "Removed Accept-Encoding: RustLoad reads responses uncompressed so sizes and JSON extraction stay exact"
                    .into(),
            );
        }
        Imported {
            requests,
            warnings: self.warnings,
            variables: vec![],
        }
    }
}

fn field(name: &str, value: &str) -> Field {
    Field {
        name: name.to_owned(),
        value: value.to_owned(),
        secret: sensitive(name),
    }
}

fn build(raw: Raw, id: u64, notes: &mut Notes) -> Result<Request, String> {
    let mut url = url::Url::parse(raw.url.trim()).map_err(|_| "not an absolute URL".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!("{} URLs are not supported", url.scheme()));
    }
    let method = raw.method.to_ascii_uppercase();
    if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE") {
        return Err(format!("{method} is not supported"));
    }
    let mut basic = raw.basic;
    if !url.username().is_empty() {
        // Credentials in the URL become Basic Auth; URLs never carry them.
        basic.get_or_insert_with(|| {
            format!(
                "{}:{}",
                percent_decode(url.username()),
                percent_decode(url.password().unwrap_or_default())
            )
        });
        let _ = url.set_username("");
        let _ = url.set_password(None);
    }
    let mut query: Vec<Field> = url
        .query_pairs()
        .filter(|(k, _)| !k.is_empty())
        .map(|(k, v)| field(&k, &v))
        .collect();
    query.extend(
        raw.query
            .iter()
            .filter(|(k, _)| !k.is_empty())
            .map(|(k, v)| field(k, v)),
    );
    url.set_query(None);
    url.set_fragment(None);

    let mut body = Body {
        text: raw.body,
        ..Body::default()
    };
    let mut headers = Vec::new();
    for (name, value) in raw.headers {
        let lower = name.trim().to_ascii_lowercase();
        if lower.is_empty() || lower.starts_with(':') {
            continue;
        }
        if DROPPED_HEADERS.contains(&lower.as_str()) {
            notes.dropped_encoding |= lower == "accept-encoding";
            continue;
        }
        if reqwest::header::HeaderName::from_bytes(lower.as_bytes()).is_err() {
            notes
                .warnings
                .push(format!("Skipped invalid header name {name:?}"));
            continue;
        }
        // The engine sends Content-Type itself for JSON bodies.
        if lower == "content-type"
            && value.to_ascii_lowercase().contains("json")
            && !body.text.is_empty()
            && serde_json::from_str::<serde_json::Value>(&body.text).is_ok()
        {
            body.json = true;
            continue;
        }
        headers.push(field(name.trim(), &value));
    }

    let auth = match basic {
        None => Auth::default(),
        Some(user) => {
            let (username, password) = user.split_once(':').unwrap_or((&user, ""));
            if password.is_empty() {
                notes
                    .warnings
                    .push("Basic Auth had no password; enter it before running".into());
            }
            Auth {
                kind: AuthKind::Basic,
                username: username.to_owned(),
                password: password.to_owned(),
                ..Auth::default()
            }
        }
    };
    let path = url.path().to_owned();
    let label = if path == "/" {
        url.host_str().unwrap_or_default().to_owned()
    } else {
        path
    };
    Ok(Request {
        id,
        // The method is shown next to the name everywhere, so the name is the path.
        name: label.chars().take(60).collect(),
        method,
        url: url.to_string(),
        query,
        headers,
        body,
        auth,
        ..Request::default()
    })
}

fn percent_decode(s: &str) -> String {
    url::form_urlencoded::parse(format!("x={}", s.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}

fn push(raw: Raw, requests: &mut Vec<Request>, next_id: &mut u64, notes: &mut Notes) -> bool {
    if requests.len() >= IMPORT_LIMIT {
        notes.warnings.push(format!(
            "Stopped at {IMPORT_LIMIT} requests, the plan limit"
        ));
        return false;
    }
    let what = format!("{} {}", raw.method, raw.url);
    match build(raw, *next_id, notes) {
        Ok(r) => {
            requests.push(r);
            *next_id += 1;
        }
        Err(e) => notes.warnings.push(format!("Skipped {what}: {e}")),
    }
    true
}

// ---------------------------------------------------------------- curl

enum Token {
    Word(String),
    /// Newline, `;`, `&`, `&&` or `|` outside quotes.
    Separator,
}

/// Splits text the way bash does for the quoting forms browsers emit:
/// '…', "…", $'…', backslash escapes and line continuations.
fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = input.chars().peekable();
    let end = |word: &mut String, in_word: &mut bool, tokens: &mut Vec<Token>| {
        if *in_word {
            tokens.push(Token::Word(std::mem::take(word)));
            *in_word = false;
        }
    };
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\r' => end(&mut word, &mut in_word, &mut tokens),
            '\n' | ';' | '&' | '|' => {
                end(&mut word, &mut in_word, &mut tokens);
                tokens.push(Token::Separator);
            }
            '\\' => match chars.next() {
                Some('\n') => {}
                Some('\r') if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                Some(next) => {
                    word.push(next);
                    in_word = true;
                }
                None => {}
            },
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => word.push(ch),
                        None => return Err("Unterminated ' quote".into()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('\n') => {}
                            Some(ch @ ('$' | '`' | '"' | '\\')) => word.push(ch),
                            Some(ch) => {
                                word.push('\\');
                                word.push(ch);
                            }
                            None => return Err("Unterminated \" quote".into()),
                        },
                        Some(ch) => word.push(ch),
                        None => return Err("Unterminated \" quote".into()),
                    }
                }
            }
            '$' if chars.peek() == Some(&'\'') => {
                chars.next();
                in_word = true;
                ansi_c_quoted(&mut chars, &mut word)?;
            }
            _ => {
                word.push(c);
                in_word = true;
            }
        }
    }
    end(&mut word, &mut in_word, &mut tokens);
    Ok(tokens)
}

/// Body of a bash `$'…'` string, which Chrome uses for values with special characters.
fn ansi_c_quoted(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    word: &mut String,
) -> Result<(), String> {
    let hex = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>, max: usize| {
        let mut digits = String::new();
        while digits.len() < max && chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            digits.push(chars.next().unwrap_or_default());
        }
        u32::from_str_radix(&digits, 16)
            .ok()
            .and_then(char::from_u32)
    };
    loop {
        match chars.next() {
            Some('\'') => return Ok(()),
            Some('\\') => match chars.next() {
                Some('n') => word.push('\n'),
                Some('t') => word.push('\t'),
                Some('r') => word.push('\r'),
                Some('x') => word.extend(hex(chars, 2)),
                Some('u') => word.extend(hex(chars, 4)),
                Some('U') => word.extend(hex(chars, 8)),
                Some(c @ ('\\' | '\'' | '"' | '?')) => word.push(c),
                Some(c) => {
                    word.push('\\');
                    word.push(c);
                }
                None => return Err("Unterminated $' quote".into()),
            },
            Some(c) => word.push(c),
            None => return Err("Unterminated $' quote".into()),
        }
    }
}

fn is_curl(word: &str) -> bool {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    name.eq_ignore_ascii_case("curl") || name.eq_ignore_ascii_case("curl.exe")
}

/// Long options that take a value RustLoad does not use.
const IGNORED_WITH_VALUE: [&str; 20] = [
    "output",
    "max-time",
    "connect-timeout",
    "retry",
    "retry-delay",
    "retry-max-time",
    "cert",
    "key",
    "cacert",
    "capath",
    "proxy",
    "proxy-user",
    "write-out",
    "limit-rate",
    "resolve",
    "connect-to",
    "interface",
    "max-redirs",
    "dump-header",
    "config",
];
const IGNORED_FLAGS: [&str; 12] = [
    "silent",
    "show-error",
    "verbose",
    "include",
    "fail",
    "compressed",
    "http1.1",
    "http2",
    "http2-prior-knowledge",
    "globoff",
    "no-buffer",
    "no-progress-meter",
];

/// Parses one or more curl commands, e.g. from "Copy as cURL (bash)" or
/// "Copy all as cURL". Unsupported options are reported, never executed.
pub fn from_curl(text: &str, first_id: u64) -> Result<Imported, String> {
    let mut commands: Vec<Vec<String>> = vec![vec![]];
    for token in tokenize(text)? {
        match token {
            Token::Word(w) => commands.last_mut().expect("command").push(w),
            Token::Separator => commands.push(vec![]),
        }
    }
    let mut notes = Notes::default();
    let mut requests = Vec::new();
    let mut next_id = first_id;
    let mut found = false;
    for words in commands.into_iter().filter(|c| !c.is_empty()) {
        if !is_curl(&words[0]) {
            notes.warnings.push(format!(
                "Ignored text that is not a curl command: {}",
                words[0].chars().take(40).collect::<String>()
            ));
            continue;
        }
        found = true;
        let raw = parse_curl(&words[1..], &mut notes);
        if !push(raw, &mut requests, &mut next_id, &mut notes) {
            break;
        }
    }
    if !found {
        return Err("No curl command found. Paste the output of \"Copy as cURL (bash)\".".into());
    }
    Ok(notes.finish(requests))
}

fn warn_once(notes: &mut Notes, text: String) {
    if !notes.warnings.contains(&text) {
        notes.warnings.push(text);
    }
}

fn parse_curl(args: &[String], notes: &mut Notes) -> Raw {
    let mut raw = Raw::default();
    let mut method = None;
    let mut data: Vec<String> = Vec::new();
    let mut get = false;
    let mut json = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        // Normalize `--name=value`, `--name value`, `-Xvalue` and `-X value`.
        let (option, inline): (String, Option<String>) = if let Some(long) = arg.strip_prefix("--")
        {
            match long.split_once('=') {
                Some((n, v)) => (n.to_owned(), Some(v.to_owned())),
                None => (long.to_owned(), None),
            }
        } else if arg.len() > 1 && arg.starts_with('-') {
            let mut short = arg[1..].chars();
            let first = short.next().unwrap_or_default();
            let rest: String = short.collect();
            if "XHdubAeoFmxwTE".contains(first) {
                let long = match first {
                    'X' => "request",
                    'H' => "header",
                    'd' => "data",
                    'u' => "user",
                    'b' => "cookie",
                    'A' => "user-agent",
                    'e' => "referer",
                    'o' => "output",
                    'F' => "form",
                    'm' => "max-time",
                    'x' => "proxy",
                    'w' => "write-out",
                    'T' => "upload-file",
                    _ => "cert",
                };
                (long.to_owned(), (!rest.is_empty()).then_some(rest))
            } else {
                // Bundled flags without values, e.g. -sSL.
                for c in arg[1..].chars() {
                    match c {
                        'G' => get = true,
                        'I' => method = Some("HEAD".to_owned()),
                        'L' => warn_once(notes, "Ignored -L: redirects are never followed".into()),
                        'k' => warn_once(
                            notes,
                            "Ignored -k/--insecure: TLS certificates are always verified".into(),
                        ),
                        's' | 'S' | 'v' | 'i' | 'f' | 'g' | 'N' | '4' | '6' => {}
                        other => warn_once(notes, format!("Ignored option -{other}")),
                    }
                }
                continue;
            }
        } else {
            if raw.url.is_empty() {
                raw.url = arg.clone();
            } else {
                warn_once(notes, format!("Ignored extra URL {arg}"));
            }
            continue;
        };
        let takes_value = matches!(
            option.as_str(),
            "request"
                | "header"
                | "data"
                | "data-raw"
                | "data-binary"
                | "data-ascii"
                | "data-urlencode"
                | "json"
                | "user"
                | "url"
                | "cookie"
                | "user-agent"
                | "referer"
                | "form"
                | "upload-file"
        ) || IGNORED_WITH_VALUE.contains(&option.as_str());
        let value = if takes_value {
            inline.or_else(|| args.next().cloned()).unwrap_or_default()
        } else {
            String::new()
        };
        match option.as_str() {
            "request" => method = Some(value),
            "header" => match value.split_once(':') {
                Some((n, v)) => raw.headers.push((n.trim().into(), v.trim().into())),
                None => warn_once(notes, format!("Ignored header without a value: {value}")),
            },
            "data" | "data-ascii" | "data-raw" | "data-binary" => {
                if value.starts_with('@') && option != "data-raw" {
                    warn_once(notes, "Skipped @file data: files are not read".into());
                } else if option == "data" || option == "data-ascii" {
                    data.push(value.replace(['\r', '\n'], ""));
                } else {
                    data.push(value);
                }
            }
            "data-urlencode" => {
                let encode = |s: &str| {
                    url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>()
                };
                if value.starts_with('@') || (value.contains('@') && !value.contains('=')) {
                    warn_once(notes, "Skipped @file data: files are not read".into());
                } else {
                    data.push(match value.split_once('=') {
                        Some((n, v)) if !n.is_empty() => format!("{n}={}", encode(v)),
                        Some((_, v)) => encode(v),
                        None => encode(&value),
                    });
                }
            }
            "json" => {
                data.push(value);
                json = true;
            }
            "user" => raw.basic = Some(value),
            "url" => raw.url = value,
            "cookie" => {
                if value.contains('=') {
                    raw.headers.push(("Cookie".into(), value));
                } else {
                    warn_once(notes, "Skipped cookie file: files are not read".into());
                }
            }
            "user-agent" => raw.headers.push(("User-Agent".into(), value)),
            "referer" => raw.headers.push(("Referer".into(), value)),
            "get" => get = true,
            "head" => method = Some("HEAD".to_owned()),
            "location" => warn_once(notes, "Ignored -L: redirects are never followed".into()),
            "insecure" => warn_once(
                notes,
                "Ignored -k/--insecure: TLS certificates are always verified".into(),
            ),
            "form" | "upload-file" => warn_once(
                notes,
                format!("Skipped --{option}: multipart and file uploads are not supported"),
            ),
            other if IGNORED_FLAGS.contains(&other) || IGNORED_WITH_VALUE.contains(&other) => {}
            other => warn_once(notes, format!("Ignored option --{other}")),
        }
    }
    if json {
        let has = |headers: &[(String, String)], name: &str| {
            headers.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
        };
        if !has(&raw.headers, "content-type") {
            raw.headers
                .push(("Content-Type".into(), "application/json".into()));
        }
        if !has(&raw.headers, "accept") {
            raw.headers
                .push(("Accept".into(), "application/json".into()));
        }
    }
    let body = data.join("&");
    if get {
        raw.query = url::form_urlencoded::parse(body.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
    } else {
        raw.body = body;
    }
    raw.method =
        method.unwrap_or_else(|| if raw.body.is_empty() { "GET" } else { "POST" }.to_owned());
    raw
}

// ---------------------------------------------------------------- HAR

#[derive(Deserialize)]
struct Har {
    log: HarLog,
}
#[derive(Deserialize)]
struct HarLog {
    #[serde(default)]
    entries: Vec<HarEntry>,
}
#[derive(Deserialize)]
struct HarEntry {
    request: HarRequest,
    #[serde(rename = "_resourceType", default)]
    resource_type: Option<String>,
}
#[derive(Deserialize)]
struct HarRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<HarPair>,
    #[serde(rename = "postData", default)]
    post_data: Option<HarPost>,
}
#[derive(Deserialize)]
struct HarPost {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    params: Vec<HarPair>,
}
#[derive(Deserialize)]
struct HarPair {
    name: String,
    #[serde(default)]
    value: String,
}

/// Imports the requests of a HAR 1.2 file in recorded order. With
/// `api_only`, entries the browser marked as documents, scripts, styles,
/// images, fonts or media are skipped.
pub fn from_har(bytes: &[u8], first_id: u64, api_only: bool) -> Result<Imported, String> {
    let har: Har = serde_json::from_slice(bytes).map_err(|e| format!("Not a HAR file: {e}"))?;
    let mut notes = Notes::default();
    let mut requests = Vec::new();
    let mut next_id = first_id;
    let (mut not_api, mut not_http) = (0, 0);
    for entry in har.log.entries {
        if api_only
            && entry
                .resource_type
                .as_deref()
                .is_some_and(|t| !matches!(t, "xhr" | "fetch"))
        {
            not_api += 1;
            continue;
        }
        if !entry.request.url.starts_with("http://") && !entry.request.url.starts_with("https://") {
            not_http += 1;
            continue;
        }
        let r = entry.request;
        let body = match r.post_data {
            None => String::new(),
            Some(post) => match post.text {
                Some(text) if !text.is_empty() => text,
                _ => url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(post.params.iter().map(|p| (&p.name, &p.value)))
                    .finish(),
            },
        };
        let raw = Raw {
            method: r.method,
            url: r.url,
            headers: r.headers.into_iter().map(|h| (h.name, h.value)).collect(),
            body,
            ..Raw::default()
        };
        if !push(raw, &mut requests, &mut next_id, &mut notes) {
            break;
        }
    }
    if not_api > 0 {
        notes.warnings.push(format!(
            "Skipped {not_api} non-API entries (documents, scripts, styles, images, fonts)"
        ));
    }
    if not_http > 0 {
        notes
            .warnings
            .push(format!("Skipped {not_http} entries that are not HTTP(S)"));
    }
    Ok(notes.finish(requests))
}

// ---------------------------------------------------------------- OpenAPI

mod refs;
mod yaml;
pub use refs::{REF_FILE_LIMIT, REF_FILES_MAX};
pub use yaml::{COPY_LIMIT as YAML_COPY_LIMIT, MAX_DEPTH as YAML_MAX_DEPTH};

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const OPENAPI_METHODS: [&str; 5] = ["get", "post", "put", "patch", "delete"];
/// Nesting limit for generated values; deeper schemas are left out.
const SAMPLE_DEPTH: usize = 8;

/// Follows local `$ref`s (`#/components/...`, `#/definitions/...`).
fn resolve<'a>(doc: &'a Value, mut value: &'a Value) -> &'a Value {
    for _ in 0..16 {
        match value
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| r.strip_prefix('#'))
            .and_then(|pointer| doc.pointer(pointer))
        {
            Some(target) => value = target,
            None => break,
        }
    }
    value
}

/// Builds example values for parameters, media types and schemas.
///
/// Document values come first: `example`, the first of `examples`, `default`,
/// the first `enum` value. With `types` set, values missing from the document
/// become type placeholders (see [`Sampler::placeholder`]) and `placeholders`
/// records that one was used. `allOf` members are merged; `oneOf`/`anyOf` use
/// the first option. Recursive `$ref`s stop at the first repetition.
struct Sampler<'a> {
    doc: &'a Value,
    types: bool,
    placeholders: bool,
    /// `$ref`s on the current path, to stop recursive schemas.
    refs: Vec<&'a str>,
}
impl<'a> Sampler<'a> {
    fn new(doc: &'a Value, types: bool) -> Self {
        Self {
            doc,
            types,
            placeholders: false,
            refs: Vec::new(),
        }
    }

    fn sample(&mut self, value: &'a Value, depth: usize) -> Option<Value> {
        if depth > SAMPLE_DEPTH {
            return None;
        }
        // Resolve references while remembering them for cycle detection.
        let pushed = self.refs.len();
        let mut value = value;
        for _ in 0..16 {
            let Some(reference) = value.get("$ref").and_then(Value::as_str) else {
                break;
            };
            if self.refs.contains(&reference) {
                self.refs.truncate(pushed);
                return None;
            }
            let Some(target) = reference
                .strip_prefix('#')
                .and_then(|pointer| self.doc.pointer(pointer))
            else {
                break;
            };
            self.refs.push(reference);
            value = target;
        }
        let result = self.sample_resolved(value, depth);
        self.refs.truncate(pushed);
        result
    }

    fn sample_resolved(&mut self, value: &'a Value, depth: usize) -> Option<Value> {
        if let Some(found) = document_value(self.doc, value) {
            return Some(found);
        }
        if let Some(schema) = value.get("schema") {
            return self.sample(schema, depth + 1);
        }
        if let Some(members) = value.get("allOf").and_then(Value::as_array) {
            return self.all_of(value, members, depth);
        }
        if let Some(first) = ["oneOf", "anyOf"]
            .iter()
            .find_map(|k| value.get(*k).and_then(|v| v.get(0)))
        {
            return self.sample(first, depth + 1);
        }
        let kind = schema_type(value);
        if let Some(properties) = value.get("properties").and_then(Value::as_object) {
            let required = required_names(value);
            return self.object(properties.iter(), &required, depth);
        }
        if let Some(items) = value.get("items") {
            return match self.sample(items, depth + 1) {
                Some(item) => Some(Value::Array(vec![item])),
                None if self.types => {
                    self.placeholders = true;
                    Some(Value::Array(vec![]))
                }
                None => None,
            };
        }
        self.types.then(|| self.placeholder(value, kind)).flatten()
    }

    /// An object from `properties`. Document values are always used; with
    /// `types`, the required properties (all of them when none is required)
    /// get placeholders too.
    fn object(
        &mut self,
        properties: impl Iterator<Item = (&'a String, &'a Value)>,
        required: &BTreeSet<&str>,
        depth: usize,
    ) -> Option<Value> {
        let mut object = serde_json::Map::new();
        for (name, schema) in properties {
            let wanted = required.is_empty() || required.contains(name.as_str());
            let types = self.types;
            self.types = types && wanted;
            let value = self.sample(schema, depth + 1);
            self.types = types;
            if let Some(value) = value {
                object.insert(name.clone(), value);
            }
        }
        if object.is_empty() && !self.types {
            return None;
        }
        Some(Value::Object(object))
    }

    /// Merges `allOf` members (and the schema's own properties): properties
    /// and `required` are combined, member object examples override them.
    fn all_of(&mut self, own: &'a Value, members: &'a [Value], depth: usize) -> Option<Value> {
        let mut properties: Vec<(&'a String, &'a Value)> = Vec::new();
        let mut required = required_names(own);
        let mut examples = serde_json::Map::new();
        let mut scalar = None;
        let mut queue: Vec<(&'a Value, usize)> = members.iter().rev().map(|m| (m, 0)).collect();
        if let Some(own) = own.get("properties").and_then(Value::as_object) {
            properties.extend(own.iter());
        }
        let mut seen: Vec<&'a str> = Vec::new();
        while let Some((member, level)) = queue.pop() {
            if level > SAMPLE_DEPTH {
                continue;
            }
            let reference = member.get("$ref").and_then(Value::as_str);
            if let Some(r) = reference {
                if seen.contains(&r) || self.refs.contains(&r) {
                    continue;
                }
                seen.push(r);
            }
            let member = resolve(self.doc, member);
            match document_value(self.doc, member) {
                Some(Value::Object(map)) => examples.extend(map),
                Some(other) => {
                    scalar.get_or_insert(other);
                }
                None => {}
            }
            if let Some(map) = member.get("properties").and_then(Value::as_object) {
                for (name, schema) in map {
                    properties.retain(|(n, _)| *n != name);
                    properties.push((name, schema));
                }
            }
            required.extend(required_names(member));
            if let Some(nested) = member.get("allOf").and_then(Value::as_array) {
                queue.extend(nested.iter().rev().map(|m| (m, level + 1)));
            }
        }
        if properties.is_empty() && examples.is_empty() {
            // allOf without object members, e.g. a single referenced scalar schema.
            return scalar.or_else(|| members.first().and_then(|m| self.sample(m, depth + 1)));
        }
        let mut object = match self.object(properties.into_iter(), &required, depth) {
            Some(Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        };
        object.extend(examples);
        (!object.is_empty() || self.types).then_some(Value::Object(object))
    }

    /// A type placeholder: "" (or a fixed value for common string formats),
    /// 0 (or `minimum`), false, {} or null.
    fn placeholder(&mut self, schema: &Value, kind: Option<&str>) -> Option<Value> {
        let value = match kind? {
            "string" => Value::String(
                match schema.get("format").and_then(Value::as_str) {
                    Some("date-time") => "1970-01-01T00:00:00Z",
                    Some("date") => "1970-01-01",
                    Some("uuid") => "00000000-0000-0000-0000-000000000000",
                    Some("email") => "user@example.com",
                    _ => "",
                }
                .into(),
            ),
            "integer" | "number" => match schema.get("minimum") {
                Some(min @ Value::Number(_)) => min.clone(),
                _ => Value::from(0),
            },
            "boolean" => Value::Bool(false),
            "object" => Value::Object(serde_json::Map::new()),
            "array" => Value::Array(vec![]),
            "null" => Value::Null,
            _ => return None,
        };
        self.placeholders = true;
        Some(value)
    }
}

/// `example`, the first of `examples`, `default` or the first `enum` value.
fn document_value(doc: &Value, value: &Value) -> Option<Value> {
    if let Some(example) = value.get("example") {
        return Some(example.clone());
    }
    if let Some(first) = value.get("examples").and_then(|e| match e {
        Value::Object(map) => map.values().next(),
        Value::Array(list) => list.first(),
        _ => None,
    }) {
        let first = resolve(doc, first);
        return Some(first.get("value").unwrap_or(first).clone());
    }
    if let Some(default) = value.get("default") {
        return Some(default.clone());
    }
    value.get("enum").and_then(|e| e.get(0)).cloned()
}

/// The schema's `type`; for a 3.1 type list the first that is not "null".
fn schema_type(schema: &Value) -> Option<&str> {
    match schema.get("type") {
        Some(Value::String(kind)) => Some(kind),
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|k| *k != "null")
            .or(Some("null")),
        _ => None,
    }
}

fn required_names(schema: &Value) -> BTreeSet<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A variable name usable in `{{…}}` from an OpenAPI parameter name.
fn variable_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// One entry of `servers` (OpenAPI 3). A Swagger 2.0 document has one server
/// `{scheme}://host/basePath` whose `scheme` variable lists its `schemes`.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenApiServer {
    /// URL template with `{variable}` placeholders.
    pub url: String,
    pub description: String,
    pub variables: Vec<ServerVariable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ServerVariable {
    pub name: String,
    pub default: String,
    /// The `enum` values (the default first when it is missing from them);
    /// empty when the variable has no `enum`.
    pub options: Vec<String>,
}

/// The chosen server and a value per variable of that server.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ServerChoice {
    pub server: usize,
    pub values: Vec<String>,
}
impl ServerChoice {
    /// Server `index` with its variables at their defaults.
    pub fn defaults(spec: &OpenApiSpec, index: usize) -> Self {
        Self {
            server: index,
            values: spec
                .servers
                .get(index)
                .map(|s| s.variables.iter().map(|v| v.default.clone()).collect())
                .unwrap_or_default(),
        }
    }
}

/// The servers a document lists (see [`OpenApiServer`]).
fn servers(doc: &Value) -> Vec<OpenApiServer> {
    if doc.get("swagger").is_some() {
        let Some(host) = doc.get("host").and_then(Value::as_str) else {
            return Vec::new();
        };
        let base_path = doc.get("basePath").and_then(Value::as_str).unwrap_or("");
        let mut schemes: Vec<String> = doc
            .get("schemes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if schemes.is_empty() {
            schemes.push("https".into());
        }
        return vec![OpenApiServer {
            url: format!("{{scheme}}://{host}{base_path}"),
            description: String::new(),
            variables: vec![ServerVariable {
                name: "scheme".into(),
                default: schemes[0].clone(),
                options: if schemes.len() > 1 {
                    schemes
                } else {
                    Vec::new()
                },
            }],
        }];
    }
    doc.get("servers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|server| {
            let url = server.get("url").and_then(Value::as_str)?.to_owned();
            let variables = server
                .get("variables")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .map(|(name, var)| {
                    let default = var.get("default").map(text).unwrap_or_default();
                    let mut options: Vec<String> = var
                        .get("enum")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(text)
                        .collect();
                    if !options.is_empty() && !options.contains(&default) {
                        options.insert(0, default.clone());
                    }
                    ServerVariable {
                        name: name.clone(),
                        default,
                        options,
                    }
                })
                .collect();
            Some(OpenApiServer {
                url,
                description: server
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                variables,
            })
        })
        .collect()
}

/// Checks a base URL: absolute http(s) with a host, no credentials, query or
/// unresolved variables. Returns it without a trailing slash.
fn checked_base(raw: &str) -> Result<String, String> {
    let url = url::Url::parse(raw)
        .map_err(|_| format!("Server URL {raw:?} is not absolute; enter a base URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("The base URL must be an http(s) URL with a host".into());
    }
    if !url.username().is_empty() || url.query().is_some() || raw.contains('{') {
        return Err("The base URL cannot contain credentials, a query or variables".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// One importable operation of an OpenAPI document.
#[derive(Debug, Clone)]
pub struct OpenApiOperation {
    /// Stable identity, "METHOD path" (e.g. "GET /pets/{petId}").
    pub key: String,
    pub method: String,
    pub path: String,
    /// The request name: operationId, else summary, else the path.
    pub name: String,
    pub operation_id: String,
    pub tags: Vec<String>,
    /// Request with ID 0 and the URL path only; [`OpenApiSpec::build`]
    /// assigns IDs and prepends the base URL.
    request: Request,
    variables: Vec<Field>,
    missing_values: Vec<String>,
    other_auth: Vec<String>,
    cookie_keys: Vec<String>,
    key_variables: Vec<String>,
    cookies: usize,
    no_example: bool,
    not_json: bool,
    typed_body: bool,
}
impl OpenApiOperation {
    /// Case-insensitive match against the path, name and operationId.
    pub fn matches(&self, filter: &str) -> bool {
        let filter = filter.trim().to_lowercase();
        filter.is_empty()
            || [&self.path, &self.name, &self.operation_id]
                .iter()
                .any(|s| s.to_lowercase().contains(&filter))
    }
}

/// A parsed OpenAPI document: the operations to choose from. Load once with
/// [`load_openapi`] (or [`parse_openapi`]), choose a server with
/// [`OpenApiSpec::select_server`], then [`OpenApiSpec::build`] the selected
/// operations; neither re-reads the document.
#[derive(Debug, Clone, Default)]
pub struct OpenApiSpec {
    /// Operations in document order: paths as written, methods as written
    /// within a path.
    pub operations: Vec<OpenApiOperation>,
    pub servers: Vec<OpenApiServer>,
    /// Warnings about the document itself (unresolved `$ref`s), also part of
    /// every [`OpenApiSpec::build`] result.
    pub warnings: Vec<String>,
    /// Referenced files read besides the document.
    pub referenced_files: usize,
    swagger: bool,
    other_methods: usize,
    /// Current base URL without a trailing slash; empty until a server is
    /// selected successfully.
    base: String,
}
impl OpenApiSpec {
    /// The base URL for `choice`, or `override_url` when it is not blank.
    pub fn server_url(&self, choice: &ServerChoice, override_url: &str) -> Result<String, String> {
        if !override_url.trim().is_empty() {
            return checked_base(override_url.trim());
        }
        let Some(server) = self.servers.get(choice.server) else {
            return Err(if self.swagger {
                "The document has no host; enter a base URL".into()
            } else {
                "The document has no servers; enter a base URL".into()
            });
        };
        let mut url = server.url.clone();
        for (i, var) in server.variables.iter().enumerate() {
            let value = choice.values.get(i).unwrap_or(&var.default);
            url = url.replace(&format!("{{{}}}", var.name), value);
        }
        checked_base(&url)
    }

    /// Uses [`OpenApiSpec::server_url`] for the requests [`OpenApiSpec::build`]
    /// makes. On error the previous base URL is cleared.
    pub fn select_server(
        &mut self,
        choice: &ServerChoice,
        override_url: &str,
    ) -> Result<String, String> {
        let result = self.server_url(choice, override_url);
        self.base = result.clone().unwrap_or_default();
        result
    }

    /// The base URL requests are built with; empty when none is selected.
    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// All tags in name order.
    pub fn tags(&self) -> Vec<String> {
        let tags: BTreeSet<&String> = self.operations.iter().flat_map(|o| &o.tags).collect();
        tags.into_iter().cloned().collect()
    }

    /// Requests for the operations whose keys are selected, in document order,
    /// with IDs from `first_id`. At most [`IMPORT_LIMIT`] are built; warnings
    /// and variables cover only the built operations.
    pub fn build(&self, selected: &BTreeSet<String>, first_id: u64) -> Imported {
        let mut notes = Notes {
            warnings: self.warnings.clone(),
            ..Notes::default()
        };
        let mut requests = Vec::new();
        let mut variables: Vec<Field> = Vec::new();
        let (mut missing, mut other_auth, mut cookie_keys) =
            (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        let (mut cookies, mut no_example, mut not_json, mut typed) = (0, 0, 0, 0);
        let mut built = Vec::new();
        for op in self.operations.iter().filter(|o| selected.contains(&o.key)) {
            if requests.len() >= IMPORT_LIMIT {
                notes.warnings.push(format!(
                    "Stopped at {IMPORT_LIMIT} requests, the plan limit"
                ));
                break;
            }
            built.push(op);
            let mut request = op.request.clone();
            request.id = first_id + requests.len() as u64;
            request.url = format!("{}{}", self.base, op.request.url);
            requests.push(request);
            for v in &op.variables {
                if !variables.iter().any(|f| f.name == v.name) {
                    variables.push(v.clone());
                }
            }
            missing.extend(op.missing_values.iter().cloned());
            other_auth.extend(op.other_auth.iter().cloned());
            cookie_keys.extend(op.cookie_keys.iter().cloned());
            cookies += op.cookies;
            no_example += usize::from(op.no_example);
            not_json += usize::from(op.not_json);
            typed += usize::from(op.typed_body);
        }
        let join = |set: BTreeSet<String>| set.into_iter().collect::<Vec<_>>().join(", ");
        if !missing.is_empty() {
            notes.warnings.push(format!(
                "Set values for path variables without an example: {}",
                join(missing)
            ));
        }
        if requests
            .iter()
            .any(|r: &Request| r.auth.kind == AuthKind::Bearer)
        {
            notes
                .warnings
                .push("Bearer requests use {{access_token}}; enter it as a secret variable".into());
        }
        let key_vars: BTreeSet<&String> = built.iter().flat_map(|o| &o.key_variables).collect();
        if !key_vars.is_empty() {
            let refs: Vec<String> = key_vars.iter().map(|v| format!("{{{{{v}}}}}")).collect();
            notes.warnings.push(format!(
                "API key requests use {}; enter the key as a secret variable",
                refs.join(", ")
            ));
        }
        if requests.iter().any(|r| r.auth.kind == AuthKind::Basic) {
            notes
                .warnings
                .push("Basic Auth requests need a username and password before running".into());
        }
        if !cookie_keys.is_empty() {
            notes.warnings.push(format!(
                "API keys sent as cookies are not supported (no cookie support): {}",
                join(cookie_keys)
            ));
        }
        if !other_auth.is_empty() {
            notes.warnings.push(format!(
                "Add credentials for these security schemes manually: {}",
                join(other_auth)
            ));
        }
        for (count, what) in [
            (
                self.other_methods,
                "HEAD/OPTIONS/TRACE operations skipped (unsupported methods)",
            ),
            (cookies, "cookie parameters skipped (no cookie support)"),
            (typed, "bodies built from schema types; review the values"),
            (
                no_example,
                "JSON bodies left empty (no example or schema in the document)",
            ),
            (not_json, "non-JSON request bodies left empty"),
        ] {
            if count > 0 {
                notes.warnings.push(format!("{count} {what}"));
            }
        }
        let mut imported = notes.finish(requests);
        imported.variables = variables;
        imported
    }
}

/// Imports every operation of an OpenAPI 3.x or Swagger 2.0 JSON or YAML document.
/// See [`parse_openapi`] and [`OpenApiSpec::build`].
pub fn from_openapi(bytes: &[u8], first_id: u64, base_override: &str) -> Result<Imported, String> {
    let spec = parse_openapi(bytes, base_override)?;
    let all = spec.operations.iter().map(|o| o.key.clone()).collect();
    Ok(spec.build(&all, first_id))
}

/// A security scheme definition by name (OpenAPI 3 or Swagger 2.0).
fn security_scheme<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    let key = name.replace('~', "~0").replace('/', "~1");
    doc.pointer(&format!("/components/securitySchemes/{key}"))
        .or_else(|| doc.pointer(&format!("/securityDefinitions/{key}")))
        .map(|s| resolve(doc, s))
}

enum Scheme<'a> {
    Bearer,
    Basic,
    /// Header or query API key: location, parameter name.
    ApiKey(&'a str, &'a str),
    CookieKey,
    Other,
}
fn classify(scheme: Option<&Value>) -> Scheme<'_> {
    let Some(scheme) = scheme else {
        return Scheme::Other;
    };
    let get = |k: &str| scheme.get(k).and_then(Value::as_str);
    let http = get("scheme").unwrap_or_default();
    match get("type") {
        Some("http") if http.eq_ignore_ascii_case("bearer") => Scheme::Bearer,
        Some("oauth2" | "openIdConnect") => Scheme::Bearer,
        Some("basic") => Scheme::Basic,
        Some("http") if http.eq_ignore_ascii_case("basic") => Scheme::Basic,
        Some("apiKey") => match (get("in"), get("name")) {
            (Some(at @ ("header" | "query")), Some(name)) if !name.trim().is_empty() => {
                Scheme::ApiKey(at, name)
            }
            (Some("cookie"), _) => Scheme::CookieKey,
            _ => Scheme::Other,
        },
        _ => Scheme::Other,
    }
}

/// Variable names for header/query API key schemes in the order the document
/// defines them: `api_key`, `api_key_2`, …
fn api_key_variables(doc: &Value) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    for definitions in [
        doc.pointer("/components/securitySchemes"),
        doc.get("securityDefinitions"),
    ]
    .into_iter()
    .flatten()
    .filter_map(Value::as_object)
    {
        for (name, scheme) in definitions {
            if names.contains_key(name) {
                continue;
            }
            if let Scheme::ApiKey(..) = classify(Some(resolve(doc, scheme))) {
                let var = match names.len() {
                    0 => "api_key".to_owned(),
                    n => format!("api_key_{}", n + 1),
                };
                names.insert(name.clone(), var);
            }
        }
    }
    names
}

/// Parses an OpenAPI 3.x or Swagger 2.0 JSON or YAML document into its
/// GET/POST/PUT/PATCH/DELETE operations with the first server (variables at
/// their defaults) or `base_override`. External `$ref`s are not followed; see
/// [`parse_openapi_in`].
pub fn parse_openapi(bytes: &[u8], base_override: &str) -> Result<OpenApiSpec, String> {
    with_default_server(load_openapi(bytes, None)?, base_override)
}

/// [`parse_openapi`] for a document in `base_dir`: `$ref`s to local files in
/// that folder tree are followed (see [`load_openapi`]).
pub fn parse_openapi_in(
    bytes: &[u8],
    base_dir: &Path,
    base_override: &str,
) -> Result<OpenApiSpec, String> {
    with_default_server(load_openapi(bytes, Some(base_dir))?, base_override)
}

fn with_default_server(mut spec: OpenApiSpec, base_override: &str) -> Result<OpenApiSpec, String> {
    let choice = ServerChoice::defaults(&spec, 0);
    spec.select_server(&choice, base_override)?;
    Ok(spec)
}

/// Parses an OpenAPI 3.x or Swagger 2.0 JSON or YAML document into its
/// GET/POST/PUT/PATCH/DELETE operations and servers. Path parameters become
/// `{{variables}}`; required query and header parameters use the document's
/// examples; JSON bodies use examples and otherwise type placeholders.
///
/// With `base_dir` (the folder of the document's file), `$ref`s to other
/// JSON or YAML files in that folder tree are read; URL references are never
/// downloaded. Unresolved references are listed in
/// [`OpenApiSpec::warnings`]. The first server is selected when its URL is
/// usable; otherwise call [`OpenApiSpec::select_server`].
pub fn load_openapi(bytes: &[u8], base_dir: Option<&Path>) -> Result<OpenApiSpec, String> {
    let mut doc = refs::parse_document(bytes)
        .map_err(|e| format!("Not a valid JSON or YAML document: {e}"))?;
    // Unquoted YAML versions (`swagger: 2.0`, `openapi: 3.1`) arrive as numbers.
    let version = match doc.get("openapi").or_else(|| doc.get("swagger")) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    if !(version.starts_with("3.") || version == "2.0") {
        return Err("Not an OpenAPI 3.x or Swagger 2.0 document".into());
    }
    let (warnings, referenced_files) = refs::bundle(&mut doc, base_dir);
    let doc = doc;
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .ok_or("The document has no paths")?;
    let key_vars = api_key_variables(&doc);
    let mut spec = OpenApiSpec {
        servers: servers(&doc),
        swagger: doc.get("swagger").is_some(),
        warnings,
        referenced_files,
        ..OpenApiSpec::default()
    };
    for (path, item) in paths {
        let item = resolve(&doc, item);
        let Some(operations) = item.as_object() else {
            continue;
        };
        for (method, op) in operations {
            if OPENAPI_METHODS.contains(&method.as_str()) {
                spec.operations.push(operation(
                    &doc,
                    &key_vars,
                    path,
                    item,
                    method,
                    resolve(&doc, op),
                ));
            } else if matches!(method.as_str(), "head" | "options" | "trace") {
                spec.other_methods += 1;
            }
        }
    }
    let first = ServerChoice::defaults(&spec, 0);
    let _ = spec.select_server(&first, "");
    Ok(spec)
}

fn operation(
    doc: &Value,
    key_vars: &BTreeMap<String, String>,
    path: &str,
    item: &Value,
    method: &str,
    op: &Value,
) -> OpenApiOperation {
    let mut result = OpenApiOperation {
        key: format!("{} {path}", method.to_ascii_uppercase()),
        method: method.to_ascii_uppercase(),
        path: path.to_owned(),
        name: String::new(),
        operation_id: op
            .get("operationId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        tags: op
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        request: Request::default(),
        variables: Vec::new(),
        missing_values: Vec::new(),
        other_auth: Vec::new(),
        cookie_keys: Vec::new(),
        key_variables: Vec::new(),
        cookies: 0,
        no_example: false,
        not_json: false,
        typed_body: false,
    };
    let add_variable = |variables: &mut Vec<Field>, name: String, value: String, secret| {
        if !variables.iter().any(|v: &Field| v.name == name) {
            variables.push(Field {
                name,
                value,
                secret,
            });
        }
    };
    // Operation parameters override path-level ones with the same name and location.
    let mut params: Vec<&Value> = Vec::new();
    for list in [item.get("parameters"), op.get("parameters")]
        .into_iter()
        .flatten()
    {
        for p in list.as_array().into_iter().flatten() {
            let p = resolve(doc, p);
            let key = |v: &Value| (v.get("name").cloned(), v.get("in").cloned());
            params.retain(|q| key(q) != key(p));
            params.push(p);
        }
    }
    let mut url_path = path.to_owned();
    let (mut query, mut headers) = (Vec::new(), Vec::new());
    let mut body = Body::default();
    let set_body = |body: &mut Body, result: &mut OpenApiOperation, schema: &Value| {
        let mut sampler = Sampler::new(doc, true);
        match sampler.sample(schema, 0) {
            Some(v) => {
                body.text = serde_json::to_string_pretty(&v).unwrap_or_default();
                body.json = true;
                result.typed_body = sampler.placeholders;
            }
            None => result.no_example = true,
        }
    };
    for p in params {
        let name = p.get("name").and_then(Value::as_str).unwrap_or_default();
        let required = p.get("required").and_then(Value::as_bool).unwrap_or(false);
        let example = Sampler::new(doc, false).sample(p, 0).map(|v| text(&v));
        match p.get("in").and_then(Value::as_str).unwrap_or_default() {
            "path" => {
                let var = variable_name(name);
                url_path = url_path.replace(&format!("{{{name}}}"), &format!("{{{{{var}}}}}"));
                if example.is_none() {
                    result.missing_values.push(var.clone());
                }
                add_variable(
                    &mut result.variables,
                    var,
                    example.unwrap_or_default(),
                    sensitive(name),
                );
            }
            "query" if required || example.is_some() => {
                query.push(field(name, &example.unwrap_or_default()));
            }
            "header" if required => {
                let lower = name.to_ascii_lowercase();
                if !matches!(lower.as_str(), "accept" | "content-type" | "authorization") {
                    headers.push(field(name, &example.unwrap_or_default()));
                }
            }
            "cookie" => result.cookies += 1,
            // Swagger 2.0 body parameter.
            "body" => set_body(&mut body, &mut result, p),
            _ => {}
        }
    }
    if let Some(content) = op
        .get("requestBody")
        .map(|b| resolve(doc, b))
        .and_then(|b| b.get("content"))
        .and_then(Value::as_object)
    {
        match content.iter().find(|(kind, _)| kind.contains("json")) {
            Some((_, media)) => set_body(&mut body, &mut result, media),
            None => result.not_json = true,
        }
    }

    // Security requirements are alternatives; each lists schemes used together.
    // Use the first one RustLoad can fill completely, else an anonymous one,
    // else the first one as far as possible.
    let mut auth = Auth::default();
    let requirements: Vec<&serde_json::Map<String, Value>> = op
        .get("security")
        .or_else(|| doc.get("security"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .collect();
    let supported = |name: &String| {
        matches!(
            classify(security_scheme(doc, name)),
            Scheme::Bearer | Scheme::Basic | Scheme::ApiKey(..)
        )
    };
    let chosen = requirements
        .iter()
        .find(|r| !r.is_empty() && r.keys().all(supported))
        .or_else(|| requirements.iter().find(|r| r.is_empty()))
        .or(requirements.first());
    for scheme_name in chosen.into_iter().flat_map(|r| r.keys()) {
        match classify(security_scheme(doc, scheme_name)) {
            Scheme::Bearer if auth.kind == AuthKind::None => {
                // A reference keeps the token out of the plan; its value is secret.
                auth.kind = AuthKind::Bearer;
                auth.bearer = "{{access_token}}".into();
                add_variable(
                    &mut result.variables,
                    "access_token".into(),
                    String::new(),
                    true,
                );
            }
            Scheme::Basic if auth.kind == AuthKind::None => auth.kind = AuthKind::Basic,
            Scheme::ApiKey(at, name) => {
                let var = key_vars
                    .get(scheme_name)
                    .cloned()
                    .unwrap_or_else(|| "api_key".into());
                let key = Field {
                    name: name.to_owned(),
                    value: format!("{{{{{var}}}}}"),
                    secret: true,
                };
                let list = if at == "header" {
                    &mut headers
                } else {
                    &mut query
                };
                list.retain(|f: &Field| !f.name.eq_ignore_ascii_case(name));
                list.push(key);
                if !result.key_variables.contains(&var) {
                    result.key_variables.push(var.clone());
                }
                add_variable(&mut result.variables, var, String::new(), true);
            }
            Scheme::CookieKey => result.cookie_keys.push(scheme_name.clone()),
            _ => result.other_auth.push(scheme_name.clone()),
        }
    }
    let name = op
        .get("operationId")
        .or_else(|| op.get("summary"))
        .and_then(Value::as_str)
        .unwrap_or(path);
    result.name = name.chars().take(60).collect();
    result.request = Request {
        id: 0,
        name: result.name.clone(),
        method: result.method.clone(),
        url: url_path,
        query,
        headers,
        body,
        auth,
        ..Request::default()
    };
    result
}
