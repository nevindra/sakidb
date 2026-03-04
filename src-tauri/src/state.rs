use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

use dashmap::DashMap;
use sakidb_core::types::ConnectionId;
use sakidb_postgres::PostgresDriver;
use sakidb_store::Store;

pub struct AppState {
    pub driver: Arc<PostgresDriver>,
    pub store: Arc<Mutex<Store>>,
    pub restore_cancelled: Arc<AtomicBool>,
    pub export_cancel_flags: Arc<DashMap<ConnectionId, Arc<AtomicBool>>>,
}
