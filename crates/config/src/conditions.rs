//! The condition maps of the file: `queue_conditions`, `merge_conditions`,
//! and the `conditions` of a profile or of a priority rule.

use std::fmt;

use cihub_conditions::{Attribute, Condition, Conditions, Expr, Operator};
use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_saphyr::Spanned;

use crate::{ConfigError, Position};

/// A YAML node of a condition map, with the position of each part.
pub(crate) enum RawNode {
    Null,
    Scalar(String),
    /// A number as YAML reads it; the value of a condition is its text in
    /// the file, so that a check `3.10` stays `3.10`.
    Number(String),
    List(Vec<Spanned<RawNode>>),
    Map(Vec<(Spanned<String>, Spanned<RawNode>)>),
}

const CHECK_STATES: &str = "a map of check states: success, neutral, skipped, failure, pending";

type Entries = [(Spanned<String>, Spanned<RawNode>)];

/// Reads the condition maps of `text`, the file.
#[derive(Clone, Copy)]
pub(crate) struct Reader<'a> {
    pub text: &'a str,
}

fn position<T>(node: &Spanned<T>) -> Position {
    Position::of(&node.referenced)
}

fn error<T>(node: &Spanned<T>, message: impl Into<String>) -> ConfigError {
    ConfigError::parsing(Some(position(node)), message)
}

fn map<'a>(node: &'a Spanned<RawNode>, expected: &str) -> Result<&'a Entries, ConfigError> {
    match &node.value {
        RawNode::Map(entries) => Ok(entries),
        _ => Err(error(node, format!("expected {expected}"))),
    }
}

/// A map inside a condition: an empty one would hold, or fail, without a
/// word to the person who wrote it.
fn filled<'a>(node: &'a Spanned<RawNode>, expected: &str) -> Result<&'a Entries, ConfigError> {
    let entries = map(node, expected)?;
    if entries.is_empty() {
        return Err(error(
            node,
            format!("expected {expected}, not an empty map"),
        ));
    }

    Ok(entries)
}

/// `merge_conditions` accept only required checks: `exprs` come from `key`.
fn only_required_checks(key: &Spanned<String>, exprs: Vec<Expr>) -> Result<Vec<Expr>, ConfigError> {
    let refused = exprs
        .iter()
        .find(|expr| !matches!(expr, Expr::Condition(c) if c.required_check().is_some()));
    if let Some(expr) = refused {
        return Err(error(
            key,
            format!("merge_conditions accept only `checks.success`, not `{expr}`"),
        ));
    }

    Ok(exprs)
}

impl<'a> Reader<'a> {
    /// The conditions of a map; none without a map.
    pub(crate) fn conditions(
        self,
        node: Option<&Spanned<RawNode>>,
    ) -> Result<Conditions, ConfigError> {
        let Some(node) = node else {
            return Ok(Conditions::default());
        };

        self.entries(map(node, "a map of conditions")?)
            .map(Conditions)
    }

    /// The conditions of a profile or of a priority rule: a map that holds
    /// something, as an empty one would match every PR without a word.
    pub(crate) fn selecting_conditions(
        self,
        node: &Spanned<RawNode>,
    ) -> Result<Conditions, ConfigError> {
        self.entries(filled(node, "a map of conditions")?)
            .map(Conditions)
    }

    /// `merge_conditions` accept only `checks.success` for now.
    pub(crate) fn merge_conditions(
        self,
        node: Option<&Spanned<RawNode>>,
    ) -> Result<Conditions, ConfigError> {
        let Some(node) = node else {
            return Ok(Conditions::default());
        };

        let mut conditions = Vec::new();
        for (key, value) in map(node, "a map of conditions")? {
            let exprs = match key.value.as_str() {
                "checks" => self.merge_checks(value)?,
                _ => only_required_checks(key, self.entry(key, value)?)?,
            };
            conditions.extend(exprs);
        }
        Ok(Conditions(conditions))
    }

    /// The states of `checks`, each refused at its own key.
    fn merge_checks(self, value: &Spanned<RawNode>) -> Result<Vec<Expr>, ConfigError> {
        let mut exprs = Vec::new();
        for (state, value) in filled(value, CHECK_STATES)? {
            exprs.extend(self.merge_state(state, value)?);
        }
        Ok(exprs)
    }

    /// One state of `checks`; under `success`, each operator is refused at its own key.
    fn merge_state(
        self,
        state: &Spanned<String>,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<Expr>, ConfigError> {
        let name = format!("checks.{}", state.value);
        if state.value != "success" || !matches!(value.value, RawNode::Map(_)) {
            return only_required_checks(state, self.attribute(&name, state, value)?);
        }

        let mut exprs = Vec::new();
        for (key, value) in filled(value, &format!("the operators of `{name}`"))? {
            let built = self.operator_entry(Attribute::CheckSuccess, key, value)?;
            exprs.extend(only_required_checks(key, built)?);
        }
        Ok(exprs)
    }

    fn entries(self, entries: &Entries) -> Result<Vec<Expr>, ConfigError> {
        let mut exprs = Vec::new();
        for (key, value) in entries {
            exprs.extend(self.entry(key, value)?);
        }
        Ok(exprs)
    }

    fn entry(
        self,
        key: &Spanned<String>,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<Expr>, ConfigError> {
        let block: fn(Vec<Expr>) -> Expr = match key.value.as_str() {
            "all" => Expr::All,
            "any" => Expr::Any,
            "none" => Expr::None,
            "checks" => return self.checks(value),
            name if name.contains('.') => {
                return Err(error(key, format!("unknown attribute `{name}`")));
            }
            name => return self.attribute(name, key, value),
        };

        let maps = match &value.value {
            RawNode::List(maps) if !maps.is_empty() => maps,
            _ => {
                return Err(error(
                    value,
                    format!("`{}` takes a list of condition maps", key.value),
                ));
            }
        };
        let list = maps
            .iter()
            .map(|node| self.group(node))
            .collect::<Result<_, _>>()?;

        Ok(vec![block(list)])
    }

    /// One map of a block: its only condition, or all of them.
    fn group(self, node: &Spanned<RawNode>) -> Result<Expr, ConfigError> {
        let mut exprs = self.entries(filled(node, "a map of conditions")?)?;
        if exprs.len() == 1
            && let Some(expr) = exprs.pop()
        {
            return Ok(expr);
        }

        Ok(Expr::All(exprs))
    }

    fn checks(self, value: &Spanned<RawNode>) -> Result<Vec<Expr>, ConfigError> {
        let mut exprs = Vec::new();
        for (state, value) in filled(value, CHECK_STATES)? {
            let name = format!("checks.{}", state.value);
            exprs.extend(self.attribute(&name, state, value)?);
        }
        Ok(exprs)
    }

    fn attribute(
        self,
        name: &str,
        key: &Spanned<String>,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<Expr>, ConfigError> {
        let attribute = Attribute::from_name(name)
            .ok_or_else(|| error(key, format!("unknown attribute `{name}`")))?;
        let RawNode::Map(_) = &value.value else {
            return self.condition(attribute, Operator::Is, value);
        };

        let entries = filled(value, &format!("the operators of `{name}`"))?;
        let mut exprs = Vec::new();
        for (key, value) in entries {
            exprs.extend(self.operator_entry(attribute, key, value)?);
        }
        refuse_empty_range(&format!("{attribute}.max"), entries, &exprs)?;
        Ok(exprs)
    }

    /// One operator key of an attribute, with its values.
    fn operator_entry(
        self,
        attribute: Attribute,
        key: &Spanned<String>,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<Expr>, ConfigError> {
        match (key.value.as_str(), &value.value) {
            ("count", RawNode::Map(_)) => self.count(attribute, value),
            (name, _) => {
                let operator = operator(attribute, Operator::from_key(name), key)?;
                self.condition(attribute, operator, value)
            }
        }
    }

    /// `count` with `min` and `max`.
    fn count(
        self,
        attribute: Attribute,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<Expr>, ConfigError> {
        let entries = filled(value, "`min` or `max`")?;
        let mut exprs = Vec::new();
        for (bound, value) in entries {
            let operator = match bound.value.as_str() {
                "min" => Operator::CountMin,
                "max" => Operator::CountMax,
                _ => {
                    return Err(error(
                        bound,
                        format!(
                            "unknown key `{}` under `count`: expected min or max",
                            bound.value
                        ),
                    ));
                }
            };
            exprs.extend(self.condition(attribute, operator, value)?);
        }
        refuse_empty_range(&format!("{attribute}.count.max"), entries, &exprs)?;
        Ok(exprs)
    }

    fn condition(
        self,
        attribute: Attribute,
        operator: Operator,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<Expr>, ConfigError> {
        let nodes = self.scalars(value)?;
        let values: Vec<String> = nodes.iter().map(|(text, _)| text.clone()).collect();

        Condition::build(attribute, operator, &values)
            .map(|conditions| conditions.into_iter().map(Expr::Condition).collect())
            .map_err(|failure| {
                let at = failure
                    .value
                    .and_then(|n| nodes.get(n))
                    .map_or(value, |(_, node)| node);
                let path = match operator.key() {
                    Some(key) => format!("{attribute}.{key}"),
                    None => attribute.to_string(),
                };
                error(at, format!("{path}: {}", failure.kind))
            })
    }

    /// The values of a condition: a scalar, or a list of scalars.
    fn scalars(
        self,
        value: &Spanned<RawNode>,
    ) -> Result<Vec<(String, &Spanned<RawNode>)>, ConfigError> {
        let not_a_value = |node| error(node, "expected a value or a list of values");
        match &value.value {
            RawNode::List(list) => list
                .iter()
                .map(|node| {
                    self.scalar(node)
                        .map(|text| (text, node))
                        .ok_or_else(|| not_a_value(node))
                })
                .collect(),
            RawNode::Null => Ok(Vec::new()),
            _ => self
                .scalar(value)
                .map(|text| vec![(text, value)])
                .ok_or_else(|| not_a_value(value)),
        }
    }

    fn scalar(self, node: &Spanned<RawNode>) -> Option<String> {
        match &node.value {
            RawNode::Scalar(text) => Some(text.clone()),
            RawNode::Number(read) => Some(self.source(node).unwrap_or(read).to_owned()),
            _ => None,
        }
    }

    /// The text of `node` in the file, where it is defined.
    fn source(self, node: &Spanned<RawNode>) -> Option<&'a str> {
        let span = node.defined.span();
        let start = usize::try_from(span.byte_offset()?).ok()?;
        let len = usize::try_from(span.byte_len()?).ok()?;
        self.text.get(start..start.checked_add(len)?)
    }
}

/// A `max` below the `min` of its own map: no value could hold.
fn refuse_empty_range(path: &str, entries: &Entries, exprs: &[Expr]) -> Result<(), ConfigError> {
    let Some((_, max)) = entries.iter().find(|(key, _)| key.value == "max") else {
        return Ok(());
    };
    let (low, high) = exprs
        .iter()
        .filter_map(|expr| match expr {
            Expr::Condition(condition) => condition.bounds(),
            _ => None,
        })
        .fold((i64::MIN, i64::MAX), |(low, high), (min, max)| {
            (low.max(min), high.min(max))
        });
    if low > high {
        return Err(error(max, format!("{path}: below `min`")));
    }

    Ok(())
}

/// The operator of a key, or an error that lists the keys of the attribute.
fn operator(
    attribute: Attribute,
    operator: Option<Operator>,
    key: &Spanned<String>,
) -> Result<Operator, ConfigError> {
    operator.ok_or_else(|| {
        let keys = Operator::keys(attribute).collect::<Vec<_>>().join(", ");
        error(
            key,
            format!(
                "unknown key `{}` under `{attribute}`: expected {keys}",
                key.value
            ),
        )
    })
}

impl<'de> Deserialize<'de> for RawNode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(RawNodeVisitor)
    }
}

struct RawNodeVisitor;

impl<'de> Visitor<'de> for RawNodeVisitor {
    type Value = RawNode;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a condition map, a list or a value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<RawNode, E> {
        Ok(RawNode::Scalar(v.to_string()))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<RawNode, E> {
        Ok(RawNode::Number(v.to_string()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<RawNode, E> {
        Ok(RawNode::Number(v.to_string()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<RawNode, E> {
        Ok(RawNode::Number(v.to_string()))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<RawNode, E> {
        Ok(RawNode::Scalar(v.to_owned()))
    }

    fn visit_unit<E: de::Error>(self) -> Result<RawNode, E> {
        Ok(RawNode::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<RawNode, E> {
        Ok(RawNode::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<RawNode, A::Error> {
        let mut list = Vec::new();
        while let Some(node) = seq.next_element()? {
            list.push(node);
        }
        Ok(RawNode::List(list))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RawNode, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry()? {
            entries.push(entry);
        }
        Ok(RawNode::Map(entries))
    }
}
