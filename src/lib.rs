use std::{borrow::Cow, collections::BTreeMap, io};

use camino::Utf8PathBuf;
use rusqlite::Connection;
use serde::Deserialize;

type Migration = Cow<'static, str>;

const VERSION_TABLE: &str = "monarch_db_schema_version";

/// ConnectionConfiguration describes how to open a new Sqlite connection.
#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfiguration {
    #[serde(default)]
    pub database: Option<Utf8PathBuf>,
}

/// MonarchConfiguration describes migrations stored in a directory and read at runtime.
#[derive(Debug, Clone, Deserialize)]
pub struct MonarchConfiguration {
    pub name: String,
    pub enable_foreign_keys: bool,
    pub migration_directory: Utf8PathBuf,
}

/// StaticMonarchConfiguration is a configuration for MonarchDB that is used when the migrations are known at compile time.
#[derive(Debug, Clone)]
pub struct StaticMonarchConfiguration<const N: usize> {
    pub name: &'static str,
    pub enable_foreign_keys: bool,
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
    pub fn open_in_memory(&self) -> rusqlite::Result<Connection> {
        let connection = Connection::open_in_memory()?;
        self.migrations(connection)
    }

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

    pub fn current_version(&self) -> u32 {
        self.migrations.len() as u32
    }

    fn get_migration(&self, version: u32) -> Option<&str> {
        self.migrations
            .get(version as usize)
            .map(|query| query.as_ref())
    }

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

    pub fn migrations(&self, mut connection: Connection) -> rusqlite::Result<Connection> {
        let migrations = Migrations {
            connection: &mut connection,
            monarch: self,
        };
        migrations.prepare()?;
        Ok(connection)
    }
}

pub struct Migrations<'c> {
    connection: &'c mut Connection,
    monarch: &'c MonarchDB,
}

impl<'c> Migrations<'c> {
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
