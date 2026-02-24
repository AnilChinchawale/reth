//! XDC Network Reth Node
//!
//! This is the main entry point for the XDC Network Reth node.

use clap::Parser;

/// XDC Network Reth CLI
#[derive(Debug, Parser)]
#[command(name = "xdc-reth")]
#[command(about = "XDC Network Reth - XDPoS consensus node")]
#[command(version)]
struct Cli {
    /// Chain specification to use (xdc, apothem)
    #[arg(long, value_name = "CHAIN", default_value = "xdc")]
    chain: String,

    /// Enable validator mode (participate in consensus)
    #[arg(long)]
    validator: bool,

    /// Data directory
    #[arg(long, value_name = "DIR")]
    datadir: Option<String>,
}

fn main() -> eyre::Result<()> {
    // Parse CLI arguments (for validation)
    let _cli = Cli::parse();

    println!("XDC Reth node - coming soon");
    println!("The XDC node builder is under active development.");
    println!("Core consensus (XDPoS) and node structure are implemented.");
    println!("Integration with Reth's node builder will be completed soon.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        // Just verify CLI parsing doesn't panic
        let args = vec!["xdc-reth", "--chain", "xdc"];
        let _cli = Cli::try_parse_from(args);
    }
}
