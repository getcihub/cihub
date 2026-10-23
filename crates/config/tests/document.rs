//! The YAML that CIHub writes reads back as the document it comes from.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::fs;
use std::path::Path;

use cihub_api_types::{ConditionBlock, ConditionDocument, ConfigDocument, LineChange};
use cihub_config::{RepoConfig, diff_lines, write_document};

/// Reads `text`, writes its document, and reads the YAML back.
fn round_trip(text: &str) -> (ConfigDocument, String, ConfigDocument) {
    let document = RepoConfig::parse(text).unwrap().document();
    let written = write_document(&document).unwrap();
    let read = RepoConfig::parse(&written)
        .unwrap_or_else(|error| panic!("{error}\n{written}"))
        .document();
    (document, written, read)
}

#[test]
fn each_example_reads_back_as_its_document() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema/config/examples");
    let mut count = 0;
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        let (document, written, read) = round_trip(&text);

        assert_eq!(read, document, "{}\n{written}", path.display());
        assert_eq!(
            write_document(&read).unwrap(),
            written,
            "the YAML of a document is stable"
        );
        count += 1;
    }
    assert!(count >= 3, "the examples are read");
}

#[test]
fn a_file_keeps_its_forms_and_its_values() {
    let text = "version: 1
queues:
  - branch: main
    testing_concurrency: 3
    required_statuses: null
    queue_conditions:
      checks:
        success: [3.10, \"dependabot[bot]\"]
      draft: false
  - branch: release/*
    required_statuses: []
priority_rules:
  - name: high
    conditions:
      labels: [x]
    priority: 10
";
    let (_, written, _) = round_trip(text);

    assert!(
        written.starts_with("# yaml-language-server: $schema="),
        "{written}"
    );
    for line in [
        "    testing_concurrency: 3",
        "    required_statuses: null",
        "          - 3.10",
        "          - dependabot[bot]",
        "      draft: false",
        "  - branch: release/*",
        "    required_statuses: []",
        "    priority: high",
    ] {
        assert!(
            written.lines().any(|written| written == line),
            "{line}\n{written}"
        );
    }
    assert!(!written.contains("defaults:"), "{written}");
}

#[test]
fn two_conditions_that_a_map_cannot_hold_go_in_an_all_block() {
    let any = |value: &str| ConditionDocument::Condition {
        attribute: "labels".into(),
        operator: Some("any".into()),
        values: vec![value.into()],
    };
    let none = |value: &str| ConditionDocument::Condition {
        attribute: "labels".into(),
        operator: Some("none".into()),
        values: vec![value.into()],
    };
    let mut document = RepoConfig::parse("version: 1\n").unwrap().document();
    document.defaults.queue_conditions = vec![none("a"), none("b")];
    let joined = write_document(&document).unwrap();

    assert!(
        joined.contains("      none:\n        - a\n        - b\n"),
        "{joined}"
    );

    document.defaults.queue_conditions = vec![any("a"), any("b")];
    let written = write_document(&document).unwrap();
    let read = RepoConfig::parse(&written).unwrap().document();

    assert_eq!(
        read.defaults.queue_conditions,
        [ConditionDocument::Block {
            block: ConditionBlock::All,
            conditions: vec![any("a"), any("b")],
        }],
        "`any` of a and `any` of b is not `any` of both: {written}"
    );
}

#[test]
fn a_diff_gives_the_lines_that_stay_go_and_come() {
    let diff = diff_lines("a\nb\nc\n", "a\nc\nd\n");
    let changes: Vec<(LineChange, &str)> = diff
        .iter()
        .map(|line| (line.change, line.text.as_str()))
        .collect();

    assert_eq!(
        changes,
        [
            (LineChange::Same, "a"),
            (LineChange::Removed, "b"),
            (LineChange::Same, "c"),
            (LineChange::Added, "d"),
        ]
    );
    assert_eq!(diff_lines("", "a").len(), 1);
}

#[test]
fn a_condition_that_the_format_does_not_have_is_not_written() {
    let mut document = RepoConfig::parse("version: 1\n").unwrap().document();
    document.defaults.queue_conditions = vec![ConditionDocument::Condition {
        attribute: "labels: x\nqueues".into(),
        operator: None,
        values: vec!["a".into()],
    }];

    let error = write_document(&document).unwrap_err();

    assert!(error.message.starts_with("unknown condition"), "{error}");

    document.defaults.queue_conditions = vec![ConditionDocument::Block {
        block: ConditionBlock::Any,
        conditions: vec![ConditionDocument::Condition {
            attribute: "labels".into(),
            operator: Some("some".into()),
            values: vec!["a".into()],
        }],
    }];

    assert!(write_document(&document).is_err());
}
