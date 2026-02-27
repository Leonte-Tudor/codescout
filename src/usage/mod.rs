pub mod db;

use crate::agent::Agent;
use anyhow::Result;
use serde_json::Value;
use std::time::Instant;

pub struct UsageRecorder {
    agent: Agent,
}

impl UsageRecorder {
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    pub async fn record<F, Fut>(&self, tool_name: &str, f: F) -> Result<Value>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Value>>,
    {
        let start = Instant::now();
        let result = f().await;
        let latency_ms = start.elapsed().as_millis() as i64;
        // Best-effort — never let recording fail the tool call
        let _ = self.write(tool_name, latency_ms, &result).await;
        result
    }

    async fn write(&self, tool_name: &str, latency_ms: i64, result: &Result<Value>) -> Result<()> {
        let project_root = self.agent.with_project(|p| Ok(p.root.clone())).await?;
        let conn = db::open_db(&project_root)?;
        let (outcome, overflowed, error_msg) = classify_result(result);
        db::write_record(
            &conn,
            tool_name,
            latency_ms,
            outcome,
            overflowed,
            error_msg.as_deref(),
        )?;
        Ok(())
    }
}

pub(crate) fn classify_result(result: &Result<Value>) -> (&'static str, bool, Option<String>) {
    match result {
        Err(e) => ("error", false, Some(e.to_string())),
        Ok(v) => {
            if let Some(msg) = v.get("error").and_then(Value::as_str) {
                ("recoverable_error", false, Some(msg.to_string()))
            } else if v.get("overflow").is_some() {
                ("success", true, None)
            } else {
                ("success", false, None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classify_error_result() {
        let r: anyhow::Result<serde_json::Value> = Err(anyhow::anyhow!("boom"));
        let (outcome, overflowed, msg) = classify_result(&r);
        assert_eq!(outcome, "error");
        assert!(!overflowed);
        assert_eq!(msg.as_deref(), Some("boom"));
    }

    #[test]
    fn classify_recoverable_error() {
        let v = json!({ "error": "path not found", "hint": "check path" });
        let r: anyhow::Result<serde_json::Value> = Ok(v);
        let (outcome, overflowed, msg) = classify_result(&r);
        assert_eq!(outcome, "recoverable_error");
        assert!(!overflowed);
        assert_eq!(msg.as_deref(), Some("path not found"));
    }

    #[test]
    fn classify_overflow_success() {
        let v = json!({ "symbols": [], "overflow": { "shown": 200, "total": 500 } });
        let r: anyhow::Result<serde_json::Value> = Ok(v);
        let (outcome, overflowed, _msg) = classify_result(&r);
        assert_eq!(outcome, "success");
        assert!(overflowed);
    }

    #[test]
    fn classify_clean_success() {
        let v = json!({ "symbols": [{"name": "foo"}] });
        let r: anyhow::Result<serde_json::Value> = Ok(v);
        let (outcome, overflowed, msg) = classify_result(&r);
        assert_eq!(outcome, "success");
        assert!(!overflowed);
        assert!(msg.is_none());
    }
}
