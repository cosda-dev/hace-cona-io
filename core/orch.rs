use alloc::{boxed::Box, string::String, sync::Arc};
use core::cell::RefCell;

use crate::{
    backpressure::{gate_now, BackpressureState, CircuitBreaker},
    egress::{build_response, status_response},
    idempotency::IdempotencyGuard,
    ingress::{handle_http, handle_mcp, handle_rac, HttpRequest, McpPacket, RacPacket},
    loader::AmoLoader,
    registry::StrategyRegistry,
    resolver::ResolverCache,
    router::{route_with_strategy, RacexView, RouteContext, RoutingPolicy},
    strategy::IoStrategy,
};

use super::{context::UnifiedResponse, kernel::IoKernel, registry::IoRouteRegistry};

pub struct IoOrchestrator {
    pub registry: IoRouteRegistry,
    pub kernel: IoKernel,
    pub backpressure: BackpressureState,
    pub breaker: CircuitBreaker,
    pub resolver: ResolverCache,
    pub idempotency: RefCell<IdempotencyGuard>,
    pub loader: RefCell<AmoLoader>,
    pub strategy: Option<Box<dyn IoStrategy>>,
    pub strategy_registry: StrategyRegistry,
}

impl Default for IoOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl IoOrchestrator {
    pub fn new() -> Self {
        Self {
            registry: IoRouteRegistry::default(),
            kernel: IoKernel::new(),
            backpressure: BackpressureState::default(),
            breaker: CircuitBreaker::default(),
            resolver: ResolverCache::default(),
            idempotency: RefCell::new(IdempotencyGuard::new(8192)),
            loader: RefCell::new(AmoLoader::default()),
            strategy: None,
            strategy_registry: StrategyRegistry::default(),
        }
    }

    pub fn set_strategy(&mut self, strategy: Box<dyn IoStrategy>) {
        self.strategy = Some(strategy);
    }

    pub fn clear_strategy(&mut self) {
        self.strategy = None;
    }

    pub fn enable_trading_strategy(&mut self) {
        self.set_strategy(Box::new(crate::trading_strategy::TradingStrategy));
    }

    pub fn register_strategy(&self, domain: &str, strategy: Arc<dyn IoStrategy>) {
        self.strategy_registry.register(domain, strategy);
    }

    pub fn unregister_strategy(&self, domain: &str) {
        let _ = self.strategy_registry.unregister(domain);
    }

    pub fn set_fallback_strategy(&self, strategy: Arc<dyn IoStrategy>) {
        self.strategy_registry.set_fallback(strategy);
    }

    pub fn clear_fallback_strategy(&self) {
        self.strategy_registry.clear_fallback();
    }

    pub fn register_route(&mut self, route: String, fan: String) {
        self.registry.register(route, fan);
    }

    pub fn dispatch(&self, route: &str) -> Vec<u8> {
        self.dispatch_payload(route, route)
    }

    pub fn dispatch_payload(&self, route: &str, payload: &str) -> Vec<u8> {
        let req = handle_rac(RacPacket {
            qpid: 0,
            uri: route.to_string(),
            payload: payload.as_bytes().to_vec(),
            authority_actor: "SW".to_string(),
            signed: true,
        });
        let resp = self.dispatch_unified(req);
        resp.payload
    }

    pub fn resolve_jid(&self, route: &str) -> Option<u16> {
        self.kernel.resolve_jid(route)
    }

    pub fn dispatch_rac(&self, packet: RacPacket) -> UnifiedResponse {
        self.dispatch_unified(handle_rac(packet))
    }

    pub fn dispatch_http(&self, req: HttpRequest) -> UnifiedResponse {
        self.dispatch_unified(handle_http(req))
    }

    pub fn dispatch_mcp(&self, pkt: McpPacket) -> UnifiedResponse {
        self.dispatch_unified(handle_mcp(pkt))
    }

    fn dispatch_unified(&self, req: crate::core::context::UnifiedRequest) -> UnifiedResponse {
        self.backpressure.inc_queue();
        let _queue_guard = ScopeGuard::new(|| self.backpressure.dec_queue());

        if !self.breaker.check(now_ns()) {
            return status_response(req.source, 503, "circuit open");
        }

        let domain = strategy_domain(&req);
        let dynamic_strategy = self.strategy_registry.resolve(domain.as_str());
        let strategy = self.strategy.as_deref().or(dynamic_strategy.as_deref());
        let qpid_key = strategy_qpid(&req, strategy);
        if !self.idempotency.borrow_mut().check_qpid(qpid_key) {
            return status_response(req.source, 409, "duplicate qpid");
        }

        if let Err(err) = gate_now(&self.backpressure) {
            return status_response(req.source, 429, err);
        }

        let uri = if req.uri.starts_with("rac://")
            || req.uri.starts_with("http://")
            || req.uri.starts_with("mcp://")
        {
            req.uri.clone()
        } else {
            format!("rac://{0}", req.uri)
        };

        let fan_desc = match self.resolver.resolve_uri(uri.as_str()) {
            Ok(f) => f,
            Err(_) => return status_response(req.source, 404, "fan not found"),
        };

        let local = self.backpressure.snapshot();
        let decision = route_with_strategy(
            &RouteContext {
                fan: fan_desc,
                pressure: local,
                racex: RacexView::default(),
            },
            RoutingPolicy::PreferLocal,
            Some(&req),
            strategy,
        );
        let target = decision.target;

        let amo_id = match target {
            crate::router::Target::Local(id) => id,
            crate::router::Target::Remote(_, id) => id,
        };

        let amo_key = format!("amo.{amo_id}");
        let mut loader = self.loader.borrow_mut();
        if loader.verify_hooks(amo_key.as_str()).is_err() {
            return status_response(req.source, 403, "amo validation failed");
        }
        loader.ensure_loaded(amo_key.as_str());

        self.backpressure.inc_inflight();
        let start = now_ns();
        let output = self
            .kernel
            .execute_bound(req.clone(), fan_desc.fan_name);
        let elapsed = now_ns().saturating_sub(start);
        self.backpressure.update_latency(elapsed);
        self.backpressure.dec_inflight();

        self.breaker.on_success();
        build_response(req.source, output.into_bytes())
    }
}

struct ScopeGuard<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> ScopeGuard<F> {
    fn new(f: F) -> Self {
        Self(Some(f))
    }
}

impl<F: FnOnce()> Drop for ScopeGuard<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

fn now_ns() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        return SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
    }

    #[cfg(not(feature = "std"))]
    {
        0
    }
}

fn strategy_qpid(
    req: &crate::core::context::UnifiedRequest,
    strategy: Option<&dyn IoStrategy>,
) -> u64 {
    let Some(strategy) = strategy else {
        return req.qpid;
    };
    let Some(salt) = strategy.idempotency_salt(req) else {
        return req.qpid;
    };

    let mut hasher = blake3::Hasher::new();
    hasher.update(&req.qpid.to_le_bytes());
    hasher.update(salt.as_slice());
    let digest = hasher.finalize();
    let mut key = [0u8; 8];
    key.copy_from_slice(&digest.as_bytes()[..8]);
    u64::from_le_bytes(key)
}

fn strategy_domain(req: &crate::core::context::UnifiedRequest) -> String {
    let uri = req.uri.as_str();
    let no_scheme = uri
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(uri);
    if let Some((head, _)) = no_scheme.split_once('/') {
        if !head.is_empty() {
            return head.to_string();
        }
    }
    if req.authority.actor.contains("trade") {
        return "trade".to_string();
    }
    "default".to_string()
}
