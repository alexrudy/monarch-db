use camino::Utf8PathBuf;
use monarch_db::{ConnectionConfiguration, MonarchConfiguration, MonarchDB};
use std::process;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        return Ok(());
    }

    match args[1].as_str() {
        "migrate" => {
            if args.len() != 5 {
                eprintln!(
                    "Usage: {} migrate <migrations_dir> <app_name> <sqlite_url>",
                    args[0]
                );
                process::exit(1);
            }
            migrate_command(&args[2], &args[3], &args[4])?;
        }
        "version" => {
            if args.len() != 5 {
                eprintln!(
                    "Usage: {} version <migrations_dir> <app_name> <sqlite_url>",
                    args[0]
                );
                process::exit(1);
            }
            version_command(&args[2], &args[3], &args[4])?;
        }
        "help" | "--help" | "-h" => {
            print_usage(&args[0]);
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage(&args[0]);
            process::exit(1);
        }
    }

    Ok(())
}

fn print_usage(program_name: &str) {
    println!("Monarch-DB Migration Tool");
    println!();
    println!("USAGE:");
    println!("    {program_name} <COMMAND> <ARGS>");
    println!();
    println!("COMMANDS:");
    println!("    migrate <migrations_dir> <app_name> <sqlite_url>    Run migrations");
    println!(
        "    version <migrations_dir> <app_name> <sqlite_url>    Show current migration version"
    );
    println!("    help                                                Show this help message");
    println!();
    println!("ARGS:");
    println!("    <migrations_dir>    Path to directory containing migration files");
    println!("    <app_name>          Name of the application (used for version tracking)");
    println!("    <sqlite_url>        SQLite database URL (file path or ':memory:')");
    println!();
    println!("EXAMPLES:");
    println!("    {program_name} migrate ./migrations my_app ./database.db");
    println!("    {program_name} version ./migrations my_app ./database.db");
    println!("    {program_name} migrate ./migrations my_app :memory:");
}

fn migrate_command(
    migrations_dir: &str,
    app_name: &str,
    sqlite_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Running migrations...");
    println!("  Migrations directory: {migrations_dir}");
    println!("  Application name: {app_name}");
    println!("  Database: {sqlite_url}");
    println!();

    let config = MonarchConfiguration {
        name: app_name.to_string(),
        enable_foreign_keys: true,
        migration_directory: Utf8PathBuf::from(migrations_dir),
    };

    let monarch_db = MonarchDB::from_configuration(config)?;
    let total_migrations = monarch_db.current_version();

    println!("Found {total_migrations} migration(s)");

    let connection_config = if sqlite_url == ":memory:" {
        ConnectionConfiguration { database: None }
    } else {
        ConnectionConfiguration {
            database: Some(Utf8PathBuf::from(sqlite_url)),
        }
    };

    let connection = monarch_db.create_connection(&connection_config)?;

    // Check final version to see how many migrations were applied
    let mut stmt = connection
        .prepare("SELECT version FROM monarch_db_schema_version WHERE monarch_schema = ?1")?;
    let final_version: u32 = stmt.query_row([app_name], |row| row.get(0))?;

    println!("Migration completed successfully!");
    println!("Current schema version: {final_version}");

    if final_version == total_migrations {
        println!("Database is up to date.");
    } else {
        println!("Applied {final_version} new migration(s)");
    }

    Ok(())
}

fn version_command(
    migrations_dir: &str,
    app_name: &str,
    sqlite_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking migration version...");
    println!("  Migrations directory: {migrations_dir}");
    println!("  Application name: {app_name}");
    println!("  Database: {sqlite_url}");
    println!();

    let config = MonarchConfiguration {
        name: app_name.to_string(),
        enable_foreign_keys: true,
        migration_directory: Utf8PathBuf::from(migrations_dir),
    };

    let monarch_db = MonarchDB::from_configuration(config)?;
    let available_migrations = monarch_db.current_version();

    println!("Available migrations: {available_migrations}");

    let connection_config = if sqlite_url == ":memory:" {
        ConnectionConfiguration { database: None }
    } else {
        ConnectionConfiguration {
            database: Some(Utf8PathBuf::from(sqlite_url)),
        }
    };

    // Check if database exists and has version table
    let connection = match monarch_db.create_connection(&connection_config) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to connect to database: {e}");
            println!("Current schema version: 0 (database not initialized)");
            return Ok(());
        }
    };

    // Query current version
    let mut stmt = connection
        .prepare("SELECT version FROM monarch_db_schema_version WHERE monarch_schema = ?1")?;
    let current_version: Result<u32, _> = stmt.query_row([app_name], |row| row.get(0));

    match current_version {
        Ok(version) => {
            println!("Current schema version: {version}");
            if version < available_migrations {
                println!(
                    "Migrations pending: {} -> {} ({} new migration(s))",
                    version,
                    available_migrations,
                    available_migrations - version
                );
            } else if version == available_migrations {
                println!("Database is up to date.");
            } else {
                println!(
                    "Warning: Current version ({version}) is higher than available migrations ({available_migrations})"
                );
            }
        }
        Err(_) => {
            println!("Current schema version: 0 (schema not initialized for this app)");
            if available_migrations > 0 {
                println!(
                    "Migrations pending: 0 -> {available_migrations} ({available_migrations} new migration(s))"
                );
            }
        }
    }

    Ok(())
}
