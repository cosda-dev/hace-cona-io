// cona/io/rac/dispatcher_bridge.rs
//
// Dispatcher Bridge: Connect cona RAC to me/race dispatcher
//
// Era 5 Architecture:
//   Bridges cona RAC adapters to me/race dispatcher.
//   Converts between SioEnvelope and RealityContract.
//
// Integration:
//   - cona RAC → SioEnvelope → RealityContract → dispatcher
//   - me/race dispatcher → RealityContract → SioEnvelope → cona RAC

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};

// Re-export SIO types
use crate::wire::{SioType, SioEnvelope, SioRacExt, RacProtocol};

/// Intent ID type
pub type IntentId = u128;

/// Dispatch plan for routing
#[derive(Debug, Clone)]
pub struct DispatchPlan {
    /// Intent ID
    pub intent_id: IntentId,
    /// Target protocol
    pub protocol: RacProtocol,
    /// SIO type
    pub sio_type: SioType,
    /// Priority (0=highest)
    pub priority: u8,
    /// Handler name (optional)
    pub handler: Option<String>,
}

impl DispatchPlan {
    /// Create new dispatch plan
    pub fn new(intent_id: IntentId, protocol: RacProtocol, sio_type: SioType) -> Self {
        Self {
            intent_id,
            protocol,
            sio_type,
            priority: 50,
            handler: None,
        }
    }
    
    /// With priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
    
    /// With handler
    pub fn with_handler(mut self, handler: &'static str) -> Self {
        self.handler = Some(handler.to_string());
        self
    }
}

/// Bridge error
#[derive(Debug, Clone)]
pub enum DispatcherBridgeError {
    EncodeError(String),
    DecodeError(String),
    UnknownIntent(IntentId),
    RoutingFailed(String),
}

impl core::fmt::Debug for DispatcherBridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EncodeError(s) => write!(f, "EncodeError({})", s),
            Self::DecodeError(s) => write!(f, "DecodeError({})", s),
            Self::UnknownIntent(id) => write!(f, "UnknownIntent({})", id),
            Self::RoutingFailed(s) => write!(f, "RoutingFailed({})", s),
        }
    }
}

/// Convert SioEnvelope to dispatch plan
pub fn envelope_to_dispatch_plan(envelope: &SioEnvelope) -> Result<DispatchPlan, DispatcherBridgeError> {
    let intent_id = envelope.intent_id();
    let sio_type = envelope.sio_type();
    
    // Determine protocol based on SIO type and envelope metadata
    let protocol = match sio_type {
        SioType::Intent | SioType::Reasoning => {
            // Check if remote
            if envelope.is_remote() {
                RacProtocol::Racex
            } else {
                RacProtocol::Raci
            }
        },
        SioType::Execute => RacProtocol::Raci, // Local execution
        SioType::Legal | SioType::Finance => RacProtocol::Racid, // In-device
        _ => RacProtocol::Raci,
    };
    
    Ok(DispatchPlan::new(intent_id, protocol, sio_type))
}

/// Resolve dispatch plan to handler name
pub fn resolve_handler(plan: &DispatchPlan) -> &'static str {
    // Map intent ID ranges to handler names
    match plan.intent_id {
        0..=999 => "kernel-handler",
        1000..=1999 => "omina-handler",
        2000..=2999 => "aura-handler",
        _ => "default-handler",
    }
}

/// Bridge state
pub struct DispatcherBridge {
    /// Enable protocol routing
    pub enable_protocol_routing: bool,
    /// Enable intent resolution
    pub enable_intent_resolution: bool,
}

impl Default for DispatcherBridge {
    fn default() -> Self {
        Self {
            enable_protocol_routing: true,
            enable_intent_resolution: true,
        }
    }
}

impl DispatcherBridge {
    /// Create new bridge
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Route envelope to dispatch plan
    pub fn route(&self, envelope: &SioEnvelope) -> Result<DispatchPlan, DispatcherBridgeError> {
        let mut plan = envelope_to_dispatch_plan(envelope)?;
        
        if self.enable_intent_resolution {
            let handler = resolve_handler(&plan);
            plan = plan.with_handler(handler);
        }
        
        Ok(plan)
    }
    
    /// Get recommended protocol for intent
    pub fn recommended_protocol(&self, intent_id: IntentId, sio_type: SioType) -> RacProtocol {
        match sio_type {
            SioType::Intent => {
                if intent_id < 1000 {
                    RacProtocol::Raci // Local kernel
                } else {
                    RacProtocol::Racex // Remote service
                }
            },
            SioType::Reasoning => RacProtocol::Racin, // Same-process
            SioType::Execute => RacProtocol::Raci, // Local FEM
            SioType::Legal | SioType::Finance => RacProtocol::Racid, // In-device IPC
            SioType::Memory | SioType::Data => RacProtocol::Racin, // Same-process
            SioType::Vision => RacProtocol::Racex, // Remote processing
        }
    }
}

// ─── Protocol Selection ──────────────────────────────────────────────────────

/// Select optimal protocol based on context
pub fn select_protocol(
    sio_type: SioType,
    is_remote: bool,
    is_same_process: bool,
    latency_requirement_ns: u64,
) -> RacProtocol {
    // Check latency requirements first
    if latency_requirement_ns < 1_000_000 {
        // < 1ms: must use same-process
        return RacProtocol::Racin;
    }
    
    // Check process boundary
    if is_same_process {
        return RacProtocol::Racin;
    }
    
    // Check remote
    if is_remote {
        return RacProtocol::Racex;
    }
    
    // Check SIO type
    match sio_type {
        SioType::Intent | SioType::Reasoning => {
            if is_remote {
                RacProtocol::Racex
            } else {
                RacProtocol::Raci
            }
        },
        SioType::Execute => RacProtocol::Raci,
        SioType::Legal | SioType::Finance => RacProtocol::Racid,
        SioType::Memory | SioType::Data => RacProtocol::Racin,
        SioType::Vision => RacProtocol::Racex,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dispatch_plan() {
        let plan = DispatchPlan::new(100, RacProtocol::Raci, SioType::Intent);
        assert_eq!(plan.intent_id, 100);
        assert_eq!(plan.protocol, RacProtocol::Raci);
    }
    
    #[test]
    fn test_handler_resolution() {
        let plan = DispatchPlan::new(500, RacProtocol::Raci, SioType::Intent);
        let handler = resolve_handler(&plan);
        assert_eq!(handler, "kernel-handler");
        
        let plan2 = DispatchPlan::new(1500, RacProtocol::Raci, SioType::Intent);
        let handler2 = resolve_handler(&plan2);
        assert_eq!(handler2, "omina-handler");
    }
    
    #[test]
    fn test_protocol_selection() {
        let proto = select_protocol(SioType::Execute, false, false, 1_000_000);
        assert_eq!(proto, RacProtocol::Raci);
        
        let proto2 = select_protocol(SioType::Legal, false, false, 1_000_000);
        assert_eq!(proto2, RacProtocol::Racid);
        
        let proto3 = select_protocol(SioType::Intent, false, true, 500_000);
        assert_eq!(proto3, RacProtocol::Racin);
    }
}