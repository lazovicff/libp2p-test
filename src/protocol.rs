use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use libp2p::core::upgrade::{read_length_prefixed, write_length_prefixed};
use libp2p::request_response::{ProtocolName, RequestResponseCodec};
use std::io::Result;

#[derive(Debug, Clone)]
pub struct NeighbourRequestProtocol {
    version: NeighbourRequestProtocolVersion,
}

impl NeighbourRequestProtocol {
    pub fn new() -> Self {
        Self {
            version: NeighbourRequestProtocolVersion::V1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NeighbourRequestProtocolVersion {
    V1,
}

#[derive(Clone)]
pub struct NeighbourRequestCodec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Success,
    MaxNumOfNeighboursReached,
    Other(u8),
}

impl ProtocolName for NeighbourRequestProtocol {
    fn protocol_name(&self) -> &[u8] {
        match self.version {
            NeighbourRequestProtocolVersion::V1 => b"/neighbour_request/1",
        }
    }
}

#[async_trait]
impl RequestResponseCodec for NeighbourRequestCodec {
    type Protocol = NeighbourRequestProtocol;
    type Request = Request;
    type Response = Response;

    async fn read_request<T>(
        &mut self,
        protocol: &Self::Protocol,
        _: &mut T,
    ) -> Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        match protocol.version {
            NeighbourRequestProtocolVersion::V1 => Ok(Request),
        }
    }

    async fn read_response<T>(
        &mut self,
        protocol: &Self::Protocol,
        io: &mut T,
    ) -> Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        match protocol.version {
            NeighbourRequestProtocolVersion::V1 => {
                let status_bytes = read_length_prefixed(io, 1).await?;
                let response = match status_bytes[0] {
                    0 => Response::Success,
                    1 => Response::MaxNumOfNeighboursReached,
                    code => Response::Other(code),
                };
                Ok(response)
            }
        }
    }

    async fn write_request<T>(
        &mut self,
        protocol: &Self::Protocol,
        _: &mut T,
        _: Self::Request,
    ) -> Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        match protocol.version {
            NeighbourRequestProtocolVersion::V1 => Ok(()),
        }
    }

    async fn write_response<T>(
        &mut self,
        protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        match protocol.version {
            NeighbourRequestProtocolVersion::V1 => {
                let mut bytes = Vec::new();
                match res {
                    Response::Success => bytes.push(0),
                    Response::MaxNumOfNeighboursReached => bytes.push(1),
                    Response::Other(code) => bytes.push(code),
                };
                write_length_prefixed(io, &bytes).await?;
                Ok(())
            }
        }
    }
}
