use super::super::routes::DashboardState;
use crate::config::project::ProjectConfig;
use axum::extract::State;
use axum::Json;
use serde_json::Value;

pub async fn get_config(State(state): State<DashboardState>) -> Json<Value> {
    let config = ProjectConfig::load_or_default(&state.project_root)
        .unwrap_or_else(|_| ProjectConfig::default_for("unknown".to_string()));
    Json(serde_json::to_value(config).unwrap_or_default())
}
