use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use core::time::Duration;

#[cfg(feature = "std")]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureSnapshot {
    pub cpu: f32,
    pub memory: f32,
    pub queue: f32,
}

#[derive(Debug)]
pub struct BackpressureState {
    pub queue_depth: AtomicU32,
    pub in_flight: AtomicU32,
    pub avg_latency_ns: AtomicU64,
    pub last_tick_ns: AtomicU64,
    max_queue: u32,
    max_inflight: u32,
}

impl Default for BackpressureState {
    fn default() -> Self {
        Self::new(1024, 256)
    }
}

impl BackpressureState {
    pub const fn new(max_queue: u32, max_inflight: u32) -> Self {
        Self {
            queue_depth: AtomicU32::new(0),
            in_flight: AtomicU32::new(0),
            avg_latency_ns: AtomicU64::new(0),
            last_tick_ns: AtomicU64::new(0),
            max_queue,
            max_inflight,
        }
    }

    pub fn snapshot(&self) -> PressureSnapshot {
        let queue = self.queue_depth.load(Ordering::Relaxed) as f32;
        let inflight = self.in_flight.load(Ordering::Relaxed) as f32;

        PressureSnapshot {
            cpu: (inflight / self.max_inflight as f32).min(1.0),
            memory: estimate_memory_pressure(),
            queue: (queue / self.max_queue as f32).min(1.0),
        }
    }

    pub fn inc_queue(&self) {
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_queue(&self) {
        self.queue_depth.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inc_inflight(&self) {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_inflight(&self) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn update_latency(&self, new_ns: u64) {
        let old = self.avg_latency_ns.load(Ordering::Relaxed);
        let updated = (old.saturating_mul(7)).saturating_add(new_ns) / 8;
        self.avg_latency_ns.store(updated, Ordering::Relaxed);
        self.last_tick_ns.store(now_ns(), Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureAction {
    Accept,
    Throttle,
    Reject,
}

pub fn decide(p: &PressureSnapshot) -> PressureAction {
    if p.queue > 0.9 || p.memory > 0.9 {
        PressureAction::Reject
    } else if p.queue > 0.7 || p.cpu > 0.8 {
        PressureAction::Throttle
    } else {
        PressureAction::Accept
    }
}

pub fn gate_now(state: &BackpressureState) -> Result<(), &'static str> {
    match decide(&state.snapshot()) {
        PressureAction::Accept => Ok(()),
        PressureAction::Throttle => Ok(()),
        PressureAction::Reject => Err("429: pressure overload"),
    }
}

pub async fn gate(state: &BackpressureState) -> Result<(), &'static str> {
    let snapshot = state.snapshot();
    match decide(&snapshot) {
        PressureAction::Accept => Ok(()),
        PressureAction::Throttle => {
            throttle_delay(snapshot).await;
            Ok(())
        }
        PressureAction::Reject => Err("429: pressure overload"),
    }
}

pub async fn throttle_delay(p: PressureSnapshot) {
    let delay_ms = ((p.queue * 100.0) as u64).min(200);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}

#[derive(Debug)]
pub struct CircuitBreaker {
    pub failure_count: AtomicU32,
    pub open_until: AtomicU64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            open_until: AtomicU64::new(0),
        }
    }
}

impl CircuitBreaker {
    pub fn check(&self, now_ns: u64) -> bool {
        now_ns >= self.open_until.load(Ordering::Relaxed)
    }

    pub fn on_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
    }

    pub fn on_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn maybe_trip(&self, threshold: u32, duration: Duration) {
        if self.failure_count.load(Ordering::Relaxed) > threshold {
            self.trip(duration);
        }
    }

    pub fn trip(&self, duration: Duration) {
        let now = now_ns();
        self.open_until
            .store(now.saturating_add(duration.as_nanos() as u64), Ordering::Relaxed);
    }
}

pub async fn hp_port_entry(
    bp: &BackpressureState,
    cb: &CircuitBreaker,
) -> Result<(), &'static str> {
    if !cb.check(now_ns()) {
        return Err("circuit open");
    }
    gate(bp).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureSignal {
    pub cpu: u8,
    pub memory: u8,
    pub queue: u8,
}

pub fn export_signal(p: PressureSnapshot) -> PressureSignal {
    PressureSignal {
        cpu: (p.cpu * 100.0).min(100.0) as u8,
        memory: (p.memory * 100.0).min(100.0) as u8,
        queue: (p.queue * 100.0).min(100.0) as u8,
    }
}

fn estimate_memory_pressure() -> f32 {
    // v100a deterministic placeholder: we can replace with OS probe in v200.
    0.25
}

fn now_ns() -> u64 {
    #[cfg(feature = "std")]
    {
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
