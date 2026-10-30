//! Builds requests from browser "Copy as cURL (bash)" commands and HAR files.
//! Only request data is imported; HAR responses are never read or stored.
use crate::core::{Auth, AuthKind, Body, Field, Request, sensitive};
use serde::Deserialize;

/// Same as the plan's request limit.
pub const IMPORT_LIMIT: usize = 500;
pub const HAR_SIZE_LIMIT: u64 = 50 * 1024 * 1024;

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
