mod event;
mod keys;
mod relay_manager;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nosotros")]
#[command(about = "A command-line Nostr client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new keypair
    Keygen,
    /// Post a text note
    Post { text: String },
    /// Connect to relay and listen for events
    Listen { relay_url: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Keygen => {
            let keypair = keys::generate_keypair()?;
            println!("Generated new keypair:");
            println!("Private key: {}", keypair.secret_key_hex());
            println!("Public key: {}", keypair.public_key_hex());
        }
        Commands::Post { text } => {
            println!("Would post: {}", text);
            // TODO: Implement posting
        }
        Commands::Listen { relay_url } => {
            println!("Connecting to relay: {}", relay_url);
            let mut relay_manager = relay_manager::RelayManager::new();

            match relay_manager.add_relay(&relay_url).await {
                Ok(()) => {
                    match relay_manager.connect_relay(&relay_url).await {
                        Ok(_connection) => {
                            println!("Successfully connected to relay: {}", relay_url);
                            println!("Connection established - ready to listen for events");
                            // TODO: Implement actual event listening
                        }
                        Err(e) => {
                            eprintln!("Failed to connect to relay: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Invalid relay URL: {}", e);
                }
            }
        }
    }

    Ok(())
}
