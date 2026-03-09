use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;

use sakidb_core::driver::*;
use sakidb_core::error::Result;
use sakidb_core::types::*;
use sakidb_core::SakiError;

/// Multiple trait Arcs pointing to the same driver allocation.
pub struct DriverEntry {
    pub driver: Arc<dyn Driver>,
    pub sql: Option<Arc<dyn SqlDriver>>,
    pub introspector: Option<Arc<dyn Introspector>>,
    pub exporter: Option<Arc<dyn Exporter>>,
    pub restorer: Option<Arc<dyn Restorer>>,
    pub formatter: Option<Arc<dyn SqlFormatter>>,
    pub key_value: Option<Arc<dyn KeyValueDriver>>,
    pub document: Option<Arc<dyn DocumentDriver>>,
}

pub struct DriverRegistry {
    entries: HashMap<EngineType, DriverEntry>,
    connections: DashMap<ConnectionId, EngineType>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            connections: DashMap::new(),
        }
    }

    pub fn register(&mut self, engine: EngineType, entry: DriverEntry) {
        self.entries.insert(engine, entry);
    }

    pub fn available_engines(&self) -> Vec<EngineType> {
        self.entries.keys().copied().collect()
    }

    /// Connect — routes to the right driver, records connection ownership.
    pub async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId> {
        let entry = self
            .entries
            .get(&config.engine)
            .ok_or(SakiError::NotSupported(format!("{:?}", config.engine)))?;
        let conn_id = entry.driver.connect(config).await?;
        self.connections.insert(conn_id, config.engine);
        Ok(conn_id)
    }

    /// Disconnect — routes and cleans up ownership mapping.
    pub async fn disconnect(&self, conn_id: &ConnectionId) -> Result<()> {
        let driver = self.driver_for(conn_id)?;
        driver.disconnect(conn_id).await?;
        self.connections.remove(conn_id);
        Ok(())
    }

    pub fn driver_for(&self, conn_id: &ConnectionId) -> Result<&dyn Driver> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .map(|e| e.driver.as_ref())
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))
    }

    pub fn driver_by_engine(&self, engine: &EngineType) -> Result<&dyn Driver> {
        self.entries
            .get(engine)
            .map(|e| e.driver.as_ref())
            .ok_or(SakiError::NotSupported(format!("{:?}", engine)))
    }

    pub fn sql_for(&self, conn_id: &ConnectionId) -> Result<&dyn SqlDriver> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .and_then(|e| e.sql.as_deref())
            .ok_or(SakiError::NotSupported("SQL".into()))
    }

    pub fn introspector_for(&self, conn_id: &ConnectionId) -> Result<&dyn Introspector> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .and_then(|e| e.introspector.as_deref())
            .ok_or(SakiError::NotSupported("introspection".into()))
    }

    pub fn exporter_for(&self, conn_id: &ConnectionId) -> Result<&dyn Exporter> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .and_then(|e| e.exporter.as_deref())
            .ok_or(SakiError::NotSupported("export".into()))
    }

    pub fn restorer_for(&self, conn_id: &ConnectionId) -> Result<&dyn Restorer> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .and_then(|e| e.restorer.as_deref())
            .ok_or(SakiError::NotSupported("restore".into()))
    }

    pub fn formatter_for(&self, conn_id: &ConnectionId) -> Result<&dyn SqlFormatter> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .and_then(|e| e.formatter.as_deref())
            .ok_or(SakiError::NotSupported("sql formatter".into()))
    }

    pub fn formatter_arc_for(&self, conn_id: &ConnectionId) -> Result<Arc<dyn SqlFormatter>> {
        let engine = *self
            .connections
            .get(conn_id)
            .ok_or(SakiError::ConnectionNotFound(conn_id.0.to_string()))?
            .value();
        self.entries
            .get(&engine)
            .and_then(|e| e.formatter.clone())
            .ok_or(SakiError::NotSupported("sql formatter".into()))
    }

    pub fn capabilities_for(&self, conn_id: &ConnectionId) -> Result<EngineCapabilities> {
        Ok(self.driver_for(conn_id)?.capabilities())
    }
}
