//! relay-admin — CLI tool for managing relay server accounts.
//!
//! Run inside the relay-server Docker container or on the server directly:
//!
//!   relay-admin add-user    --db <path> --username <name> [--password <pw>]
//!   relay-admin list-users  --db <path>
//!   relay-admin delete-user --db <path> --username <name>
//!   relay-admin reset-password --db <path> --username <name> [--password <pw>]
//!
//! If `--password` is omitted the tool prompts interactively (hidden input).
//! The plaintext password is never stored — only Argon2id-derived hashes and
//! AES-256-GCM wrapped master keys are written to the database.

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "relay-admin")]
#[command(about = "Manage relay server accounts (provisioning tool)")]
struct Cli {
    /// Path to the SQLite database file.
    #[arg(long, env = "RELAY_DB_PATH")]
    db: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new account.
    AddUser {
        #[arg(long)]
        username: String,
        /// Omit to prompt interactively (recommended).
        #[arg(long)]
        password: Option<String>,
    },
    /// List all accounts.
    ListUsers,
    /// Delete an account and all its data.
    DeleteUser {
        #[arg(long)]
        username: String,
    },
    /// Reset an account's password (generates new salts + new master key).
    ResetPassword {
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Rename an existing account. Credentials and user_id stay the same.
    RenameUser {
        #[arg(long)]
        username: String,
        #[arg(long)]
        new_username: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let pool = bitfun_relay_server::db::connect(&cli.db).await?;

    match cli.command {
        Command::AddUser { username, password } => {
            let password = resolve_password(password)?;
            let user_id = bitfun_relay_server::admin::add_user(&pool, &username, &password).await?;
            println!("Created account: username='{username}' user_id={user_id}");
        }
        Command::ListUsers => {
            let users = bitfun_relay_server::admin::list_users(&pool).await?;
            if users.is_empty() {
                println!("No accounts found.");
            } else {
                println!("{:<24} {:<38} {}", "USERNAME", "USER_ID", "CREATED");
                println!("{}", "-".repeat(80));
                for (username, user_id, created) in users {
                    let dt = chrono::DateTime::from_timestamp(created, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| created.to_string());
                    println!("{:<24} {:<38} {dt}", username, user_id);
                }
            }
        }
        Command::DeleteUser { username } => {
            bitfun_relay_server::admin::delete_user(&pool, &username).await?;
            println!("Deleted account: {username}");
        }
        Command::ResetPassword { username, password } => {
            let password = resolve_password(password)?;
            bitfun_relay_server::admin::reset_password(&pool, &username, &password).await?;
            println!("Password reset for: {username}");
            println!("NOTE: All previously synced sessions/settings are now unreadable");
            println!("      (they were encrypted with the old master key).");
        }
        Command::RenameUser {
            username,
            new_username,
        } => {
            bitfun_relay_server::admin::rename_user(&pool, &username, &new_username).await?;
            println!("Renamed: {username} → {new_username}");
        }
    }

    Ok(())
}

/// Use the provided password, or prompt interactively with hidden input.
fn resolve_password(provided: Option<String>) -> Result<String> {
    match provided {
        Some(p) if p.len() >= 8 => Ok(p),
        Some(_) => Err(anyhow!("password must be at least 8 characters")),
        None => {
            let p1 = rpassword::prompt_password("Enter password: ")?;
            if p1.len() < 8 {
                return Err(anyhow!("password must be at least 8 characters"));
            }
            let p2 = rpassword::prompt_password("Confirm password: ")?;
            if p1 != p2 {
                return Err(anyhow!("passwords do not match"));
            }
            Ok(p1)
        }
    }
}
