use std::default::Default;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::Read;
use std::net;
use std::str;

use toml;

pub struct TermcastConfig {
    pub caster: net::SocketAddr,
    pub watcher: net::SocketAddr,
    pub motd: Option<String>,
}


#[derive(Debug)]
pub enum ConfigError {
    Nothing,
    InvalidAddr(net::AddrParseError),
    Io(io::Error),
}

const CASTER_LISTEN: &'static str = "127.0.0.1:31337";
const WATCHER_LISTEN: &'static str = "127.0.0.1:2300";
const MOTD: Option<String> = None;

impl Default for TermcastConfig {
    fn default() -> Self {
        TermcastConfig {
            caster: CASTER_LISTEN.parse().unwrap(),
            watcher: WATCHER_LISTEN.parse().unwrap(),
            motd: MOTD,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::Io(err)
    }
}

fn get_option(toml_value: &toml::Value, option_name: &str) -> Option<String> {
    match toml_value {
        &toml::Value::Table(ref table) => {
            if let Some(option_value) = table.get(option_name) {
                get_option(option_value, "")
            }
            else {
                None
            }
        },
        &toml::Value::String(ref string) => {
            Some(string.clone())
        },
        _ => None,
    }
}

fn parse_socketaddr(addr: String) -> Result<net::SocketAddr, ConfigError> {
    addr.parse().map_err(ConfigError::InvalidAddr)
}

impl TermcastConfig {
    pub fn from_config(config_file_path: &str) -> Result<Self, ConfigError> {
        let mut config = TermcastConfig::default();

        let mut config_file = try!(File::open(&config_file_path));
        let mut contents = String::new();
        try!(config_file.read_to_string(&mut contents));
        let mut parser = toml::Parser::new(&contents);
        let options = parser.parse().unwrap();
        if let Some(server_config) = options.get("server") {
            let c = get_option(&server_config, "caster_listen")
                        .ok_or(ConfigError::Nothing)
                        .and_then(parse_socketaddr);
            match c {
                Ok(addr) => { config.caster = addr }
                Err(ConfigError::InvalidAddr(e)) => {
                    println!("Invalid caster listen address: {}.", e);
                }
                Err(_) => { }
            }

            let c = get_option(&server_config, "watcher_listen")
                        .ok_or(ConfigError::Nothing)
                        .and_then(parse_socketaddr);
            match c {
                Ok(addr) => { config.watcher = addr }
                Err(ConfigError::InvalidAddr(e)) => {
                    println!("Invalid watcher listen address: {}.", e);
                }
                Err(_) => { }
            }
        }

        return Ok(config);
    }
}
