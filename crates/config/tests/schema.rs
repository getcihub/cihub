//! The JSON schema of `.cihub.yaml` (`schema/config/v1.json`) is written by
//! hand. These tests compare it to the parser and to `cihub-conditions`: a
//! key, a value, a range, an attribute or an operator that the code adds or
//! removes fails here until the schema follows.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::collections::BTreeSet;

use cihub_conditions::{Attribute, Condition, ConditionErrorKind, Operator};
use cihub_config::RepoConfig;
use serde_json::Value;

const SCHEMA: &str = include_str!("../../../schema/config/v1.json");

fn schema() -> Value {
    serde_json::from_str(SCHEMA).unwrap()
}

/// A node with its `$ref` and its lone `allOf` followed.
fn resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    if let Some(target) = node.get("$ref").and_then(Value::as_str) {
        let name = target.strip_prefix("#/definitions/").unwrap();
        return resolve(root, &root["definitions"][name]);
    }

    match node
        .get("allOf")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        Some([only]) => resolve(root, only),
        _ => node,
    }
}

fn definition<'a>(root: &'a Value, name: &str) -> &'a Value {
    &root["definitions"][name]
}

fn keys(node: &Value) -> BTreeSet<String> {
    node["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("no properties in {node}"))
        .keys()
        .cloned()
        .collect()
}

fn names(list: &str) -> BTreeSet<String> {
    list.split(", ").map(str::to_owned).collect()
}

/// What the parser expects instead of the unknown `zz` of `yaml`, read from
/// its error.
fn expected(yaml: &str) -> BTreeSet<String> {
    let message = RepoConfig::parse(yaml).unwrap_err().message;
    let (_, list) = message
        .split_once("expected one of ")
        .unwrap_or_else(|| panic!("{message}"));
    names(list)
}

fn queue(body: &str) -> String {
    format!("version: 1\nqueues:\n  - branch: main\n{body}")
}

/// The keys of the queue that hold a map of settings.
const NESTED: [&str; 4] = ["window", "batching", "failure_budget", "label_commands"];

#[test]
fn the_keys_are_those_of_the_parser() {
    let root = schema();
    let queue_schema = definition(&root, "queue");
    let queue_keys = expected(&queue("    zz: 1\n"));

    assert_eq!(keys(&root), expected("version: 1\nzz: 1\n"));
    assert_eq!(keys(queue_schema), queue_keys);
    let mut defaults = queue_keys;
    defaults.remove("branch");
    defaults.remove("profiles");
    assert_eq!(keys(definition(&root, "defaults")), defaults);
    assert_eq!(
        keys(definition(&root, "profile")),
        expected(&queue("    profiles:\n      - name: a\n        zz: 1\n"))
    );
    assert_eq!(
        keys(definition(&root, "priority_rule")),
        expected("version: 1\npriority_rules:\n  - name: a\n    zz: 1\n")
    );

    for key in NESTED {
        let node = resolve(&root, &queue_schema["properties"][key]);
        assert_eq!(
            keys(node),
            expected(&queue(&format!("    {key}:\n      zz: 1\n"))),
            "{key}"
        );
    }
}

/// Whether the parser takes `line` in a queue, alone or with the strategy
/// that some values need.
fn takes(line: &str) -> bool {
    ["", "    strategy: sequential\n"]
        .iter()
        .any(|context| RepoConfig::parse(&queue(&format!("{context}{line}"))).is_ok())
}

fn enum_values(node: &Value) -> Option<BTreeSet<String>> {
    let values = node.get("enum")?.as_array()?;
    Some(
        values
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect(),
    )
}

#[test]
fn the_values_are_those_that_the_parser_takes() {
    let root = schema();
    let profile = definition(&root, "profile");
    let settings = definition(&root, "queue")["properties"]
        .as_object()
        .unwrap();

    for (key, node) in settings {
        let Some(values) = enum_values(resolve(&root, node)) else {
            continue;
        };
        let taken: BTreeSet<String> = expected(&queue(&format!("    {key}: zz\n")))
            .into_iter()
            .filter(|value| takes(&format!("    {key}: {value}\n")))
            .collect();
        assert_eq!(values, taken, "{key}");
    }

    for key in ["merge_method", "merge_commit_title"] {
        let values = enum_values(resolve(&root, &profile["properties"][key])).unwrap();
        let profile_with = |value: &str| {
            format!(
                "    strategy: sequential\n    profiles:\n      - name: a\n        {key}: {value}\n"
            )
        };
        let taken: BTreeSet<String> = expected(&queue(&profile_with("zz")))
            .into_iter()
            .filter(|value| RepoConfig::parse(&queue(&profile_with(value))).is_ok())
            .collect();
        assert_eq!(values, taken, "profile {key}");
    }
}

/// The YAML of the integer setting at `path` set to `value`. A window
/// bound comes with the other bound that lets it reach its range.
fn setting(path: &[&str], value: i64) -> String {
    match path {
        ["window", "floor"] => format!("    window:\n      floor: {value}\n      ceiling: 100\n"),
        ["window", "ceiling"] => format!("    window:\n      floor: 1\n      ceiling: {value}\n"),
        [key] => format!("    {key}: {value}\n"),
        [key, inner] => format!("    {key}:\n      {inner}: {value}\n"),
        _ => unreachable!("a setting has at most two levels"),
    }
}

#[test]
fn the_ranges_are_those_of_the_parser() {
    let root = schema();
    let settings = definition(&root, "queue")["properties"]
        .as_object()
        .unwrap();
    for (key, node) in settings {
        let node = resolve(&root, node);
        let mut integers = vec![(vec![key.as_str()], node)];
        if NESTED.contains(&key.as_str()) {
            for (inner, value) in node["properties"].as_object().unwrap() {
                integers.push((vec![key.as_str(), inner.as_str()], value));
            }
        }

        for (path, node) in integers {
            if node["type"] != "integer" {
                continue;
            }
            // Every integer setting of the code has a range.
            let (Some(min), Some(max)) = (node["minimum"].as_i64(), node["maximum"].as_i64())
            else {
                panic!("{path:?} has no range in the schema");
            };
            let parses = |value| takes(&setting(&path, value));
            assert!(parses(min) && parses(max), "{path:?} takes {min} and {max}");
            assert!(
                !parses(min - 1) && !parses(max + 1),
                "{path:?} refuses outside {min}-{max}"
            );
        }
    }
}

#[test]
fn the_priorities_are_those_of_the_parser() {
    let root = schema();
    let priority = &definition(&root, "priority_rule")["properties"]["priority"];
    let rule = |value: &str| {
        RepoConfig::parse(&format!(
            "version: 1\npriority_rules:\n  - name: a\n    conditions:\n      draft: false\n    priority: {value}\n"
        ))
    };
    let [names, numbers] = priority["oneOf"].as_array().unwrap().as_slice() else {
        panic!("a priority is a name or a number");
    };

    for name in enum_values(names).unwrap() {
        assert!(rule(&name).is_ok(), "{name}");
    }
    let (min, max) = (
        numbers["minimum"].as_i64().unwrap(),
        numbers["maximum"].as_i64().unwrap(),
    );

    assert!(rule(&min.to_string()).is_ok() && rule(&max.to_string()).is_ok());
    assert!(rule(&(max + 1).to_string()).is_err() && rule("top").is_err());
}

/// The node of `attribute` in the condition map.
fn attribute_node(root: &Value, attribute: Attribute) -> &Value {
    let conditions = definition(root, "conditions");
    let node = match attribute.name().split_once('.') {
        Some((group, state)) => &conditions["properties"][group]["properties"][state],
        None => &conditions["properties"][attribute.name()],
    };
    assert!(!node.is_null(), "`{attribute}` is not in the schema");
    resolve(root, node)
}

/// The variants of a node: its `oneOf`, or itself.
fn variants<'a>(root: &'a Value, node: &'a Value) -> Vec<&'a Value> {
    match node.get("oneOf").and_then(Value::as_array) {
        Some(list) => list.iter().map(|variant| resolve(root, variant)).collect(),
        None => vec![node],
    }
}

/// A value that the schema gives for the attribute itself, and the values
/// of its closed choice if it has one.
fn samples(root: &Value, node: &Value) -> (String, Option<BTreeSet<String>>) {
    let first = variants(root, node)[0];
    let first = variants(root, first)[0];
    if let Some(values) = enum_values(first) {
        let sample = values.iter().next().unwrap().clone();
        return (sample, Some(values));
    }

    let sample = match (first["type"].as_str(), first["pattern"].as_str()) {
        (Some("boolean"), _) => "true",
        (Some("integer"), _) => "5",
        (_, Some(_)) => "2h",
        _ => "x",
    };
    (sample.to_owned(), None)
}

#[test]
fn the_conditions_are_those_of_cihub_conditions() {
    let root = schema();
    let conditions = definition(&root, "conditions");
    let mut map_keys: BTreeSet<String> = ["all", "any", "none", "checks"].map(String::from).into();
    let mut states = BTreeSet::new();

    for &attribute in Attribute::ALL {
        match attribute.name().split_once('.') {
            Some((_, state)) => states.insert(state.to_owned()),
            None => map_keys.insert(attribute.name().to_owned()),
        };
    }

    assert_eq!(keys(conditions), map_keys);
    assert_eq!(keys(&conditions["properties"]["checks"]), states);

    for &attribute in Attribute::ALL {
        let node = attribute_node(&root, attribute);
        let operators: BTreeSet<String> = variants(&root, node)
            .iter()
            .filter(|variant| variant["type"] == "object")
            .flat_map(|variant| keys(variant))
            .collect();
        let code: BTreeSet<String> = Operator::keys(attribute).map(str::to_owned).collect();
        let (sample, choice) = samples(&root, node);

        assert_eq!(operators, code, "operators of `{attribute}`");
        assert!(
            Condition::build(attribute, Operator::Is, std::slice::from_ref(&sample)).is_ok(),
            "`{attribute}` takes `{sample}`"
        );

        let Some(choice) = choice else {
            continue;
        };
        let error = Condition::build(attribute, Operator::Is, &["zz".to_owned()]).unwrap_err();
        let ConditionErrorKind::InvalidChoice { .. } = error.kind else {
            panic!("`{attribute}` is a choice in the schema");
        };
        let message = error.to_string();
        let (_, list) = message.split_once("expected one of ").unwrap();

        assert_eq!(choice, names(list), "values of `{attribute}`");
    }
}

/// The path of each property without a description in `node`, and in the
/// maps that it writes inline; a `$ref` is checked as its own definition.
fn undescribed(root: &Value, node: &Value, path: &str, missing: &mut Vec<String>) {
    for variant in variants(root, node) {
        let Some(properties) = variant.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (key, property) in properties {
            let path = format!("{path}.{key}");
            let resolved = resolve(root, property);
            if property
                .get("description")
                .or(resolved.get("description"))
                .is_none()
            {
                missing.push(path.clone());
            }
            if std::ptr::eq(resolved, property) {
                undescribed(root, property, &path, missing);
            }
        }
    }
}

#[test]
fn every_key_has_a_description() {
    let root = schema();
    let mut missing = Vec::new();

    undescribed(&root, &root, "", &mut missing);
    for (name, node) in root["definitions"].as_object().unwrap() {
        undescribed(&root, node, name, &mut missing);
    }

    assert_eq!(missing, [] as [String; 0]);
}

/// The example of the spec and the file of the repository are the two
/// files that a reader copies first.
#[test]
fn the_examples_declare_the_schema_and_parse() {
    let id = schema()["$id"].as_str().unwrap().to_owned();
    let spec = include_str!("../../../specs/CONFIGURATION.md");
    let (_, example) = spec.split_once("```yaml\n").unwrap();
    let (example, _) = example.split_once("```").unwrap();
    let repository = include_str!("../../../.cihub.yaml");

    for (name, text) in [("spec", example), (".cihub.yaml", repository)] {
        let first = text.lines().next().unwrap();
        assert_eq!(
            first,
            format!("# yaml-language-server: $schema={id}"),
            "{name}"
        );
        RepoConfig::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}
