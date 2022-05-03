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
use std::time::Duration;

use futures::prelude::*;

use crate::protocol::{EigenTrustCodec, EigenTrustProtocol};
use crate::EigenError;
use crate::Peer;

async fn basic_transport(keypair: Keypair) -> Result<Boxed<(PeerId, StreamMuxerBox)>, EigenError> {
    let noise_keys = NoiseKeypair::<X25519Spec>::new()
        .into_authentic(&keypair)
        .map_err(|e| {
            log::error!("NoiseKeypair.into_authentic {}", e);
            EigenError::InvalidKeypair
        })?;

    let transport = TcpConfig::new();

    Ok(transport
        .nodelay(true)
        .upgrade(Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(YamuxConfig::default())
        .timeout(Duration::from_secs(20))
        .boxed())
}

pub async fn setup_node(
    local_key: Keypair,
    local_address: Multiaddr,
    bootstrap_nodes: Vec<(PeerId, Multiaddr)>,
    max_connections: u32,
) -> Result<Swarm<RequestResponse<EigenTrustCodec>>, EigenError> {
    // Setting up the request/response protocol.
    let protocols = once((EigenTrustProtocol::new(), ProtocolSupport::Full));
    let cfg = RequestResponseConfig::default();
    let req_proto = RequestResponse::new(EigenTrustCodec, protocols.clone(), cfg.clone());

    // Setting up the transport and swarm.
    let local_peer_id = PeerId::from(local_key.public());
    let transport = basic_transport(local_key).await?;
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
    for (peer_id, peer_addr) in bootstrap_nodes {
        if peer_id == local_peer_id {
            continue;
        }

        let res = swarm.dial(peer_addr).map_err(|_| EigenError::DialError);
        log::debug!("swarm.dial {:?}", res);
    }

    Ok(swarm)
}

pub async fn start_loop(peer: &mut Peer, swarm: &mut Swarm<RequestResponse<EigenTrustCodec>>) {
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
