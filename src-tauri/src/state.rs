use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

use dashmap::DashMap;
use sakidb_core::types::ConnectionId;
use sakidb_store::Store;

use crate::registry::DriverRegistry;

pub struct AppState {
    pub registry: Arc<DriverRegistry>,
    pub store: Arc<Mutex<Store>>,
    pub restore_cancelled: Arc<AtomicBool>,
    pub export_cancel_flags: Arc<DashMap<ConnectionId, Arc<AtomicBool>>>,
}
