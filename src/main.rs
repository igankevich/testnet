use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::process::ExitCode;

use bincode::decode_from_slice;
use bincode::encode_to_vec;
use bincode::Decode;
use bincode::Encode;
use clap::Parser;
use testnet::Context;
use testnet::NetConfig;
use testnet::Network;
use testnet::NodeConfig;

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
            eprintln!("{}", e);
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
    let config = NetConfig {
        main: |mut context| {
            let env = Environment::new(&context);
            let all_data = context.broadcast_all(env.encode()?)?;
            let mut command = Command::new(&args.program);
            for (i, data) in all_data.into_iter().enumerate() {
                let env = Environment::decode(&data)?;
                let infix = i.to_string();
                env.set_for_command(&infix, &mut command);
            }
            env.set_for_command("NODE", &mut command);
            Err(command.args(&args.args).exec().into())
        },
        nodes: vec![NodeConfig::default(); args.nodes],
    };
    let network = Network::new(config)?;
    network.wait()?;
    Ok(())
}

#[derive(Encode, Decode)]
struct Environment {
    envs: [(String, String); 6],
}

impl Environment {
    fn new(context: &Context) -> Self {
        let node = context.current_node();
        Self {
            envs: [
                ("INDEX".into(), context.current_node_index().to_string()),
                ("NAME".into(), node.name.clone()),
                ("IFNAME".into(), context.current_node_ifname().to_string()),
                ("IFADDR".into(), node.ifaddr.to_string()),
                ("IPADDR".into(), node.ifaddr.addr().to_string()),
                ("PREFIX_LEN".into(), node.ifaddr.prefix_len().to_string()),
            ],
        }
    }

    fn set_for_command(&self, infix: &str, command: &mut Command) {
        for (key, value) in self.envs.iter() {
            let key = format!("TESTNET_{}_{}", infix, key);
            command.env(key, value);
        }
    }

    fn encode(&self) -> Result<Vec<u8>, std::io::Error> {
        encode_to_vec(self, bincode_config()).map_err(std::io::Error::other)
    }

    fn decode(data: &[u8]) -> Result<Self, std::io::Error> {
        let (object, ..): (Self, usize) =
            decode_from_slice(data, bincode_config()).map_err(std::io::Error::other)?;
        Ok(object)
    }
}

const fn bincode_config() -> bincode::config::Configuration<
    bincode::config::LittleEndian,
    bincode::config::Fixint,
    bincode::config::Limit<MAX_MESSAGE_SIZE>,
> {
    bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding()
        .with_limit::<MAX_MESSAGE_SIZE>()
}

pub(crate) const MAX_MESSAGE_SIZE: usize = 4096 * 16;
