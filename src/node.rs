use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::core::upgrade::Version;
use libp2p::identity::Keypair;
use libp2p::noise::{Keypair as NoiseKeypair, NoiseConfig, X25519Spec};
use libp2p::request_response::{
    ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
    RequestResponseMessage,
};
use libp2p::swarm::{Swarm, SwarmEvent};
use libp2p::tcp::TcpConfig;
use libp2p::yamux::YamuxConfig;
use libp2p::{Multiaddr, PeerId, Transport};

use std::iter::once;
use std::str::FromStr;
use std::time::Duration;

use futures::prelude::*;

use crate::protocol::{NeighbourRequestCodec, NeighbourRequestProtocol, Request, Response};
use crate::EigenError;

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
    let mut swarm = Swarm::new(transport, req_proto, local_peer_id);
    swarm.listen_on(local_address).map_err(|e| {
        log::debug!("swarm.listen_on {:?}", e);
        EigenError::ListenFailed
    })?;

    // We want to connect to all bootstrap nodes.
    for info in bootstrap_nodes.iter() {
        let addr = Multiaddr::from_str(info[0]).map_err(|e| {
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

        swarm.behaviour_mut().add_address(&peer_id, addr.clone());
        swarm.behaviour_mut().send_request(&peer_id, Request);
    }

    Ok(swarm)
}

fn handle_request_response_event(
    req_res: &mut RequestResponse<NeighbourRequestCodec>,
    event: RequestResponseEvent<Request, Response>,
) {
    match event {
        RequestResponseEvent::Message {
            peer,
            message: RequestResponseMessage::Request { channel, .. },
        } => {
            log::info!("Received request from {:?}", peer);
            req_res.send_response(channel, Response::Success);
        }
		RequestResponseEvent::Message {
			peer,
			message: RequestResponseMessage::Response { response, .. },
		} => {
			log::info!("Received response from {:?}", peer);
			if let Response::Success = response {
				log::info!("Successfully connected to {:?}", peer);
			}
			log::info!("Response: {:?}", response);
		}
        RequestResponseEvent::InboundFailure {
            peer,
            request_id,
            error,
        } => {
            log::info!(
                "Failed to handle request ({:?}) from {:?}: {:?}",
                request_id,
                peer,
                error
            );
        }
        RequestResponseEvent::OutboundFailure {
            peer,
            request_id,
            error,
        } => {
            log::info!(
                "Failed to send request ({:?}) to {:?}: {:?}",
                request_id,
                peer,
                error
            );
        }
        RequestResponseEvent::ResponseSent { peer, request_id } => {
            log::info!("Sent response to {:?} ({:?})", peer, request_id);
        }
    }
}

pub async fn start_loop(swarm: &mut Swarm<RequestResponse<NeighbourRequestCodec>>) {
    println!("");
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => log::info!("Listening on {:?}", address),
            SwarmEvent::Behaviour(event) => {
                handle_request_response_event(swarm.behaviour_mut(), event)
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                log::info!("Connection established with {:?}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                log::info!("Connection closed with {:?} ({:?})", peer_id, cause);
            }
            SwarmEvent::IncomingConnection { send_back_addr, .. } => {
                log::debug!("Incoming connection to {:?}", send_back_addr);
            }
            SwarmEvent::IncomingConnectionError {
                send_back_addr,
                error,
                ..
            } => {
                log::debug!(
                    "Incoming connection error to {:?} ({:?})",
                    send_back_addr,
                    error
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                log::debug!("Outgoing connection error with {:?} ({:?})", peer_id, error);
            }
            SwarmEvent::BannedPeer { peer_id, endpoint } => {
                log::debug!("Banned peer {:?} ({:?})", peer_id, endpoint);
            }
            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => {
                log::debug!("Expired listen addr {:?} ({:?})", listener_id, address);
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                log::info!(
                    "Listener closed {:?} ({:?}) ({:?})",
                    listener_id,
                    addresses,
                    reason
                );
            }
            SwarmEvent::ListenerError { listener_id, error } => {
                log::info!("Listener error {:?} ({:?})", listener_id, error);
            }
            SwarmEvent::Dialing(peer_id) => {
                log::info!("Dialing {:?}", peer_id);
            }
        }
    }
}
