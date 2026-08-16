// cona/io/rac/racbox_integration.rs
//
// Racbox Integration: Connect hydas/racbox to cona RAC layer
//
// Era 5 Architecture:
//   Integrates RacboxActor from hydas/racbox with cona RAC adapters.
//   Provides lease-based SIO persistence and URI resolution.
//
// RacboxActor methods:
//   - mount(actor) -> mount_path
//   - request_lease(actor, quota) -> lease_id
//   - commit_flush(lease, data) -> sio_id
//   - release_lease(lease) -> ()
//   - resolve_uri(uri) -> sio_id

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};

// Re-export racbox types
pub use hydas_racbox::{
    RacboxActor, RacboxResult, RacboxError,
    LeaseId, SioId, ActorId,
};

/// Racbox integration state
pub struct RacboxIntegration {
    /// Mount path prefix
    mount_prefix: String,
    /// Active leases
    active_leases: Vec<LeaseId>,
    /// URI scheme
    uri_scheme: &'static str,
}

impl Default for RacboxIntegration {
    fn default() -> Self {
        Self::new("/tmp/racbox".to_string())
    }
}

impl RacboxIntegration {
    /// Create new integration
    pub fn new(mount_prefix: String) -> Self {
        Self {
            mount_prefix,
            active_leases: Vec::new(),
            uri_scheme: "racbox",
        }
    }
    
    /// Get mount prefix
    pub fn mount_prefix(&self) -> &str {
        &self.mount_prefix
    }
    
    /// Get URI scheme
    pub fn uri_scheme(&self) -> &'static str {
        self.uri_scheme
    }
    
    /// Check if URI is racbox scheme
    pub fn is_racbox_uri(uri: &str) -> bool {
        uri.starts_with("racbox://")
    }
    
    /// Parse racbox URI to actor ID
    pub fn parse_uri(uri: &str) -> Result<ActorId, RacboxUriError> {
        if !Self::is_racbox_uri(uri) {
            return Err(RacboxUriError::InvalidScheme(uri.to_string()));
        }
        
        // Format: racbox://actor_id
        let path = uri.trim_start_matches("racbox://");
        if path.is_empty() {
            return Err(RacboxUriError::EmptyActorId);
        }
        
        Ok(ActorId(path.to_string()))
    }
}

/// URI parsing error
#[derive(Debug, Clone)]
pub enum RacboxUriError {
    InvalidScheme(String),
    EmptyActorId,
    InvalidFormat,
}

impl core::fmt::Debug for RacboxUriError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidScheme(s) => write!(f, "InvalidScheme({})", s),
            Self::EmptyActorId => write!(f, "EmptyActorId"),
            Self::InvalidFormat => write!(f, "InvalidFormat"),
        }
    }
}

/// Racbox lease handle
pub struct RacboxLeaseHandle {
    lease_id: LeaseId,
    quota: u64,
    used: u64,
}

impl RacboxLeaseHandle {
    /// Get remaining quota
    pub fn remaining(&self) -> u64 {
        self.quota.saturating_sub(self.used)
    }
    
    /// Check if quota exhausted
    pub fn is_exhausted(&self) -> bool {
        self.remaining() == 0
    }
    
    /// Get lease ID
    pub fn lease_id(&self) -> &LeaseId {
        &self.lease_id
    }
}

/// Racbox SIO handle
pub struct RacboxSioHandle {
    sio_id: SioId,
    mount_path: String,
}

impl RacboxSioHandle {
    /// Get SIO ID
    pub fn sio_id(&self) -> &SioId {
        &self.sio_id
    }
    
    /// Get mount path
    pub fn mount_path(&self) -> &str {
        &self.mount_path
    }
}

/// Racbox integration error
#[derive(Debug, Clone)]
pub enum RacboxIntegrationError {
    LeaseNotFound(LeaseId),
    QuotaExceeded,
    MountFailed(String),
    ResolveFailed(String),
}

impl core::fmt::Debug for RacboxIntegrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LeaseNotFound(id) => write!(f, "LeaseNotFound({:?})", id),
            Self::QuotaExceeded => write!(f, "QuotaExceeded"),
            Self::MountFailed(s) => write!(f, "MountFailed({})", s),
            Self::ResolveFailed(s) => write!(f, "ResolveFailed({})", s),
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_uri_parsing() {
        let uri = "racbox://actor-123";
        let actor_id = RacboxIntegration::parse_uri(uri).unwrap();
        assert_eq!(actor_id.0, "actor-123");
    }
    
    #[test]
    fn test_invalid_scheme() {
        let uri = "http://example.com";
        let result = RacboxIntegration::parse_uri(uri);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_lease_handle() {
        let handle = RacboxLeaseHandle {
            lease_id: LeaseId("test-lease".to_string()),
            quota: 1000,
            used: 300,
        };
        assert_eq!(handle.remaining(), 700);
        assert!(!handle.is_exhausted());
    }
}