use alloc::{collections::BTreeMap, string::String};

#[derive(Debug, Default)]
pub struct IoRouteRegistry {
    routes: BTreeMap<String, String>,
}

impl IoRouteRegistry {
    pub fn register(&mut self, route: String, fan: String) {
        self.routes.insert(route, fan);
    }

    pub fn resolve(&self, route: &str) -> Option<&str> {
        self.routes.get(route).map(String::as_str)
    }
}
