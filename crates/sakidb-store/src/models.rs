use serde::{Deserialize, Serialize};

// ── Connection models ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    #[serde(skip)]
    pub password: String,
    pub ssl_mode: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_connected_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInput {
    pub name: String,
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub ssl_mode: String,
}

// ── Query models ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    pub sql: String,
    pub connection_id: Option<String>,
    pub database_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: String,
    pub sql: String,
    pub connection_id: Option<String>,
    pub database_name: Option<String>,
    pub executed_at: String,
    pub execution_time_ms: Option<i64>,
    pub row_count: Option<i64>,
}
