//! # Monarch-DB
//!
//! Monarch-DB is a lightweight SQLite database migration tool designed to run whenever the first
//! connection in an app opens. It provides a simple, reliable way to manage SQLite database
//! schema evolution in Rust applications.
//!
//! ## Quick Start
//!
//! ```rust
//! use monarch_db::{StaticMonarchConfiguration, MonarchDB, ConnectionConfiguration};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Define your migrations at compile time
//! let config = StaticMonarchConfiguration {
//!     name: "my_app",
//!     enable_foreign_keys: true,
//!     migrations: [
//!         // Migration 1: Create users table
//!         r#"
//!         CREATE TABLE users (
//!             id INTEGER PRIMARY KEY AUTOINCREMENT,
//!             username TEXT NOT NULL UNIQUE,
//!             email TEXT NOT NULL,
//!             created_at DATETIME DEFAULT CURRENT_TIMESTAMP
//!         );
//!         "#,
//!         // Migration 2: Create posts table
//!         r#"
//!         CREATE TABLE posts (
//!             id INTEGER PRIMARY KEY AUTOINCREMENT,
//!             user_id INTEGER NOT NULL,
//!             title TEXT NOT NULL,
//!             content TEXT,
//!             created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
//!             FOREIGN KEY (user_id) REFERENCES users(id)
//!         );
//!         "#,
//!     ],
//! };
//!
//! // Convert to MonarchDB instance
//! let monarch_db: MonarchDB = config.into();
//!
//! // Create connection configuration
//! let connection_config = ConnectionConfiguration {
//!     database: None, // Use in-memory database for this example
//! };
//!
//! // Create database connection with migrations applied
//! let connection = monarch_db.create_connection(&connection_config)?;
//!
//! // Use your database normally
//! connection.execute(
//!     "INSERT INTO users (username, email) VALUES (?, ?)",
//!     ["alice2", "alice2@example.com"],
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Directory-Based Configuration
//!
//! Use directory-based configuration when you want to manage migrations as separate files:
//!
//! ```rust,no_run
//! use monarch_db::{MonarchConfiguration, MonarchDB, ConnectionConfiguration};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = MonarchConfiguration {
//!     name: "my_app".to_string(),
//!     enable_foreign_keys: true,
//!     migration_directory: "./migrations".into(),
//! };
//!
//! let monarch_db = MonarchDB::from_configuration(config)?;
//!
//! let connection_config = ConnectionConfiguration {
//!     database: Some("./my_app.db".into()),
//! };
//!
//! let connection = monarch_db.create_connection(&connection_config)?;
//!
//! // Database is ready with all migrations applied
//! # Ok(())
//! # }
//! ```
//!
//! ## Configuration Types
//!
//! - [`StaticMonarchConfiguration`] - For compile-time embedded migrations
//! - [`MonarchConfiguration`] - For runtime directory-based migrations
//! - [`ConnectionConfiguration`] - For specifying database file paths
//!
//! ## Core Types
//!
//! - [`MonarchDB`] - Main migration manager that applies schema changes
//! - [`Migrations`] - Helper for applying migrations to database connections
//!

use std::{borrow::Cow, collections::BTreeMap, io};

use camino::Utf8PathBuf;
use rusqlite::Connection;

type Migration = Cow<'static, str>;

const VERSION_TABLE: &str = "monarch_db_schema_version";

/// Configuration for opening a new SQLite database connection.
///
/// This struct controls how a database connection is established, including
/// whether to use a file-based database or an in-memory database.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct ConnectionConfiguration {
    /// Optional path to the database file.
    ///
    /// If `None`, an in-memory database will be used. If `Some`, the database
    /// will be persisted to the specified file path.
    #[cfg_attr(feature = "serde", serde(default))]
    pub database: Option<Utf8PathBuf>,
}

/// Configuration for MonarchDB that loads migrations from a directory at runtime.
///
/// This configuration is used when migrations are stored as separate files in a
/// directory and need to be loaded dynamically when the application starts.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct MonarchConfiguration {
    /// The name of the database schema, used for tracking migration versions.
    pub name: String,
    /// Whether to enable foreign key constraints in SQLite.
    pub enable_foreign_keys: bool,
    /// Path to the directory containing migration files.
    pub migration_directory: Utf8PathBuf,
}

/// Configuration for MonarchDB with compile-time known migrations.
///
/// This configuration is used when all migrations are embedded in the binary
/// at compile time, typically using `include_str!` or similar macros.
/// This provides better performance and eliminates runtime file I/O.
#[derive(Debug, Clone)]
pub struct StaticMonarchConfiguration<const N: usize> {
    /// The name of the database schema, used for tracking migration versions.
    pub name: &'static str,
    /// Whether to enable foreign key constraints in SQLite.
    pub enable_foreign_keys: bool,
    /// Array of migration SQL strings, ordered from oldest to newest.
    pub migrations: [&'static str; N],
}

impl<const N: usize> From<StaticMonarchConfiguration<N>> for MonarchDB {
    fn from(configuration: StaticMonarchConfiguration<N>) -> Self {
        MonarchDB {
            name: configuration.name.into(),
            enable_foreign_keys: configuration.enable_foreign_keys,
            migrations: configuration
                .migrations
                .iter()
                .map(|q| Cow::Borrowed(*q))
                .collect(),
        }
    }
}

/// MonarchDB manages schema migrations and new connections for a database.
#[derive(Debug)]
pub struct MonarchDB {
    name: Cow<'static, str>,
    enable_foreign_keys: bool,
    migrations: Vec<Migration>,
}

impl MonarchDB {
    /// Creates a new in-memory SQLite database connection with migrations applied.
    ///
    /// This is useful for testing or for applications that need a temporary database.
    /// All migrations will be automatically applied to the in-memory database.
    ///
    /// # Returns
    ///
    /// Returns a `rusqlite::Result<Connection>` with migrations applied on success.
    pub fn open_in_memory(&self) -> rusqlite::Result<Connection> {
        let connection = Connection::open_in_memory()?;
        self.migrations(connection)
    }

    /// Creates a new MonarchDB instance from a configuration that loads migrations from disk.
    ///
    /// This reads all migration files from the specified directory and creates a MonarchDB
    /// instance that can be used to manage database connections and schema migrations.
    ///
    /// # Arguments
    ///
    /// * `configuration` - A MonarchConfiguration containing the migration directory path,
    ///   database name, and foreign key settings.
    ///
    /// # Returns
    ///
    /// Returns a `io::Result<Self>` containing the configured MonarchDB instance.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The migration directory cannot be read
    /// - Any migration file cannot be read
    /// - File system operations fail
    pub fn from_configuration(configuration: MonarchConfiguration) -> io::Result<Self> {
        let mut migrations = BTreeMap::new();
        for diritem in configuration.migration_directory.read_dir_utf8()? {
            let entry = diritem?;

            if entry.file_type()?.is_file() {
                let query = std::fs::read_to_string(entry.path())?;
                migrations.insert(entry.file_name().to_owned(), Cow::from(query));
            }
        }

        Ok(MonarchDB {
            name: configuration.name.into(),
            enable_foreign_keys: configuration.enable_foreign_keys,
            migrations: migrations.into_values().collect(),
        })
    }

    /// Returns the current schema version, which is the number of migrations available.
    ///
    /// This represents the latest version that the database schema can be migrated to.
    ///
    /// # Returns
    ///
    /// Returns the number of migrations as a `u32`.
    pub fn current_version(&self) -> u32 {
        self.migrations.len() as u32
    }

    fn get_migration(&self, version: u32) -> Option<&str> {
        self.migrations
            .get(version as usize)
            .map(|query| query.as_ref())
    }

    /// Creates a new SQLite database connection with migrations applied.
    ///
    /// If a database path is specified in the configuration, opens that file.
    /// Otherwise, creates an in-memory database. All migrations will be automatically
    /// applied to ensure the schema is up to date.
    ///
    /// # Arguments
    ///
    /// * `configuration` - A ConnectionConfiguration specifying the database path.
    ///   If `database` is None, an in-memory database will be created.
    ///
    /// # Returns
    ///
    /// Returns a `rusqlite::Result<Connection>` with migrations applied on success.
    pub fn create_connection(
        &self,
        configuration: &ConnectionConfiguration,
    ) -> rusqlite::Result<Connection> {
        let connection = if let Some(path) = configuration.database.as_deref() {
            Connection::open(path)?
        } else {
            Connection::open_in_memory()?
        };
        self.migrations(connection)
    }

    /// Applies all necessary migrations to an existing database connection.
    ///
    /// This method takes ownership of a connection and returns it after applying
    /// all migrations to bring the schema up to the current version. It will
    /// also configure foreign key constraints if enabled.
    ///
    /// # Arguments
    ///
    /// * `connection` - An existing SQLite connection to migrate.
    ///
    /// # Returns
    ///
    /// Returns the connection with migrations applied on success.
    pub fn migrations(&self, mut connection: Connection) -> rusqlite::Result<Connection> {
        let migrations = Migrations {
            connection: &mut connection,
            monarch: self,
        };
        migrations.prepare()?;
        Ok(connection)
    }
}

/// Helper struct for applying migrations to a database connection.
///
/// This struct manages the migration process, ensuring that the database
/// schema is brought up to the current version by applying any pending migrations.
pub struct Migrations<'c> {
    connection: &'c mut Connection,
    monarch: &'c MonarchDB,
}

impl<'c> Migrations<'c> {
    /// Prepares the database connection by configuring settings and applying migrations.
    ///
    /// This method performs the following operations:
    /// 1. Enables foreign key constraints if configured
    /// 2. Applies any pending migrations to bring the schema up to date
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or a `rusqlite::Error` if any operation fails.
    #[tracing::instrument(level = "trace", skip_all, fields(monarch=%self.monarch.name))]
    pub fn prepare(self) -> rusqlite::Result<()> {
        if self.monarch.enable_foreign_keys {
            tracing::trace!("Set foreign keys");
            self.connection.pragma_update(None, "foreign_keys", true)?;
        }
        self.migrate()?;
        Ok(())
    }

    fn migrate(self) -> rusqlite::Result<()> {
        let tx = self.connection.transaction()?;
        let mut version = select_schema_version(&tx, &self.monarch.name)?;

        while version < self.monarch.current_version() {
            let query = self
                .monarch
                .get_migration(version)
                .expect("version <-> migration mismatch");
            tracing::trace!("Running migration to version {}", version + 1);
            tx.execute_batch(query)?;
            version += 1;
        }

        set_schema_version(&tx, &self.monarch.name, version)?;
        tx.commit()?;
        tracing::debug!("Migrations complete");
        Ok(())
    }
}

fn create_schema_version_table(connection: &Connection) -> rusqlite::Result<()> {
    let mut stmt = connection.prepare(include_str!("00.versions.sql"))?;
    stmt.execute([])?;
    Ok(())
}

fn insert_initial_schema_version(connection: &Connection, name: &str) -> rusqlite::Result<()> {
    let mut stmt = connection.prepare(&format!(
        "INSERT INTO {VERSION_TABLE} (monarch_schema, version) VALUES (:name, 0)"
    ))?;
    stmt.execute(&[(":name", name)])?;
    Ok(())
}

fn select_schema_version(connection: &Connection, name: &str) -> rusqlite::Result<u32> {
    let mut stmt = connection.prepare("SELECT name FROM sqlite_master WHERE name = :table")?;

    let has_version_tbl: Option<Result<String, _>> = stmt
        .query_map(&[(":table", VERSION_TABLE)], |row| row.get(0))?
        .next();

    match has_version_tbl {
        Some(Ok(_)) => {}
        Some(Err(error)) => {
            return Err(error);
        }
        None => {
            tracing::trace!("Create schema version table {VERSION_TABLE}");
            create_schema_version_table(connection)?;
            insert_initial_schema_version(connection, name)?;
            return Ok(0u32);
        }
    };

    let mut stmt = connection.prepare(&format!(
        "SELECT version FROM {VERSION_TABLE} WHERE monarch_schema = :name"
    ))?;
    let version: Option<u32> = stmt
        .query_map(&[(":name", name)], |row| row.get::<_, u32>(0))?
        .next()
        .transpose()?;
    if let Some(version) = version {
        tracing::trace!(%version, "Get schema version");
        Ok(version)
    } else {
        tracing::trace!("Insert new version for {name}");
        insert_initial_schema_version(connection, name)?;
        Ok(0)
    }
}

fn set_schema_version(connection: &Connection, name: &str, version: u32) -> rusqlite::Result<()> {
    tracing::trace!(%version, "Set schema version for {name}");
    let mut stmt = connection.prepare(&format!(
        "UPDATE {VERSION_TABLE} SET version = :version WHERE monarch_schema = :name"
    ))?;
    stmt.execute(rusqlite::named_params! { ":version": version, ":name": name})?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_monarch_configuration_creation() {
        let config = StaticMonarchConfiguration {
            name: "test_db",
            enable_foreign_keys: true,
            migrations: [
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "ALTER TABLE users ADD COLUMN email TEXT;",
            ],
        };

        assert_eq!(config.name, "test_db");
        assert!(config.enable_foreign_keys);
        assert_eq!(config.migrations.len(), 2);
    }

    #[test]
    fn test_static_configuration_to_monarch_db() {
        let config = StaticMonarchConfiguration {
            name: "test_db",
            enable_foreign_keys: false,
            migrations: ["CREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT NOT NULL);"],
        };

        let monarch_db: MonarchDB = config.into();
        assert_eq!(monarch_db.current_version(), 1);
        assert_eq!(monarch_db.name, "test_db");
        assert!(!monarch_db.enable_foreign_keys);
    }

    #[test]
    fn test_open_in_memory_with_static_migrations() -> rusqlite::Result<()> {
        let config = StaticMonarchConfiguration {
            name: "test_memory_db",
            enable_foreign_keys: true,
            migrations: [
                "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
                "CREATE INDEX idx_items_name ON items(name);",
            ],
        };

        let monarch_db: MonarchDB = config.into();
        let connection = monarch_db.open_in_memory()?;

        // Verify the table was created
        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='items'")?;
        let table_exists: bool = stmt.query_map([], |_| Ok(true))?.next().is_some();
        assert!(table_exists);

        // Verify the index was created
        let mut stmt = connection.prepare(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_items_name'",
        )?;
        let index_exists: bool = stmt.query_map([], |_| Ok(true))?.next().is_some();
        assert!(index_exists);

        Ok(())
    }

    #[test]
    fn test_create_connection_with_static_migrations() -> rusqlite::Result<()> {
        let config = StaticMonarchConfiguration {
            name: "test_file_db",
            enable_foreign_keys: false,
            migrations: [
                "CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT NOT NULL, price REAL);",
            ],
        };

        let monarch_db: MonarchDB = config.into();
        let connection_config = ConnectionConfiguration { database: None };
        let connection = monarch_db.create_connection(&connection_config)?;

        // Verify the table was created
        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='products'")?;
        let table_exists: bool = stmt.query_map([], |_| Ok(true))?.next().is_some();
        assert!(table_exists);

        // Test inserting data
        connection.execute(
            "INSERT INTO products (name, price) VALUES (?, ?)",
            ["Test Product", "19.99"],
        )?;

        // Verify data was inserted
        let mut stmt = connection.prepare("SELECT COUNT(*) FROM products")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    fn test_migration_versioning() -> rusqlite::Result<()> {
        let config = StaticMonarchConfiguration {
            name: "versioning_test",
            enable_foreign_keys: false,
            migrations: [
                "CREATE TABLE v1_table (id INTEGER PRIMARY KEY);",
                "CREATE TABLE v2_table (id INTEGER PRIMARY KEY);",
                "CREATE TABLE v3_table (id INTEGER PRIMARY KEY);",
            ],
        };

        let monarch_db: MonarchDB = config.into();
        assert_eq!(monarch_db.current_version(), 3);

        let connection = monarch_db.open_in_memory()?;

        // Verify all tables were created
        let table_names = ["v1_table", "v2_table", "v3_table"];
        for table_name in table_names {
            let mut stmt = connection.prepare(&format!(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='{table_name}'"
            ))?;
            let table_exists: bool = stmt.query_map([], |_| Ok(true))?.next().is_some();
            assert!(table_exists, "Table {table_name} should exist");
        }

        Ok(())
    }
}
