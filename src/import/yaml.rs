//! YAML documents for the OpenAPI importer, read with yaml-rust2's event
//! parser and converted to the same `serde_json::Value` a JSON document gives.
//!
//! - Mapping order is kept (serde_json's `preserve_order`).
//! - Scalars follow the YAML 1.2 core schema: `null`/`~`/empty, `true`/
//!   `false`, decimal, `0x` and `0o` integers and decimal floats; everything
//!   else is text (so YAML 1.1 `yes`/`no`/`on`/`off` stay text). Quoted and
//!   block scalars are always text. Integers beyond 64 bits become floats, as
//!   in JSON. `.inf`/`.nan` and floats that overflow cannot be JSON numbers and
//!   stay text as written.
//! - Mapping keys are the key's text as written (`200:` → `"200"`, `true:` →
//!   `"true"`); a mapping or sequence used as a key is refused. The YAML 1.1
//!   merge key `<<` (a mapping or a list of mappings) is applied; keys written
//!   in the mapping win, then earlier merged mappings.
//! - Tags: `!!str` and the non-specific `!` make text, `!!null`/`!!bool`/
//!   `!!int`/`!!float` apply when the text fits that type; other tags are
//!   ignored and the node reads as untagged.
//! - Exactly one document; nesting is limited to [`MAX_DEPTH`] levels (as
//!   serde_json does for JSON) and the copies that anchors and aliases make to
//!   [`COPY_LIMIT`] estimated bytes, which stops alias bombs before they are
//!   built. Errors give a short reason and a line/column, never content.
use serde_json::{Map, Number, Value};
use std::collections::{HashMap, HashSet};
use yaml_rust2::parser::{Event, Parser, Tag};
use yaml_rust2::scanner::{Marker, ScanError, TScalarStyle};

/// Deepest nesting of mappings and sequences, aliases included.
pub const MAX_DEPTH: usize = 128;
/// Estimated bytes that anchors and aliases may copy in one document.
pub const COPY_LIMIT: usize = 64 * 1024 * 1024;
/// Estimated memory of one value besides its text.
const VALUE_COST: usize = 48;
const CORE: &str = "tag:yaml.org,2002:";

/// Parses a YAML document into a JSON value.
pub(super) fn parse(text: &str) -> Result<Value, String> {
    // Pass 1: syntax, one document, depth, and which anchors are used, so pass
    // 2 only keeps copies of those (nothing is built here).
    let mut used = HashSet::new();
    let mut parser = Parser::new_from_str(text);
    let (mut depth, mut documents) = (0usize, 0usize);
    loop {
        let (event, mark) = parser.next_token().map_err(scan_error)?;
        match event {
            Event::StreamEnd => break,
            Event::DocumentStart => {
                documents += 1;
                if documents > 1 {
                    return Err(at("more than one YAML document", mark));
                }
            }
            Event::SequenceStart(..) | Event::MappingStart(..) => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(too_deep(mark));
                }
            }
            Event::SequenceEnd | Event::MappingEnd => depth -= 1,
            Event::Alias(id) => {
                used.insert(id);
            }
            _ => {}
        }
    }
    if documents == 0 {
        return Ok(Value::Null);
    }
    let mut builder = Builder {
        used,
        anchors: HashMap::new(),
        stack: Vec::new(),
        copied: 0,
        root: None,
    };
    let mut parser = Parser::new_from_str(text);
    loop {
        let (event, mark) = parser.next_token().map_err(scan_error)?;
        if event == Event::StreamEnd {
            break;
        }
        builder.event(event, mark)?;
    }
    Ok(builder.root.map_or(Value::Null, |done| done.value))
}

fn at(what: &str, mark: Marker) -> String {
    format!(
        "YAML: {what} (line {}, column {})",
        mark.line(),
        mark.col() + 1
    )
}

fn too_deep(mark: Marker) -> String {
    at(&format!("nested deeper than {MAX_DEPTH} levels"), mark)
}

fn scan_error(e: ScanError) -> String {
    at(&crate::core::clip(e.info(), 80), *e.marker())
}

/// A finished value with its height (0 for scalars) and estimated size.
struct Done {
    value: Value,
    height: usize,
    size: usize,
}

enum Key {
    Name(String),
    Merge,
}

enum Node {
    Seq(Vec<Value>),
    Map(Map<String, Value>, Option<Key>),
}

struct Frame {
    node: Node,
    anchor: usize,
    height: usize,
    size: usize,
    mark: Marker,
}

struct Builder {
    used: HashSet<usize>,
    /// Copies of the anchors that aliases use, by anchor id (ids are unique,
    /// even when a name is reused).
    anchors: HashMap<usize, Done>,
    stack: Vec<Frame>,
    copied: usize,
    root: Option<Done>,
}

impl Builder {
    fn event(&mut self, event: Event, mark: Marker) -> Result<(), String> {
        match event {
            Event::Scalar(text, style, anchor, tag) => {
                let size = VALUE_COST + text.len();
                if self.wants_key() {
                    // A key: its text as written.
                    let merge = text == "<<" && style == TScalarStyle::Plain && tag.is_none();
                    if anchor != 0 && self.used.contains(&anchor) {
                        self.keep(anchor, &Value::String(text.clone()), 0, size, mark)?;
                    }
                    self.set_key(if merge { Key::Merge } else { Key::Name(text) });
                    return Ok(());
                }
                let value = scalar(text, style, tag.as_ref());
                self.finish(value, 0, size, anchor, mark)
            }
            Event::Alias(id) => {
                let Some(&Done { height, size, .. }) = self.anchors.get(&id) else {
                    return Err(at("alias to an anchor that is not complete", mark));
                };
                if self.stack.len() + height > MAX_DEPTH {
                    return Err(too_deep(mark));
                }
                if self.wants_key() {
                    let name = match &self.anchors[&id].value {
                        Value::String(s) => s.clone(),
                        Value::Null => String::new(),
                        v @ (Value::Bool(_) | Value::Number(_)) => v.to_string(),
                        _ => return Err(at("a mapping key must be a scalar", mark)),
                    };
                    self.set_key(Key::Name(name));
                    return Ok(());
                }
                // Counted before the copy is made.
                self.charge(size, mark)?;
                let value = self.anchors[&id].value.clone();
                self.finish(value, height, size, 0, mark)
            }
            Event::SequenceStart(anchor, _) | Event::MappingStart(anchor, _) => {
                if self.wants_key() {
                    return Err(at("a mapping key must be a scalar", mark));
                }
                let node = if matches!(event, Event::SequenceStart(..)) {
                    Node::Seq(Vec::new())
                } else {
                    Node::Map(Map::new(), None)
                };
                self.stack.push(Frame {
                    node,
                    anchor,
                    height: 0,
                    size: VALUE_COST,
                    mark,
                });
                Ok(())
            }
            Event::SequenceEnd | Event::MappingEnd => {
                let frame = self
                    .stack
                    .pop()
                    .ok_or_else(|| at("unbalanced document", mark))?;
                let value = match frame.node {
                    Node::Seq(items) => Value::Array(items),
                    Node::Map(map, _) => Value::Object(map),
                };
                self.finish(
                    value,
                    frame.height + 1,
                    frame.size,
                    frame.anchor,
                    frame.mark,
                )
            }
            _ => Ok(()),
        }
    }

    /// Whether the next node is a key of the open mapping.
    fn wants_key(&self) -> bool {
        matches!(
            self.stack.last(),
            Some(Frame {
                node: Node::Map(_, None),
                ..
            })
        )
    }

    fn set_key(&mut self, next: Key) {
        if let Some(Frame {
            node: Node::Map(_, key),
            ..
        }) = self.stack.last_mut()
        {
            *key = Some(next);
        }
    }

    /// Counts a copy against [`COPY_LIMIT`].
    fn charge(&mut self, size: usize, mark: Marker) -> Result<(), String> {
        self.copied = self.copied.saturating_add(size);
        if self.copied > COPY_LIMIT {
            return Err(at(
                &format!(
                    "anchors and aliases expand to more than {} MiB",
                    COPY_LIMIT >> 20
                ),
                mark,
            ));
        }
        Ok(())
    }

    /// Keeps a copy of an anchored value that an alias uses.
    fn keep(
        &mut self,
        anchor: usize,
        value: &Value,
        height: usize,
        size: usize,
        mark: Marker,
    ) -> Result<(), String> {
        self.charge(size, mark)?;
        let value = value.clone();
        self.anchors.insert(
            anchor,
            Done {
                value,
                height,
                size,
            },
        );
        Ok(())
    }

    /// Adds a finished value to its parent (or makes it the document).
    fn finish(
        &mut self,
        value: Value,
        height: usize,
        size: usize,
        anchor: usize,
        mark: Marker,
    ) -> Result<(), String> {
        if anchor != 0 && self.used.contains(&anchor) {
            self.keep(anchor, &value, height, size, mark)?;
        }
        let Some(parent) = self.stack.last_mut() else {
            self.root = Some(Done {
                value,
                height,
                size,
            });
            return Ok(());
        };
        parent.height = parent.height.max(height);
        parent.size = parent.size.saturating_add(size);
        match &mut parent.node {
            Node::Seq(items) => items.push(value),
            Node::Map(map, key) => match key.take() {
                Some(Key::Name(name)) => {
                    parent.size = parent.size.saturating_add(name.len());
                    map.insert(name, value);
                }
                Some(Key::Merge) => merge(map, value, mark)?,
                None => return Err(at("a mapping key must be a scalar", mark)),
            },
        }
        Ok(())
    }
}

/// Applies a `<<` merge key: keys already in the mapping win, then earlier
/// mappings of a list.
fn merge(map: &mut Map<String, Value>, value: Value, mark: Marker) -> Result<(), String> {
    let sources = match value {
        Value::Object(source) => vec![source],
        Value::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Value::Object(source) => Ok(source),
                _ => Err(()),
            })
            .collect::<Result<_, _>>()
            .map_err(|()| at("merge key `<<` needs mappings", mark))?,
        _ => return Err(at("merge key `<<` needs a mapping", mark)),
    };
    for source in sources {
        for (k, v) in source {
            map.entry(k).or_insert(v);
        }
    }
    Ok(())
}

/// The JSON value of a scalar (see the module notes).
fn scalar(text: String, style: TScalarStyle, tag: Option<&Tag>) -> Value {
    let core = tag.filter(|t| t.handle == CORE).map(|t| t.suffix.as_str());
    let non_specific =
        tag.is_some_and(|t| t.suffix == "!" || (t.handle == "!" && t.suffix.is_empty()));
    if non_specific || core == Some("str") {
        return Value::String(text);
    }
    let plain = style == TScalarStyle::Plain;
    let typed = match core {
        Some("null") => null(&text).then_some(Value::Null),
        Some("bool") => boolean(&text).map(Value::Bool),
        Some("int" | "float") => number(&text),
        _ if plain => null(&text)
            .then_some(Value::Null)
            .or_else(|| boolean(&text).map(Value::Bool))
            .or_else(|| number(&text)),
        _ => None,
    };
    typed.unwrap_or(Value::String(text))
}

fn null(text: &str) -> bool {
    matches!(text, "" | "~" | "null" | "Null" | "NULL")
}

fn boolean(text: &str) -> Option<bool> {
    match text {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

/// YAML 1.2 core integers and floats that JSON can hold.
fn number(text: &str) -> Option<Value> {
    let radix = |digits: &str, radix| {
        (!digits.is_empty())
            .then(|| u64::from_str_radix(digits, radix).ok())
            .flatten()
            .map(|n| Value::Number(n.into()))
    };
    if let Some(hex) = text.strip_prefix("0x") {
        return radix(hex, 16);
    }
    if let Some(octal) = text.strip_prefix("0o") {
        return radix(octal, 8);
    }
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(i) => (&unsigned[..i], Some(&unsigned[i + 1..])),
        None => (unsigned, None),
    };
    let (whole, fraction) = match mantissa.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (mantissa, None),
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let valid_mantissa = digits(whole)
        && fraction.is_none_or(digits)
        && (!whole.is_empty() || fraction.is_some_and(|f| !f.is_empty()));
    let valid_exponent = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['-', '+']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    if !valid_mantissa || !valid_exponent {
        return None;
    }
    if fraction.is_none() && exponent.is_none() {
        if let Ok(n) = text.parse::<i64>() {
            return Some(Value::Number(n.into()));
        }
        if let Ok(n) = unsigned.parse::<u64>()
            && !text.starts_with('-')
        {
            return Some(Value::Number(n.into()));
        }
    }
    text.parse::<f64>()
        .ok()
        .and_then(Number::from_f64)
        .map(Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_follow_the_core_schema() {
        let plain = |t: &str| scalar(t.into(), TScalarStyle::Plain, None);
        assert_eq!(plain("42"), Value::from(42));
        assert_eq!(plain("-7"), Value::from(-7));
        assert_eq!(plain("0x1F"), Value::from(31));
        assert_eq!(plain("0o17"), Value::from(15));
        assert_eq!(plain("1.5e3"), Value::from(1500.0));
        assert_eq!(plain(".5"), Value::from(0.5));
        assert_eq!(
            plain("18446744073709551616"),
            Value::from(1.8446744073709552e19)
        );
        for text in [
            ".inf", "-.Inf", ".nan", "1e999", "yes", "off", "1_000", "1:20", "0x", "+", ".",
        ] {
            assert_eq!(plain(text), Value::from(text), "{text}");
        }
        assert_eq!(plain("~"), Value::Null);
        assert_eq!(plain("False"), Value::Bool(false));
        assert_eq!(
            scalar("42".into(), TScalarStyle::DoubleQuoted, None),
            Value::from("42")
        );
    }

    #[test]
    fn keys_tags_anchors_and_merges() {
        let doc = parse(
            "responses:\n  200: {description: ok}\n  default: {}\ntrue: 1\n3.0: x\n~: n\n\
             s: !!str 200\nn: ! 12\ni: !!int \"7\"\nlocal: !Ref 5\nq: '<<'\n\
             base: &base {a: 1, b: 2}\nmore: &more {b: 9, c: 3}\n\
             m: {<<: [*base, *more], a: 0}\n'<<': literal\nk: &k name\n*k : aliased\n",
        )
        .unwrap();
        let keys: Vec<&str> = doc["responses"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["200", "default"]);
        assert_eq!(doc["true"], Value::from(1));
        assert_eq!(doc["3.0"], Value::from("x"));
        assert_eq!(doc["~"], Value::from("n"));
        assert_eq!(doc["s"], Value::from("200"));
        assert_eq!(doc["n"], Value::from("12"));
        assert_eq!(doc["i"], Value::from(7));
        assert_eq!(doc["local"], Value::from(5));
        assert_eq!(doc["q"], Value::from("<<"));
        assert_eq!(doc["m"], serde_json::json!({"a": 0, "b": 2, "c": 3}));
        assert_eq!(doc["<<"], Value::from("literal"));
        assert_eq!(doc["name"], Value::from("aliased"));
    }

    #[test]
    fn documents_are_bounded() {
        let err = |text: &str| parse(text).unwrap_err();
        assert!(err("a: 1\n---\nb: 2\n").contains("more than one YAML document"));
        assert!(err("? [a]\n: 1\n").contains("a mapping key must be a scalar"));
        assert!(err("m: {<<: 1}\n").contains("merge key"));
        // Depth: written nesting and nesting reached through an alias.
        let deep = format!("{}{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        assert!(err(&deep).contains("nested deeper than 128 levels"));
        let ok = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(parse(&ok).is_ok());
        let half = MAX_DEPTH / 2 + 1;
        let via_alias = format!(
            "a: &a {}{}\nb: {}*a{}\n",
            "[".repeat(half),
            "]".repeat(half),
            "[".repeat(half),
            "]".repeat(half)
        );
        assert!(err(&via_alias).contains("nested deeper"));
        assert_eq!(parse("").unwrap(), Value::Null);
        assert_eq!(parse("# only a comment\n").unwrap(), Value::Null);
    }
}
