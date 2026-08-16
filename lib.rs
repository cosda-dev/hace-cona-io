#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod backpressure;
pub mod binding;
pub mod core;
pub mod egress;
pub mod idempotency;
pub mod ingress;
pub mod loader;
pub mod registry;
pub mod resolver;
pub mod router;
pub mod strategy;
pub mod trading_strategy;
pub mod rac;
pub mod solder;

pub use core::context::{AuthorityToken, IoExecutionContext, IoSource, Qpid, UnifiedRequest, UnifiedResponse};
pub use core::kernel::IoKernel;
pub use core::orch::IoOrchestrator;
pub use rac::RacRouter;
pub use registry::StrategyRegistry;
