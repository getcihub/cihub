//! The `OpenAPI` document of the API v1, as `/api/v1/openapi.json` serves it:
//! a reader of the reference finds what each route answers, and what each
//! field means.

#![allow(clippy::unwrap_used, reason = "test code: a panic is a failed test")]

use std::collections::BTreeSet;

use cihub_api_types::{ErrorCode, MAX_LIMIT};
use serde_json::Value;

fn document() -> Value {
    let document = cihub_web::openapi(&[cihub_mergequeue::openapi()]);
    serde_json::to_value(document).unwrap()
}

fn operations(document: &Value) -> Vec<(String, &Value)> {
    let paths = document["paths"].as_object().unwrap();

    paths
        .iter()
        .flat_map(|(path, item)| {
            let item = item.as_object().unwrap();
            item.iter()
                .map(move |(method, operation)| (format!("{method} {path}"), operation))
        })
        .collect()
}

#[test]
fn each_route_has_a_tag_a_title_and_a_description() {
    let document = document();

    for (route, operation) in operations(&document) {
        let summary = operation["summary"].as_str().unwrap_or_default();

        assert_eq!(
            operation["tags"].as_array().map(Vec::len),
            Some(1),
            "{route}"
        );
        assert!(
            !summary.is_empty() && !summary.contains('\n') && !summary.ends_with('.'),
            "{route}: the summary is a short title, `{summary}`"
        );
        assert!(operation["description"].is_string(), "{route}");
    }
}

#[test]
fn each_route_documents_its_errors_with_an_example_of_each() {
    let document = document();

    for (route, operation) in operations(&document) {
        let responses = operation["responses"].as_object().unwrap();

        assert!(responses.contains_key("401"), "{route}");
        for (status, response) in responses {
            let described = response["description"].as_str().unwrap_or_default();
            assert!(!described.is_empty(), "{route} {status}");
            if status.starts_with('2') {
                continue;
            }

            let examples = response["content"]["application/problem+json"]["examples"]
                .as_object()
                .unwrap();
            for (code, example) in examples {
                assert_eq!(
                    example["value"]["status"].to_string(),
                    *status,
                    "{route}: `{code}`"
                );
            }
        }
    }
}

#[test]
fn each_error_code_but_an_unknown_route_is_answered_by_a_route() {
    let document = document();
    let answered: BTreeSet<String> = operations(&document)
        .into_iter()
        .flat_map(|(_, operation)| {
            let responses = operation["responses"].as_object().unwrap();
            responses
                .values()
                .filter_map(|response| {
                    response["content"]["application/problem+json"]["examples"].as_object()
                })
                .flat_map(|examples| examples.keys().cloned())
                .collect::<Vec<_>>()
        })
        .collect();

    for code in ErrorCode::all().filter(|code| *code != ErrorCode::RouteNotFound) {
        assert!(answered.contains(&code.to_string()), "`{code}`");
    }
}

#[test]
fn the_introduction_gives_the_limits_of_the_server() {
    let document = document();
    let introduction = document["info"]["description"].as_str().unwrap();

    assert!(introduction.contains(&format!("1 to {MAX_LIMIT}")));
    assert!(introduction.contains(&format!(
        "{} requests per minute",
        cihub_web::REQUESTS_PER_MINUTE
    )));
}

#[test]
fn each_field_has_a_description() {
    let document = document();
    let schemas = document["components"]["schemas"].as_object().unwrap();

    for (name, schema) in schemas {
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        for (property, value) in properties {
            // A field that can be `null` and names a schema holds its
            // description in the branch of the schema.
            let described =
                value["description"].is_string() || value["oneOf"][0]["description"].is_string();
            assert!(described, "{name}.{property}");
        }
    }
}
