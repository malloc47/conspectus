//! In-place edits of a JSON document's text, for the harness settings
//! file `hook init` and `hook remove` change (ADR 0208).
//!
//! [`splice`] rewrites a document so it parses to a new value while every
//! byte of every value that did not change stays as it was written: key
//! order, indentation, spacing, escapes, number spellings, line endings,
//! and whatever surrounds the document. It walks the value the text holds
//! beside the new one and edits only where they differ:
//!
//! - A member or element that is gone is cut out with the separator
//!   before it when it is its container's last item, else with the one
//!   after it, so appending an item and cutting it again gives back the
//!   bytes that were there.
//! - A new member or element is appended after its container's last
//!   item, on a line of its own at the first item's indentation when the
//!   items sit on lines of their own, else on the same line. An empty
//!   container that gains items opens them one level deeper than the line
//!   it is on.
//! - Elements of an array are matched by value first; each one left over
//!   is then paired, in order, with the new element most like it, and an
//!   element with no partner is cut or appended.
//! - A value whose type or scalar changed is replaced where it stands.
//!
//! New text follows its container: pretty-printed by the document's own
//! indentation unit (two spaces when it shows none) where the items spread
//! over lines, on one line where they do not. Keys of a new object come in
//! the order `serde_json` keeps them.
//!
//! The text must be strict JSON. The result is parsed back and compared
//! with the new value, so a splice yields exactly that value or fails.

use std::collections::BTreeSet;
use std::io;

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::ser::{Formatter, PrettyFormatter};
use serde_json::{Map, Value};

/// How deep the scanner follows nesting: `serde_json`'s own limit, which
/// the text has already passed.
const MAX_DEPTH: usize = 128;

/// The indentation unit of a document that shows none.
const DEFAULT_UNIT: &str = "  ";

/// Rewrite `text`, a strict JSON document, so it parses to `new`, editing
/// only the values that differ from those it holds.
pub(super) fn splice(text: &str, new: &Value) -> Result<String> {
    let old: Value = serde_json::from_str(text).context("the text is not valid JSON")?;
    if old == *new {
        return Ok(text.to_string());
    }
    let root = Scanner { text, pos: 0 }.document()?;
    let mut editor = Editor::new(text, &root);
    editor.value(&old, new, &root, Style::Pretty)?;
    let spliced = apply(text, editor.edits)?;
    let check: Value =
        serde_json::from_str(&spliced).context("the edited text is not valid JSON")?;
    ensure!(
        check == *new,
        "the edited text does not hold the intended value"
    );
    Ok(spliced)
}

/// Where one value sits in the text, from its first byte to just past its
/// last.
struct Node {
    start: usize,
    end: usize,
    body: Body,
}

enum Body {
    Scalar,
    Object(Vec<Member>),
    Array(Vec<Node>),
}

struct Member {
    key: String,
    /// The key's opening quote.
    start: usize,
    /// Just past the key's closing quote.
    key_end: usize,
    value: Node,
}

impl Node {
    /// Each item's span: an element's own, or a member's from its key to
    /// the end of its value.
    fn items(&self) -> Vec<(usize, usize)> {
        match &self.body {
            Body::Scalar => Vec::new(),
            Body::Object(members) => members
                .iter()
                .map(|member| (member.start, member.value.end))
                .collect(),
            Body::Array(elements) => elements
                .iter()
                .map(|element| (element.start, element.end))
                .collect(),
        }
    }

    /// Each item's value: an element, or a member's value.
    fn values(&self) -> Vec<&Node> {
        match &self.body {
            Body::Scalar => Vec::new(),
            Body::Object(members) => members.iter().map(|member| &member.value).collect(),
            Body::Array(elements) => elements.iter().collect(),
        }
    }

    fn is_filled_container(&self) -> bool {
        match &self.body {
            Body::Scalar => false,
            Body::Object(members) => !members.is_empty(),
            Body::Array(elements) => !elements.is_empty(),
        }
    }
}

/// Finds where each value of a strict JSON text sits.
struct Scanner<'a> {
    text: &'a str,
    pos: usize,
}

impl Scanner<'_> {
    fn document(mut self) -> Result<Node> {
        let root = self.value(0)?;
        self.skip_space();
        ensure!(
            self.pos == self.text.len(),
            "unexpected text after the document at byte {}",
            self.pos
        );
        Ok(root)
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        if found {
            self.pos += 1;
        }
        found
    }

    fn expect(&mut self, byte: u8) -> Result<()> {
        ensure!(
            self.eat(byte),
            "expected `{}` at byte {}",
            char::from(byte),
            self.pos
        );
        Ok(())
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<Node> {
        ensure!(depth < MAX_DEPTH, "the document nests too deeply");
        self.skip_space();
        let start = self.pos;
        let body = match self.peek() {
            Some(b'{') => Body::Object(self.members(depth)?),
            Some(b'[') => Body::Array(self.elements(depth)?),
            Some(b'"') => {
                self.string()?;
                Body::Scalar
            }
            Some(_) => {
                self.bare()?;
                Body::Scalar
            }
            None => bail!("the document ends early"),
        };
        Ok(Node {
            start,
            end: self.pos,
            body,
        })
    }

    fn string(&mut self) -> Result<()> {
        self.expect(b'"')?;
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some(b'\\') => self.pos += 2,
                Some(_) => self.pos += 1,
                None => bail!("a string is not closed"),
            }
        }
    }

    /// A number, `true`, `false`, or `null`.
    fn bare(&mut self) -> Result<()> {
        let start = self.pos;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
        ) {
            self.pos += 1;
        }
        ensure!(self.pos > start, "unexpected text at byte {start}");
        Ok(())
    }

    fn members(&mut self, depth: usize) -> Result<Vec<Member>> {
        self.expect(b'{')?;
        let mut members = Vec::new();
        self.skip_space();
        if self.eat(b'}') {
            return Ok(members);
        }
        loop {
            self.skip_space();
            let start = self.pos;
            self.string()?;
            let key_end = self.pos;
            let quoted = self
                .text
                .get(start..key_end)
                .context("a key ends inside a character")?;
            let key = serde_json::from_str(quoted).context("a key is not a valid string")?;
            self.skip_space();
            self.expect(b':')?;
            let value = self.value(depth + 1)?;
            members.push(Member {
                key,
                start,
                key_end,
                value,
            });
            self.skip_space();
            if !self.eat(b',') {
                self.expect(b'}')?;
                return Ok(members);
            }
        }
    }

    fn elements(&mut self, depth: usize) -> Result<Vec<Node>> {
        self.expect(b'[')?;
        let mut elements = Vec::new();
        self.skip_space();
        if self.eat(b']') {
            return Ok(elements);
        }
        loop {
            elements.push(self.value(depth + 1)?);
            self.skip_space();
            if !self.eat(b',') {
                self.expect(b']')?;
                return Ok(elements);
            }
        }
    }
}

/// How new text is laid out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Style {
    /// Over lines, indented by the document's unit.
    Pretty,
    /// On one line, a space after each comma and colon.
    Spaced,
    /// On one line, with no spaces.
    Compact,
}

/// How a container lays out its items, read from those it has.
struct Shape {
    /// What goes between the opening bracket and the first item when an
    /// emptied container is filled again.
    first: String,
    /// What goes between a comma and the item after it.
    lead: String,
    /// What goes between the last item and the closing bracket when the
    /// container is filled again.
    close: String,
    /// What goes between a new member's key and its value.
    colon: String,
    /// How a new item is written.
    style: Style,
    /// The indentation inside a new pretty-printed item.
    indent: String,
}

/// One replacement of the bytes `start..end`.
struct Edit {
    start: usize,
    end: usize,
    text: String,
}

/// Collects the edits that turn the old value's text into the new one's.
struct Editor<'a> {
    text: &'a str,
    newline: &'static str,
    unit: String,
    edits: Vec<Edit>,
}

impl<'a> Editor<'a> {
    /// The line ending is CRLF when the text uses it anywhere; the unit is
    /// the indentation of the root's first item when that sits on a line
    /// of its own.
    fn new(text: &'a str, root: &Node) -> Self {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let unit = root
            .items()
            .first()
            .and_then(|&(first, _)| {
                let (_, indent) = text[root.start + 1..first].rsplit_once('\n')?;
                (!indent.is_empty()).then(|| indent.to_string())
            })
            .unwrap_or_else(|| DEFAULT_UNIT.to_string());
        Self {
            text,
            newline,
            unit,
            edits: Vec::new(),
        }
    }

    fn edit(&mut self, start: usize, end: usize, text: String) {
        self.edits.push(Edit { start, end, text });
    }

    /// Edit `node`, whose text holds `old`, to hold `new`. `style` is how
    /// its container writes a new item.
    fn value(&mut self, old: &Value, new: &Value, node: &Node, style: Style) -> Result<()> {
        if old == new {
            return Ok(());
        }
        match (old, new, &node.body) {
            (Value::Object(old), Value::Object(new), Body::Object(members)) => {
                self.object(old, new, node, members, style)
            }
            (Value::Array(old), Value::Array(new), Body::Array(elements)) => {
                self.array(old, new, node, elements, style)
            }
            _ => self.replace(node, new, style),
        }
    }

    fn object(
        &mut self,
        old: &Map<String, Value>,
        new: &Map<String, Value>,
        node: &Node,
        members: &[Member],
        style: Style,
    ) -> Result<()> {
        let mut keys = BTreeSet::new();
        for member in members {
            ensure!(
                keys.insert(member.key.as_str()),
                "the key `{}` appears twice in one object",
                member.key
            );
        }
        let shape = self.shape(node, style);
        let mut dropped = Vec::new();
        for (index, member) in members.iter().enumerate() {
            match (old.get(&member.key), new.get(&member.key)) {
                (Some(was), Some(now)) => self.value(was, now, &member.value, shape.style)?,
                _ => dropped.push(index),
            }
        }
        let mut added = Vec::new();
        for (key, value) in new.iter().filter(|(key, _)| !old.contains_key(*key)) {
            let rendered = self.render(value, shape.style, &shape.indent)?;
            added.push(format!(
                "{}{}{rendered}",
                serde_json::to_string(key)?,
                shape.colon
            ));
        }
        self.rearrange(node, &shape, &dropped, &added);
        Ok(())
    }

    fn array(
        &mut self,
        old: &[Value],
        new: &[Value],
        node: &Node,
        elements: &[Node],
        style: Style,
    ) -> Result<()> {
        let steps = align(old, new);
        // A new element before one that stays would need an insertion in
        // the middle; such an array is written anew.
        let first_add = steps.iter().position(|step| matches!(step, Step::Add(_)));
        let last_kept = steps
            .iter()
            .rposition(|step| matches!(step, Step::Same(..) | Step::Change(..)));
        if let (Some(add), Some(kept)) = (first_add, last_kept)
            && add < kept
        {
            return self.replace(node, &Value::Array(new.to_vec()), style);
        }
        let shape = self.shape(node, style);
        let mut dropped = Vec::new();
        let mut added = Vec::new();
        for step in steps {
            match step {
                Step::Same(..) => {}
                Step::Change(was, now) => {
                    self.value(&old[was], &new[now], &elements[was], shape.style)?;
                }
                Step::Drop(was) => dropped.push(was),
                Step::Add(now) => added.push(self.render(&new[now], shape.style, &shape.indent)?),
            }
        }
        self.rearrange(node, &shape, &dropped, &added);
        Ok(())
    }

    /// Cut the `dropped` items of `node` and append the `added` texts.
    fn rearrange(&mut self, node: &Node, shape: &Shape, dropped: &[usize], added: &[String]) {
        let items = node.items();
        let (open, close) = (node.start + 1, node.end - 1);
        if dropped.len() == items.len() {
            if !added.is_empty() {
                let joined = added.join(&format!(",{}", shape.lead));
                self.edit(
                    open,
                    close,
                    format!("{}{joined}{}", shape.first, shape.close),
                );
            } else if !items.is_empty() {
                self.edit(open, close, String::new());
            }
            return;
        }
        for (first, last) in runs(dropped) {
            if last + 1 == items.len() {
                // The last items go with the separator before them; one
                // item at least stays before them.
                self.edit(items[first - 1].1, items[last].1, String::new());
            } else {
                self.edit(items[first].0, items[last + 1].0, String::new());
            }
        }
        if !added.is_empty() {
            let at = items[items.len() - 1].1;
            let text = added
                .iter()
                .map(|item| format!(",{}{item}", shape.lead))
                .collect();
            self.edit(at, at, text);
        }
    }

    /// Replace `node` whole with `new`, laid out as `node` was: over
    /// lines when it spanned them, on one line when it shows how, and
    /// otherwise as its container writes items.
    fn replace(&mut self, node: &Node, new: &Value, style: Style) -> Result<()> {
        let was = &self.text[node.start..node.end];
        let style = match node.body {
            Body::Scalar => style,
            _ if was.contains('\n') => Style::Pretty,
            _ => one_line_style(was).unwrap_or(style),
        };
        let indent = line_indent(self.text, node.start).to_string();
        let text = self.render(new, style, &indent)?;
        self.edit(node.start, node.end, text);
        Ok(())
    }

    /// How `node`'s items are laid out, or, when it has none, how a
    /// container in `parent`'s style lays them out.
    fn shape(&self, node: &Node, parent: Style) -> Shape {
        let text = self.text;
        let items = node.items();
        let colon = match &node.body {
            Body::Object(members) => members
                .first()
                .map(|member| text[member.key_end..member.value.start].to_string()),
            _ => None,
        };
        let (Some(&(first_start, first_end)), Some(&(_, last_end))) = (items.first(), items.last())
        else {
            let own = line_indent(text, node.start);
            let indent = format!("{own}{}", self.unit);
            let (first, lead, close, colon) = match parent {
                Style::Pretty => (
                    format!("{}{indent}", self.newline),
                    format!("{}{indent}", self.newline),
                    format!("{}{own}", self.newline),
                    ": ",
                ),
                Style::Spaced => (String::new(), " ".to_string(), String::new(), ": "),
                Style::Compact => (String::new(), String::new(), String::new(), ":"),
            };
            return Shape {
                first,
                lead,
                close,
                colon: colon.to_string(),
                style: parent,
                indent,
            };
        };
        let first = &text[node.start + 1..first_start];
        let close = text[last_end..node.end - 1].to_string();
        if let Some((_, indent)) = first.rsplit_once('\n') {
            // Items on lines of their own are pretty-printed, unless every
            // one that holds something is kept on one line.
            let filled: Vec<&str> = node
                .values()
                .into_iter()
                .filter(|value| value.is_filled_container())
                .map(|value| &text[value.start..value.end])
                .collect();
            let style = match filled.first() {
                Some(one) if filled.iter().all(|value| !value.contains('\n')) => {
                    one_line_style(one).unwrap_or(Style::Spaced)
                }
                _ => Style::Pretty,
            };
            return Shape {
                first: first.to_string(),
                lead: format!("{}{indent}", self.newline),
                close,
                colon: colon.unwrap_or_else(|| ": ".to_string()),
                style,
                indent: indent.to_string(),
            };
        }
        let after_comma = items.get(1).map(|&(second, _)| {
            let between = &text[first_end..second];
            between.split_once(',').map_or("", |(_, after)| after)
        });
        let spaced = after_comma.is_some_and(|after| !after.is_empty())
            || !first.is_empty()
            || colon.as_deref().is_some_and(|colon| colon.contains(' '));
        let style = if spaced {
            Style::Spaced
        } else {
            Style::Compact
        };
        let lead = after_comma
            .unwrap_or(if spaced { " " } else { "" })
            .to_string();
        Shape {
            first: first.to_string(),
            lead,
            close,
            colon: colon.unwrap_or_else(|| if spaced { ": " } else { ":" }.to_string()),
            style,
            indent: line_indent(text, node.start).to_string(),
        }
    }

    /// `value` written in `style`; a pretty one's inner lines start with
    /// `indent`.
    fn render(&self, value: &Value, style: Style, indent: &str) -> Result<String> {
        let mut out = Vec::new();
        match style {
            Style::Pretty => {
                let formatter = PrettyFormatter::with_indent(self.unit.as_bytes());
                value.serialize(&mut serde_json::Serializer::with_formatter(
                    &mut out, formatter,
                ))?;
            }
            Style::Spaced => {
                value.serialize(&mut serde_json::Serializer::with_formatter(
                    &mut out, Spaced,
                ))?;
            }
            Style::Compact => value.serialize(&mut serde_json::Serializer::new(&mut out))?,
        }
        let text = String::from_utf8(out)?;
        Ok(match style {
            Style::Pretty => text.replace('\n', &format!("{}{indent}", self.newline)),
            Style::Spaced | Style::Compact => text,
        })
    }
}

/// How one-line text is spaced: `Spaced` with a space after a colon or
/// comma, `Compact` with a colon or comma and no such space, and nothing
/// when it has neither, as `[1]` or `{}`.
fn one_line_style(text: &str) -> Option<Style> {
    if text.contains(": ") || text.contains(", ") {
        Some(Style::Spaced)
    } else if text.contains([':', ',']) {
        Some(Style::Compact)
    } else {
        None
    }
}

/// The spaces and tabs that start the line `at` is on.
fn line_indent(text: &str, at: usize) -> &str {
    let line = text[..at].rfind('\n').map_or(0, |newline| newline + 1);
    let rest = &text[line..at];
    &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
}

/// Consecutive runs in ascending indices, as inclusive bounds.
fn runs(indices: &[usize]) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &index in indices {
        match runs.last_mut() {
            Some((_, last)) if *last + 1 == index => *last = index,
            _ => runs.push((index, index)),
        }
    }
    runs
}

/// `edits` applied to `text`. They must not overlap.
fn apply(text: &str, mut edits: Vec<Edit>) -> Result<String> {
    edits.sort_by_key(|edit| (edit.start, edit.end));
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        ensure!(edit.start >= at, "two edits of the text overlap");
        out.push_str(&text[at..edit.start]);
        out.push_str(&edit.text);
        at = edit.end;
    }
    out.push_str(&text[at..]);
    Ok(out)
}

/// One element's fate in an array edit: old and new indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Same(usize, usize),
    Change(usize, usize),
    Drop(usize),
    Add(usize),
}

/// Match `old`'s elements to `new`'s, in order: equal elements first (a
/// longest common subsequence), then, between two of those, a run of
/// equal length pairs off element by element, and runs of unequal length
/// pair the elements most alike.
fn align(old: &[Value], new: &[Value]) -> Vec<Step> {
    let (rows, columns) = (old.len(), new.len());
    let mut common = vec![vec![0usize; columns + 1]; rows + 1];
    for i in (0..rows).rev() {
        for j in (0..columns).rev() {
            common[i][j] = if old[i] == new[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let mut steps = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut gap_i, mut gap_j) = (0, 0);
    while i < rows && j < columns {
        if old[i] == new[j] {
            pair_gap(&mut steps, old, new, gap_i..i, gap_j..j);
            steps.push(Step::Same(i, j));
            i += 1;
            j += 1;
            (gap_i, gap_j) = (i, j);
        } else if common[i + 1][j] >= common[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pair_gap(&mut steps, old, new, gap_i..rows, gap_j..columns);
    steps
}

/// The steps for `old[olds]` against `new[news]`, between two equal pairs.
fn pair_gap(
    steps: &mut Vec<Step>,
    old: &[Value],
    new: &[Value],
    olds: std::ops::Range<usize>,
    news: std::ops::Range<usize>,
) {
    if olds.len() == news.len() {
        steps.extend(olds.zip(news).map(|(was, now)| Step::Change(was, now)));
        return;
    }
    let (rows, columns) = (olds.len(), news.len());
    let score = |i: usize, j: usize| similarity(&old[olds.start + i], &new[news.start + j]);
    // best[i][j]: the most likeness pairing old[i..] with new[j..] reaches.
    let mut best = vec![vec![0usize; columns + 1]; rows + 1];
    for i in (0..rows).rev() {
        for j in (0..columns).rev() {
            let paired = match score(i, j) {
                0 => 0,
                likeness => best[i + 1][j + 1] + likeness,
            };
            best[i][j] = best[i + 1][j].max(best[i][j + 1]).max(paired);
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < rows || j < columns {
        if i < rows && j < columns {
            let likeness = score(i, j);
            if likeness > 0 && best[i][j] == best[i + 1][j + 1] + likeness {
                steps.push(Step::Change(olds.start + i, news.start + j));
                i += 1;
                j += 1;
                continue;
            }
        }
        if i < rows && (j == columns || best[i][j] == best[i + 1][j]) {
            steps.push(Step::Drop(olds.start + i));
            i += 1;
        } else {
            steps.push(Step::Add(news.start + j));
            j += 1;
        }
    }
}

/// How much of `a` survives in `b`: the scalars two objects share at the
/// same keys, or two arrays share as equal elements. Values of different
/// kinds, or unequal scalars, share nothing.
fn similarity(a: &Value, b: &Value) -> usize {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => a
            .iter()
            .filter_map(|(key, was)| {
                let now = b.get(key)?;
                Some(if was == now {
                    weight(was)
                } else {
                    similarity(was, now)
                })
            })
            .sum(),
        (Value::Array(a), Value::Array(b)) => a
            .iter()
            .filter(|element| b.contains(element))
            .map(weight)
            .sum(),
        (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => 0,
        _ => usize::from(a == b),
    }
}

/// The scalars inside `value`, counting an empty container as one.
fn weight(value: &Value) -> usize {
    match value {
        Value::Object(members) => members.values().map(weight).sum::<usize>().max(1),
        Value::Array(elements) => elements.iter().map(weight).sum::<usize>().max(1),
        _ => 1,
    }
}

/// One line with a space after each comma and colon:
/// `{"a": 1, "b": [2, 3]}`.
struct Spaced;

impl Formatter for Spaced {
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            writer.write_all(b", ")
        }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }
}

#[cfg(test)]
#[path = "json_splice_tests.rs"]
mod tests;
