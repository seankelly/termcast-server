extern crate termcastd;

use termcastd::TermcastServer;
use termcastd::config::TermcastConfig;

fn main() {
    let tc_config = TermcastConfig {
        caster: "127.0.0.1:31337".parse().unwrap(),
        watcher: "127.0.0.1:2300".parse().unwrap(),
        motd: None,
    };

    if let Ok(mut termcast) = TermcastServer::new(tc_config) {
        termcast.run();
    }
    else {
    }
}
