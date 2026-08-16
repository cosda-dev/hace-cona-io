// cona/io/rac/mock_router.rs
//
// MockRacRouter — Multi-FEM simulation (in-memory RACEX).
//
// Mục đích: test Hyda distributed dispatch WITHOUT real network.
// Production: thay handler registry bằng real RACEX transport (HTTP/gRPC).
//
// Architecture:
//   MockRacRouter
//     ├── fem_registry: BTreeMap<FemId, Arc<FemHandle>>
//     ├── circuit_map:  BTreeMap<FemId, RepCircuit> (per-target)
//     └── pcb_graph:   PcbGraph (optional — for graph-based dispatch)
//
// Features:
//   - register_fem(id, handler_fn)  — register a simulated remote FEM
//   - call(fem_id, fan, input)      — route to remote FEM handler
//   - circuit breaker per FEM       — trip after N failures
//   - collect SepiLinks across hops — build SepiChain
//
// Invariants:
//   RC.RACEX.MOCK.NO_NETWORK  — no tokio, no real I/O
//   RC.RACEX.MOCK.DETERMINISTIC — same input → same output
//   RC.FEM.NO_CROSS_DIRECT    — all remote calls go through MockRacRouter

use alloc::{
    collections::BTreeMap,
    string::String,
    vec::Vec,
};
use core::sync::atomic::{AtomicU32, Ordering};

// ─── FemHandle ───────────────────────────────────────────────────────────────

/// Simulated remote FEM: a named handler + metadata.
#[derive(Clone)]
pub struct FemHandle {
    pub fem_id:  String,
    pub handler: fn(fan: &str, input: &[u8]) -> Vec<u8>,
}

impl FemHandle {
    pub fn new(fem_id: impl Into<String>, handler: fn(&str, &[u8]) -> Vec<u8>) -> Self {
        Self { fem_id: fem_id.into(), handler }
    }

    pub fn execute(&self, fan: &str, input: &[u8]) -> Vec<u8> {
        (self.handler)(fan, input)
    }
}

// ─── Per-FEM circuit breaker ─────────────────────────────────────────────────

struct FemCircuit {
    failures:     AtomicU32,
    open_until:   core::sync::atomic::AtomicU64,
    threshold:    u32,
    cool_epochs:  u64,
}

impl FemCircuit {
    fn new(threshold: u32, cool_epochs: u64) -> Self {
        Self {
            failures: AtomicU32::new(0),
            open_until: core::sync::atomic::AtomicU64::new(0),
            threshold,
            cool_epochs,
        }
    }

    fn is_open(&self, epoch: u64) -> bool {
        let until = self.open_until.load(Ordering::Relaxed);
        until > 0 && epoch < until
    }

    fn on_success(&self) {
        self.failures.store(0, Ordering::Relaxed);
        self.open_until.store(0, Ordering::Relaxed);
    }

    fn on_failure(&self, epoch: u64) {
        let f = self.failures.fetch_add(1, Ordering::Relaxed) + 1;
        if f >= self.threshold {
            self.open_until.store(epoch + self.cool_epochs, Ordering::Relaxed);
        }
    }
}

// ─── RouteResult ─────────────────────────────────────────────────────────────

/// Result of a single hop in MockRacRouter.
#[derive(Debug, Clone)]
pub struct RouteResult {
    pub fem_id:      String,
    pub fan:         String,
    pub output:      Vec<u8>,
    pub input_hash:  [u8; 32],
    pub output_hash: [u8; 32],
    pub epoch:       u64,
}

impl RouteResult {
    pub fn hop_feh(&self) -> [u8; 32] {
        let mut mat = Vec::with_capacity(6 + 64);
        mat.extend_from_slice(b"feh-v1");
        mat.extend_from_slice(self.fem_id.as_bytes());
        mat.extend_from_slice(self.fan.as_bytes());
        mat.extend_from_slice(&self.input_hash);
        mat.extend_from_slice(&self.output_hash);
        *blake3::hash(&mat).as_bytes()
    }
}

// ─── RouterError ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterError {
    FemNotFound(String),
    CircuitOpen(String),
    FanNotFound(String),
}

// ─── MockRacRouter ───────────────────────────────────────────────────────────

/// In-memory RACEX mock router — no network required.
///
/// Usage:
/// ```
/// let mut router = MockRacRouter::new("cluster-1");
/// router.register_fem(FemHandle::new("fem-b", |fan, input| {
///     format!("fem-b:{fan}:{}", String::from_utf8_lossy(input)).into_bytes()
/// }));
/// let res = router.call("fem-b", "payment.execute", b"amount=100", 1).unwrap();
/// ```
pub struct MockRacRouter {
    pub cluster_id: String,
    fems:      BTreeMap<String, FemHandle>,
    circuits:  BTreeMap<String, FemCircuit>,
    cb_threshold:  u32,
    cb_cool_epochs: u64,
}

impl MockRacRouter {
    pub fn new(cluster_id: impl Into<String>) -> Self {
        Self {
            cluster_id: cluster_id.into(),
            fems: BTreeMap::new(),
            circuits: BTreeMap::new(),
            cb_threshold: 5,
            cb_cool_epochs: 30,
        }
    }

    pub fn with_circuit_config(mut self, threshold: u32, cool_epochs: u64) -> Self {
        self.cb_threshold = threshold;
        self.cb_cool_epochs = cool_epochs;
        self
    }

    /// Register a simulated remote FEM.
    pub fn register_fem(&mut self, fem: FemHandle) {
        self.circuits.insert(
            fem.fem_id.clone(),
            FemCircuit::new(self.cb_threshold, self.cb_cool_epochs),
        );
        self.fems.insert(fem.fem_id.clone(), fem);
    }

    /// Call a specific FEM's fan handler.
    pub fn call(
        &self,
        fem_id: &str,
        fan:    &str,
        input:  &[u8],
        epoch:  u64,
    ) -> Result<RouteResult, RouterError> {
        // Circuit check
        if let Some(cb) = self.circuits.get(fem_id) {
            if cb.is_open(epoch) {
                return Err(RouterError::CircuitOpen(fem_id.into()));
            }
        }

        let fem = self.fems.get(fem_id)
            .ok_or_else(|| RouterError::FemNotFound(fem_id.into()))?;

        let output = fem.execute(fan, input);

        let input_hash  = *blake3::hash(input).as_bytes();
        let output_hash = *blake3::hash(&output).as_bytes();

        if let Some(cb) = self.circuits.get(fem_id) {
            cb.on_success();
        }

        Ok(RouteResult {
            fem_id:  fem_id.into(),
            fan:     fan.into(),
            output,
            input_hash,
            output_hash,
            epoch,
        })
    }

    /// Multi-hop call: route through a chain of (fem_id, fan) pairs.
    pub fn call_chain(
        &self,
        hops:  &[(String, String)], // [(fem_id, fan)]
        input: &[u8],
        epoch: u64,
    ) -> Result<Vec<RouteResult>, RouterError> {
        let mut results = Vec::new();
        let mut current = input.to_vec();

        for (fem_id, fan) in hops {
            let r = self.call(fem_id, fan, &current, epoch)?;
            current = r.output.clone();
            results.push(r);
        }
        Ok(results)
    }

    /// Inject a failure for testing circuit breaker.
    pub fn inject_failure(&self, fem_id: &str, epoch: u64) {
        if let Some(cb) = self.circuits.get(fem_id) {
            cb.on_failure(epoch);
        }
    }

    pub fn fem_count(&self) -> usize { self.fems.len() }
    pub fn has_fem(&self, fem_id: &str) -> bool { self.fems.contains_key(fem_id) }

    /// Build RAC URI for a specific FEM + FAN.
    pub fn rac_uri(&self, fem_id: &str, fan: &str) -> String {
        alloc::format!("rac://hyda/{}/{}/fan/{}", self.cluster_id, fem_id, fan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_fem(_fan: &str, input: &[u8]) -> Vec<u8> { input.to_vec() }
    fn upper_fem(fan: &str, input: &[u8]) -> Vec<u8> {
        let s = core::str::from_utf8(input).unwrap_or_default();
        alloc::format!("[{}] {}", fan, s.to_uppercase()).into_bytes()
    }

    fn make_router() -> MockRacRouter {
        let mut r = MockRacRouter::new("cluster-1").with_circuit_config(3, 10);
        r.register_fem(FemHandle::new("fem-a", echo_fem));
        r.register_fem(FemHandle::new("fem-b", upper_fem));
        r
    }

    #[test]
    fn single_hop_routes_correctly() {
        let r = make_router();
        let res = r.call("fem-b", "core.upper", b"hello", 1).unwrap();
        let s = String::from_utf8(res.output.clone()).unwrap();
        assert!(s.contains("HELLO"));
        assert_ne!(res.hop_feh(), [0u8; 32]);
    }

    #[test]
    fn unknown_fem_returns_error() {
        let r = make_router();
        assert!(matches!(
            r.call("fem-z", "core.echo", b"x", 1),
            Err(RouterError::FemNotFound(_))
        ));
    }

    #[test]
    fn circuit_opens_after_failures() {
        let r = make_router();
        for i in 0..3 { r.inject_failure("fem-a", i); }
        assert!(matches!(
            r.call("fem-a", "core.echo", b"x", 2),
            Err(RouterError::CircuitOpen(_))
        ));
    }

    #[test]
    fn multi_hop_chain_pipes_output() {
        let r = make_router();
        let hops = vec![
            ("fem-a".into(), "core.echo".into()),
            ("fem-b".into(), "core.upper".into()),
        ];
        let results = r.call_chain(&hops, b"world", 1).unwrap();
        assert_eq!(results.len(), 2);
        assert!(String::from_utf8(results[1].output.clone()).unwrap().contains("WORLD"));
    }

    #[test]
    fn rac_uri_format() {
        let r = make_router();
        assert_eq!(
            r.rac_uri("fem-b", "payment.execute"),
            "rac://hyda/cluster-1/fem-b/fan/payment.execute"
        );
    }
}
