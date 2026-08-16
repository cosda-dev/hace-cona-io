// cona/io/rac/wire.rs
//
// Wire: Connect cona/io/rac to rr-infra-io-rac-adapter
//
// Era 5 Architecture:
//   This module wires cona RAC adapters (allo_router, racid, racin, racex)
//   to the rr-infra-io-rac-adapter DNA trait system.
//
// Integration:
//   - cona/io/rac/* → rr-infra-io-rac-adapter (DNA trait)
//   - Hook system: rr-infra-hooks
//   - Execute plane: rr-infra-dna

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};

// Re-export from rr-infra-io-rac-adapter
pub use rr_infra_io_rac_adapter::{
    RacProtocol, ChannelState, ChannelSecurity, AuthMethod,
    RetryPolicy, SioType, SioEnvelope, SioRacExt,
    RacPayload, RacOutput, RacResult, RacAdapterError,
    RacAdapterDNA, RacPreExecuteHook, RacPostExecuteHook,
    RacHookContext, RacHookRegistry, RacHookError,
    RoyalPartnerDNA, Payload, Output, DnaError,
    Aid, AuthorityLevel, AstcProfile, IoSchema,
    IpoDescriptor, PegDescriptor, HookPoint,
};

// Re-export from local modules
pub use crate::allo_router::{
    RacRouteDecision, EntityClassification, RoutePriority,
    AlliagCognitiveRouter, CompilerError,
};

pub use crate::racid::{RacidChannel, RacidServer, RacidClient, RacidMessage, RacidMsgType, RacidError};
pub use crate::racin::{RacinChannel, RacinMessage, RacinMsgType, RacinError, SharedMemoryRegion, FfiBridge};
pub use crate::racex::{RacexChannel, RacexClient, RacexServer, RacexMessage, RacexMsgType, RacexError, RacexEndpoint, RemoteTransport};

// ─── Wire Configuration ─────────────────────────────────────────────────────

/// Wire configuration for connecting cona RAC to rr-infra
#[derive(Debug, Clone)]
pub struct RacWireConfig {
    /// Enable DNA trait implementation
    pub enable_dna: bool,
    /// Enable hook integration
    pub enable_hooks: bool,
    /// Enable execute plane integration
    pub enable_execute_plane: bool,
    /// Default protocol
    pub default_protocol: RacProtocol,
}

impl Default for RacWireConfig {
    fn default() -> Self {
        Self {
            enable_dna: true,
            enable_hooks: true,
            enable_execute_plane: true,
            default_protocol: RacProtocol::Raci,
        }
    }
}

/// Wire state
pub struct RacWire {
    config: RacWireConfig,
    router: AlliagCognitiveRouter,
    hook_context: Option<RacHookContext>,
}

impl RacWire {
    /// Create a new wire
    pub fn new(config: RacWireConfig) -> Self {
        Self {
            config,
            router: AlliagCognitiveRouter::new(),
            hook_context: None,
        }
    }
    
    /// Create with default config
    pub fn default_config() -> Self {
        Self::new(RacWireConfig::default())
    }
    
    /// Initialize hook context
    pub fn init_hooks(&mut self, protocol: RacProtocol) {
        if self.config.enable_hooks {
            self.hook_context = Some(RacHookContext::new(protocol));
        }
    }
    
    /// Execute pre-hook validation
    pub fn pre_execute(&self, protocol: RacProtocol, sio_type: SioType) -> Result<(), RacHookError> {
        if self.config.enable_hooks {
            RacPreExecuteHook::validate(protocol, sio_type)
        } else {
            Ok(())
        }
    }
    
    /// Execute post-hook processing
    pub fn post_execute(&self, protocol: RacProtocol, result: &mut [u8]) -> Result<(), RacHookError> {
        if self.config.enable_hooks {
            RacPostExecuteHook::process(protocol, result)
        } else {
            Ok(())
        }
    }
    
    /// Get the router
    pub fn router(&self) -> &AlliagCognitiveRouter {
        &self.router
    }
    
    /// Get DNA for a specific protocol
    pub fn get_dna(&self, protocol: RacProtocol) -> RacAdapterDNA {
        RacAdapterDNA::new(Aid::new(1), protocol)
    }
}

// ─── Protocol Channel Factory ────────────────────────────────────────────────

/// Factory for creating protocol-specific channels
pub struct RacChannelFactory;

impl RacChannelFactory {
    /// Create a channel for the given protocol
    pub fn create_channel(protocol: RacProtocol) -> Result<Box<dyn RacChannelTrait>, RacChannelError> {
        match protocol {
            RacProtocol::Raci => {
                // Local Alliag - no channel needed, direct call
                Err(RacChannelError::NotApplicable("RACI uses direct call".to_string()))
            },
            RacProtocol::Racin => {
                let channel = RacinChannel::new();
                Ok(Box::new(channel))
            },
            RacProtocol::Racid => {
                let path = crate::racid::paths::default_path();
                let channel = RacidChannel::new(&path);
                Ok(Box::new(channel))
            },
            RacProtocol::Racex => {
                let endpoint = RacexEndpoint::new("https://localhost:8080", RemoteTransport::Http);
                let channel = RacexChannel::new(endpoint);
                Ok(Box::new(channel))
            },
            RacProtocol::Racas | RacProtocol::Racag => {
                // Human/Remote - handled by Alliha/RACAG
                Err(RacChannelError::NotApplicable("Use AlliagCognitiveRouter".to_string()))
            },
        }
    }
}

/// Channel trait for polymorphic channel handling
pub trait RacChannelTrait {
    fn protocol(&self) -> RacProtocol;
    fn state(&self) -> ChannelState;
    fn send(&mut self, data: &[u8]) -> Result<(), RacChannelError>;
    fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, RacChannelError>;
}

/// Channel error
#[derive(Debug, Clone)]
pub enum RacChannelError {
    NotApplicable(String),
    NotConnected,
    SendFailed(String),
    RecvFailed(String),
}

impl core::fmt::Debug for RacChannelError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotApplicable(s) => write!(f, "NotApplicable({})", s),
            Self::NotConnected => write!(f, "NotConnected"),
            Self::SendFailed(s) => write!(f, "SendFailed({})", s),
            Self::RecvFailed(s) => write!(f, "RecvFailed({})", s),
        }
    }
}

impl RacChannelTrait for RacinChannel {
    fn protocol(&self) -> RacProtocol {
        RacProtocol::Racin
    }
    
    fn state(&self) -> ChannelState {
        self.state()
    }
    
    fn send(&mut self, _data: &[u8]) -> Result<(), RacChannelError> {
        // RACIN implementation
        Ok(())
    }
    
    fn recv(&mut self, _buffer: &mut [u8]) -> Result<usize, RacChannelError> {
        // RACIN implementation
        Ok(0)
    }
}

impl RacChannelTrait for RacidChannel {
    fn protocol(&self) -> RacProtocol {
        RacProtocol::Racid
    }
    
    fn state(&self) -> ChannelState {
        self.state()
    }
    
    fn send(&mut self, _data: &[u8]) -> Result<(), RacChannelError> {
        // RACID implementation
        Ok(())
    }
    
    fn recv(&mut self, _buffer: &mut [u8]) -> Result<usize, RacChannelError> {
        // RACID implementation
        Ok(0)
    }
}

impl RacChannelTrait for RacexChannel {
    fn protocol(&self) -> RacProtocol {
        RacProtocol::Racex
    }
    
    fn state(&self) -> ChannelState {
        self.state()
    }
    
    fn send(&mut self, _data: &[u8]) -> Result<(), RacChannelError> {
        // RACEX implementation
        Ok(())
    }
    
    fn recv(&mut self, _buffer: &mut [u8]) -> Result<usize, RacChannelError> {
        // RACEX implementation
        Ok(0)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wire_config() {
        let config = RacWireConfig::default();
        assert!(config.enable_dna);
        assert!(config.enable_hooks);
        assert_eq!(config.default_protocol, RacProtocol::Raci);
    }
    
    #[test]
    fn test_wire_creation() {
        let wire = RacWire::default_config();
        assert!(wire.router().router_id() == 0); // Default router has id 0
    }
    
    #[test]
    fn test_dna_creation() {
        let wire = RacWire::default_config();
        let dna = wire.get_dna(RacProtocol::Raci);
        assert_eq!(dna.dna_id(), "rr.rac.raci.v1");
    }
    
    #[test]
    fn test_hook_validation() {
        let wire = RacWire::default_config();
        
        // Valid: Remote Alliag produces Intent
        assert!(wire.pre_execute(RacProtocol::Racex, SioType::Intent).is_ok());
        
        // Invalid: Remote Alliag produces Execute
        assert!(wire.pre_execute(RacProtocol::Racex, SioType::Execute).is_err());
    }
}