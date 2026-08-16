use crate::core::context::{IoSource, UnifiedResponse};

pub fn build_response(source: IoSource, payload: Vec<u8>) -> UnifiedResponse {
    UnifiedResponse {
        status: 200,
        fah: blake3::hash(&payload).into(),
        payload,
        source,
    }
}

pub fn status_response(source: IoSource, status: u16, msg: &str) -> UnifiedResponse {
    UnifiedResponse {
        status,
        fah: blake3::hash(msg.as_bytes()).into(),
        payload: msg.as_bytes().to_vec(),
        source,
    }
}
