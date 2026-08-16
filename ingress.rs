use alloc::{string::String, vec::Vec};

use crate::core::context::{AuthorityToken, IoSource, Qpid, UnifiedRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RacPacket {
    pub qpid: Qpid,
    pub uri: String,
    pub payload: Vec<u8>,
    pub authority_actor: String,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub qpid: Qpid,
    pub uri: String,
    pub body: Vec<u8>,
    pub auth_subject: String,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPacket {
    pub qpid: Qpid,
    pub uri: String,
    pub payload: Vec<u8>,
    pub node: String,
    pub signed: bool,
}

pub fn handle_rac(raw: RacPacket) -> UnifiedRequest {
    UnifiedRequest {
        qpid: raw.qpid,
        uri: normalize_uri(&raw.uri),
        payload: raw.payload,
        source: IoSource::Rac,
        authority: AuthorityToken {
            actor: raw.authority_actor,
            signed: raw.signed,
        },
        hop_count: 0,
    }
}

pub fn handle_http(req: HttpRequest) -> UnifiedRequest {
    UnifiedRequest {
        qpid: req.qpid,
        uri: normalize_uri(&req.uri),
        payload: req.body,
        source: IoSource::Http,
        authority: AuthorityToken {
            actor: req.auth_subject,
            signed: req.signed,
        },
        hop_count: 0,
    }
}

pub fn handle_mcp(pkt: McpPacket) -> UnifiedRequest {
    UnifiedRequest {
        qpid: pkt.qpid,
        uri: normalize_uri(&pkt.uri),
        payload: pkt.payload,
        source: IoSource::Mcp,
        authority: AuthorityToken {
            actor: pkt.node,
            signed: pkt.signed,
        },
        hop_count: 1,
    }
}

fn normalize_uri(uri: &str) -> String {
    let trimmed = uri.trim();
    if trimmed.ends_with('/') && trimmed.len() > 1 {
        trimmed.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}
