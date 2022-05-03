use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::core::upgrade::Version;
use libp2p::identity::Keypair;
use libp2p::noise::{Keypair as NoiseKeypair, NoiseConfig, X25519Spec};
use libp2p::request_response::{ProtocolSupport, RequestResponse, RequestResponseConfig};
use libp2p::swarm::{ConnectionLimits, Swarm, SwarmBuilder, SwarmEvent};
use libp2p::tcp::TcpConfig;
use libp2p::yamux::YamuxConfig;
use libp2p::{Multiaddr, PeerId, Transport};

use std::iter::once;
use std::str::FromStr;
use std::time::Duration;

use futures::prelude::*;

use crate::protocol::{NeighbourRequestCodec, NeighbourRequestProtocol};
use crate::EigenError;
use crate::Peer;

async fn development_transport(
    keypair: Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, EigenError> {
    let noise_keys = NoiseKeypair::<X25519Spec>::new()
        .into_authentic(&keypair)
        .map_err(|_| EigenError::InvalidKeypair)?;

    let transport = TcpConfig::new().nodelay(true);

    Ok(transport
        .upgrade(Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(YamuxConfig::default())
        .timeout(Duration::from_secs(20))
        .boxed())
}

pub async fn setup_node(
    key: Option<String>,
    address: Option<String>,
    default_address: &str,
    bootstrap_nodes: [[&str; 2]; 2],
    max_connections: u32,
) -> Result<Swarm<RequestResponse<NeighbourRequestCodec>>, EigenError> {
    // Taking the keypair from the command line or generating a new one.
    let local_key = if let Some(key) = key {
        let decoded_key = bs58::decode(&key).into_vec().map_err(|e| {
            log::debug!("bs58::decode {:?}", e);
            EigenError::InvalidKeypair
        })?;
        Keypair::from_protobuf_encoding(&decoded_key).map_err(|e| {
            log::debug!("Keypair::from_protobuf_encoding {:?}", e);
            EigenError::InvalidKeypair
        })?
    } else {
        Keypair::generate_ed25519()
    };

    // Taking the address from the command line arguments or the default one.
    let local_address = if let Some(addr) = address {
        Multiaddr::from_str(&addr).map_err(|e| {
            log::debug!("Multiaddr::from_str {:?}", e);
            EigenError::InvalidAddress
        })?
    } else {
        Multiaddr::from_str(&default_address).map_err(|e| {
            log::debug!("Multiaddr::from_str {:?}", e);
            EigenError::InvalidAddress
        })?
    };

    // Setting up the request/response protocol.
    let protocols = once((NeighbourRequestProtocol::new(), ProtocolSupport::Full));
    let cfg = RequestResponseConfig::default();
    let req_proto = RequestResponse::new(NeighbourRequestCodec, protocols.clone(), cfg.clone());

    // Setting up the transport and swarm.
    let local_peer_id = PeerId::from(local_key.public());
    let transport = development_transport(local_key).await?;
    let connection_limits =
        ConnectionLimits::default().with_max_established_per_peer(Some(max_connections));
    let mut swarm = SwarmBuilder::new(transport, req_proto, local_peer_id)
        .connection_limits(connection_limits)
        .build();
    swarm.listen_on(local_address).map_err(|e| {
        log::debug!("swarm.listen_on {:?}", e);
        EigenError::ListenFailed
    })?;

    // We want to connect to all bootstrap nodes.
    for info in bootstrap_nodes.iter() {
        // We can also contact the address.
        let peer_addr = Multiaddr::from_str(info[0]).map_err(|e| {
            log::debug!("Multiaddr::from_str {:?}", e);
            EigenError::InvalidAddress
        })?;
        let peer_id = PeerId::from_str(info[1]).map_err(|e| {
            log::debug!("PeerId::from_str {:?}", e);
            EigenError::InvalidPeerId
        })?;

        if peer_id == local_peer_id {
            continue;
        }

        let res = swarm.dial(peer_addr).map_err(|_| EigenError::DialError);
        log::debug!("swarm.dial {:?}", res);
    }

    Ok(swarm)
}

pub async fn start_loop(
    peer: &mut Peer,
    swarm: &mut Swarm<RequestResponse<NeighbourRequestCodec>>,
) {
    println!("");
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => log::info!("Listening on {:?}", address),
            SwarmEvent::Behaviour(event) => {
                log::debug!("ReqRes event {:?}", event);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                let res = peer.add_neighbour(peer_id);
                if let Err(e) = res {
                    log::error!("Failed to add neighbour {:?}", e);
                }
                log::info!("Connection established with {:?}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                let res = peer.remove_neighbour(peer_id);
                if let Err(e) = res {
                    log::error!("Failed to remove neighbour {:?}", e);
                }
                log::info!("Connection closed with {:?} ({:?})", peer_id, cause);
            }
            SwarmEvent::Dialing(peer_id) => {
                log::info!("Dialing {:?}", peer_id);
            }
            e => log::debug!("{:?}", e),
        }
    }
}
