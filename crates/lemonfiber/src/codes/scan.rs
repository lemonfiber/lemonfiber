//! Reading every problem code out of the source that declares it.
//!
//! Codes are declared beside the code that raises them rather than in a table, so
//! there is nothing to enumerate at run time. This reads the declarations instead,
//! the way the compiler reads them: a lexer that tells code from a string from a
//! comment, and a brace count that says which half of a file ships.
//!
//! Reading source text is coarse. It is kept honest by refusing to guess: a
//! declaration whose name is not a literal, and a file whose braces do not balance
//! by its last line, are both reported rather than left out of the answer.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The call every code is declared with.
const DECLARATION: &str = "Code::new(";

/// Every code the crates under this root declare and ship, with where each is
/// declared, or what stopped the read.
///
/// The reader takes the whole of `crates/*/src`, so a code declared in any crate of
/// the workspace is found by the same pass. Files a parent gates behind
/// `#[cfg(test)]` are dropped whole, and declarations inside a gated block are
/// dropped where they stand.
///
/// One declaration per entry rather than a set, so that a code declared twice is
/// still two entries and whoever asks can say where each of them is.
pub(crate) fn declared(root: &Path) -> Result<Vec<(PathBuf, String)>, Vec<String>> {
    let sources = sources(root);
    if sources.is_empty() {
        return Err(vec![format!(
            "no Rust sources under {} — the reader was pointed somewhere else",
            root.display()
        )]);
    }

    let scans: BTreeMap<PathBuf, FileScan> = sources
        .into_iter()
        .map(|(path, text)| (path, scan(&text)))
        .collect();
    let gated = gated_files(&scans);

    let mut complaints = Vec::new();
    let mut codes = Vec::new();
    for (path, scanned) in &scans {
        let shown = path.display();
        if !scanned.balanced {
            complaints.push(format!(
                "{shown}: braces did not balance, so the reader lost its place"
            ));
        }
        let gated_whole = gated
            .iter()
            .any(|module| path == module || path.starts_with(module));
        for declaration in &scanned.declarations {
            let line = declaration.line;
            match &declaration.name {
                None => complaints.push(format!(
                    "{shown}:{line}: a code declared with something other than a literal name"
                )),
                Some(name) if !declaration.test_only && !gated_whole => {
                    codes.push((path.clone(), name.clone()));
                }
                Some(_) => {}
            }
        }
    }

    if complaints.is_empty() {
        Ok(codes)
    } else {
        Err(complaints)
    }
}

/// Every `.rs` file under a crate's `src`, keyed by its path relative to `crates`.
fn sources(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut found = BTreeMap::new();
    let crates = root.join("crates");
    collect(&crates, &crates, &mut found);
    found
}

/// Adds every source file beneath one directory, walking into the ones it holds.
fn collect(dir: &Path, base: &Path, found: &mut BTreeMap<PathBuf, String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, base, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path.strip_prefix(base).unwrap_or(&path).to_path_buf();
            // A crate's tests and examples are not what it ships, and neither
            // compiles a `#[cfg(test)]` block that would mark them as such.
            if relative
                .components()
                .nth(1)
                .is_some_and(|part| part.as_os_str() == "src")
            {
                found.insert(relative, fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
}

/// The files and directories a parent module declares only for tests.
///
/// Both the file and the directory of the same name are named, because a module may
/// be either, and anything beneath the directory is gated with it.
fn gated_files(scans: &BTreeMap<PathBuf, FileScan>) -> BTreeSet<PathBuf> {
    let mut gated = BTreeSet::new();
    for (path, scanned) in scans {
        let parent = path.parent().unwrap_or(Path::new(""));
        let base = match path.file_stem().and_then(std::ffi::OsStr::to_str) {
            Some("lib" | "main" | "mod") | None => parent.to_path_buf(),
            Some(stem) => parent.join(stem),
        };
        for module in &scanned.gated {
            gated.insert(base.join(format!("{module}.rs")));
            gated.insert(base.join(module));
        }
    }
    gated
}

/// One `Code::new(…)` the reader found.
struct Declaration {
    /// The name it was given, or nothing where that was not a literal.
    name: Option<String>,
    /// The line the name closed on.
    line: usize,
    /// Whether it stands inside a block compiled only for tests.
    test_only: bool,
}

/// What one file's declarations amount to.
#[derive(Default)]
struct FileScan {
    /// Every declaration in the file, in the order they close.
    declarations: Vec<Declaration>,
    /// The modules this file declares only for tests.
    gated: Vec<String>,
    /// Whether the braces balanced by the last line.
    balanced: bool,
}

/// Reads one file's declarations, and which of them the tests alone compile.
fn scan(text: &str) -> FileScan {
    let mut reader = Reader::default();
    let mut gate = Gate::default();
    let mut found = FileScan::default();

    for (offset, line) in text.lines().enumerate() {
        let read = reader.read(line);
        let trimmed = read.code.trim();
        if gate.arm(trimmed) {
            continue;
        }

        let test_only = gate.test_only();
        for name in read.declarations {
            found.declarations.push(Declaration {
                name,
                line: offset + 1,
                test_only,
            });
        }

        found.gated.extend(gate.open(&read.code, trimmed));
        gate.follow(&read.code);
    }

    found.balanced = gate.balanced();
    found
}

/// Which half of a file the reader stands in.
#[derive(Default)]
struct Gate {
    /// How deep in braces the reader stands.
    depth: i64,
    /// The depth a block compiled only for tests opened at.
    test_from: Option<i64>,
    /// A `#[cfg(test)]` has been read and the item it gates has not begun.
    armed: bool,
}

impl Gate {
    /// Whether what stands here is compiled only for tests.
    fn test_only(&self) -> bool {
        self.test_from.is_some() || self.armed
    }

    /// Whether the braces the reader followed have all been closed.
    fn balanced(&self) -> bool {
        self.depth == 0
    }

    /// Takes the attribute that gates whatever follows it, saying whether this line
    /// was that attribute and nothing else.
    fn arm(&mut self, trimmed: &str) -> bool {
        let attribute = self.test_from.is_none() && trimmed == "#[cfg(test)]";
        self.armed |= attribute;
        attribute
    }

    /// Opens the gated item where this line begins it, naming the module it declares.
    ///
    /// An item is a block, which runs until its braces close, or a declaration ending
    /// in a semicolon, which runs to the end of this line. Anything else — an
    /// attribute of its own, a signature the formatter spread — holds the gate open.
    fn open(&mut self, code: &str, trimmed: &str) -> Option<String> {
        if !self.armed || self.test_from.is_some() {
            return None;
        }
        if code.contains('{') {
            self.test_from = Some(self.depth);
            self.armed = false;
            return None;
        }
        if !trimmed.ends_with(';') {
            return None;
        }
        self.armed = false;
        module_declared(trimmed)
    }

    /// Follows one line's braces, closing a gated block where they end it.
    fn follow(&mut self, code: &str) {
        for letter in code.chars() {
            match letter {
                '{' => self.depth += 1,
                '}' => self.depth -= 1,
                _ => {}
            }
        }
        if self.test_from.is_some_and(|opened| self.depth <= opened) {
            self.test_from = None;
        }
    }
}

/// The module a line declares, where the line declares one.
fn module_declared(line: &str) -> Option<String> {
    let (_, name) = line.strip_suffix(';')?.rsplit_once("mod ")?;
    let name = name.trim();
    (!name.is_empty()
        && name
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '_'))
    .then(|| name.to_owned())
}

/// What one line left behind.
struct Line {
    /// The line with its strings and comments taken out, which is where braces count.
    code: String,
    /// The name of every declaration that closed on it, or nothing where it was not
    /// a literal.
    declarations: Vec<Option<String>>,
}

/// What the reader is in the middle of.
#[derive(Default)]
enum Lex {
    /// Ordinary code.
    #[default]
    Plain,
    /// Inside a `"…"` string.
    Text,
    /// Inside a raw string opened with this many hashes.
    Raw(usize),
    /// Inside a block comment nested this deep.
    Comment(usize),
}

/// Where the reader stands, carried from one line to the next.
#[derive(Default)]
struct Reader {
    /// What it is in the middle of.
    lex: Lex,
    /// A declaration has been opened and its name has not begun.
    expecting: bool,
    /// The name being read, where a declaration's string is open.
    capture: Option<String>,
}

impl Reader {
    /// Reads one line, giving back its code and the declarations it closed.
    fn read(&mut self, line: &str) -> Line {
        let mut code = String::new();
        let mut declarations = Vec::new();
        let mut rest = line;
        while !rest.is_empty() {
            rest = match self.lex {
                Lex::Plain => self.plain(rest, &mut code, &mut declarations),
                Lex::Text => self.text(rest, &mut declarations),
                Lex::Raw(hashes) => self.raw(rest, hashes, &mut declarations),
                Lex::Comment(depth) => self.comment(rest, depth),
            };
        }
        Line { code, declarations }
    }

    /// Reads ordinary code up to whatever changes what is being read.
    fn plain<'a>(
        &mut self,
        rest: &'a str,
        code: &mut String,
        declarations: &mut Vec<Option<String>>,
    ) -> &'a str {
        if rest.starts_with("//") {
            return "";
        }
        if let Some(after) = rest.strip_prefix("/*") {
            self.lex = Lex::Comment(1);
            return after;
        }
        if let Some(after) = rest.strip_prefix(DECLARATION) {
            if self.expecting {
                declarations.push(None);
            }
            self.expecting = true;
            code.push_str(DECLARATION);
            return after;
        }
        if let Some((hashes, after)) = raw_opening(rest) {
            self.lex = Lex::Raw(hashes);
            self.begin();
            return after;
        }
        if let Some(after) = rest.strip_prefix('"') {
            self.lex = Lex::Text;
            self.begin();
            return after;
        }
        if let Some(after) = character(rest) {
            return after;
        }
        let mut letters = rest.chars();
        let leading = letters.next().unwrap_or(' ');
        if self.expecting && !leading.is_whitespace() {
            declarations.push(None);
            self.expecting = false;
        }
        code.push(leading);
        letters.as_str()
    }

    /// Reads inside a `"…"` string.
    fn text<'a>(&mut self, rest: &'a str, declarations: &mut Vec<Option<String>>) -> &'a str {
        if let Some(after) = rest.strip_prefix('\\') {
            return beyond(after);
        }
        if let Some(after) = rest.strip_prefix('"') {
            self.lex = Lex::Plain;
            self.close(declarations);
            return after;
        }
        self.absorb(rest)
    }

    /// Reads inside a raw string, which ends only at a quote and its hashes.
    fn raw<'a>(
        &mut self,
        rest: &'a str,
        hashes: usize,
        declarations: &mut Vec<Option<String>>,
    ) -> &'a str {
        if let Some(after) = raw_closing(rest, hashes) {
            self.lex = Lex::Plain;
            self.close(declarations);
            return after;
        }
        self.absorb(rest)
    }

    /// Reads inside a block comment, which may hold another.
    fn comment<'a>(&mut self, rest: &'a str, depth: usize) -> &'a str {
        if let Some(after) = rest.strip_prefix("*/") {
            self.lex = if depth <= 1 {
                Lex::Plain
            } else {
                Lex::Comment(depth - 1)
            };
            return after;
        }
        if let Some(after) = rest.strip_prefix("/*") {
            self.lex = Lex::Comment(depth + 1);
            return after;
        }
        beyond(rest)
    }

    /// Takes one character of a string into the name being read, where one is.
    fn absorb<'a>(&mut self, rest: &'a str) -> &'a str {
        let mut letters = rest.chars();
        let leading = letters.next().unwrap_or(' ');
        if let Some(name) = self.capture.as_mut() {
            name.push(leading);
        }
        letters.as_str()
    }

    /// Starts reading a name, where the string opening is a declaration's.
    fn begin(&mut self) {
        if self.expecting {
            self.capture = Some(String::new());
            self.expecting = false;
        }
    }

    /// Closes a name, where one was being read.
    fn close(&mut self, declarations: &mut Vec<Option<String>>) {
        if let Some(name) = self.capture.take() {
            declarations.push(Some(name));
        }
    }
}

/// The hashes opening a raw string here, and what follows its quote.
fn raw_opening(rest: &str) -> Option<(usize, &str)> {
    let after = rest.strip_prefix("br").or_else(|| rest.strip_prefix('r'))?;
    let hashes = after.chars().take_while(|letter| *letter == '#').count();
    let opened = after.get(hashes..).unwrap_or_default().strip_prefix('"')?;
    Some((hashes, opened))
}

/// What follows a raw string's closing quote, where it closes here.
fn raw_closing(rest: &str, hashes: usize) -> Option<&str> {
    let after = rest.strip_prefix('"')?;
    if after.chars().take_while(|letter| *letter == '#').count() < hashes {
        return None;
    }
    Some(after.get(hashes..).unwrap_or_default())
}

/// What follows a character literal, where one starts here.
fn character(rest: &str) -> Option<&str> {
    let after = rest.strip_prefix('\'')?;
    let body = match after.strip_prefix('\\') {
        Some(escaped) => beyond(escaped),
        None => beyond(after),
    };
    body.strip_prefix('\'')
}

/// The text beyond its first character.
fn beyond(rest: &str) -> &str {
    let mut letters = rest.chars();
    letters.next();
    letters.as_str()
}

#[cfg(test)]
mod tests {
    use super::{declared, module_declared, scan, FileScan};
    use std::path::{Path, PathBuf};

    /// Every code one piece of source declares and ships, in the order found.
    fn shipped_in(text: &str) -> Vec<String> {
        scan(text)
            .declarations
            .into_iter()
            .filter(|declaration| !declaration.test_only)
            .filter_map(|declaration| declaration.name)
            .collect()
    }

    /// Every code one piece of source declares for its tests alone.
    fn tested_in(text: &str) -> Vec<String> {
        scan(text)
            .declarations
            .into_iter()
            .filter(|declaration| declaration.test_only)
            .filter_map(|declaration| declaration.name)
            .collect()
    }

    /// The lines carrying a declaration the reader could not read a name from.
    fn unreadable_in(text: &str) -> Vec<usize> {
        scan(text)
            .declarations
            .into_iter()
            .filter(|declaration| declaration.name.is_none())
            .map(|declaration| declaration.line)
            .collect()
    }

    /// The shape every code is declared in.
    #[test]
    fn it_reads_a_code_declared_beside_what_raises_it() {
        let source = "pub const LEAKING: Code = Code::new(\"VPN-1\");\n";

        assert_eq!(shipped_in(source), ["VPN-1"]);
        assert!(scan(source).balanced);
    }

    /// A declaration written out where it is raised is read the same way.
    #[test]
    fn it_reads_a_code_qualified_by_its_module() {
        let source = "fn raise() {\n    crate::error::Code::new(\"VPN-2\");\n}\n";

        assert_eq!(shipped_in(source), ["VPN-2"]);
    }

    /// A block the tests alone compile is not part of what ships.
    #[test]
    fn it_leaves_out_a_code_declared_inside_a_test_module() {
        let source = "\
pub const REAL: Code = Code::new(\"VPN-1\");

#[cfg(test)]
mod tests {
    const CODE: Code = Code::new(\"TEST-1\");

    fn nested() {
        Code::new(\"TEST-2\");
    }
}
";

        assert_eq!(shipped_in(source), ["VPN-1"]);
        assert_eq!(tested_in(source), ["TEST-1", "TEST-2"]);
    }

    /// The gate closes where the block does, not at the end of the file.
    #[test]
    fn it_reads_what_follows_a_test_module_as_shipped_again() {
        let source = "\
#[cfg(test)]
mod tests {
    const CODE: Code = Code::new(\"TEST-1\");
}

pub const AFTER: Code = Code::new(\"VPN-3\");
";

        assert_eq!(shipped_in(source), ["VPN-3"]);
    }

    /// A gate on one item ends with that item.
    #[test]
    fn it_gates_a_single_function_and_no_more() {
        let source = "\
impl Thing {
    #[cfg(test)]
    fn only_for_tests() -> Code {
        Code::new(\"TEST-1\")
    }

    fn shipped() -> Code {
        Code::new(\"VPN-4\")
    }
}
";

        assert_eq!(shipped_in(source), ["VPN-4"]);
        assert_eq!(tested_in(source), ["TEST-1"]);
    }

    /// A gate reaches past whatever stands between it and the item it gates.
    #[test]
    fn it_holds_the_gate_open_until_the_item_begins() {
        let source = "\
#[cfg(test)]
#[must_use]
fn only_for_tests() -> Code {
    Code::new(\"TEST-3\")
}

pub const REAL: Code = Code::new(\"VPN-13\");
";

        assert!(scan(source).balanced);
        assert_eq!(shipped_in(source), ["VPN-13"]);
        assert_eq!(tested_in(source), ["TEST-3"]);
    }

    /// A gate on a one-line item ends with the line.
    #[test]
    fn it_gates_a_declaration_written_on_one_line() {
        let source = "\
#[cfg(test)]
const ONLY: Code = Code::new(\"TEST-1\");

pub const REAL: Code = Code::new(\"VPN-5\");
";

        assert_eq!(shipped_in(source), ["VPN-5"]);
        assert_eq!(tested_in(source), ["TEST-1"]);
    }

    /// A gate on a module declaration names the file that module lives in.
    #[test]
    fn it_names_the_module_a_parent_gates_behind_its_tests() {
        let source = "#[cfg(test)]\npub(crate) mod fixtures;\n\nmod shipped;\n";

        assert_eq!(scan(source).gated, ["fixtures"]);
    }

    /// Only a module declaration names a module.
    #[test]
    fn it_takes_a_module_name_from_nothing_else() {
        assert_eq!(
            module_declared("mod fixtures;").as_deref(),
            Some("fixtures")
        );
        assert_eq!(
            module_declared("pub(crate) mod fixtures;").as_deref(),
            Some("fixtures")
        );
        assert_eq!(module_declared("mod ;"), None);
        assert_eq!(module_declared("mod not-a-name;"), None);
        assert_eq!(module_declared("use std::fs;"), None);
        assert_eq!(module_declared("mod fixtures"), None);
    }

    /// The file a gated module lives in, and the directory of the same name.
    #[test]
    fn it_gates_both_shapes_a_module_can_take() {
        let mut scans = std::collections::BTreeMap::new();
        scans.insert(
            PathBuf::from("lemonfiber/src/render.rs"),
            FileScan {
                gated: vec!["fixtures".to_owned()],
                ..FileScan::default()
            },
        );
        scans.insert(
            PathBuf::from("lemonfiber/src/lib.rs"),
            FileScan {
                gated: vec!["helpers".to_owned()],
                ..FileScan::default()
            },
        );

        let gated = super::gated_files(&scans);

        assert!(gated.contains(&PathBuf::from("lemonfiber/src/render/fixtures.rs")));
        assert!(gated.contains(&PathBuf::from("lemonfiber/src/render/fixtures")));
        assert!(gated.contains(&PathBuf::from("lemonfiber/src/helpers.rs")));
        assert!(gated.contains(&PathBuf::from("lemonfiber/src/helpers")));
    }

    /// A name a comment mentions is not a declaration.
    #[test]
    fn it_reads_nothing_out_of_a_comment() {
        let source = "\
// Code::new(\"COMMENT-1\") is not a declaration.
/// Neither is Code::new(\"COMMENT-2\").
/* Code::new(\"COMMENT-3\") /* nested */ still is not. */
pub const REAL: Code = Code::new(\"VPN-6\");
";

        assert_eq!(shipped_in(source), ["VPN-6"]);
    }

    /// A name a string quotes is not a declaration.
    ///
    /// This module quotes the call it looks for, so a reader that searched raw text
    /// would find its own source and report it as a code that cannot be read.
    #[test]
    fn it_reads_nothing_out_of_a_string() {
        let source = "\
const DECLARATION: &str = \"Code::new(\";
const RAW: &str = r#\"Code::new(\"RAW-1\")\"#;
pub const REAL: Code = Code::new(\"VPN-7\");
";

        assert_eq!(shipped_in(source), ["VPN-7"]);
        assert!(unreadable_in(source).is_empty());
    }

    /// A brace inside a string does not move the reader's depth.
    #[test]
    fn it_counts_no_brace_written_inside_a_string() {
        let source = "\
const FIXTURE: &str = r#\"{\"queue\":{\"slots\":[
    {\"name\":\"one\"}
]}}\"#;
const PROSE: &str = \"a { without its pair\";
const ESCAPED: &str = \"a \\\" quote and a {\";
pub const REAL: Code = Code::new(\"VPN-8\");
";

        assert!(scan(source).balanced);
        assert_eq!(shipped_in(source), ["VPN-8"]);
    }

    /// A string with no hashes closes at its first quote.
    #[test]
    fn it_closes_a_raw_string_that_opened_with_no_hashes() {
        let source = "const PLAIN: &str = r\"{\";\npub const REAL: Code = Code::new(\"VPN-9\");\n";

        assert!(scan(source).balanced);
        assert_eq!(shipped_in(source), ["VPN-9"]);
    }

    /// A name given as a raw string is still a name.
    #[test]
    fn it_reads_a_name_written_as_a_raw_string() {
        let source = "pub const ODD: Code = Code::new(r\"VPN-10\");\n";

        assert_eq!(shipped_in(source), ["VPN-10"]);
    }

    /// A brace written as a character does not move the reader's depth.
    #[test]
    fn it_counts_no_brace_written_as_a_character() {
        let source = "\
fn shape<'a>(open: char) -> &'a str {
    match open {
        '{' => \"opens\",
        '\\'' => \"quotes\",
        _ => \"other\",
    }
}
pub const REAL: Code = Code::new(\"VPN-11\");
";

        assert!(scan(source).balanced);
        assert_eq!(shipped_in(source), ["VPN-11"]);
    }

    /// A file whose braces do not balance is reported rather than trusted.
    #[test]
    fn it_reports_a_file_it_lost_its_place_in() {
        assert!(!scan("fn open() {\n").balanced);
    }

    /// A name the reader cannot read is reported rather than dropped.
    #[test]
    fn it_reports_a_declaration_whose_name_is_not_a_literal() {
        assert_eq!(unreadable_in("const A: Code = Code::new(NAME);\n"), [1]);
        assert_eq!(
            unreadable_in("const A: Code = Code::new(\n    NAME,\n);\n"),
            [2]
        );
        assert_eq!(unreadable_in("Code::new(Code::new(\"VPN-1\"));\n"), [1]);
    }

    /// A name split from its call by a line break is still read.
    #[test]
    fn it_reads_a_name_the_formatter_moved_to_its_own_line() {
        let source = "const A: Code = Code::new(\n    \"VPN-12\",\n);\n";

        assert_eq!(shipped_in(source), ["VPN-12"]);
    }

    /// A workspace of one file, written for the reader to fail on.
    fn scratch(name: &str, source: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("lemonfiber-codes-{}-{name}", std::process::id()));
        let src = root.join("crates").join("probe").join("src");
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&src);
        let _ = std::fs::write(src.join("lib.rs"), source);
        root
    }

    /// What one reading of a workspace complained about.
    fn complaints_about(name: &str, source: &str) -> Vec<String> {
        let root = scratch(name, source);
        let read = declared(&root);
        let _ = std::fs::remove_dir_all(&root);
        read.err().unwrap_or_default()
    }

    /// A file the reader lost its place in is named rather than trusted.
    #[test]
    fn it_names_the_file_whose_braces_did_not_balance() {
        let complaints = complaints_about("unbalanced", "fn open() {\n");

        assert_eq!(complaints.len(), 1);
        assert!(
            complaints.join(" ").contains("probe/src/lib.rs: braces"),
            "{complaints:?}"
        );
    }

    /// A name the reader cannot read is named, with the line it stands on.
    #[test]
    fn it_names_the_line_whose_code_it_could_not_read() {
        let complaints = complaints_about(
            "unreadable",
            "// a first line\nconst A: Code = Code::new(NAME);\n",
        );

        assert_eq!(complaints.len(), 1);
        assert!(
            complaints.join(" ").contains("probe/src/lib.rs:2:"),
            "{complaints:?}"
        );
    }

    /// A workspace the reader is happy with yields its codes.
    #[test]
    fn it_reads_a_whole_workspace_of_one_file() {
        let root = scratch("whole", "pub const A: Code = Code::new(\"PROBE-1\");\n");
        let read = declared(&root);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(
            read.unwrap_or_default(),
            vec![(PathBuf::from("probe/src/lib.rs"), "PROBE-1".to_owned())]
        );
    }

    /// Pointed at nothing, the reader says so rather than reporting no codes.
    #[test]
    fn it_refuses_a_root_that_holds_no_sources() {
        let read = declared(Path::new("/nowhere-that-holds-a-workspace"));
        let complaints = read.err().unwrap_or_default();

        assert_eq!(complaints.len(), 1);
        assert!(complaints.join(" ").contains("no Rust sources"));
    }
}
