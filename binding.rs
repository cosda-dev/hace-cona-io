use crate::core::context::{IoExecutionContext, UnifiedRequest};

pub fn bind(req: UnifiedRequest, fan: &str) -> IoExecutionContext {
    IoExecutionContext {
        qpid: req.qpid,
        fan: fan.to_string(),
        payload: req.payload,
        source: req.source,
    }
}
