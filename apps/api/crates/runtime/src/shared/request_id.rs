//! The identifier that ties a log line to the response a person is holding.
//!
//! Stored in a task-local rather than passed as an argument, because the
//! alternative does not survive contact with the error path. A handler can
//! receive the id as an extractor argument and put it in a successful
//! response, but `?` returns early with an error value that has nowhere to
//! carry it — so every failure would arrive without the one field that makes
//! a support conversation possible.
//!
//! A task-local is safe here in a way a global never is: axum drives each
//! request on its own task, and the middleware scopes the value to exactly
//! that task's future. Nothing outside a request can see a value, and no two
//! requests can see each other's.

use std::fmt;

use uuid::Uuid;

tokio::task_local! {
    static CURRENT: RequestId;
}

/// A per-request identifier.
///
/// Version 7 rather than 4: v7 begins with a millisecond timestamp, so ids
/// sort chronologically. Sorting a log file by request id then orders it by
/// time for free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestId(String);

impl RequestId {
    /// Mint a new identifier.
    ///
    /// Always generated here, never read from an inbound header. A value a
    /// caller supplies ends up in log lines, and text from outside the system
    /// that lands in a log is how log injection works.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::now_v7().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Run `future` with this id visible to everything it awaits.
    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }

    /// The id of the request being handled, if there is one.
    ///
    /// Returns `None` outside a request — a background job, a test, a startup
    /// task. That is a real answer, not a failure, so callers render it as an
    /// absent field rather than inventing an id that correlates with nothing.
    #[must_use]
    pub fn current() -> Option<Self> {
        CURRENT.try_with(Clone::clone).ok()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_differ() {
        assert_ne!(RequestId::generate(), RequestId::generate());
    }

    #[test]
    fn ids_sort_chronologically() {
        // v7 leads with a timestamp, so lexical order is time order. A v4 id
        // would fail this, which is the point of pinning the version.
        let first = RequestId::generate();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = RequestId::generate();
        assert!(first.as_str() < second.as_str());
    }

    #[tokio::test]
    async fn the_scoped_id_is_visible_to_everything_inside() {
        let id = RequestId::generate();
        let expected = id.clone();

        id.scope(async {
            assert_eq!(RequestId::current(), Some(expected.clone()));

            // Still visible after an await point, which is the case that
            // matters: a handler reaches its response after several.
            tokio::task::yield_now().await;
            assert_eq!(RequestId::current(), Some(expected));
        })
        .await;
    }

    #[tokio::test]
    async fn there_is_no_id_outside_a_request() {
        assert_eq!(RequestId::current(), None);
    }

    #[tokio::test]
    async fn scopes_do_not_leak_into_each_other() {
        let first = RequestId::generate();
        let second = RequestId::generate();
        let (a, b) = (first.clone(), second.clone());

        let one = tokio::spawn(first.scope(async move {
            tokio::task::yield_now().await;
            RequestId::current()
        }));
        let two = tokio::spawn(second.scope(async move { RequestId::current() }));

        let (one, two) = (one.await, two.await);
        assert_eq!(one.ok().flatten(), Some(a));
        assert_eq!(two.ok().flatten(), Some(b));
    }
}
