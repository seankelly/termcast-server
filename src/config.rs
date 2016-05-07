use std::default::Default;
use std::net::SocketAddr;

pub struct TermcastConfig {
    pub caster: SocketAddr,
    pub watcher: SocketAddr,
    pub motd: Option<String>,
}

const CASTER_LISTEN: &'static str = "127.0.0.1:31337";
const WATCHER_LISTEN: &'static str = "127.0.0.1:2300";
const MOTD: Option<String> = None;

impl Default for TermcastConfig {
    fn default() -> Self {
        TermcastConfig {
            caster: CASTER_LISTEN.parse().unwrap(),
            watcher: WATCHER_LISTEN.parse().unwrap(),
            motd: None,
        }
    }
}
