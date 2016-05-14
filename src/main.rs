extern crate getopts;
extern crate termcastd;

use getopts::Options;
use std::env;
use termcastd::TermcastServer;
use termcastd::config::TermcastConfig;

fn get_options() -> TermcastConfig {
    let args: Vec<String> = env::args().collect();
    let mut options = Options::new();
    options.optflag("f", "foreground", "Run in the foreground.");
    options.optopt("c", "config", "Configuration file to use.", "FILE");

    let matches = match options.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => panic!(e.to_string()),
    };

    let tc_config = match matches.opt_str("c") {
        Some(config_file) => {
            match TermcastConfig::from_config(&config_file) {
                Ok(c) => c,
                Err(_e) => TermcastConfig::default(),
            }
        },
        None => TermcastConfig::default(),
    };

    return tc_config;
}

fn main() {
    let tc_config = get_options();

    if let Ok(mut termcast) = TermcastServer::new(tc_config) {
        termcast.run();
    }
    else {
    }
}
