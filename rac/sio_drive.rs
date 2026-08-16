// cona/io/rac/sio_drive.rs
//
// SIO Drive: Persistence layer for SIO data
//
// Era 5 Architecture:
//   Provides persistent storage for SIO envelopes and payloads.
//   Integrates with racbox for lease-based storage.
//
// SIO Drive responsibilities:
//   - Store SIO envelopes with metadata
//   - Query SIO by ID, type, timestamp
//   - Manage SIO lifecycle (create, update, delete)
//   - Handle SIO compression and encryption

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{string::{String, ToString}, vec::Vec, boxed::Box, sync::Arc};

// Re-export SIO types from wire module
use crate::wire::{SioType, SioEnvelope, SioRacExt};

/// SIO Drive configuration
#[derive(Debug, Clone)]
pub struct SioDriveConfig {
    /// Storage root path
    pub root_path: String,
    /// Max storage size in bytes
    pub max_size: u64,
    /// Compression enabled
    pub compression: bool,
    /// Encryption enabled
    pub encryption: bool,
}

impl Default for SioDriveConfig {
    fn default() -> Self {
        Self {
            root_path: "/tmp/sio_drive".to_string(),
            max_size: 1024 * 1024 * 1024, // 1GB
            compression: true,
            encryption: false,
        }
    }
}

/// SIO metadata for persistence
#[derive(Debug, Clone)]
pub struct SioMetadata {
    /// SIO ID
    pub id: String,
    /// SIO type
    pub sio_type: SioType,
    /// Created timestamp (ns since epoch)
    pub created_at: u64,
    /// Updated timestamp (ns since epoch)
    pub updated_at: u64,
    /// Size in bytes
    pub size: u32,
    /// Checksum (optional)
    pub checksum: Option<String>,
    /// Tags for indexing
    pub tags: Vec<String>,
}

impl SioMetadata {
    /// Create new metadata
    pub fn new(id: String, sio_type: SioType, size: u32) -> Self {
        let now = Self::current_timestamp_ns();
        Self {
            id,
            sio_type,
            created_at: now,
            updated_at: now,
            size,
            checksum: None,
            tags: Vec::new(),
        }
    }
    
    /// Update timestamp
    pub fn touch(&mut self) {
        self.updated_at = Self::current_timestamp_ns();
    }
    
    /// Add tag
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }
    
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

/// SIO Drive entry
#[derive(Debug, Clone)]
pub struct SioDriveEntry {
    /// Metadata
    pub metadata: SioMetadata,
    /// Payload (may be compressed/encrypted)
    pub payload: Vec<u8>,
}

impl SioDriveEntry {
    /// Create new entry from envelope
    pub fn from_envelope(envelope: SioEnvelope) -> Result<Self, SioDriveError> {
        let payload = envelope.encode();
        let metadata = SioMetadata::new(
            envelope.intent_id().to_string(),
            envelope.sio_type(),
            payload.len() as u32,
        );
        
        Ok(Self { metadata, payload })
    }
    
    /// Convert to envelope
    pub fn to_envelope(&self) -> Result<SioEnvelope, SioDriveError> {
        SioEnvelope::decode(&self.payload).map_err(|e| SioDriveError::Decode(e.to_string()))
    }
}

/// SIO Drive error
#[derive(Debug, Clone)]
pub enum SioDriveError {
    StorageFull,
    NotFound(String),
    Encode(String),
    Decode(String),
    Io(String),
}

impl core::fmt::Debug for SioDriveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StorageFull => write!(f, "StorageFull"),
            Self::NotFound(s) => write!(f, "NotFound({})", s),
            Self::Encode(s) => write!(f, "Encode({})", s),
            Self::Decode(s) => write!(f, "Decode({})", s),
            Self::Io(s) => write!(f, "Io({})", s),
        }
    }
}

/// SIO Drive - persistence layer
pub struct SioDrive {
    config: SioDriveConfig,
    /// In-memory index for fast lookup
    index: SioDriveIndex,
}

impl Default for SioDrive {
    fn default() -> Self {
        Self::new(SioDriveConfig::default())
    }
}

impl SioDrive {
    /// Create new SIO Drive
    pub fn new(config: SioDriveConfig) -> Self {
        Self {
            config,
            index: SioDriveIndex::new(),
        }
    }
    
    /// Store an SIO envelope
    pub fn store(&mut self, envelope: SioEnvelope) -> Result<SioId, SioDriveError> {
        let entry = SioDriveEntry::from_envelope(envelope)?;
        let id = entry.metadata.id.clone();
        
        // Check storage limit
        let new_size = entry.payload.len() as u64;
        if self.used_size() + new_size > self.config.max_size {
            return Err(SioDriveError::StorageFull);
        }
        
        // Store entry
        self.index.insert(id.clone(), entry);
        Ok(SioId(id))
    }
    
    /// Retrieve an SIO by ID
    pub fn get(&self, id: &str) -> Result<SioEnvelope, SioDriveError> {
        self.index
            .get(id)
            .and_then(|e| e.to_envelope())
            .ok_or_else(|| SioDriveError::NotFound(id.to_string()))
    }
    
    /// Delete an SIO by ID
    pub fn delete(&mut self, id: &str) -> Result<(), SioDriveError> {
        self.index.remove(id).ok_or_else(|| SioDriveError::NotFound(id.to_string()))
    }
    
    /// Query SIOs by type
    pub fn query_by_type(&self, sio_type: SioType) -> Vec<SioId> {
        self.index
            .by_type(&sio_type)
            .map(|id| SioId(id.clone()))
            .collect()
    }
    
    /// Query SIOs by tag
    pub fn query_by_tag(&self, tag: &str) -> Vec<SioId> {
        self.index
            .by_tag(tag)
            .map(|id| SioId(id.clone()))
            .collect()
    }
    
    /// Get total used size
    pub fn used_size(&self) -> u64 {
        self.index.total_size()
    }
    
    /// Get entry count
    pub fn count(&self) -> usize {
        self.index.len()
    }
}

/// SIO ID wrapper
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SioId(String);

impl SioId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for SioId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Internal Index ─────────────────────────────────────────────────────────

use alloc::collections::BTreeMap;

struct SioDriveIndex {
    entries: BTreeMap<String, SioDriveEntry>,
    by_type_map: BTreeMap<SioType, Vec<String>>,
    by_tag_map: BTreeMap<String, Vec<String>>,
}

impl SioDriveIndex {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            by_type_map: BTreeMap::new(),
            by_tag_map: BTreeMap::new(),
        }
    }
    
    fn insert(&mut self, id: String, entry: SioDriveEntry) {
        let sio_type = entry.metadata.sio_type.clone();
        let tags = entry.metadata.tags.clone();
        
        // Insert into main index
        self.entries.insert(id.clone(), entry);
        
        // Update type index
        self.by_type_map
            .entry(sio_type)
            .or_default()
            .push(id.clone());
        
        // Update tag index
        for tag in tags {
            self.by_tag_map
                .entry(tag)
                .or_default()
                .push(id.clone());
        }
    }
    
    fn get(&self, id: &str) -> Option<&SioDriveEntry> {
        self.entries.get(id)
    }
    
    fn remove(&mut self, id: &str) -> Option<SioDriveEntry> {
        if let Some(entry) = self.entries.remove(id) {
            // Remove from type index
            if let Some(ids) = self.by_type_map.get_mut(&entry.metadata.sio_type) {
                ids.retain(|i| i != id);
            }
            // Remove from tag index
            for tag in &entry.metadata.tags {
                if let Some(ids) = self.by_tag_map.get_mut(tag) {
                    ids.retain(|i| i != id);
                }
            }
            Some(entry)
        } else {
            None
        }
    }
    
    fn by_type(&self, sio_type: &SioType) -> impl Iterator<Item = &String> {
        self.by_type_map
            .get(sio_type)
            .into_iter()
            .flat_map(|v| v.iter())
    }
    
    fn by_tag(&self, tag: &str) -> impl Iterator<Item = &String> {
        self.by_tag_map
            .get(tag)
            .into_iter()
            .flat_map(|v| v.iter())
    }
    
    fn total_size(&self) -> u64 {
        self.entries.values().map(|e| e.payload.len() as u64).sum()
    }
    
    fn len(&self) -> usize {
        self.entries.len()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sio_metadata() {
        let mut meta = SioMetadata::new("test-1".to_string(), SioType::Intent, 100);
        meta.add_tag("test".to_string());
        assert_eq!(meta.tags.len(), 1);
    }
    
    #[test]
    fn test_sio_drive_store() {
        let config = SioDriveConfig::default();
        let mut drive = SioDrive::new(config);
        
        let envelope = SioEnvelope::new_intent(1, b"test payload".to_vec());
        let result = drive.store(envelope);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_sio_drive_query() {
        let config = SioDriveConfig::default();
        let mut drive = SioDrive::new(config);
        
        let envelope = SioEnvelope::new_intent(1, b"test".to_vec());
        drive.store(envelope).unwrap();
        
        let intents = drive.query_by_type(SioType::Intent);
        assert!(!intents.is_empty());
    }
}