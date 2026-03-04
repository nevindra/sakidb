use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use std::time::Duration;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;
use tokio_postgres::NoTls;
use tokio_postgres_rustls::MakeRustlsConnect;
use tracing::{debug, error, info, warn};

use sakidb_core::types::{ConnectionConfig, ConnectionId, SslMode};
use sakidb_core::SakiError;

/// A TLS certificate verifier that accepts any server certificate.
/// This matches PostgreSQL's `sslmode=require` behavior: encrypt the connection
/// but don't verify the server's identity. Certificate verification would
/// require `sslmode=verify-ca` or `sslmode=verify-full`, which we don't support yet.
#[derive(Debug)]
struct NoServerVerification;

impl ServerCertVerifier for NoServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn make_tls_connector() -> MakeRustlsConnect {
    // Ensure ring crypto provider is available (idempotent — ignores if already installed)
    let _ = rustls::crypto::ring::default_provider().install_default();

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoServerVerification))
        .with_no_client_auth();

    MakeRustlsConnect::new(config)
}

pub struct ConnectionManager {
    pools: Arc<RwLock<HashMap<ConnectionId, Pool>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn connect(&self, config: &ConnectionConfig) -> Result<ConnectionId, SakiError> {
        let pg_config = build_pg_config(config);

        info!(
            host = %config.host,
            port = config.port,
            database = %config.database,
            user = %config.username,
            ssl_mode = ?config.ssl_mode,
            "connecting to database"
        );

        // Verified recycling pings the server when reusing idle connections,
        // detecting dead connections from server restarts or network drops.
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Verified,
        };

        // Pool sizing: 5 connections allows execute_paged (which needs 2 concurrent)
        // plus introspect calls without exhausting the pool. Timeouts prevent
        // indefinite blocking if the pool is saturated.
        let pool = match config.ssl_mode {
            SslMode::Disable => {
                let mgr = Manager::from_config(pg_config, NoTls, mgr_config);
                Pool::builder(mgr)
                    .max_size(5)
                    .wait_timeout(Some(Duration::from_secs(30)))
                    .create_timeout(Some(Duration::from_secs(10)))
                    .recycle_timeout(Some(Duration::from_secs(5)))
                    .runtime(Runtime::Tokio1)
                    .build()
                    .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?
            }
            SslMode::Prefer | SslMode::Require => {
                let tls = make_tls_connector();
                let mgr = Manager::from_config(pg_config, tls, mgr_config);
                Pool::builder(mgr)
                    .max_size(5)
                    .wait_timeout(Some(Duration::from_secs(30)))
                    .create_timeout(Some(Duration::from_secs(10)))
                    .recycle_timeout(Some(Duration::from_secs(5)))
                    .runtime(Runtime::Tokio1)
                    .build()
                    .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?
            }
        };

        // Verify the connection actually works
        let _ = pool
            .get()
            .await
            .map_err(|e| {
                error!(
                    host = %config.host,
                    port = config.port,
                    database = %config.database,
                    error = %e,
                    "connection failed"
                );
                SakiError::ConnectionFailed(e.to_string())
            })?;

        let id = ConnectionId::new();
        self.pools.write().await.insert(id, pool);

        info!(conn_id = %id.0, host = %config.host, database = %config.database, "connected");

        Ok(id)
    }

    pub async fn disconnect(&self, conn_id: &ConnectionId) -> Result<(), SakiError> {
        let mut pools = self.pools.write().await;
        if let Some(pool) = pools.remove(conn_id) {
            pool.close();
            info!(conn_id = %conn_id.0, "disconnected");
            Ok(())
        } else {
            warn!(conn_id = %conn_id.0, "disconnect: connection not found");
            Err(SakiError::ConnectionNotFound(conn_id.0.to_string()))
        }
    }

    pub async fn get_pool(&self, conn_id: &ConnectionId) -> Result<Pool, SakiError> {
        self.pools
            .read()
            .await
            .get(conn_id)
            .cloned()
            .ok_or_else(|| SakiError::ConnectionNotFound(conn_id.0.to_string()))
    }

    pub async fn test_connection(config: &ConnectionConfig) -> Result<(), SakiError> {
        let pg_config = build_pg_config(config);

        debug!(
            host = %config.host,
            port = config.port,
            database = %config.database,
            user = %config.username,
            ssl_mode = ?config.ssl_mode,
            "testing connection"
        );

        match config.ssl_mode {
            SslMode::Disable => {
                let (client, connection) = pg_config
                    .connect(NoTls)
                    .await
                    .map_err(|e| {
                        warn!(host = %config.host, error = %e, "test connection failed");
                        SakiError::ConnectionFailed(e.to_string())
                    })?;

                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        debug!(error = %e, "test connection background error");
                    }
                });

                client
                    .simple_query("SELECT 1")
                    .await
                    .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?;
            }
            SslMode::Prefer | SslMode::Require => {
                let tls = make_tls_connector();
                let (client, connection) = pg_config
                    .connect(tls)
                    .await
                    .map_err(|e| {
                        warn!(host = %config.host, error = %e, "test connection failed (SSL)");
                        SakiError::ConnectionFailed(e.to_string())
                    })?;

                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        debug!(error = %e, "test connection background error");
                    }
                });

                client
                    .simple_query("SELECT 1")
                    .await
                    .map_err(|e| SakiError::ConnectionFailed(e.to_string()))?;
            }
        }

        info!(host = %config.host, database = %config.database, "test connection successful");

        Ok(())
    }
}

fn build_pg_config(config: &ConnectionConfig) -> tokio_postgres::Config {
    let mut pg_config = tokio_postgres::Config::new();
    pg_config
        .host(&config.host)
        .port(config.port)
        .dbname(&config.database)
        .user(&config.username)
        .password(&config.password);

    match config.ssl_mode {
        SslMode::Disable => {
            pg_config.ssl_mode(tokio_postgres::config::SslMode::Disable);
        }
        SslMode::Prefer => {
            pg_config.ssl_mode(tokio_postgres::config::SslMode::Prefer);
        }
        SslMode::Require => {
            pg_config.ssl_mode(tokio_postgres::config::SslMode::Require);
        }
    }

    pg_config
}
