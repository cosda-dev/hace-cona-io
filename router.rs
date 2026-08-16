use alloc::vec::Vec;

use crate::{
    core::context::UnifiedRequest,
    backpressure::PressureSnapshot,
    resolver::{AmoId, FanDescriptor},
    strategy::{IoStrategy, PressureScore, QosClass, RetryPlan, RouteHint},
};

pub type NodeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Local(AmoId),
    Remote(NodeId, AmoId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodePressure {
    pub node_id: NodeId,
    pub pressure: PressureSnapshot,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RacexView {
    pub peers: Vec<NodePressure>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteContext {
    pub fan: FanDescriptor,
    pub pressure: PressureSnapshot,
    pub racex: RacexView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteDecision {
    pub target: Target,
    pub qos: QosClass,
    pub retry: RetryPlan,
    pub pressure_score: PressureScore,
    pub idempotency_salt: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingPolicy {
    LocalOnly,
    PreferLocal,
    PressureAware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteEntry {
    pub fan: FanDescriptor,
    pub amo_target: AmoId,
    pub policy: RoutingPolicy,
}

pub fn select_target(entry: &RouteEntry, pressure: PressureSnapshot) -> Target {
    match entry.policy {
        RoutingPolicy::LocalOnly => Target::Local(entry.amo_target),
        RoutingPolicy::PreferLocal if pressure.queue < 0.8 => Target::Local(entry.amo_target),
        RoutingPolicy::PressureAware if pressure.queue < 0.8 => Target::Local(entry.amo_target),
        _ => Target::Remote(0, entry.amo_target),
    }
}

pub fn route(ctx: &RouteContext, policy: RoutingPolicy) -> Target {
    route_with_strategy(ctx, policy, None, None).target
}

pub fn route_with_strategy(
    ctx: &RouteContext,
    policy: RoutingPolicy,
    req: Option<&UnifiedRequest>,
    strategy: Option<&dyn IoStrategy>,
) -> RouteDecision {
    let mut target = match policy {
        RoutingPolicy::LocalOnly => Target::Local(ctx.fan.amo_id),
        RoutingPolicy::PreferLocal => {
            if ctx.pressure.queue < 0.8 {
                Target::Local(ctx.fan.amo_id)
            } else {
                fallback_remote(ctx)
            }
        }
        RoutingPolicy::PressureAware => select_best_node(ctx),
    };

    let mut pressure_score = base_pressure_score(ctx);
    if let (Some(req), Some(strategy)) = (req, strategy) {
        pressure_score += strategy.pressure_bias(ctx, req);
        if pressure_score > 0.95 {
            target = fallback_remote(ctx);
        }
        if let Some(hint) = strategy.select_route(ctx, req) {
            target = apply_hint(target, hint, ctx.fan.amo_id);
        }
    }

    let qos = match (req, strategy) {
        (Some(req), Some(strategy)) => strategy.qos_policy(req).unwrap_or(QosClass::Standard),
        _ => QosClass::Standard,
    };

    let retry = match (req, strategy) {
        (Some(req), Some(strategy)) => strategy
            .retry_policy(req, 0)
            .unwrap_or(RetryPlan::Backoff {
                max_attempts: 3,
                base_ms: 25,
            }),
        _ => RetryPlan::Backoff {
            max_attempts: 3,
            base_ms: 25,
        },
    };

    let idempotency_salt = match (req, strategy) {
        (Some(req), Some(strategy)) => strategy.idempotency_salt(req),
        _ => None,
    };

    RouteDecision {
        target,
        qos,
        retry,
        pressure_score,
        idempotency_salt,
    }
}

fn base_pressure_score(ctx: &RouteContext) -> PressureScore {
    (ctx.pressure.queue + ctx.pressure.cpu * 0.5 + ctx.pressure.memory * 0.2).min(1.5)
}

fn apply_hint(target: Target, hint: RouteHint, fallback_amo: AmoId) -> Target {
    match hint {
        RouteHint::Keep => target,
        RouteHint::ForceLocal => Target::Local(fallback_amo),
        RouteHint::ForceRemote(node_id) => Target::Remote(node_id, fallback_amo),
    }
}

pub fn route_legacy(ctx: &RouteContext, policy: RoutingPolicy) -> Target {
    match policy {
        RoutingPolicy::LocalOnly => Target::Local(ctx.fan.amo_id),
        RoutingPolicy::PreferLocal => {
            if ctx.pressure.queue < 0.8 {
                Target::Local(ctx.fan.amo_id)
            } else {
                fallback_remote(ctx)
            }
        }
        RoutingPolicy::PressureAware => select_best_node(ctx),
    }
}

fn fallback_remote(ctx: &RouteContext) -> Target {
    for peer in &ctx.racex.peers {
        if peer.pressure.queue < 0.7 {
            return Target::Remote(peer.node_id, ctx.fan.amo_id);
        }
    }
    Target::Local(ctx.fan.amo_id)
}

fn select_best_node(ctx: &RouteContext) -> Target {
    let mut best_score = f32::MAX;
    let mut best_node: Option<NodeId> = None;

    for peer in &ctx.racex.peers {
        let score = peer.pressure.queue + peer.pressure.cpu * 0.5;
        if score < best_score {
            best_score = score;
            best_node = Some(peer.node_id);
        }
    }

    match best_node {
        Some(node) if best_score < ctx.pressure.queue => Target::Remote(node, ctx.fan.amo_id),
        _ => Target::Local(ctx.fan.amo_id),
    }
}
