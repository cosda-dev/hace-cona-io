use alloc::string::String;

use crate::{binding::bind, core::context::UnifiedRequest, rac::RacRouter};

pub struct IoKernel {
    pub router: RacRouter,
}

impl Default for IoKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl IoKernel {
    pub fn new() -> Self {
        Self {
            router: RacRouter::new(),
        }
    }

    pub fn execute(&self, route: &str, payload: &str) -> String {
        self.router
            .dispatch(route, payload)
            .unwrap_or_else(|err| format!("io-kernel error={err} route={route}"))
    }

    pub fn execute_fan(&self, fan: &str, payload: &str) -> String {
        self.router
            .dispatch_fan(fan, payload)
            .unwrap_or_else(|err| format!("io-kernel error={err} fan={fan}"))
    }

    pub fn resolve_jid(&self, route: &str) -> Option<u16> {
        self.router.resolve_jid(route).ok()
    }

    pub fn execute_bound(&self, req: UnifiedRequest, fan: &str) -> String {
        let bound = bind(req, fan);
        let payload = String::from_utf8(bound.payload).unwrap_or_default();
        self.execute_fan(bound.fan.as_str(), payload.as_str())
    }
}
