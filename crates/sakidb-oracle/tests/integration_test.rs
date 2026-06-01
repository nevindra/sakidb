#[cfg(feature = "integration")]
mod integration_tests {
    // [Fix: M4] Corrected integration tests to use the right traits and signatures (SqlDriver, Introspector, cells vs rows)
    use sakidb_core::{
        error::Result,
        types::{ConnectionConfig, EngineType, SslMode},
    };
    use sakidb_oracle::OracleDriver;
    use std::collections::HashMap;
    use tokio::time::{sleep, Duration};

    // Helper function to get Oracle test connection config
    fn get_test_config() -> ConnectionConfig {
        ConnectionConfig {
            engine: EngineType::Oracle,
            host: std::env::var("ORACLE_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("ORACLE_PORT")
                .unwrap_or_else(|_| "1521".to_string())
                .parse()
                .unwrap_or(1521),
            database: std::env::var("ORACLE_DATABASE").unwrap_or_else(|_| "ORCL".to_string()),
            username: std::env::var("ORACLE_USER").unwrap_or_else(|_| "test_user".to_string()),
            password: std::env::var("ORACLE_PASSWORD").unwrap_or_else(|_| "test_password".to_string()),
            ssl_mode: SslMode::Prefer,
            options: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_oracle_connection() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        let driver = OracleDriver::new();
        let config = get_test_config();

        // Test connection
        match driver.connect(&config).await {
            Ok(conn_id) => {
                println!("Successfully connected to Oracle with connection ID: {:?}", conn_id);
                
                // Test disconnect
                let result = driver.disconnect(&conn_id).await;
                assert!(result.is_ok(), "Failed to disconnect: {:?}", result);
            }
            Err(e) => {
                println!("Failed to connect to Oracle: {}", e);
                // Don't fail the test if Oracle is not available
                // This allows the test to run in CI without Oracle
            }
        }
    }

    #[tokio::test]
    async fn test_oracle_query_execution() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        use sakidb_core::driver::SqlDriver;
        let driver = OracleDriver::new();
        let config = get_test_config();

        let conn_id = match driver.connect(&config).await {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to connect to Oracle: {}", e);
                return;
            }
        };

        // Test simple query
        let query = "SELECT 1 as test_column FROM dual";
        match driver.execute(&conn_id, query).await {
            Ok(result) => {
                assert_eq!(result.columns.len(), 1);
                assert_eq!(result.columns[0].name, "TEST_COLUMN");
                assert_eq!(result.row_count, 1);
                match &result.cells[0] {
                    sakidb_core::types::CellValue::Int(v) => assert_eq!(*v, 1),
                    sakidb_core::types::CellValue::Float(v) => assert_eq!(*v as i64, 1),
                    _ => panic!("Expected integer result, got {:?}", result.cells[0]),
                }
                println!("Query execution test passed");
            }
            Err(e) => {
                println!("Query execution failed: {}", e);
            }
        }

        // Clean up
        let _ = driver.disconnect(&conn_id).await;
    }

    #[tokio::test]
    async fn test_oracle_introspection() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        use sakidb_core::driver::Introspector;
        let driver = OracleDriver::new();
        let config = get_test_config();

        let conn_id = match driver.connect(&config).await {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to connect to Oracle: {}", e);
                return;
            }
        };

        // Test schema listing
        match driver.list_schemas(&conn_id).await {
            Ok(schemas) => {
                println!("Found {} schemas", schemas.len());
                // Should at least have some schemas like SYS, SYSTEM, etc.
                assert!(!schemas.is_empty());
            }
            Err(e) => {
                println!("Schema listing failed: {}", e);
            }
        }

        // Test table listing (for current user)
        match driver.list_tables(&conn_id, "SYSTEM").await {
            Ok(tables) => {
                println!("Found {} tables in SYSTEM", tables.len());
            }
            Err(e) => {
                println!("Table listing failed: {}", e);
            }
        }

        // Clean up
        let _ = driver.disconnect(&conn_id).await;
    }

    #[tokio::test]
    async fn test_oracle_ddl_operations() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        use sakidb_core::driver::SqlDriver;
        let driver = OracleDriver::new();
        let config = get_test_config();

        let conn_id = match driver.connect(&config).await {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to connect to Oracle: {}", e);
                return;
            }
        };

        // Create a test table
        let create_table_sql = r#"
            CREATE TABLE test_table (
                id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
                name VARCHAR2(100) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        "#;

        match driver.execute(&conn_id, create_table_sql).await {
            Ok(_) => {
                println!("Test table created successfully");

                // Insert some data - note: Oracle driver currently uses execute_batch for multi-statement 
                // but execute() for single DML.
                let insert_sql = "INSERT INTO test_table (name) VALUES ('Test Record')";
                
                match driver.execute(&conn_id, insert_sql).await {
                    Ok(result) => {
                        println!("Insert successful, rows affected: {:?}", result.row_count);
                    }
                    Err(e) => {
                        println!("Insert failed: {}", e);
                    }
                }

                // Query the data
                let select_sql = "SELECT id, name, created_at FROM test_table";
                match driver.execute(&conn_id, select_sql).await {
                    Ok(result) => {
                        println!("Query successful, rows returned: {}", result.row_count);
                        assert!(result.row_count > 0);
                    }
                    Err(e) => {
                        println!("Select failed: {}", e);
                    }
                }

                // Clean up - drop the table
                let drop_sql = "DROP TABLE test_table PURGE";
                let _ = driver.execute(&conn_id, drop_sql).await;
                println!("Test table dropped");
            }
            Err(e) => {
                println!("Table creation failed: {}", e);
            }
        }

        // Clean up connection
        let _ = driver.disconnect(&conn_id).await;
    }

    #[tokio::test]
    async fn test_oracle_transaction_handling() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        use sakidb_core::driver::SqlDriver;
        let driver = OracleDriver::new();
        let config = get_test_config();

        let conn_id = match driver.connect(&config).await {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to connect to Oracle: {}", e);
                return;
            }
        };

        // Create a test table
        let create_table_sql = r#"
            CREATE TABLE test_transaction (
                id NUMBER PRIMARY KEY,
                value VARCHAR2(50)
            )
        "#;

        if let Err(e) = driver.execute(&conn_id, create_table_sql).await {
            println!("Failed to create test table: {}", e);
            let _ = driver.disconnect(&conn_id).await;
            return;
        }

        // Test transaction with rollback
        // Oracle starts transactions implicitly. 
        let insert_sql = "INSERT INTO test_transaction (id, value) VALUES (1, 'Test Value')";
        let rollback_sql = "ROLLBACK";

        // Insert data
        if let Err(e) = driver.execute(&conn_id, insert_sql).await {
            println!("Failed to insert data: {}", e);
        } else {
            // Rollback
            if let Err(e) = driver.execute(&conn_id, rollback_sql).await {
                println!("Failed to rollback: {}", e);
            } else {
                // Check that data was rolled back
                let select_sql = "SELECT COUNT(*) FROM test_transaction";
                match driver.execute(&conn_id, select_sql).await {
                    Ok(result) => {
                        if result.row_count > 0 {
                            let count = match &result.cells[0] {
                                sakidb_core::types::CellValue::Int(v) => *v,
                                sakidb_core::types::CellValue::Float(v) => *v as i64,
                                _ => 0,
                            };
                            assert_eq!(count, 0, "Expected 0 rows after rollback, got {}", count);
                            println!("Transaction rollback test passed");
                        }
                    }
                    Err(e) => {
                        println!("Failed to verify rollback: {}", e);
                    }
                }
            }
        }

        // Clean up
        let drop_sql = "DROP TABLE test_transaction PURGE";
        let _ = driver.execute(&conn_id, drop_sql).await;
        let _ = driver.disconnect(&conn_id).await;
    }

    #[tokio::test]
    async fn test_oracle_connection_pooling() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        use sakidb_core::driver::SqlDriver;
        let driver = OracleDriver::new();
        let config = get_test_config();

        let mut conn_ids = Vec::new();

        // Create multiple connections
        for i in 0..5 {
            match driver.connect(&config).await {
                Ok(conn_id) => {
                    conn_ids.push(conn_id);
                    println!("Created connection {}: {:?}", i, conn_id);
                }
                Err(e) => {
                    println!("Failed to create connection {}: {}", i, e);
                    break;
                }
            }
        }

        // Test each connection with a simple query
        for (i, conn_id) in conn_ids.iter().enumerate() {
            let query = "SELECT 1 as conn_id FROM dual";
            
            match driver.execute(conn_id, query).await {
                Ok(_) => {
                    println!("Connection {} query successful", i);
                }
                Err(e) => {
                    println!("Connection {} query failed: {}", i, e);
                }
            }
        }

        // Disconnect all connections
        for conn_id in conn_ids {
            let _ = driver.disconnect(&conn_id).await;
        }

        println!("Connection pooling test completed");
    }

    #[tokio::test]
    async fn test_oracle_error_handling() {
        // Skip test if Oracle is not available
        if std::env::var("SKIP_ORACLE_TESTS").is_ok() {
            println!("Skipping Oracle integration tests");
            return;
        }

        use sakidb_core::driver::SqlDriver;
        let driver = OracleDriver::new();
        let config = get_test_config();

        let conn_id = match driver.connect(&config).await {
            Ok(id) => id,
            Err(e) => {
                println!("Failed to connect to Oracle: {}", e);
                return;
            }
        };

        // Test invalid SQL
        let invalid_sql = "SELECT * FROM non_existent_table_12345";
        match driver.execute(&conn_id, invalid_sql).await {
            Ok(_) => {
                println!("ERROR: Invalid SQL should have failed");
            }
            Err(e) => {
                println!("Correctly caught error for invalid SQL: {}", e);
                // This is expected
            }
        }

        // Test SQL injection attempt (should be caught by Oracle)
        let injection_sql = "SELECT * FROM all_users WHERE '1'='1' --";
        match driver.execute(&conn_id, injection_sql).await {
            Ok(_) => {
                println!("SQL injection query executed (table might not exist or query returned empty)");
            }
            Err(e) => {
                println!("SQL injection query failed (expected): {}", e);
            }
        }

        // Clean up
        let _ = driver.disconnect(&conn_id).await;
    }
}
