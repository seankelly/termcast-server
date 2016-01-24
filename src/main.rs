extern crate getopts;
extern crate termcastd;

use termcastd::TermcastServer;
use termcastd::config::TermcastConfig;

fn main() {
    let tc_config = TermcastConfig::default();

    if let Ok(mut termcast) = TermcastServer::new(tc_config) {
        termcast.run();
    }
    else {
    }
}
