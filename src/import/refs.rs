//! External `$ref`s of an OpenAPI document: referenced local files are read
//! once and embedded in the document so every reference becomes a local
//! `#/…` pointer that the rest of the importer follows as before.
//!
//! Rules: nothing is ever downloaded (URL references are listed and left
//! unresolved); only regular files inside the document's folder tree are read
//! (paths are canonicalized, so `..` and symlinks that lead outside are
//! refused); each file may be at most [`REF_FILE_LIMIT`] bytes and at most
//! [`REF_FILES_MAX`] files are read. A file is read once however often it is
//! referenced, so files that refer to each other do not loop; references
//! inside a file resolve against that file. Messages name the reference,
//! never file contents.
use super::percent_decode;
use crate::core::clip;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Size limit of one referenced file.
pub const REF_FILE_LIMIT: u64 = 10 * 1024 * 1024;
/// Most referenced files read for one document.
pub const REF_FILES_MAX: usize = 50;
/// Where referenced files are embedded in the document (an array, one entry
/// per file).
const EMBED_KEY: &str = "x-rustload-files";

/// Keys whose values are data (examples, defaults), not definitions, unless
/// they name a property.
const DATA_KEYS: [&str; 4] = ["example", "default", "enum", "const"];
/// Keys whose object values map names to definitions: their keys are names,
/// so a property called "example" or "$ref" is still a definition.
const NAME_MAPS: [&str; 17] = [
    "properties",
    "patternProperties",
    "definitions",
    "$defs",
    "dependentSchemas",
    "schemas",
    "parameters",
    "responses",
    "securitySchemes",
    "securityDefinitions",
    "examples",
    "requestBodies",
    "headers",
    "links",
    "callbacks",
    "pathItems",
    "paths",
];

/// Parses one document or referenced file. Only JSON for now; a YAML reader
/// plugs in here.
pub(super) fn parse_document(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// Embeds the local files `doc` refers to and rewrites every `$ref` to a
/// local pointer. Without `base_dir` (a document that is not a file), external
/// references are only listed. Returns the warnings and the number of files
/// read.
pub(super) fn bundle(doc: &mut Value, base_dir: Option<&Path>) -> (Vec<String>, usize) {
    let mut bundler = Bundler {
        root: base_dir.and_then(|d| d.canonicalize().ok()),
        has_dir: base_dir.is_some(),
        files: Vec::new(),
        dirs: Vec::new(),
        by_path: HashMap::new(),
        warnings: Vec::new(),
    };
    let mut root = std::mem::take(doc);
    bundler.rewrite(&mut root, None, None);
    let mut next = 0;
    while next < bundler.files.len() {
        let mut file = std::mem::take(&mut bundler.files[next]);
        bundler.rewrite(&mut file, Some(next), Some(&root));
        bundler.files[next] = file;
        next += 1;
    }
    let count = bundler.files.len();
    if count > 0
        && let Some(map) = root.as_object_mut()
    {
        map.insert(EMBED_KEY.into(), Value::Array(bundler.files));
    }
    *doc = root;
    (bundler.warnings, count)
}

struct Bundler {
    /// Canonical folder of the document; files must be inside it.
    root: Option<PathBuf>,
    has_dir: bool,
    files: Vec<Value>,
    /// Folder of each file, for the references inside it.
    dirs: Vec<PathBuf>,
    by_path: HashMap<PathBuf, usize>,
    warnings: Vec<String>,
}
impl Bundler {
    fn warn(&mut self, reference: &str, what: &str) {
        let text = format!("$ref \"{}\" {what}; left unresolved", clip(reference, 120));
        if !self.warnings.contains(&text) {
            self.warnings.push(text);
        }
    }

    /// Rewrites the references of one file (`current`: None for the document
    /// itself). `root` is the document while a referenced file is rewritten.
    fn rewrite(&mut self, file: &mut Value, current: Option<usize>, root: Option<&Value>) {
        // Distinct references in document order, so the file limit keeps the
        // first files a reader sees.
        let (mut found, mut seen) = (Vec::new(), HashSet::new());
        visit_refs(file, false, &mut |r| {
            if seen.insert(r.clone()) {
                found.push(r.clone());
            }
        });
        let mut targets = HashMap::new();
        for reference in found {
            if let Some(target) = self.target(&reference, current, file, root) {
                targets.insert(reference, target);
            }
        }
        visit_refs(file, false, &mut |r| {
            if let Some(target) = targets.get(r.as_str()) {
                r.clone_from(target);
            }
        });
    }

    /// The local pointer `reference` stands for, or None (with a warning).
    fn target(
        &mut self,
        reference: &str,
        current: Option<usize>,
        this: &Value,
        root: Option<&Value>,
    ) -> Option<String> {
        let (file, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        let pointer = percent_decode(fragment);
        if !pointer.is_empty() && !pointer.starts_with('/') {
            self.warn(reference, "is not a JSON pointer");
            return None;
        }
        let index = if file.is_empty() {
            current
        } else {
            Some(self.load(reference, file, current)?)
        };
        let doc = match index {
            i if i == current => this,
            None => root.unwrap_or(this),
            Some(i) => &self.files[i],
        };
        let found = doc.pointer(&pointer).is_some();
        if !found {
            self.warn(reference, "points to nothing");
            return None;
        }
        Some(match index {
            None => format!("#{pointer}"),
            Some(i) => format!("#/{EMBED_KEY}/{i}{pointer}"),
        })
    }

    /// Reads a referenced file (once) and returns its index.
    fn load(&mut self, reference: &str, file: &str, current: Option<usize>) -> Option<usize> {
        let name = percent_decode(file);
        if let Ok(url) = url::Url::parse(&name)
            && url.scheme().len() > 1
        {
            let what = if matches!(url.scheme(), "http" | "https") {
                "is a URL and RustLoad never downloads references"
            } else {
                "uses an unsupported scheme"
            };
            self.warn(reference, what);
            return None;
        }
        let Some(root) = self.root.clone() else {
            let what = if self.has_dir {
                "could not be read (the document's folder is not accessible)"
            } else {
                "refers to another file; open the document from a file to follow it"
            };
            self.warn(reference, what);
            return None;
        };
        let relative = Path::new(&name);
        if relative.has_root() || relative.is_absolute() {
            self.warn(
                reference,
                "is an absolute path; only files next to the document are read",
            );
            return None;
        }
        let dir = current.map_or(&root, |i| &self.dirs[i]);
        let Ok(path) = dir.join(relative).canonicalize() else {
            self.warn(reference, "names a file that does not exist");
            return None;
        };
        if !path.starts_with(&root) {
            self.warn(reference, "leads outside the document's folder");
            return None;
        }
        if let Some(&index) = self.by_path.get(&path) {
            return Some(index);
        }
        if self.files.len() >= REF_FILES_MAX {
            self.warn(
                reference,
                &format!("would read more than {REF_FILES_MAX} referenced files"),
            );
            return None;
        }
        let bytes = match read_limited(&path) {
            Ok(bytes) => bytes,
            Err(what) => {
                self.warn(reference, what);
                return None;
            }
        };
        let Ok(value) = parse_document(&bytes) else {
            self.warn(
                reference,
                "names a file that is not JSON (YAML is not supported yet)",
            );
            return None;
        };
        let index = self.files.len();
        self.files.push(value);
        self.dirs
            .push(path.parent().map_or(root, Path::to_path_buf));
        self.by_path.insert(path, index);
        Some(index)
    }
}

fn read_limited(path: &Path) -> Result<Vec<u8>, &'static str> {
    let mut file = std::fs::File::open(path).map_err(|_| "could not be read")?;
    let meta = file.metadata().map_err(|_| "could not be read")?;
    if !meta.is_file() {
        return Err("does not name a file");
    }
    if meta.len() > REF_FILE_LIMIT {
        return Err("names a file larger than 10 MiB");
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(REF_FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "could not be read")?;
    if bytes.len() as u64 > REF_FILE_LIMIT {
        return Err("names a file larger than 10 MiB");
    }
    Ok(bytes)
}

/// Calls `f` with every `$ref` string, skipping example and default data.
/// `names`: the keys of this object are names (see [`NAME_MAPS`]).
fn visit_refs(value: &mut Value, names: bool, f: &mut impl FnMut(&mut String)) {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if !names {
                    if key == "$ref" {
                        if let Value::String(r) = child {
                            f(r);
                        }
                        continue;
                    }
                    if DATA_KEYS.contains(&key.as_str()) {
                        continue;
                    }
                }
                let child_names = !names && NAME_MAPS.contains(&key.as_str());
                visit_refs(child, child_names, f);
            }
        }
        Value::Array(items) => {
            for item in items {
                visit_refs(item, false, f);
            }
        }
        _ => {}
    }
}
