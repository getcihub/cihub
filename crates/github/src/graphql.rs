//! The GraphQL API of GitHub: a query, its errors, and the lists of its
//! answers.

use cihub_domain::JobClass;
use cihub_scm::{ConnectionRef, ScmError};
use reqwest::StatusCode;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::GithubApp;

/// The answer of a GraphQL query: its data and its errors.
#[derive(Deserialize)]
struct GraphqlAnswer {
    data: Option<Value>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

/// An error of a GraphQL query.
#[derive(Deserialize)]
struct GraphqlError {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    message: String,
}

/// A list in a GraphQL answer; GitHub can give `null` for an item.
#[derive(Deserialize)]
pub(crate) struct Nodes<T> {
    pub nodes: Vec<Option<T>>,
}

/// Reads a part of the data of a GraphQL answer as `T`.
pub(crate) fn decode<T: DeserializeOwned>(value: &Value) -> Result<T, ScmError> {
    T::deserialize(value).map_err(|err| ScmError::Protocol(err.to_string()))
}

impl GithubApp {
    /// The `data` of a GraphQL answer. An error of type `NOT_FOUND` leaves
    /// its field `null`, as for a PR that does not exist.
    pub(crate) async fn query(
        &self,
        connection: &ConnectionRef,
        class: JobClass,
        query: &str,
        variables: Value,
    ) -> Result<Value, ScmError> {
        let answer = self.graphql(connection, class, query, variables).await?;
        let answer = answer.expect_status(&[StatusCode::OK])?;
        let GraphqlAnswer { data, errors } = answer.json()?;

        for error in &errors {
            match error.kind.as_deref() {
                Some("NOT_FOUND") => {}
                Some("RATE_LIMITED") => return Err(self.graphql_limit_reached(connection.id)),
                _ => return Err(ScmError::Protocol(error.message.clone())),
            }
        }

        data.ok_or_else(|| ScmError::Protocol("GraphQL answer without data".into()))
    }
}
