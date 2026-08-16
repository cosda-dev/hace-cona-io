#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoLifecycle {
    Registered,
    Ready,
    Serving,
}
