// cona/io/rac/mod.rs
//
// Cona RAC Module - Cognitive Routing Layer
//
// Era 6 Architecture:
//   Cona is a CONSUMER of hace-io-rac transport layer.
//   Cona implements cognitive routing (AlliagCognitiveRouter) but
//   delegates actual transport to hace-io-rac.
//
//   hace/io/rac* = transport DNA (kernel drivers)
//   hace/me/race = RAC execution runtime (kernel)
//   Cona/* = consumer/assembler only (user apps)
//
// This module re-exports:
//   - AlliagCognitiveRouter from allo_router (Cona's responsibility)
//   - Transport types from hace-io-rac (delegated to canonical layer)

use hace_cona_fem_shell::FemShellOrchestrator;

// ─── Cona's Cognitive Routing (NOT delegated) ────────────────────────────────
pub mod allo_router;        // Alliag cognitive routing (Cona's responsibility)
pub mod dispatcher_bridge;  // Protocol selection (Cona's responsibility)
pub mod race_integration;   // me/race integration (Cona's responsibility)

// ─── Deprecated: These are now in hace-io-rac ───────────────────────────────
// pub mod racid;  // Now: hace_io_rac::racid::RacidTransport
// pub mod racin;  // Now: hace_io_rac::racin::RacinTransport
// pub mod racex;  // Now: hace_io_rac::racex::RacexTransport

// ─── Mock Router (for testing) ──────────────────────────────────────────────
pub mod mock_router;

// ─── Legacy wire module (deprecated - use race_integration) ─────────────────
pub mod wire;               // Deprecated: Use race_integration instead
pub mod racbox_integration; // RacboxActor lease integration
pub mod sio_drive;          // SIO persistence

pub use mock_router::{FemHandle, MockRacRouter, RouteResult, RouterError};
pub use allo_router::{
    RacProtocol, ChannelState, ChannelSecurity, AuthMethod,
    RacRouteDecision, EntityClassification, RoutePriority, RetryPolicy,
    AlliagCognitiveRouter, SioRacExt, CompilerError,
};

// ─── Re-export from hace-io-rac (canonical transport layer) ──────────────────
pub use hace_io_rac::{
    RacTransport, RacTransportError,
    SioEnvelope, SioType, SioMetadata,
    SioResult, TransportConfig, SecurityLevel, RetryPolicy, TransportStats,
    TransportRegistry,
};

// ─── Transport aliases (for backward compatibility) ──────────────────────────
pub type RacidChannel = hace_io_rac::racid::RacidTransport;
pub type RacinChannel = hace_io_rac::racin::RacinTransport;
pub type RacexChannel = hace_io_rac::racex::RacexTransport;

pub const RAC_LAYER: &str = "RACI";
pub const CANON_SCOPE: &str = "instance-local adaptive dispatch";

pub struct RacRouter {
    shell: FemShellOrchestrator,
}

impl Default for RacRouter {
    fn default() -> Self { Self::new() }
}

impl RacRouter {
    pub fn new() -> Self { Self { shell: FemShellOrchestrator::new() } }

    pub fn canonical_layer(&self) -> &'static str {
        hace_io_rac::RACIN_LAYER
    }

    pub fn resolve_jid(&self, uri: &str) -> Result<u16, String> {
        hace_io_rac::resolve_uri_to_jid(uri).map_err(|err| format!("{err:?}"))
    }

    pub fn dispatch(&self, route: &str, payload: &str) -> Result<String, String> {
        self.shell.execute_route(route, payload.to_string())
    }

    pub fn dispatch_fan(&self, fan: &str, payload: &str) -> Result<String, String> {
        self.shell.execute_fan(fan, payload.to_string())
    }
}
