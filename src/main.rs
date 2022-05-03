use clap::Parser;
use libp2p::PeerId;
use env_logger::Builder;
use log::LevelFilter;

mod node;
mod protocol;

use node::{setup_node, start_loop};

pub fn init_logger() {
    let mut builder = Builder::from_default_env();

    builder
        .filter(None, LevelFilter::Info)
        .format_timestamp(None)
        .init();
}


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
const NUM_NEIGHBOURS: u32 = 256;

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
    DialError,
    MaxNeighboursReached,
    NeighbourNotFound,
}

pub struct Peer {
    neighbours: [Option<PeerId>; NUM_NEIGHBOURS as usize],
}

impl Peer {
    pub fn new() -> Self {
        Peer {
            neighbours: [None; NUM_NEIGHBOURS as usize],
        }
    }

    pub fn add_neighbour(&mut self, neighbour: PeerId) -> Result<(), EigenError> {
        let first_available = self.neighbours.iter().position(|n| n.is_none());
        if let Some(index) = first_available {
            self.neighbours[index] = Some(neighbour);
            return Ok(());
        }
        Err(EigenError::MaxNeighboursReached)
    }

    pub fn remove_neighbour(&mut self, neighbour: PeerId) -> Result<(), EigenError> {
        let index = self
            .neighbours
            .iter()
            .position(|n| n.map(|n| n == neighbour).unwrap_or(false));
        if let Some(index) = index {
            self.neighbours[index] = None;
            return Ok(());
        }
        Err(EigenError::NeighbourNotFound)
    }
}

#[async_std::main]
async fn main() -> Result<(), EigenError> {
    init_logger();

    let args = Args::parse();

    let mut swarm = setup_node(
        args.key,
        args.address,
        DEFAULT_ADDRESS,
        BOOTSTRAP_PEERS,
        NUM_NEIGHBOURS,
    )
    .await?;

    let mut peer = Peer::new();
    start_loop(&mut peer, &mut swarm).await;

    Ok(())
}
