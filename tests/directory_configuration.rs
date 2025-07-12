use camino::Utf8PathBuf;
use monarch_db::{ConnectionConfiguration, MonarchConfiguration, MonarchDB};
use rusqlite::Connection;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_directory_configuration_with_file_database() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let migrations_dir = temp_dir.path().join("migrations");
    let db_path = temp_dir.path().join("directory_test.db");

    // Copy migration files to temp directory
    fs::create_dir_all(&migrations_dir)?;
    copy_migration_files(&migrations_dir)?;

    let config = MonarchConfiguration {
        name: "blog_directory".to_string(),
        enable_foreign_keys: true,
        migration_directory: Utf8PathBuf::from_path_buf(migrations_dir.to_path_buf())
            .map_err(|_| "Invalid UTF-8 path")?,
    };

    let monarch_db = MonarchDB::from_configuration(config)?;
    let connection_config = ConnectionConfiguration {
        database: Some(Utf8PathBuf::from_path_buf(db_path).map_err(|_| "Invalid UTF-8 path")?),
    };

    let connection = monarch_db.create_connection(&connection_config)?;

    // Verify all tables and indexes were created
    verify_complete_schema(&connection)?;

    // Test data operations with foreign keys enabled
    test_foreign_key_constraints(&connection)?;

    Ok(())
}

#[test]
fn test_directory_configuration_partial_migrations() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let migrations_dir = temp_dir.path().join("migrations");
    let db_path = temp_dir.path().join("partial_test.db");

    // Create directory with only first two migrations
    fs::create_dir_all(&migrations_dir)?;
    copy_partial_migration_files(&migrations_dir)?;

    let config = MonarchConfiguration {
        name: "partial_blog".to_string(),
        enable_foreign_keys: false,
        migration_directory: Utf8PathBuf::from_path_buf(migrations_dir.to_path_buf())
            .map_err(|_| "Invalid UTF-8 path")?,
    };

    let monarch_db = MonarchDB::from_configuration(config)?;
    assert_eq!(monarch_db.current_version(), 2);

    let connection_config = ConnectionConfiguration {
        database: Some(Utf8PathBuf::from_path_buf(db_path).map_err(|_| "Invalid UTF-8 path")?),
    };

    let connection = monarch_db.create_connection(&connection_config)?;

    // Verify only users and posts tables exist, not indexes
    verify_partial_schema(&connection)?;

    Ok(())
}

#[test]
fn test_directory_configuration_incremental_migration() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let migrations_dir = temp_dir.path().join("migrations");
    let db_path = temp_dir.path().join("incremental_test.db");

    fs::create_dir_all(&migrations_dir)?;

    // Start with just the first migration
    copy_single_migration_file(&migrations_dir)?;

    let config = MonarchConfiguration {
        name: "incremental_blog".to_string(),
        enable_foreign_keys: false,
        migration_directory: Utf8PathBuf::from_path_buf(migrations_dir.to_path_buf())
            .map_err(|_| "Invalid UTF-8 path")?,
    };

    let connection_config = ConnectionConfiguration {
        database: Some(
            Utf8PathBuf::from_path_buf(db_path.to_path_buf()).map_err(|_| "Invalid UTF-8 path")?,
        ),
    };

    // Create initial database with just users table
    {
        let monarch_db = MonarchDB::from_configuration(config.clone())?;
        assert_eq!(monarch_db.current_version(), 1);
        let connection = monarch_db.create_connection(&connection_config)?;

        // Verify only users table exists
        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")?;
        assert!(stmt.query_map([], |_| Ok(true))?.next().is_some());

        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='posts'")?;
        assert!(stmt.query_map([], |_| Ok(true))?.next().is_none());
    }

    // Add second migration file
    add_second_migration_file(&migrations_dir)?;

    // Reconnect and verify migration runs
    {
        let monarch_db = MonarchDB::from_configuration(config)?;
        assert_eq!(monarch_db.current_version(), 2);
        let connection = monarch_db.create_connection(&connection_config)?;

        // Verify both tables now exist
        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")?;
        assert!(stmt.query_map([], |_| Ok(true))?.next().is_some());

        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='posts'")?;
        assert!(stmt.query_map([], |_| Ok(true))?.next().is_some());
    }

    Ok(())
}

#[test]
fn test_directory_configuration_empty_directory() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let migrations_dir = temp_dir.path().join("empty_migrations");
    let db_path = temp_dir.path().join("empty_test.db");

    fs::create_dir_all(&migrations_dir)?;

    let config = MonarchConfiguration {
        name: "empty_blog".to_string(),
        enable_foreign_keys: false,
        migration_directory: Utf8PathBuf::from_path_buf(migrations_dir.to_path_buf())
            .map_err(|_| "Invalid UTF-8 path")?,
    };

    let monarch_db = MonarchDB::from_configuration(config)?;
    assert_eq!(monarch_db.current_version(), 0);

    let connection_config = ConnectionConfiguration {
        database: Some(Utf8PathBuf::from_path_buf(db_path).map_err(|_| "Invalid UTF-8 path")?),
    };

    let connection = monarch_db.create_connection(&connection_config)?;

    // Verify only the version table exists
    let mut stmt = connection.prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table'")?;
    let table_count: i64 = stmt.query_row([], |row| row.get(0))?;
    assert_eq!(table_count, 1); // Only the monarch_db_schema_version table

    Ok(())
}

fn copy_migration_files(
    migrations_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_dir = std::path::Path::new("tests/migrations");

    fs::copy(
        source_dir.join("001_create_users.sql"),
        migrations_dir.join("001_create_users.sql"),
    )?;
    fs::copy(
        source_dir.join("002_create_posts.sql"),
        migrations_dir.join("002_create_posts.sql"),
    )?;
    fs::copy(
        source_dir.join("003_add_indexes.sql"),
        migrations_dir.join("003_add_indexes.sql"),
    )?;

    Ok(())
}

fn copy_partial_migration_files(
    migrations_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_dir = std::path::Path::new("tests/migrations");

    fs::copy(
        source_dir.join("001_create_users.sql"),
        migrations_dir.join("001_create_users.sql"),
    )?;
    fs::copy(
        source_dir.join("002_create_posts.sql"),
        migrations_dir.join("002_create_posts.sql"),
    )?;

    Ok(())
}

fn copy_single_migration_file(
    migrations_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_dir = std::path::Path::new("tests/migrations");

    fs::copy(
        source_dir.join("001_create_users.sql"),
        migrations_dir.join("001_create_users.sql"),
    )?;

    Ok(())
}

fn add_second_migration_file(
    migrations_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_dir = std::path::Path::new("tests/migrations");

    fs::copy(
        source_dir.join("002_create_posts.sql"),
        migrations_dir.join("002_create_posts.sql"),
    )?;

    Ok(())
}

fn verify_complete_schema(connection: &Connection) -> rusqlite::Result<()> {
    // Check tables exist
    let tables = ["users", "posts"];
    for table in tables {
        let mut stmt = connection.prepare(&format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{table}'"
        ))?;
        assert!(
            stmt.query_map([], |_| Ok(true))?.next().is_some(),
            "Table {table} should exist"
        );
    }

    // Check indexes exist
    let indexes = [
        "idx_users_username",
        "idx_users_email",
        "idx_posts_user_id",
        "idx_posts_published",
    ];
    for index in indexes {
        let mut stmt = connection.prepare(&format!(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='{index}'"
        ))?;
        assert!(
            stmt.query_map([], |_| Ok(true))?.next().is_some(),
            "Index {index} should exist"
        );
    }

    Ok(())
}

fn verify_partial_schema(connection: &Connection) -> rusqlite::Result<()> {
    // Check tables exist
    let tables = ["users", "posts"];
    for table in tables {
        let mut stmt = connection.prepare(&format!(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='{table}'"
        ))?;
        assert!(
            stmt.query_map([], |_| Ok(true))?.next().is_some(),
            "Table {table} should exist"
        );
    }

    // Check indexes do NOT exist (since we only have first 2 migrations)
    let indexes = [
        "idx_users_username",
        "idx_users_email",
        "idx_posts_user_id",
        "idx_posts_published",
    ];
    for index in indexes {
        let mut stmt = connection.prepare(&format!(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='{index}'"
        ))?;
        assert!(
            stmt.query_map([], |_| Ok(true))?.next().is_none(),
            "Index {index} should not exist"
        );
    }

    Ok(())
}

fn test_foreign_key_constraints(connection: &Connection) -> rusqlite::Result<()> {
    // Insert a user
    connection.execute(
        "INSERT INTO users (username, email) VALUES (?, ?)",
        ["fkuser", "fk@example.com"],
    )?;

    // Get user ID
    let mut stmt = connection.prepare("SELECT id FROM users WHERE username = ?")?;
    let user_id: i64 = stmt.query_row(["fkuser"], |row| row.get(0))?;

    // Insert a post with valid foreign key
    connection.execute(
        "INSERT INTO posts (user_id, title, content) VALUES (?, ?, ?)",
        [&user_id.to_string(), "FK Test", "Testing foreign keys"],
    )?;

    // Try to insert a post with invalid foreign key (should fail with foreign keys enabled)
    let result = connection.execute(
        "INSERT INTO posts (user_id, title, content) VALUES (?, ?, ?)",
        ["999999", "Invalid FK", "This should fail"],
    );

    // If foreign keys are enabled, this should fail
    assert!(
        result.is_err(),
        "Foreign key constraint should prevent this insert"
    );

    Ok(())
}
