// cona/io/rac/racin.rs
//
// RACIN Adapter - Same-Process Communication
//
// Era 5 Architecture:
//   RACIN provides same-process inter-thread/async communication using:
//   - FFI (Foreign Function Interface)
//   - Shared Memory (via shared memory primitives)
//
// Security Invariant:
//   Remote Alliag only produces Intent/Reasoning, NEVER Execute.
//   Local Alliag is the ONLY entity allowed to compile Execute.
//
// RACIN is optimized for low-latency intra-process communication.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicU32, Ordering};

// Re-export from allo_router for RAC protocol types
#[cfg(feature = "allo_router")]
use crate::allo_router::{
    RacProtocol, RacRouteDecision, EntityClassification, RoutePriority,
    RetryPolicy, ChannelState, ChannelSecurity, AuthMethod,
};

#[cfg(not(feature = "allo_router"))]
use crate::{
    RacProtocol, RacRouteDecision, EntityClassification, RoutePriority,
    RetryPolicy, ChannelState, ChannelSecurity, AuthMethod,
};

// ─── RACIN Channel ───────────────────────────────────────────────────────────

/// RACIN Channel for same-process communication
pub struct RacinChannel {
    /// Channel identifier
    channel_id: u32,
    /// Connection state
    state: ChannelState,
    /// Sequence number for messages
    sequence: AtomicU32,
    /// Shared memory region (if using shared memory transport)
    shared_memory: Option<SharedMemoryRegion>,
}

impl RacinChannel {
    /// Create a new RACIN channel
    pub fn new() -> Self {
        Self {
            channel_id: Self::generate_channel_id(),
            state: ChannelState::Idle,
            sequence: AtomicU32::new(0),
            shared_memory: None,
        }
    }
    
    /// Create with shared memory region
    pub fn with_shared_memory(region: SharedMemoryRegion) -> Self {
        Self {
            channel_id: Self::generate_channel_id(),
            state: ChannelState::Idle,
            sequence: AtomicU32::new(0),
            shared_memory: Some(region),
        }
    }
    
    /// Get the channel ID
    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }
    
    /// Get the current state
    pub fn state(&self) -> ChannelState {
        self.state
    }
    
    /// Get the protocol type
    pub fn protocol(&self) -> RacProtocol {
        RacProtocol::Racin
    }
    
    /// Generate unique channel ID
    fn generate_channel_id() -> u32 {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }
    
    /// Get next sequence number
    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for RacinChannel {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Shared Memory Region ────────────────────────────────────────────────────

/// Shared memory region for RACIN
pub struct SharedMemoryRegion {
    /// Region identifier
    id: u32,
    /// Memory address (if mapped)
    address: *mut u8,
    /// Size of the region
    size: usize,
    /// Reference count
    ref_count: AtomicU32,
}

impl SharedMemoryRegion {
    /// Create a new shared memory region
    pub fn new(size: usize) -> Result<Self, RacinError> {
        // Platform-specific shared memory allocation
        #[cfg(feature = "std")]
        {
            // Use std::alloc for now - in production would use shared memory primitives
            let layout = core::alloc::Layout::from_size_align(size, 4096)
                .map_err(|_| RacinError::AllocationFailed)?;
            
            // Safety: layout is valid and size > 0
            let address = unsafe { std::alloc::alloc(layout) };
            
            if address.is_null() {
                return Err(RacinError::AllocationFailed);
            }
            
            Ok(Self {
                id: Self::generate_id(),
                address,
                size,
                ref_count: AtomicU32::new(1),
            })
        }
        
        #[cfg(not(feature = "std"))]
        {
            Err(RacinError::NoStdNotSupported)
        }
    }
    
    /// Create a shared memory region from an existing address
    pub fn from_address(address: *mut u8, size: usize) -> Self {
        Self {
            id: Self::generate_id(),
            address,
            size,
            ref_count: AtomicU32::new(1),
        }
    }
    
    /// Get the memory address
    pub fn address(&self) -> *mut u8 {
        self.address
    }
    
    /// Get the size
    pub fn size(&self) -> usize {
        self.size
    }
    
    /// Increment reference count
    pub fn retain(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }
    
    /// Decrement reference count and deallocate if zero
    pub fn release(&mut self) {
        if self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.deallocate();
        }
    }
    
    /// Deallocate the memory
    fn deallocate(&mut self) {
        #[cfg(feature = "std")]
        {
            let layout = core::alloc::Layout::from_size_align(self.size, 4096)
                .expect("Invalid layout");
            // Safety: address was allocated with this layout
            unsafe { std::alloc::dealloc(self.address, layout) };
        }
        self.address = core::ptr::null_mut();
    }
    
    /// Generate unique region ID
    fn generate_id() -> u32 {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }
}

impl Drop for SharedMemoryRegion {
    fn drop(&mut self) {
        self.release();
    }
}

// ─── RACIN Message ───────────────────────────────────────────────────────────

/// RACIN message envelope (zero-copy capable)
#[derive(Debug, Clone)]
pub struct RacinMessage<'a> {
    /// Message sequence number
    pub sequence: u32,
    /// Message type
    pub msg_type: RacinMsgType,
    /// Payload reference (zero-copy)
    pub payload: &'a [u8],
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RacinMsgType {
    /// Intent SIO
    Intent,
    /// Reasoning SIO
    Reasoning,
    /// Execute SIO
    Execute,
    /// Legal SIO
    Legal,
    /// Finance SIO
    Finance,
    /// Memory SIO
    Memory,
    /// Data SIO
    Data,
    /// Vision SIO
    Vision,
    /// Control message (ping/pong/close)
    Control,
}

impl<'a> RacinMessage<'a> {
    /// Create a new message
    pub fn new(msg_type: RacinMsgType, payload: &'a [u8]) -> Self {
        Self {
            sequence: 0, // Will be set by channel
            msg_type,
            payload,
            timestamp_ns: Self::current_timestamp_ns(),
        }
    }
    
    /// Get current timestamp in nanoseconds
    fn current_timestamp_ns() -> u64 {
        #[cfg(feature = "std")]
        {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        }
        #[cfg(not(feature = "std"))]
        {
            0
        }
    }
}

// ─── FFI Bridge ─────────────────────────────────────────────────────────────

/// FFI bridge for calling into RACIN from other languages/libraries
pub struct FfiBridge {
    /// Bridge identifier
    id: u32,
}

impl FfiBridge {
    /// Create a new FFI bridge
    pub fn new() -> Self {
        Self {
            id: Self::generate_id(),
        }
    }
    
    /// Generate unique bridge ID
    fn generate_id() -> u32 {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for FfiBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Error Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RacinError {
    /// Channel is not connected
    NotConnected,
    /// Connection failed
    ConnectionFailed { reason: String },
    /// Send failed
    SendFailed { reason: String },
    /// Receive failed
    RecvFailed { reason: String },
    /// Shared memory allocation failed
    AllocationFailed,
    /// Shared memory not supported in no_std
    NoStdNotSupported,
    /// Timeout
    Timeout,
    /// Buffer too small
    BufferTooSmall,
    /// Unknown error
    Unknown { code: i32 },
}

impl RacinError {
    pub fn as_str(&self) -> &'static str {
        match self {
            RacinError::NotConnected => "not_connected",
            RacinError::ConnectionFailed { .. } => "connection_failed",
            RacinError::SendFailed { .. } => "send_failed",
            RacinError::RecvFailed { .. } => "recv_failed",
            RacinError::AllocationFailed => "allocation_failed",
            RacinError::NoStdNotSupported => "no_std_not_supported",
            RacinError::Timeout => "timeout",
            RacinError::BufferTooSmall => "buffer_too_small",
            RacinError::Unknown { .. } => "unknown",
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_racin_channel() {
        let channel = RacinChannel::new();
        assert_eq!(channel.state(), ChannelState::Idle);
        assert_eq!(channel.protocol(), RacProtocol::Racin);
    }
    
    #[test]
    fn test_shared_memory_region() {
        #[cfg(feature = "std")]
        {
            let region = SharedMemoryRegion::new(4096).unwrap();
            assert!(!region.address().is_null());
            assert_eq!(region.size(), 4096);
        }
    }
    
    #[test]
    fn test_racin_message() {
        let data = [1u8, 2, 3, 4];
        let msg = RacinMessage::new(RacinMsgType::Intent, &data);
        assert_eq!(msg.msg_type, RacinMsgType::Intent);
        assert_eq!(msg.payload, &[1, 2, 3, 4]);
    }
}