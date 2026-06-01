#[cfg(test)]
mod tests {
    use crate::introspect::OracleIntrospector;
    use oracle::Connection as OracleConnection;
    use sakidb_core::error::SakiError;
    use sakidb_core::types::ConnectionId;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use dashmap::DashMap;

    #[tokio::test]
    async fn test_introspector_creation() {
        let connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>> = Arc::new(DashMap::new());
        let introspector = OracleIntrospector::new(connections);
        assert!(introspector.connections.is_empty());
    }

    #[test]
    fn test_get_connection_not_found() {
        let connections: Arc<DashMap<ConnectionId, Arc<RwLock<OracleConnection>>>> = Arc::new(DashMap::new());
        let introspector = OracleIntrospector::new(connections);
        let conn_id = ConnectionId::new();

        let result = introspector.get_connection(&conn_id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SakiError::ConnectionNotFound(_)));
    }

    #[test]
    fn test_sql_query_generation() {
        let schema = "TEST_SCHEMA";
        let table = "TEST_TABLE";

        let expected = format!(
            "SELECT column_name, data_type, nullable, data_default\
             FROM all_tab_columns\
             WHERE owner = UPPER('{}') AND table_name = UPPER('{}')\
             ORDER BY column_id",
            schema, table
        );

        assert!(expected.contains("UPPER('TEST_SCHEMA')"));
        assert!(expected.contains("UPPER('TEST_TABLE')"));
    }

    #[test]
    fn test_foreign_key_query_formatting() {
        let schema = "TEST_SCHEMA";
        let table = "TEST_TABLE";

        let query = format!(
            "SELECT a.constraint_name FROM all_constraints a \
             WHERE a.owner = UPPER('{}') AND a.table_name = UPPER('{}') \
             AND a.constraint_type = 'R'",
            schema, table
        );

        assert!(query.contains("UPPER('TEST_SCHEMA')"));
        assert!(query.contains("UPPER('TEST_TABLE')"));
        assert!(query.contains("constraint_type = 'R'"));
    }
}
