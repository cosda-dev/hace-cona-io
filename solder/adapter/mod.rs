use alloc::vec::Vec;

pub trait IoSolderAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn handle(&self, input: Vec<u8>) -> Vec<u8>;
}
