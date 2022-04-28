use clap::Parser;
use logger::init_logger;
use node::{setup_node, start_loop};
use std::collections::HashSet;

mod logger;
mod node;
mod protocol;

const BOOTSTRAP_PEERS: [[&str; 2]; 2] = [
    [
        "/ip4/127.0.0.1/tcp/58584",
        "12D3KooWLyTCx9j2FMcsHe81RMoDfhXbdyyFgNGQMdcrnhShTvQh",
    ],
    [
        "/ip4/127.0.0.1/tcp/58601",
        "12D3KooWKBKXsLwbmVBySEmbKayJzfWp3tPCKrnDCsmNy9prwjvy",
    ],
];

const DEFAULT_ADDRESS: &str = "/ip4/0.0.0.0/tcp/0";
const NUM_NEIGHBOURS: usize = 256;

#[derive(Parser, Debug)]
struct Args {
    #[clap(short, long)]
    key: Option<String>,
    #[clap(short, long)]
    address: Option<String>,
}

#[derive(Debug)]
pub enum EigenError {
    InvalidKeypair,
    InvalidAddress,
    InvalidPeerId,
    ListenFailed,
}

struct Peer {
	neighbours: [String; NUM_NEIGHBOURS],
}

#[async_std::main]
async fn main() -> Result<(), EigenError> {
    init_logger();

    let args = Args::parse();

    let mut swarm = setup_node(args.key, args.address, DEFAULT_ADDRESS, BOOTSTRAP_PEERS).await?;
    start_loop(&mut swarm).await;

    Ok(())
}
