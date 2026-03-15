//! Operational modes and contexts (mirrors Serena's context/mode system).

use serde::{Deserialize, Serialize};

/// A named operational mode that adjusts which tools are enabled and
/// how the agent presents itself to the LLM.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Read-only planning pass — no editing tools exposed
    Planning,
    /// Full editing capabilities
    Editing,
    /// Interactive session with all tools
    #[default]
    Interactive,
    /// Single-shot one-time task
    OneShot,
}

/// Deployment context that configures tool visibility and prompts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Context {
    /// Running as a standalone MCP server agent
    #[default]
    Agent,
    /// Embedded in a desktop IDE-like application
    DesktopApp,
    /// Assisting an IDE extension
    IdeAssistant,
}
