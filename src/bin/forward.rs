use std::string;

use anyhow::Result;
use aztunnel::tunnel_client;
// https://stackoverflow.com/questions/73840760/cannot-find-package-in-the-crate-root
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// starts a tcp tunnel
    TCP {
        /// Network port to use
        #[arg(value_parser = clap::value_parser!(u16).range(1..))]
        port: u16,
    },
}

#[tokio::main]
async fn execute(command: Command) -> Result<()> {
    match command {
        Command::TCP { port } => {
            let client = tunnel_client::TcpClient::new(port).await?;
            client.handle_server_request();
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    execute(cli.command);
}
