//! Operator CLI for the jobs that cannot go through the HTTP API.
//!
//! It reads the same `configuration/` directory as the server, so running it
//! inside a deployment's container needs no connection details of its own.

use lab_inventory::bootstrap::set_password;
use lab_inventory::configuration::get_configuration;
use lab_inventory::domain::UserPassword;
use lab_inventory::startup::get_connection_pool;
use secrecy::Secret;
use std::io::{IsTerminal, Read, Write};

const USAGE: &str = "\
lab-inventory-admin — operator commands for a Lab Inventory deployment

USAGE:
    lab-inventory-admin set-password <username>
    lab-inventory-admin version

The password is read from the LAB_INVENTORY_PASSWORD environment variable when
it is set, and from stdin otherwise.

The database is taken from configuration/, exactly as the server reads it, so
APP_ENVIRONMENT and any APP_* overrides apply here too.
";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("set-password") => {
            let Some(username) = arguments.next() else {
                anyhow::bail!("set-password needs a username.\n\n{USAGE}");
            };
            if arguments.next().is_some() {
                anyhow::bail!("set-password takes a single username.\n\n{USAGE}");
            }
            set_password_command(&username).await
        }
        // The deployed image carries no other way to say which build it is.
        Some("version") | Some("-V") | Some("--version") => {
            println!("lab-inventory {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("help") | Some("-h") | Some("--help") => {
            print!("{USAGE}");
            Ok(())
        }
        Some(command) => anyhow::bail!("Unknown command `{command}`.\n\n{USAGE}"),
        None => anyhow::bail!("No command given.\n\n{USAGE}"),
    }
}

async fn set_password_command(username: &str) -> Result<(), anyhow::Error> {
    let password = read_password()?;

    let configuration = get_configuration()?;
    let pool = get_connection_pool(&configuration.database);
    let updated = set_password(&pool, username, password).await?;

    if !updated {
        anyhow::bail!("No user named `{username}` exists.");
    }
    println!("Password updated for `{username}`.");
    Ok(())
}

/// Takes the password from the environment, falling back to stdin.
///
/// The environment is what a `docker compose run` invocation can supply
/// without the password reaching a shell history; stdin is what a pipe from a
/// password manager uses.
fn read_password() -> Result<Secret<String>, anyhow::Error> {
    if let Ok(password) = std::env::var("LAB_INVENTORY_PASSWORD") {
        return validated(password);
    }

    if std::io::stdin().is_terminal() {
        // Without a TTY-aware prompt the password would echo, so say where it
        // is expected from rather than reading it in the clear.
        print!(
            "Password (input is echoed; pipe it in or set LAB_INVENTORY_PASSWORD to avoid that): "
        );
        std::io::stdout().flush()?;
    }
    let mut password = String::new();
    std::io::stdin().read_to_string(&mut password)?;
    validated(password.trim_end_matches(['\r', '\n']).to_string())
}

/// Holds the CLI to the same password policy as the API.
fn validated(password: String) -> Result<Secret<String>, anyhow::Error> {
    UserPassword::parse(Secret::new(password))
        .map(|password| password.0)
        .map_err(|error| anyhow::anyhow!(error))
}
