mod connection;
mod executor;
mod formatter;
pub mod instantclient;
mod introspect;
mod restore;
pub(crate) mod sql_split;

pub use connection::OracleDriver;
pub use executor::OracleExecutor;
pub use formatter::OracleFormatter;
pub use instantclient::{
    download_instantclient_with_progress, ensure_instantclient, get_driver_status,
    OracleDriverStatus,
};
pub use introspect::OracleIntrospector;
pub use restore::OracleRestorer;

#[cfg(test)]
mod connection_test;
#[cfg(test)]
mod executor_test;
#[cfg(test)]
mod formatter_test;
#[cfg(test)]
mod introspect_test;
