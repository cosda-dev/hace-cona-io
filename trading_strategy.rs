use alloc::vec::Vec;

use crate::{
    core::context::UnifiedRequest,
    router::RouteContext,
    strategy::{IoStrategy, PressureScore, QosClass, RetryPlan, RouteHint},
};

#[derive(Debug, Default, Clone, Copy)]
pub struct TradingStrategy;

impl TradingStrategy {
    fn is_trade_request(req: &UnifiedRequest) -> bool {
        req.uri.contains("/trade")
            || req.uri.contains("trade.")
            || req.authority.actor == "trading-bot"
    }
}

impl IoStrategy for TradingStrategy {
    fn select_route(
        &self,
        ctx: &RouteContext,
        req: &UnifiedRequest,
    ) -> Option<RouteHint> {
        if !Self::is_trade_request(req) {
            return None;
        }

        if ctx.pressure.queue < 0.95 {
            return Some(RouteHint::ForceLocal);
        }

        let mut best_remote = None;
        let mut best_queue = 1.1_f32;
        for peer in &ctx.racex.peers {
            if peer.pressure.queue < best_queue {
                best_queue = peer.pressure.queue;
                best_remote = Some(peer.node_id);
            }
        }

        best_remote.map(RouteHint::ForceRemote)
    }

    fn qos_policy(&self, req: &UnifiedRequest) -> Option<QosClass> {
        if Self::is_trade_request(req) {
            Some(QosClass::LowLatency)
        } else {
            None
        }
    }

    fn retry_policy(&self, req: &UnifiedRequest, _status: u16) -> Option<RetryPlan> {
        if Self::is_trade_request(req) {
            Some(RetryPlan::NoRetry)
        } else {
            None
        }
    }

    fn pressure_bias(
        &self,
        _ctx: &RouteContext,
        req: &UnifiedRequest,
    ) -> PressureScore {
        if Self::is_trade_request(req) {
            -0.35
        } else {
            0.0
        }
    }

    fn idempotency_salt(&self, req: &UnifiedRequest) -> Option<Vec<u8>> {
        if Self::is_trade_request(req) {
            Some(b"domain:trading:v1".to_vec())
        } else {
            None
        }
    }
}
