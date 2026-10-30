//! The changes of the queues of an organization (`GET /orgs/{org}/stream`),
//! and a PR followed until it ends.

use std::time::Duration;

use cihub_api_types::{ErrorCode, Pr, QueueChanged};
use reqwest::Method;

use crate::{Client, ClientError, RepoPath, segment};

/// The server pings every 15 seconds: a stream silent for longer is lost.
const SILENT_FOR: Duration = Duration::from_secs(45);

/// The events of a stream as they come.
pub struct QueueStream {
    response: reqwest::Response,
    buffer: Vec<u8>,
}

impl QueueStream {
    /// The next change of a queue; `None` when the stream ended, was lost, or
    /// stayed silent past the pings of the server.
    pub async fn next(&mut self) -> Option<QueueChanged> {
        loop {
            if let Some(changed) = self.take_event() {
                return Some(changed);
            }
            let chunk = tokio::time::timeout(SILENT_FOR, self.response.chunk()).await;
            let Ok(Ok(Some(chunk))) = chunk else {
                return None;
            };
            self.buffer.extend_from_slice(&chunk);
        }
    }

    /// The first `queue_changed` of the events that the buffer holds whole;
    /// the pings and the other events are dropped.
    fn take_event(&mut self) -> Option<QueueChanged> {
        while let Some(end) = self.buffer.windows(2).position(|pair| pair == b"\n\n") {
            let event: Vec<u8> = self.buffer.drain(..end + 2).collect();
            if let Some(changed) = queue_changed(&String::from_utf8_lossy(&event)) {
                return Some(changed);
            }
        }
        None
    }
}

/// The change that an event carries, when it is a `queue_changed`.
fn queue_changed(event: &str) -> Option<QueueChanged> {
    let mut name = None;
    let mut data = None;
    for line in event.lines() {
        if let Some(value) = line.strip_prefix("event:") {
            name = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("data:") {
            data = Some(value.trim());
        }
    }
    if name != Some("queue_changed") {
        return None;
    }

    serde_json::from_str(data?).ok()
}

/// Waits for the changes of one queue: on the stream of its organization, or
/// every `poll_every` while the server refuses one more stream.
pub struct QueueWatch<'a> {
    client: &'a Client,
    org: String,
    queue_id: String,
    poll_every: Duration,
    stream: Option<QueueStream>,
}

impl QueueWatch<'_> {
    /// Returns when the queue may have changed: a `queue_changed` of the
    /// queue, a stream opened again (changes may have been lost meanwhile),
    /// or the next poll.
    ///
    /// # Errors
    ///
    /// The stream cannot open, for another reason than its limit.
    pub async fn changed(&mut self) -> Result<(), ClientError> {
        while let Some(stream) = &mut self.stream {
            match stream.next().await {
                Some(changed) if changed.queue_id == self.queue_id => return Ok(()),
                Some(_) => {}
                None => self.stream = None,
            }
        }

        self.open().await
    }

    async fn open(&mut self) -> Result<(), ClientError> {
        match self.client.stream(&self.org).await {
            Ok(stream) => {
                self.stream = Some(stream);
                Ok(())
            }
            Err(ClientError::Api(problem)) if problem.code == ErrorCode::TooManyStreams => {
                tokio::time::sleep(self.poll_every).await;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

impl Client {
    /// The changes of the queues of the organization `org`, as they come.
    ///
    /// # Errors
    ///
    /// The request fails, or CIHub refuses it: `too_many_streams` past 8
    /// streams of the token or the person.
    pub async fn stream(&self, org: &str) -> Result<QueueStream, ClientError> {
        let path = format!("/orgs/{}/stream", segment(org));
        let response = self.respond(Method::GET, &path, None::<&()>).await?;

        Ok(QueueStream {
            response,
            buffer: Vec::new(),
        })
    }

    /// Waits for the changes of the queue `queue_id` of the organization
    /// `org`; while the server refuses one more stream, every `poll_every`.
    pub fn watch_queue(&self, org: &str, queue_id: &str, poll_every: Duration) -> QueueWatch<'_> {
        QueueWatch {
            client: self,
            org: org.to_owned(),
            queue_id: queue_id.to_owned(),
            poll_every,
            stream: None,
        }
    }

    /// Follows the PR `number` until it ends: `on_change` receives the PR,
    /// then each new version of it. Gives the PR as it ended.
    ///
    /// # Errors
    ///
    /// A read of the PR fails, or the stream cannot open, for another reason
    /// than its limit.
    pub async fn follow_pr(
        &self,
        repo: RepoPath<'_>,
        number: u64,
        poll_every: Duration,
        mut on_change: impl FnMut(&Pr),
    ) -> Result<Pr, ClientError> {
        let mut pr = self.pr(repo, number).await?;
        on_change(&pr);
        let mut watch = self.watch_queue(repo.org, &pr.queue_id, poll_every);

        while !pr.state.is_terminal() {
            watch.changed().await?;
            let read = self.pr(repo, number).await?;
            if read != pr {
                on_change(&read);
            }
            pr = read;
        }

        Ok(pr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_queue_changed_gives_a_change() {
        let changed =
            queue_changed("event: queue_changed\ndata: {\"queue_id\":\"q\",\"seq\":4}\n\n");

        assert_eq!(
            changed,
            Some(QueueChanged {
                queue_id: "q".into(),
                seq: 4
            })
        );
        assert_eq!(queue_changed(": ping\n\n"), None);
        assert_eq!(queue_changed("retry: 1000\n\n"), None);
        assert_eq!(queue_changed("event: other\ndata: {}\n\n"), None);
        assert_eq!(
            queue_changed("event: queue_changed\ndata: not json\n\n"),
            None
        );
    }
}
