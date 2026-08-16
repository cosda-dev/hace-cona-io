use alloc::{string::String, vec::Vec};

pub type Qpid = u64;
pub type Sio = Vec<u8>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoSource {
    Rac,
    Http,
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityToken {
    pub actor: String,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRequest {
    pub qpid: Qpid,
    pub uri: String,
    pub payload: Sio,
    pub source: IoSource,
    pub authority: AuthorityToken,
    pub hop_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedResponse {
    pub status: u16,
    pub payload: Sio,
    pub fah: [u8; 32],
    pub source: IoSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoExecutionContext {
    pub qpid: Qpid,
    pub fan: String,
    pub payload: Sio,
    pub source: IoSource,
}

#[derive(Debug, Clone)]
pub struct IoContext {
    pub channel: String,
    pub qpid: String,
}
