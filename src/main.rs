use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::process::ExitCode;

use clap::Parser;
use testnet::log_format;
use testnet::NetConfig;
use testnet::Network;

#[derive(Parser)]
#[command(
    about = "Testnet — run your distributed application in a test network.",
    long_about = None,
    trailing_var_arg = true,
)]
struct Args {
    /// Print version.
    #[clap(long, action)]
    version: bool,
    #[clap(short = 'n', long, default_value = "2")]
    nodes: usize,
    /// Command to run.
    program: OsString,
    /// Command arguments.
    #[clap(allow_hyphen_values = true)]
    args: Vec<OsString>,
}

fn main() -> ExitCode {
    match do_main() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            log_format!("{}", e);
            ExitCode::FAILURE
        }
    }
}

fn do_main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.version {
        println!("{}", env!("VERSION"));
        return Ok(());
    }
    // TODO write each node's env variables to a separate file
    // TODO netlink is slow (non-blocking?)
    let config = NetConfig {
        main: |context| {
            let node = context.current_node();
            Err(Command::new(&args.program)
                .args(&args.args)
                .env(
                    "TESTNET_NODE_INDEX",
                    context.current_node_index().to_string(),
                )
                .env("TESTNET_NODE_NAME", &node.name)
                .env("TESTNET_NODE_IFADDR", node.ifaddr.to_string())
                .env("TESTNET_NODE_IPADDR", node.ifaddr.addr().to_string())
                .env(
                    "TESTNET_NODE_PREFIX_LEN",
                    node.ifaddr.prefix_len().to_string(),
                )
                .exec()
                .into())
        },
        nodes: vec![Default::default(); args.nodes],
    };
    let network = Network::new(config)?;
    network.wait()?;
    Ok(())
}
