#[cfg(test)]
mod tests {
    use crate::connection::OracleDriver;
    use sakidb_core::driver::Driver;
    use sakidb_core::types::{ConnectionConfig, EngineType, SslMode};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_oracle_driver_creation() {
        let driver = OracleDriver::new();
        assert_eq!(driver.engine_type(), EngineType::Oracle);
    }

    #[tokio::test]
    async fn test_oracle_capabilities() {
        let driver = OracleDriver::new();
        let capabilities = driver.capabilities();

        assert!(capabilities.sql);
        assert!(capabilities.introspection);
        assert!(capabilities.export);
        assert!(capabilities.restore);
        assert!(!capabilities.key_value);
        assert!(!capabilities.document);

        assert!(capabilities.schemas);
        assert!(capabilities.tables);
        assert!(capabilities.views);
        assert!(capabilities.materialized_views);
        assert!(capabilities.functions);
        assert!(capabilities.sequences);
        assert!(capabilities.indexes);
        assert!(capabilities.triggers);
        assert!(capabilities.partitions);
        assert!(capabilities.explain);
        assert!(!capabilities.multi_database);
    }

    #[tokio::test]
    async fn test_build_connection_string() {
        let config = ConnectionConfig {
            engine: EngineType::Oracle,
            host: "localhost".to_string(),
            port: 1521,
            database: "ORCL".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            ssl_mode: SslMode::Prefer,
            options: HashMap::new(),
        };

        let connection_string = OracleDriver::build_connection_string(&config);
        assert_eq!(connection_string, "localhost:1521/ORCL");
    }

    #[tokio::test]
    async fn test_build_connection_string_with_tns() {
        let mut options = HashMap::new();
        options.insert("tns".to_string(), "ORCL_TNS".to_string());

        let config = ConnectionConfig {
            engine: EngineType::Oracle,
            host: "localhost".to_string(),
            port: 1521,
            database: "ORCL".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
            ssl_mode: SslMode::Prefer,
            options,
        };

        let connection_string = OracleDriver::build_connection_string(&config);
        assert_eq!(connection_string, "ORCL_TNS");
    }
}
