use monarch_db::{ConnectionConfiguration, MonarchDB, StaticMonarchConfiguration};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn test_static_configuration_with_file_database() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    let config = StaticMonarchConfiguration {
        name: "blog_static",
        enable_foreign_keys: true,
        migrations: [
            include_str!("migrations/001_create_users.sql"),
            include_str!("migrations/002_create_posts.sql"),
            include_str!("migrations/003_add_indexes.sql"),
        ],
    };

    let monarch_db: MonarchDB = config.into();
    let connection_config = ConnectionConfiguration {
        database: Some(db_path.try_into()?),
    };

    let connection = monarch_db.create_connection(&connection_config)?;

    // Verify all tables were created
    verify_schema(&connection)?;

    // Test data operations with foreign keys
    test_data_operations(&connection)?;

    Ok(())
}

#[test]
fn test_static_configuration_multiple_connections() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("shared.db");

    let config = StaticMonarchConfiguration {
        name: "shared_db",
        enable_foreign_keys: false,
        migrations: [
            include_str!("migrations/001_create_users.sql"),
            include_str!("migrations/002_create_posts.sql"),
        ],
    };

    let monarch_db: MonarchDB = config.into();
    let connection_config = ConnectionConfiguration {
        database: Some(db_path.try_into()?),
    };

    // Create first connection and add data
    {
        let connection1 = monarch_db.create_connection(&connection_config)?;
        connection1.execute(
            "INSERT INTO users (username, email) VALUES (?, ?)",
            ["alice", "alice@example.com"],
        )?;
    }

    // Create second connection and verify data persists
    {
        let connection2 = monarch_db.create_connection(&connection_config)?;
        let mut stmt = connection2.prepare("SELECT COUNT(*) FROM users")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        assert_eq!(count, 1);

        let mut stmt = connection2.prepare("SELECT username FROM users WHERE email = ?")?;
        let username: String = stmt.query_row(["alice@example.com"], |row| row.get(0))?;
        assert_eq!(username, "alice");
    }

    Ok(())
}

#[test]
fn test_static_configuration_migration_versioning() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("versioned.db");

    // First, create database with only first migration
    let config_v1 = StaticMonarchConfiguration {
        name: "versioned_db",
        enable_foreign_keys: false,
        migrations: [include_str!("migrations/001_create_users.sql")],
    };

    let monarch_db_v1: MonarchDB = config_v1.into();
    let connection_config = ConnectionConfiguration {
        database: Some(db_path.try_into()?),
    };

    {
        let connection = monarch_db_v1.create_connection(&connection_config)?;
        // Verify only users table exists
        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")?;
        assert!(stmt.query_map([], |_| Ok(true))?.next().is_some());

        let mut stmt = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='posts'")?;
        assert!(stmt.query_map([], |_| Ok(true))?.next().is_none());
    }

    // Now upgrade to version with both migrations
    let config_v2 = StaticMonarchConfiguration {
        name: "versioned_db",
        enable_foreign_keys: false,
        migrations: [
            include_str!("migrations/001_create_users.sql"),
            include_str!("migrations/002_create_posts.sql"),
        ],
    };

    let monarch_db_v2: MonarchDB = config_v2.into();
    {
        let connection = monarch_db_v2.create_connection(&connection_config)?;
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

fn verify_schema(connection: &Connection) -> rusqlite::Result<()> {
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

fn test_data_operations(connection: &Connection) -> rusqlite::Result<()> {
    // Insert a user
    connection.execute(
        "INSERT INTO users (username, email) VALUES (?, ?)",
        ["testuser", "test@example.com"],
    )?;

    // Get user ID
    let mut stmt = connection.prepare("SELECT id FROM users WHERE username = ?")?;
    let user_id: i64 = stmt.query_row(["testuser"], |row| row.get(0))?;

    // Insert a post
    connection.execute(
        "INSERT INTO posts (user_id, title, content, published) VALUES (?, ?, ?, ?)",
        [
            &user_id.to_string(),
            "Test Post",
            "This is a test post",
            "1",
        ],
    )?;

    // Verify the post was inserted
    let mut stmt = connection.prepare("SELECT COUNT(*) FROM posts WHERE user_id = ?")?;
    let count: i64 = stmt.query_row([user_id], |row| row.get(0))?;
    assert_eq!(count, 1);

    // Test join query
    let mut stmt = connection.prepare(
        "SELECT u.username, p.title FROM users u JOIN posts p ON u.id = p.user_id WHERE u.id = ?",
    )?;
    let (username, title): (String, String) =
        stmt.query_row([user_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    assert_eq!(username, "testuser");
    assert_eq!(title, "Test Post");

    Ok(())
}
