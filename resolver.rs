use alloc::{boxed::Box, string::String};
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

pub type FanId = u32;
pub type AmoId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpStage {
    Port,
    Pre,
    Execute,
    After,
    Gate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanDescriptor {
    pub fan_id: FanId,
    pub amo_id: AmoId,
    pub hp_stage: HpStage,
    pub fan_name: &'static str,
}

#[derive(Debug)]
pub struct ResolverTable {
    entries: &'static [(&'static str, FanDescriptor)],
}

impl ResolverTable {
    pub const fn new(entries: &'static [(&'static str, FanDescriptor)]) -> Self {
        Self { entries }
    }

    pub fn resolve(&self, uri: &str) -> Option<FanDescriptor> {
        for (pattern, desc) in self.entries.iter() {
            if uri.starts_with(pattern) {
                return Some(*desc);
            }
        }
        None
    }
}

const DEFAULT_ENTRIES: [(&str, FanDescriptor); 3] = [
    (
        "rac://",
        FanDescriptor {
            fan_id: 1,
            amo_id: 100,
            hp_stage: HpStage::Execute,
            fan_name: "core.echo",
        },
    ),
    (
        "http://",
        FanDescriptor {
            fan_id: 2,
            amo_id: 100,
            hp_stage: HpStage::Execute,
            fan_name: "core.upper",
        },
    ),
    (
        "mcp://",
        FanDescriptor {
            fan_id: 3,
            amo_id: 100,
            hp_stage: HpStage::Execute,
            fan_name: "core.echo",
        },
    ),
];

static DEFAULT_TABLE: ResolverTable = ResolverTable::new(&DEFAULT_ENTRIES);

#[derive(Debug)]
pub struct ResolverCache {
    ptr: AtomicPtr<ResolverTable>,
}

impl Default for ResolverCache {
    fn default() -> Self {
        let ptr = (&DEFAULT_TABLE as *const ResolverTable).cast_mut();
        Self {
            ptr: AtomicPtr::new(ptr),
        }
    }
}

impl ResolverCache {
    pub fn get(&self) -> &ResolverTable {
        let ptr = self.ptr.load(Ordering::Acquire);
        if ptr.is_null() {
            &DEFAULT_TABLE
        } else {
            unsafe { &*ptr }
        }
    }

    pub fn reload(&self, new: Box<ResolverTable>) {
        let ptr = Box::into_raw(new);
        self.ptr.store(ptr, Ordering::Release);
    }

    pub fn resolve_uri(&self, uri: &str) -> Result<FanDescriptor, &'static str> {
        self.get().resolve(uri).ok_or("fan not found")
    }
}

impl Drop for ResolverCache {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Acquire);
        let default_ptr = (&DEFAULT_TABLE as *const ResolverTable).cast_mut();
        if !ptr.is_null() && !ptr::eq(ptr, default_ptr) {
            unsafe {
                drop(Box::from_raw(ptr));
            }
        }
    }
}

pub fn canonical_uri(uri: &str) -> String {
    let normalized = uri.trim();
    if normalized.ends_with('/') && normalized.len() > 1 {
        normalized.trim_end_matches('/').to_string()
    } else {
        normalized.to_string()
    }
}
