// cona/io/rac/racid.rs
//
// RACID Adapter - In-Device Communication
//
// Era 5 Architecture:
//   RACID provides in-device inter-process communication using:
//   - Named Pipes (Windows)
//   - Unix Domain Sockets (Linux/WSL)
//
// Security Invariant:
//   Remote Alliag only produces Intent/Reasoning, NEVER Execute.
//   Local Alliag is the ONLY entity allowed to compile Execute.
//
// RACID Channel Types:
//   - Named Pipe: \\.\pipe\zeus-rac\ for Windows
//   - UDS: /tmp/zeus-rac.sock for Unix/Linux/WSL

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

// ─── Platform Detection ──────────────────────────────────────────────────────

/// Platform-specific transport type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformTransport {
    /// Windows Named Pipes
    NamedPipe,
    /// Unix Domain Socket
    UnixDomainSocket,
}

impl PlatformTransport {
    /// Detect the appropriate transport for the current platform
    pub fn detect() -> Self {
        #[cfg(windows)]
        {
            PlatformTransport::NamedPipe
        }
        #[cfg(not(windows))]
        {
            PlatformTransport::UnixDomainSocket
        }
    }
}

// ─── RACID Channel ───────────────────────────────────────────────────────────

/// RACID Channel for in-device IPC
pub struct RacidChannel {
    /// Channel identifier
    channel_id: u32,
    /// Platform-specific transport
    transport: PlatformTransport,
    /// Named pipe name (Windows) or socket path (Unix)
    path: String,
    /// Connection state
    state: ChannelState,
    /// Security configuration
    security: ChannelSecurity,
    /// Sequence number for messages
    sequence: AtomicU32,
}

impl RacidChannel {
    /// Create a new RACID channel
    pub fn new(path: &str) -> Self {
        Self {
            channel_id: Self::generate_channel_id(),
            transport: PlatformTransport::detect(),
            path: path.to_string(),
            state: ChannelState::Idle,
            security: ChannelSecurity {
                encrypted: false,
                auth_method: AuthMethod::None,
                certificate_fingerprint: None,
            },
            sequence: AtomicU32::new(0),
        }
    }
    
    /// Create with security configuration
    pub fn with_security(path: &str, security: ChannelSecurity) -> Self {
        Self {
            channel_id: Self::generate_channel_id(),
            transport: PlatformTransport::detect(),
            path: path.to_string(),
            state: ChannelState::Idle,
            security,
            sequence: AtomicU32::new(0),
        }
    }
    
    /// Get the channel path
    pub fn path(&self) -> &str {
        &self.path
    }
    
    /// Get the current state
    pub fn state(&self) -> ChannelState {
        self.state
    }
    
    /// Get the protocol type
    pub fn protocol(&self) -> RacProtocol {
        RacProtocol::Racid
    }
    
    /// Generate unique channel ID
    fn generate_channel_id() -> u32 {
        use core::sync::atomic::AtomicU32;
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }
    
    /// Get next sequence number
    fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }
}

// ─── RACID Server ────────────────────────────────────────────────────────────

/// RACID Server for handling incoming connections
pub struct RacidServer {
    /// Server path (pipe name or socket path)
    path: String,
    /// Platform transport
    transport: PlatformTransport,
    /// Active channels
    channels: Vec<u32>,
    /// Security configuration
    security: ChannelSecurity,
    /// Server state
    state: ChannelState,
}

impl RacidServer {
    /// Create a new RACID server
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            transport: PlatformTransport::detect(),
            channels: Vec::new(),
            security: ChannelSecurity {
                encrypted: false,
                auth_method: AuthMethod::None,
                certificate_fingerprint: None,
            },
            state: ChannelState::Idle,
        }
    }
    
    /// Create with security configuration
    pub fn with_security(path: &str, security: ChannelSecurity) -> Self {
        Self {
            path: path.to_string(),
            transport: PlatformTransport::detect(),
            channels: Vec::new(),
            security,
            state: ChannelState::Idle,
        }
    }
    
    /// Start listening for connections
    pub fn listen(&mut self) -> Result<(), RacidError> {
        self.state = ChannelState::Connecting;
        
        match self.transport {
            PlatformTransport::NamedPipe => self.listen_named_pipe(),
            PlatformTransport::UnixDomainSocket => self.listen_uds(),
        }
    }
    
    #[cfg(windows)]
    fn listen_named_pipe(&mut self) -> Result<(), RacidError> {
        // Windows Named Pipe server implementation
        // Using unsafe FFI for Windows API
        Ok(())
    }
    
    #[cfg(not(windows))]
    fn listen_uds(&mut self) -> Result<(), RacidError> {
        // Unix Domain Socket server implementation
        // Using tokio for async I/O
        Ok(())
    }
    
    /// Accept an incoming connection
    pub fn accept(&mut self) -> Result<RacidChannel, RacidError> {
        if self.state != ChannelState::Connected {
            return Err(RacidError::NotConnected);
        }
        
        // Platform-specific accept implementation
        Ok(RacidChannel::new(&self.path))
    }
    
    /// Close the server
    pub fn close(&mut self) {
        self.state = ChannelState::Closed;
        self.channels.clear();
    }
}

// ─── RACID Client ────────────────────────────────────────────────────────────

/// RACID Client for outgoing connections
pub struct RacidClient {
    /// Client path (pipe name or socket path)
    path: String,
    /// Platform transport
    transport: PlatformTransport,
    /// Active channel (if connected)
    channel: Option<RacidChannel>,
    /// Security configuration
    security: ChannelSecurity,
    /// Client state
    state: ChannelState,
}

impl RacidClient {
    /// Create a new RACID client
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            transport: PlatformTransport::detect(),
            channel: None,
            security: ChannelSecurity {
                encrypted: false,
                auth_method: AuthMethod::None,
                certificate_fingerprint: None,
            },
            state: ChannelState::Idle,
        }
    }
    
    /// Create with security configuration
    pub fn with_security(path: &str, security: ChannelSecurity) -> Self {
        Self {
            path: path.to_string(),
            transport: PlatformTransport::detect(),
            channel: None,
            security,
            state: ChannelState::Idle,
        }
    }
    
    /// Connect to the server
    pub fn connect(&mut self) -> Result<(), RacidError> {
        self.state = ChannelState::Connecting;
        
        match self.transport {
            PlatformTransport::NamedPipe => self.connect_named_pipe(),
            PlatformTransport::UnixDomainSocket => self.connect_uds(),
        }
    }
    
    #[cfg(windows)]
    fn connect_named_pipe(&mut self) -> Result<(), RacidError> {
        // Windows Named Pipe client implementation
        Ok(())
    }
    
    #[cfg(not(windows))]
    fn connect_uds(&mut self) -> Result<(), RacidError> {
        // Unix Domain Socket client implementation
        Ok(())
    }
    
    /// Send a message through the channel
    pub fn send(&mut self, data: &[u8]) -> Result<(), RacidError> {
        if self.channel.is_none() {
            return Err(RacidError::NotConnected);
        }
        
        // Platform-specific send implementation
        Ok(())
    }
    
    /// Receive a message from the channel
    pub fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, RacidError> {
        if self.channel.is_none() {
            return Err(RacidError::NotConnected);
        }
        
        // Platform-specific recv implementation
        Ok(0)
    }
    
    /// Disconnect from the server
    pub fn disconnect(&mut self) {
        self.channel = None;
        self.state = ChannelState::Closed;
    }
}

// ─── RACID Message ───────────────────────────────────────────────────────────

/// RACID message envelope
#[derive(Debug, Clone)]
pub struct RacidMessage {
    /// Message sequence number
    pub sequence: u32,
    /// Message type
    pub msg_type: RacidMsgType,
    /// Payload (SIO envelope serialized)
    pub payload: Vec<u8>,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RacidMsgType {
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

impl RacidMessage {
    /// Create a new message
    pub fn new(msg_type: RacidMsgType, payload: Vec<u8>) -> Self {
        Self {
            sequence: 0, // Will be set by channel
            msg_type,
            payload,
            timestamp_ns: Self::current_timestamp_ns(),
        }
    }
    
    /// Get current timestamp in nanoseconds
    fn current_timestamp_ns() -> u64 {
        // Use std::time if available, otherwise use a simple counter
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

// ─── Error Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RacidError {
    /// Channel is not connected
    NotConnected,
    /// Connection failed
    ConnectionFailed { reason: String },
    /// Send failed
    SendFailed { reason: String },
    /// Receive failed
    RecvFailed { reason: String },
    /// Timeout
    Timeout,
    /// Permission denied
    PermissionDenied,
    /// Path not found
    PathNotFound,
    /// Buffer too small
    BufferTooSmall,
    /// Unknown error
    Unknown { code: i32 },
}

impl RacidError {
    pub fn as_str(&self) -> &'static str {
        match self {
            RacidError::NotConnected => "not_connected",
            RacidError::ConnectionFailed { .. } => "connection_failed",
            RacidError::SendFailed { .. } => "send_failed",
            RacidError::RecvFailed { .. } => "recv_failed",
            RacidError::Timeout => "timeout",
            RacidError::PermissionDenied => "permission_denied",
            RacidError::PathNotFound => "path_not_found",
            RacidError::BufferTooSmall => "buffer_too_small",
            RacidError::Unknown { .. } => "unknown",
        }
    }
}

// ─── Default Paths ───────────────────────────────────────────────────────────

/// Default RACID paths by platform
pub mod paths {
    /// Windows Named Pipe path
    pub const WINDOWS_PIPE_PREFIX: &str = r"\\.\pipe\zeus-rac\";
    
    /// Unix Domain Socket path
    pub const UNIX_SOCKET_PATH: &str = "/tmp/zeus-rac.sock";
    
    /// Get the default path for the current platform
    pub fn default_path() -> String {
        #[cfg(windows)]
        {
            format!("{}default", WINDOWS_PIPE_PREFIX)
        }
        #[cfg(not(windows))]
        {
            UNIX_SOCKET_PATH.to_string()
        }
    }
    
    /// Generate a unique path for a channel
    pub fn channel_path(channel_id: u32) -> String {
        #[cfg(windows)]
        {
            format!("{}ch-{:08x}", WINDOWS_PIPE_PREFIX, channel_id)
        }
        #[cfg(not(windows))]
        {
            format!("{}.ch-{:08x}", UNIX_SOCKET_PATH, channel_id)
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_platform_transport_detection() {
        let transport = PlatformTransport::detect();
        #[cfg(windows)]
        assert_eq!(transport, PlatformTransport::NamedPipe);
        #[cfg(not(windows))]
        assert_eq!(transport, PlatformTransport::UnixDomainSocket);
    }
    
    #[test]
    fn test_default_path() {
        let path = paths::default_path();
        assert!(!path.is_empty());
    }
    
    #[test]
    fn test_channel_path() {
        let path = paths::channel_path(0x12345678);
        assert!(path.contains("12345678"));
    }
    
    #[test]
    fn test_racid_message() {
        let msg = RacidMessage::new(RacidMsgType::Intent, vec![1, 2, 3]);
        assert_eq!(msg.msg_type, RacidMsgType::Intent);
        assert_eq!(msg.payload, vec![1, 2, 3]);
    }
}