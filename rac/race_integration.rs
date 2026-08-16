// cona/io/rac/race_integration.rs
//
// RACE Integration: Connect cona RAC to me/race dispatcher
//
// Era 5 Architecture:
//   Bridges cona RAC adapters (wire, racbox, sio_drive) to me/race dispatcher.
//   Provides async dispatch with protocol selection and hook integration.
//
// Integration flow:
//   SioEnvelope → RacWire → DispatchPlan → me/race/dispatcher → RealityContract
//
// me/race responsibilities:
//   - dispatch/registry: Handler registration and dispatch
//   - dispatcher/trait.rs: RacHandler trait
//   - dispatcher/router.rs: dispatch() functions

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};

// Re-export from wire module
use crate::wire::{
    SioType, SioEnvelope, SioRacExt, RacProtocol, RacWire, RacWireConfig,
    RacChannelFactory, RacChannelTrait,
};
use crate::dispatcher_bridge::{
    DispatchPlan, DispatcherBridge, DispatcherBridgeError,
    select_protocol, resolve_handler,
};
use crate::racbox_integration::{RacboxIntegration, RacboxLeaseHandle};
use crate::sio_drive::{SioDrive, SioDriveConfig, SioId};

/// RACE integration configuration
#[derive(Debug, Clone)]
pub struct RaceIntegrationConfig {
    /// Enable racbox integration
    pub enable_racbox: bool,
    /// Enable SIO drive persistence
    pub enable_sio_drive: bool,
    /// Enable dispatcher bridge
    pub enable_dispatcher_bridge: bool,
    /// Default protocol
    pub default_protocol: RacProtocol,
    /// racbox mount prefix
    pub racbox_mount_prefix: String,
    /// SIO drive config
    pub sio_drive_config: SioDriveConfig,
}

impl Default for RaceIntegrationConfig {
    fn default() -> Self {
        Self {
            enable_racbox: true,
            enable_sio_drive: true,
            enable_dispatcher_bridge: true,
            default_protocol: RacProtocol::Raci,
            racbox_mount_prefix: "/tmp/racbox".to_string(),
            sio_drive_config: SioDriveConfig::default(),
        }
    }
}

/// RACE integration state
pub struct RaceIntegration {
    /// Configuration
    config: RaceIntegrationConfig,
    /// RAC wire (DNA + hooks)
    wire: RacWire,
    /// Dispatcher bridge
    bridge: DispatcherBridge,
    /// Racbox integration (optional)
    racbox: Option<RacboxIntegration>,
    /// SIO drive (optional)
    sio_drive: Option<SioDrive>,
    /// Active dispatch plans
    active_plans: alloc::collections::BTreeMap<u128, DispatchPlan>,
}

impl RaceIntegration {
    /// Create new integration
    pub fn new(config: RaceIntegrationConfig) -> Self {
        let wire = RacWire::new(RacWireConfig {
            enable_dna: true,
            enable_hooks: true,
            enable_execute_plane: true,
            default_protocol: config.default_protocol,
        });
        
        let bridge = DispatcherBridge::new();
        
        let racbox = if config.enable_racbox {
            Some(RacboxIntegration::new(config.racbox_mount_prefix.clone()))
        } else {
            None
        };
        
        let sio_drive = if config.enable_sio_drive {
            Some(SioDrive::new(config.sio_drive_config.clone()))
        } else {
            None
        };
        
        Self {
            config,
            wire,
            bridge,
            racbox,
            sio_drive,
            active_plans: alloc::collections::BTreeMap::new(),
        }
    }
    
    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(RaceIntegrationConfig::default())
    }
    
    /// Initialize hooks
    pub fn init(&mut self) {
        self.wire.init_hooks(self.config.default_protocol);
    }
    
    /// Process incoming SIO envelope
    pub fn process_envelope(&mut self, envelope: SioEnvelope) -> Result<DispatchPlan, RaceIntegrationError> {
        // Pre-hook validation
        let protocol = envelope.protocol();
        let sio_type = envelope.sio_type();
        
        self.wire.pre_execute(protocol, sio_type)
            .map_err(|e| RaceIntegrationError::HookError(e.to_string()))?;
        
        // Route to dispatcher bridge
        let plan = self.bridge.route(&envelope)
            .map_err(|e| RaceIntegrationError::BridgeError(e.to_string()))?;
        
        // Store plan
        self.active_plans.insert(plan.intent_id, plan.clone());
        
        // Persist to SIO drive if enabled
        if let Some(ref mut drive) = self.sio_drive {
            drive.store(envelope).map_err(|e| RaceIntegrationError::SioDriveError(e.to_string()))?;
        }
        
        Ok(plan)
    }
    
    /// Get dispatch plan by intent ID
    pub fn get_plan(&self, intent_id: u128) -> Option<&DispatchPlan> {
        self.active_plans.get(&intent_id)
    }
    
    /// Remove completed plan
    pub fn remove_plan(&mut self, intent_id: u128) -> Option<DispatchPlan> {
        self.active_plans.remove(&intent_id)
    }
    
    /// Get recommended protocol for envelope
    pub fn recommended_protocol(&self, envelope: &SioEnvelope) -> RacProtocol {
        self.bridge.recommended_protocol(envelope.intent_id(), envelope.sio_type())
    }
    
    /// Create channel for protocol
    pub fn create_channel(&self, protocol: RacProtocol) -> Result<Box<dyn RacChannelTrait>, RaceIntegrationError> {
        RacChannelFactory::create_channel(protocol)
            .map_err(|e| RaceIntegrationError::ChannelError(e.to_string()))
    }
    
    /// Get racbox integration
    pub fn racbox(&self) -> Option<&RacboxIntegration> {
        self.racbox.as_ref()
    }
    
    /// Get SIO drive
    pub fn sio_drive(&self) -> Option<&SioDrive> {
        self.sio_drive.as_ref()
    }
    
    /// Get wire
    pub fn wire(&self) -> &RacWire {
        &self.wire
    }
    
    /// Get active plan count
    pub fn active_plan_count(&self) -> usize {
        self.active_plans.len()
    }
}

/// RACE integration error
#[derive(Debug, Clone)]
pub enum RaceIntegrationError {
    HookError(String),
    BridgeError(String),
    SioDriveError(String),
    ChannelError(String),
    RacboxError(String),
}

impl core::fmt::Debug for RaceIntegrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::HookError(s) => write!(f, "HookError({})", s),
            Self::BridgeError(s) => write!(f, "BridgeError({})", s),
            Self::SioDriveError(s) => write!(f, "SioDriveError({})", s),
            Self::ChannelError(s) => write!(f, "ChannelError({})", s),
            Self::RacboxError(s) => write!(f, "RacboxError({})", s),
        }
    }
}

/// Async dispatch context for me/race integration
pub struct RaceDispatchContext {
    /// Intent ID
    pub intent_id: u128,
    /// Protocol
    pub protocol: RacProtocol,
    /// SIO type
    pub sio_type: SioType,
    /// Handler name
    pub handler: Option<String>,
    /// Lease handle (if racbox)
    pub lease: Option<RacboxLeaseHandle>,
}

impl RaceDispatchContext {
    /// Create new context
    pub fn new(intent_id: u128, protocol: RacProtocol, sio_type: SioType) -> Self {
        Self {
            intent_id,
            protocol,
            sio_type,
            handler: None,
            lease: None,
        }
    }
    
    /// With handler
    pub fn with_handler(mut self, handler: &'static str) -> Self {
        self.handler = Some(handler.to_string());
        self
    }
    
    /// With lease
    pub fn with_lease(mut self, lease: RacboxLeaseHandle) -> Self {
        self.lease = Some(lease);
        self
    }
}

/// Convert dispatch plan to dispatch context
pub fn plan_to_dispatch_context(plan: &DispatchPlan) -> RaceDispatchContext {
    let mut ctx = RaceDispatchContext::new(plan.intent_id, plan.protocol, plan.sio_type);
    if let Some(ref handler) = plan.handler {
        ctx = ctx.with_handler(handler);
    }
    ctx
}

// ─── me/race Dispatcher Trait Implementation ─────────────────────────────────

/*
/// Implement RacHandler for cona RAC adapters
/// This allows cona RAC to be used as a handler in me/race dispatcher

use hace_infra_io_rac_gate::schema::{RealityContract, Intent};

pub struct RacHandlerAdapter {
    integration: RaceIntegration,
}

impl RacHandlerAdapter {
    pub fn new(integration: RaceIntegration) -> Self {
        Self { integration }
    }
}

#[async_trait::async_trait]
impl RacHandler for RacHandlerAdapter {
    fn intent(&self) -> u128 {
        Intent::OminaUpdate.id()
    }
    
    fn name(&self) -> &'static str {
        "rac-handler"
    }
    
    async fn handle(&self, ctx: &DispatchContext, rac: &RealityContract) -> Result<(), RacDispatchError> {
        // Convert RealityContract to SioEnvelope
        let envelope = SioEnvelope::from_reality_contract(rac)?;
        
        // Process through integration
        let plan = self.integration.process_envelope(envelope)
            .map_err(|e| RacDispatchError::Internal(e.to_string()))?;
        
        // Execute dispatch
        Ok(())
    }
}
*/

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_integration_creation() {
        let integration = RaceIntegration::default_config();
        assert!(integration.racbox().is_some());
        assert!(integration.sio_drive().is_some());
    }
    
    #[test]
    fn test_dispatch_context() {
        let ctx = RaceDispatchContext::new(100, RacProtocol::Raci, SioType::Intent)
            .with_handler("test-handler");
        assert_eq!(ctx.intent_id, 100);
        assert_eq!(ctx.handler.unwrap(), "test-handler");
    }
    
    #[test]
    fn test_plan_to_context() {
        let plan = DispatchPlan::new(200, RacProtocol::Racex, SioType::Reasoning)
            .with_handler("reasoning-handler");
        let ctx = plan_to_dispatch_context(&plan);
        assert_eq!(ctx.intent_id, 200);
        assert_eq!(ctx.protocol, RacProtocol::Racex);
    }
}