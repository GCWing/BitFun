//! Inspect or explicitly delete relay records. GitHub owns account identity.
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "relay-admin", about = "Inspect GitHub-linked relay accounts")]
struct Cli {
    #[arg(long, env = "RELAY_DB_PATH")]
    db: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List GitHub-linked accounts.
    ListUsers,
    /// Explicitly delete an account and its relay data.
    DeleteUser {
        #[arg(long)]
        username: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let pool = openbitfun_relay_service::db::connect_for_admin(&cli.db).await?;
    match cli.command {
        Command::ListUsers => {
            for (login, github_id, created) in
                openbitfun_relay_service::admin::list_users(&pool).await?
            {
                println!("{login}\t{github_id}\t{created}");
            }
        }
        Command::DeleteUser { username } => {
            openbitfun_relay_service::admin::delete_user(&pool, &username).await?;
            println!("Deleted relay account: {username}");
        }
    }
    Ok(())
}
