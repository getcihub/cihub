//! A webhook delivery of GitHub as the ingestion receives it: its route, its
//! signature and its headers. No decision here: the routing loop reads the
//! payload later.

use reqwest::header::HeaderMap;
use secrecy::SecretString;
use serde_json::Value;

use crate::signature::verify_signature;

/// Where GitHub sends the webhook deliveries of the App.
pub const WEBHOOK_PATH: &str = "/webhooks/github";

/// A delivery whose signature is valid, before its translation.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceivedDelivery {
    /// `X-GitHub-Delivery`: a copy of a delivery has the same.
    pub delivery_id: String,
    /// `X-GitHub-Event`, such as `pull_request`.
    pub event: String,
    pub payload: Value,
}

/// Why a delivery is refused.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("the signature of the delivery is not valid")]
    Signature,
    #[error("the delivery has no header {0}")]
    MissingHeader(&'static str),
    #[error("the payload of the delivery is not JSON")]
    Payload(#[source] serde_json::Error),
}

/// Checks the signature of a delivery with the webhook secret of the App,
/// then reads its headers and its payload.
///
/// # Errors
///
/// The signature is missing or wrong, a header is missing, or the payload is
/// not JSON.
pub fn receive(
    secret: &SecretString,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<ReceivedDelivery, WebhookError> {
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    if !verify_signature(secret, body, header("x-hub-signature-256")) {
        return Err(WebhookError::Signature);
    }

    let delivery_id = header("x-github-delivery")
        .filter(|id| !id.is_empty())
        .ok_or(WebhookError::MissingHeader("X-GitHub-Delivery"))?;
    let event = header("x-github-event")
        .filter(|event| !event.is_empty())
        .ok_or(WebhookError::MissingHeader("X-GitHub-Event"))?;
    let payload: Value = serde_json::from_slice(body).map_err(WebhookError::Payload)?;
    Ok(ReceivedDelivery {
        delivery_id: delivery_id.to_owned(),
        event: event.to_owned(),
        payload,
    })
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use reqwest::header::HeaderValue;
    use ring::hmac;

    use super::*;

    fn signed(secret: &str, body: &[u8], pairs: &[(&'static str, &str)]) -> HeaderMap {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let tag = hmac::sign(&key, body);
        let hex = tag.as_ref().iter().fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-hub-signature-256",
            HeaderValue::from_str(&format!("sha256={hex}")).unwrap(),
        );

        for (name, value) in pairs {
            headers.insert(*name, HeaderValue::from_str(value).unwrap());
        }

        headers
    }

    const BODY: &[u8] = br#"{"action":"opened","number":1}"#;

    #[test]
    fn a_signed_delivery_gives_its_headers_and_payload() {
        let headers = signed(
            "s",
            BODY,
            &[
                ("x-github-delivery", "d1"),
                ("x-github-event", "pull_request"),
            ],
        );
        let delivery = receive(&SecretString::from("s"), &headers, BODY).unwrap();

        assert_eq!(delivery.delivery_id, "d1");
        assert_eq!(delivery.event, "pull_request");
        assert_eq!(delivery.payload["number"], 1);
    }

    #[test]
    fn a_delivery_signed_with_another_secret_is_refused() {
        let headers = signed(
            "other",
            BODY,
            &[("x-github-delivery", "d1"), ("x-github-event", "ping")],
        );
        let refused = receive(&SecretString::from("s"), &headers, BODY);
        assert!(matches!(refused, Err(WebhookError::Signature)));
    }

    #[test]
    fn a_delivery_needs_its_headers_and_a_json_payload() {
        let secret = SecretString::from("s");
        let no_id = signed("s", BODY, &[("x-github-event", "ping")]);

        assert!(matches!(
            receive(&secret, &no_id, BODY),
            Err(WebhookError::MissingHeader("X-GitHub-Delivery"))
        ));

        let no_event = signed("s", BODY, &[("x-github-delivery", "d1")]);

        assert!(matches!(
            receive(&secret, &no_event, BODY),
            Err(WebhookError::MissingHeader("X-GitHub-Event"))
        ));

        let text = b"not json";
        let headers = signed(
            "s",
            text,
            &[("x-github-delivery", "d1"), ("x-github-event", "ping")],
        );

        assert!(matches!(
            receive(&secret, &headers, text),
            Err(WebhookError::Payload(_))
        ));
    }
}
