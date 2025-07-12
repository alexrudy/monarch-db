# Monarch-DB

Monarch-DB is a lightweight [rusqlite][] database migration tool designed to run whenever the first
connection in an app opens. It provides a simple, reliable way to manage SQLite database schema
evolution in Rust applications.

[rusqlite]: https://crates.io/crates/rusqlite

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
monarch-db = "0.1"

# Optional: Enable serde support for configuration
monarch-db = { version = "0.1", features = ["serde"] }
```

## Quick Start

### Static Configuration

Use static configuration when you want to embed migrations directly in your binary:

```rust
use monarch_db::{StaticMonarchConfiguration, MonarchDB, ConnectionConfiguration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Define your migrations at compile time
    let config = StaticMonarchConfiguration {
        name: "my_app",
        enable_foreign_keys: true,
        migrations: [
            // Migration 1: Create users table
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            "#,
            // Migration 2: Create posts table
            r#"
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                content TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
            "#,
            // Migration 3: Add indexes
            r#"
            CREATE INDEX idx_users_username ON users(username);
            CREATE INDEX idx_posts_user_id ON posts(user_id);
            "#,
        ],
    };

    // Convert to MonarchDB instance
    let monarch_db: MonarchDB = config.into();

    // Create connection configuration
    let connection_config = ConnectionConfiguration {
        database: Some("./my_app.db".into()), // Use None for in-memory
    };

    // Create database connection with migrations applied
    let connection = monarch_db.create_connection(&connection_config)?;

    // Use your database normally
    connection.execute(
        "INSERT INTO users (username, email) VALUES (?, ?)",
        ["alice", "alice@example.com"],
    )?;

    Ok(())
}
```

### Directory-Based Configuration

Use directory-based configuration when you want to manage migrations as separate files:

```rust
use monarch_db::{MonarchConfiguration, MonarchDB, ConnectionConfiguration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = MonarchConfiguration {
        name: "my_app".to_string(),
        enable_foreign_keys: true,
        migration_directory: "./migrations".into(),
    };

    let monarch_db = MonarchDB::from_configuration(config)?;

    let connection_config = ConnectionConfiguration {
        database: Some("./my_app.db".into()),
    };

    let connection = monarch_db.create_connection(&connection_config)?;

    // Database is ready with all migrations applied
    Ok(())
}
```

With migration files in `./migrations/`:

```text
migrations/
├── 001_create_users.sql
├── 002_create_posts.sql
└── 003_add_indexes.sql
```

**001_create_users.sql:**

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

**002_create_posts.sql:**

```sql
CREATE TABLE posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

**003_add_indexes.sql:**

```sql
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_posts_user_id ON posts(user_id);
```

## Advanced Usage

### In-Memory Databases

Create temporary databases perfect for testing:

```rust
let connection = monarch_db.open_in_memory()?;
```

### Using with Include Files

For static configuration, you can use `include_str!` for better organization:

```rust
let config = StaticMonarchConfiguration {
    name: "my_app",
    enable_foreign_keys: true,
    migrations: [
        include_str!("../migrations/001_create_users.sql"),
        include_str!("../migrations/002_create_posts.sql"),
        include_str!("../migrations/003_add_indexes.sql"),
    ],
};
```

### Configuration with Serde

Enable the `serde` feature to deserialize configurations:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct AppConfig {
    database: monarch_db::MonarchConfiguration,
    connection: monarch_db::ConnectionConfiguration,
}

let config: AppConfig = toml::from_str(r#"
[database]
name = "my_app"
enable_foreign_keys = true
migration_directory = "./migrations"

[connection]
database = "./app.db"
"#)?;

let monarch_db = MonarchDB::from_configuration(config.database)?;
let connection = monarch_db.create_connection(&config.connection)?;
```

### Version Management

Check the current schema version:

```rust
let current_version = monarch_db.current_version();
println!("Database schema is at version: {}", current_version);
```

### Applying Migrations to Existing Connections

You can apply migrations to an existing connection:

```rust
use rusqlite::Connection;

let raw_connection = Connection::open("./my_app.db")?;
let migrated_connection = monarch_db.migrations(raw_connection)?;
```

## Command Line Interface

Monarch-DB includes a command-line tool for running migrations outside of your application code.
This is useful for deployment scripts, CI/CD pipelines, or manual database management.

### Migrate Command

Apply all pending migrations to a database:

```bash
monarch migrate <migrations_dir> <app_name> <sqlite_url>
```

**Arguments:**

- `migrations_dir` - Path to directory containing migration files
- `app_name` - Name of the application (used for version tracking)
- `sqlite_url` - SQLite database URL (file path or `:memory:`)

**Examples:**

```bash
# Apply migrations to a file database
monarch migrate ./migrations my_app ./database.db

# Apply migrations to an in-memory database
monarch migrate ./migrations my_app :memory:

# Apply migrations for a specific environment
monarch migrate ./db/migrations production_app /var/lib/myapp/prod.db
```

**Sample Output:**

```text
Running migrations...
  Migrations directory: ./migrations
  Application name: my_app
  Database: ./database.db

Found 3 migration(s)
Migration completed successfully!
Current schema version: 3
Database is up to date.
```

### Version Command

Check the current migration status without applying changes:

```bash
monarch version <migrations_dir> <app_name> <sqlite_url>
```

**Examples:**

```bash
# Check migration status
monarch version ./migrations my_app ./database.db

# Check status of a database that doesn't exist yet
monarch version ./migrations my_app ./new_database.db
```

**Sample Output:**

```text
Checking migration version...
  Migrations directory: ./migrations
  Application name: my_app
  Database: ./database.db

Available migrations: 5
Current schema version: 3
Migrations pending: 3 -> 5 (2 new migration(s))
```

## Testing

Run the test suite:

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test static_configuration
cargo test --test directory_configuration
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an
issue first to discuss what you would like to change.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
