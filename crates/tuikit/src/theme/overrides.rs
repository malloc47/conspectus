//! Applying a configuration table to a theme.
//!
//! The host reads its configuration file and hands the theme a flat list
//! of keys and values; the theme parses each value with the style-spec
//! grammar, resolves aliases, and stores the results. Nothing here reads a
//! file or knows the host's configuration format, so the same walk serves
//! any host.

use std::collections::BTreeMap;
use std::fmt;

use super::{Theme, TokenKind, TokenValue, parse_color, parse_modifier, parse_style_spec};

/// A configuration value as the host read it, before the theme parses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverrideValue<'a> {
    /// Text: a color, a modifier list, or a style spec.
    Text(&'a str),
    /// An integer: a width.
    Integer(i64),
    /// Anything else the host's format can hold, such as a table, a list,
    /// or a flag. No key accepts it.
    Other,
}

/// Why one entry of a table did not apply as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverrideProblem {
    /// The key is not a token, a registered name, or an alias.
    UnknownKey,
    /// The key takes text and the value is not text.
    NotText,
    /// The key takes a width and the value is not an integer of at least
    /// `min`.
    NotWidth {
        /// The smallest width the key accepts.
        min: usize,
    },
    /// The grammar rejected the text; the reason says why.
    Invalid(String),
    /// The key is an alias whose value lost: `by`, another key in the same
    /// table, set `target` to something else, and this key set nothing.
    Shadowed {
        /// The token or name both keys set.
        target: String,
        /// The key whose value `target` took.
        by: String,
    },
}

impl fmt::Display for OverrideProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey => f.write_str("unknown key"),
            Self::NotText => f.write_str("must be a string"),
            Self::NotWidth { min } => write!(f, "must be an integer of at least {min}"),
            Self::Invalid(reason) => f.write_str(reason),
            Self::Shadowed { target, by } => write!(
                f,
                "is ignored: it sets `{target}`, which `{by}` in the same table sets to \
                 something else"
            ),
        }
    }
}

/// One entry that did not apply as written, and why. The entry's other
/// targets, and every other entry, still apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverrideError {
    /// The entry's key, as the table spelled it.
    pub key: String,
    /// What went wrong.
    pub problem: OverrideProblem,
}

impl Theme {
    /// Applies one configuration table: each entry is a key (a token, a
    /// registered name, or an alias) and its value. Returns the entries
    /// that did not apply as written, in table order; each keeps its
    /// target's current value.
    ///
    /// Precedence is per target and per table: the target's own key wins,
    /// then its aliases in registration order (see
    /// [`Theme::register_alias`]). An entry whose value cannot be parsed is
    /// reported and treated as absent, so the next key in line applies. A
    /// key given twice uses its last value. Tables applied one after
    /// another layer: a later table overrides an earlier one whichever key
    /// each used.
    pub fn apply_overrides<'a, I>(&mut self, entries: I) -> Vec<OverrideError>
    where
        I: IntoIterator<Item = (&'a str, OverrideValue<'a>)>,
    {
        let mut table: Vec<(&str, OverrideValue<'_>)> = Vec::new();
        for (key, value) in entries {
            match table.iter_mut().find(|(seen, _)| *seen == key) {
                Some(entry) => entry.1 = value,
                None => table.push((key, value)),
            }
        }
        let index: BTreeMap<&str, usize> = table
            .iter()
            .enumerate()
            .map(|(i, (key, _))| (*key, i))
            .collect();

        let mut problems: Vec<Option<OverrideProblem>> = vec![None; table.len()];
        let mut applied = vec![false; table.len()];
        let mut targets: Vec<String> = Vec::new();
        for (i, (key, _)) in table.iter().enumerate() {
            let feeds = self.targets_of(key);
            if feeds.is_empty() {
                problems[i] = Some(OverrideProblem::UnknownKey);
            }
            for target in feeds {
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }

        let mut winners: BTreeMap<String, (usize, TokenValue)> = BTreeMap::new();
        for target in &targets {
            let Some(kind) = self.kind_of(target) else {
                continue;
            };
            for i in self
                .candidates_for(target)
                .iter()
                .filter_map(|key| index.get(key.as_str()).copied())
            {
                if problems[i].is_some() {
                    continue;
                }
                match parse(kind, table[i].1) {
                    Ok(value) => {
                        self.set(target, value);
                        applied[i] = true;
                        winners.insert(target.clone(), (i, value));
                        break;
                    }
                    Err(problem) => problems[i] = Some(problem),
                }
            }
        }

        for (i, (key, value)) in table.iter().enumerate() {
            if applied[i] || problems[i].is_some() {
                continue;
            }
            for target in self.targets_of(key) {
                let (Some(kind), Some((winner, won))) =
                    (self.kind_of(&target), winners.get(&target))
                else {
                    continue;
                };
                if parse(kind, *value).is_ok_and(|mine| mine != *won) {
                    problems[i] = Some(OverrideProblem::Shadowed {
                        target: target.clone(),
                        by: table[*winner].0.to_string(),
                    });
                    break;
                }
            }
        }

        table
            .iter()
            .zip(problems)
            .filter_map(|((key, _), problem)| {
                problem.map(|problem| OverrideError {
                    key: (*key).to_string(),
                    problem,
                })
            })
            .collect()
    }

    /// The tokens and names `key` sets: its own, then each alias target.
    fn targets_of(&self, key: &str) -> Vec<String> {
        let own = self.value(key).map(|_| key.to_string());
        own.into_iter()
            .chain(
                self.aliases()
                    .iter()
                    .filter(|alias| alias.key == key)
                    .map(|alias| alias.target.clone()),
            )
            .collect()
    }

    /// The keys that may set `target`, highest precedence first.
    fn candidates_for(&self, target: &str) -> Vec<String> {
        std::iter::once(target.to_string())
            .chain(
                self.aliases()
                    .iter()
                    .filter(|alias| alias.target == target)
                    .map(|alias| alias.key.clone()),
            )
            .collect()
    }
}

/// Parses `value` as a value of `kind`.
fn parse(kind: TokenKind, value: OverrideValue<'_>) -> Result<TokenValue, OverrideProblem> {
    match (kind, value) {
        (TokenKind::Width { min }, OverrideValue::Integer(width)) => usize::try_from(width)
            .ok()
            .filter(|width| *width >= min)
            .map(TokenValue::Width)
            .ok_or(OverrideProblem::NotWidth { min }),
        (TokenKind::Width { min }, _) => Err(OverrideProblem::NotWidth { min }),
        (TokenKind::Color, OverrideValue::Text(raw)) => parse_color(raw)
            .map(TokenValue::Color)
            .map_err(OverrideProblem::Invalid),
        (TokenKind::Modifier, OverrideValue::Text(raw)) => parse_modifier(raw)
            .map(TokenValue::Modifier)
            .map_err(OverrideProblem::Invalid),
        (TokenKind::Style, OverrideValue::Text(raw)) => parse_style_spec(raw)
            .map(TokenValue::Style)
            .map_err(OverrideProblem::Invalid),
        (_, OverrideValue::Integer(_) | OverrideValue::Other) => Err(OverrideProblem::NotText),
    }
}
