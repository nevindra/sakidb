#[cfg(test)]
mod tests {
    use crate::executor::OracleExecutor;
    use oracle::sql_type::OracleType;
    use sakidb_core::error::SakiError;
    use sakidb_core::types::ConnectionId;
    use std::sync::Arc;
    use dashmap::DashMap;

    #[test]
    fn test_convert_oracle_type_to_string() {
        assert_eq!(OracleExecutor::convert_oracle_type_to_string(&OracleType::Varchar2(100)), "VARCHAR2");
        assert_eq!(OracleExecutor::convert_oracle_type_to_string(&OracleType::Number(10, 2)), "NUMBER");
        assert_eq!(OracleExecutor::convert_oracle_type_to_string(&OracleType::Date), "DATE");
        assert_eq!(OracleExecutor::convert_oracle_type_to_string(&OracleType::Timestamp(6)), "TIMESTAMP");
    }

    #[test]
    fn test_is_query_detection() {
        assert!(OracleExecutor::is_query("SELECT * FROM users"));
        assert!(OracleExecutor::is_query("  select id from t"));
        assert!(OracleExecutor::is_query("WITH cte AS (SELECT 1) SELECT * FROM cte"));
        assert!(!OracleExecutor::is_query("INSERT INTO t VALUES (1)"));
        assert!(!OracleExecutor::is_query("UPDATE t SET x = 1"));
        assert!(!OracleExecutor::is_query("DELETE FROM t"));
        assert!(!OracleExecutor::is_query("CREATE TABLE t (id NUMBER)"));
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let connections = Arc::new(DashMap::new());
        let executor = OracleExecutor::new(connections);
        assert!(executor.connections.is_empty());
    }

    #[test]
    fn test_get_connection_not_found() {
        let connections = Arc::new(DashMap::new());
        let executor = OracleExecutor::new(connections);
        let conn_id = ConnectionId::new();

        let result = executor.get_connection(&conn_id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SakiError::ConnectionNotFound(_)));
    }
}
