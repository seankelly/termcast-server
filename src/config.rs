use std::default::Default;
use std::net::SocketAddr;

pub struct TermcastConfig {
    pub caster: SocketAddr,
    pub watcher: SocketAddr,
}

impl Default for TermcastConfig {
    fn default() -> Self {
        TermcastConfig {
            caster: "127.0.0.1:31337".parse().unwrap(),
            watcher: "127.0.0.1:2300".parse().unwrap(),
        }
    }
}
