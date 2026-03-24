//! Mux protocol layer: ID remapping and document state tracking.
//!
//! This module provides the core protocol logic for multiplexing multiple
//! LSP clients over a single LSP server connection. It handles:
//!
//! - **ID remapping**: tagging outbound request IDs with a client identifier
//!   so responses can be routed back to the correct client.
//! - **Document state**: tracking which clients have which files open so that
//!   `didOpen`/`didClose` notifications are only forwarded when the refcount
//!   transitions (first opener / last closer).

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Identifies a multiplexed client session.
pub type ClientTag = String;

/// Tag a client's request ID with their tag prefix.
///
/// Numeric ID 1 with tag "a" → string `"a:1"`
/// String ID "req-5" with tag "b" → string `"b:req-5"`
pub fn tag_request_id(id: &Value, tag: &str) -> Value {
    match id {
        Value::Number(n) => Value::String(format!("{tag}:{n}")),
        Value::String(s) => Value::String(format!("{tag}:{s}")),
        other => Value::String(format!("{tag}:{other}")),
    }
}

/// Extract the client tag and original ID from a tagged response ID.
///
/// `"a:1"` → `Some(("a", Number(1)))`
/// `"b:req-5"` → `Some(("b", String("req-5")))`
///
/// Returns `None` if the ID is not a string or has no colon separator.
pub fn untag_response_id(id: &Value) -> Option<(String, Value)> {
    let s = id.as_str()?;
    let colon_pos = s.find(':')?;
    let tag = s[..colon_pos].to_string();
    let original_raw = &s[colon_pos + 1..];

    // Try to parse as a number first (restoring the original numeric type).
    let original = if let Ok(n) = original_raw.parse::<i64>() {
        Value::Number(n.into())
    } else if let Ok(n) = original_raw.parse::<u64>() {
        Value::Number(n.into())
    } else {
        Value::String(original_raw.to_string())
    };

    Some((tag, original))
}

/// Tracks which clients have which documents open against the shared LSP server.
///
/// The mux must suppress duplicate `didOpen`/`didClose` notifications: only the
/// first opener triggers `didOpen`, and only the last closer triggers `didClose`.
/// Version numbers are remapped to a monotonically increasing sequence per URI
/// so the server never sees out-of-order versions from interleaved clients.
pub struct DocumentState {
    /// URI → (set of client tags that have it open, monotonic version counter)
    files: HashMap<String, (HashSet<String>, i64)>,
}

impl DocumentState {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Client opens a file. Returns `true` if `didOpen` should be forwarded
    /// to the LSP server (i.e. this is the first opener).
    pub fn open(&mut self, uri: &str, tag: &str) -> bool {
        let entry = self
            .files
            .entry(uri.to_string())
            .or_insert_with(|| (HashSet::new(), 0));
        let is_first = entry.0.is_empty();
        entry.0.insert(tag.to_string());
        is_first
    }

    /// Client closes a file. Returns `true` if `didClose` should be forwarded
    /// to the LSP server (i.e. this was the last closer).
    pub fn close(&mut self, uri: &str, tag: &str) -> bool {
        if let Some(entry) = self.files.get_mut(uri) {
            entry.0.remove(tag);
            if entry.0.is_empty() {
                self.files.remove(uri);
                return true;
            }
        }
        false
    }

    /// Get the next monotonic version for a `didChange` on this URI.
    ///
    /// Each call increments the counter, ensuring the LSP server always sees
    /// strictly increasing version numbers regardless of which client sent
    /// the change.
    pub fn next_version(&mut self, uri: &str) -> i64 {
        let entry = self
            .files
            .entry(uri.to_string())
            .or_insert_with(|| (HashSet::new(), 0));
        entry.1 += 1;
        entry.1
    }

    /// Remove all file ownership for a disconnecting client.
    ///
    /// Returns URIs that need `didClose` forwarded (files where this client
    /// was the sole opener).
    pub fn disconnect(&mut self, tag: &str) -> Vec<String> {
        let mut to_close = Vec::new();
        let mut to_remove = Vec::new();

        for (uri, (clients, _)) in self.files.iter_mut() {
            clients.remove(tag);
            if clients.is_empty() {
                to_close.push(uri.clone());
                to_remove.push(uri.clone());
            }
        }

        for uri in &to_remove {
            self.files.remove(uri);
        }

        to_close
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── ID remapping ────────────────────────────────────────────

    #[test]
    fn tag_request_id_numeric() {
        let id = json!(1);
        let tagged = tag_request_id(&id, "a");
        assert_eq!(tagged, json!("a:1"));
    }

    #[test]
    fn tag_request_id_string() {
        let id = json!("req-5");
        let tagged = tag_request_id(&id, "b");
        assert_eq!(tagged, json!("b:req-5"));
    }

    #[test]
    fn untag_response_id_numeric() {
        let tagged = json!("a:1");
        let (tag, original) = untag_response_id(&tagged).unwrap();
        assert_eq!(tag, "a");
        assert_eq!(original, json!(1));
    }

    #[test]
    fn untag_response_id_string() {
        let tagged = json!("b:req-5");
        let (tag, original) = untag_response_id(&tagged).unwrap();
        assert_eq!(tag, "b");
        assert_eq!(original, json!("req-5"));
    }

    #[test]
    fn untag_response_id_no_colon() {
        let tagged = json!("no_colon");
        assert!(untag_response_id(&tagged).is_none());
    }

    #[test]
    fn untag_response_id_non_string() {
        let tagged = json!(42);
        assert!(untag_response_id(&tagged).is_none());
    }

    #[test]
    fn untag_response_id_null() {
        let tagged = json!(null);
        assert!(untag_response_id(&tagged).is_none());
    }

    // ── DocumentState::open ─────────────────────────────────────

    #[test]
    fn open_first_client_forwards() {
        let mut state = DocumentState::new();
        assert!(state.open("file:///a.rs", "client-a"));
    }

    #[test]
    fn open_second_client_suppresses() {
        let mut state = DocumentState::new();
        assert!(state.open("file:///a.rs", "client-a"));
        assert!(!state.open("file:///a.rs", "client-b"));
    }

    #[test]
    fn open_same_client_twice_suppresses() {
        let mut state = DocumentState::new();
        assert!(state.open("file:///a.rs", "client-a"));
        assert!(!state.open("file:///a.rs", "client-a"));
    }

    // ── DocumentState::close ────────────────────────────────────

    #[test]
    fn close_last_client_forwards() {
        let mut state = DocumentState::new();
        state.open("file:///a.rs", "client-a");
        assert!(state.close("file:///a.rs", "client-a"));
    }

    #[test]
    fn close_non_last_client_suppresses() {
        let mut state = DocumentState::new();
        state.open("file:///a.rs", "client-a");
        state.open("file:///a.rs", "client-b");
        assert!(!state.close("file:///a.rs", "client-a"));
    }

    #[test]
    fn close_then_last_close_forwards() {
        let mut state = DocumentState::new();
        state.open("file:///a.rs", "client-a");
        state.open("file:///a.rs", "client-b");
        assert!(!state.close("file:///a.rs", "client-a"));
        assert!(state.close("file:///a.rs", "client-b"));
    }

    #[test]
    fn close_unknown_uri_returns_false() {
        let mut state = DocumentState::new();
        assert!(!state.close("file:///unknown.rs", "client-a"));
    }

    // ── DocumentState::next_version ─────────────────────────────

    #[test]
    fn next_version_monotonically_increasing() {
        let mut state = DocumentState::new();
        state.open("file:///a.rs", "client-a");
        assert_eq!(state.next_version("file:///a.rs"), 1);
        assert_eq!(state.next_version("file:///a.rs"), 2);
        assert_eq!(state.next_version("file:///a.rs"), 3);
    }

    #[test]
    fn next_version_independent_per_uri() {
        let mut state = DocumentState::new();
        state.open("file:///a.rs", "client-a");
        state.open("file:///b.rs", "client-a");
        assert_eq!(state.next_version("file:///a.rs"), 1);
        assert_eq!(state.next_version("file:///b.rs"), 1);
        assert_eq!(state.next_version("file:///a.rs"), 2);
    }

    // ── DocumentState::disconnect ───────────────────────────────

    #[test]
    fn disconnect_closes_exclusive_files() {
        let mut state = DocumentState::new();
        state.open("file:///exclusive.rs", "client-a");
        state.open("file:///shared.rs", "client-a");
        state.open("file:///shared.rs", "client-b");

        let closed = state.disconnect("client-a");
        assert_eq!(closed, vec!["file:///exclusive.rs"]);
    }

    #[test]
    fn disconnect_keeps_shared_files_open() {
        let mut state = DocumentState::new();
        state.open("file:///shared.rs", "client-a");
        state.open("file:///shared.rs", "client-b");

        let closed = state.disconnect("client-a");
        assert!(closed.is_empty());

        // client-b is still the opener
        assert!(state.close("file:///shared.rs", "client-b"));
    }

    #[test]
    fn disconnect_unknown_client_is_noop() {
        let mut state = DocumentState::new();
        state.open("file:///a.rs", "client-a");
        let closed = state.disconnect("client-x");
        assert!(closed.is_empty());
    }
}
