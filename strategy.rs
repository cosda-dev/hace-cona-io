use alloc::vec::Vec;

use crate::{
    core::context::UnifiedRequest,
    router::{NodeId, RouteContext},
};

pub type PressureScore = f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteHint {
    Keep,
    ForceLocal,
    ForceRemote(NodeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QosClass {
    Standard,
    LowLatency,
    Durable,
    Throughput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPlan {
    NoRetry,
    Immediate { max_attempts: u8 },
    Backoff { max_attempts: u8, base_ms: u16 },
}

pub trait IoStrategy: Send + Sync {
    fn select_route(
        &self,
        _ctx: &RouteContext,
        _req: &UnifiedRequest,
    ) -> Option<RouteHint> {
        None
    }

    fn qos_policy(&self, _req: &UnifiedRequest) -> Option<QosClass> {
        None
    }

    fn retry_policy(&self, _req: &UnifiedRequest, _status: u16) -> Option<RetryPlan> {
        None
    }

    fn pressure_bias(
        &self,
        _ctx: &RouteContext,
        _req: &UnifiedRequest,
    ) -> PressureScore {
        0.0
    }

    fn idempotency_salt(&self, _req: &UnifiedRequest) -> Option<Vec<u8>> {
        None
    }
}
