use std::default::Default;
use std::fs::File;
use std::io::Read;
use std::io::Error as IoError;
use std::net;
use std::str;

use toml;

pub struct TermcastConfig {
    pub caster: net::SocketAddr,
    pub watcher: net::SocketAddr,
    pub motd: Option<String>,
}


#[derive(Debug)]
enum ConfigError {
    Nothing,
    Invalid(net::AddrParseError),
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
    addr.parse().map_err(ConfigError::Invalid)
}

impl TermcastConfig {
    pub fn from_config(config_file_path: &str) -> Result<Self, IoError> {
        let mut config = TermcastConfig::default();

        let mut config_file = try!(File::open(&config_file_path));
        let mut contents = Vec::new();
        let _bytes_read = try!(config_file.read_to_end(&mut contents));
        let contents_str = str::from_utf8(&contents).unwrap();
        let mut parser = toml::Parser::new(&contents_str);
        let options = parser.parse().unwrap();
        if let Some(server_config) = options.get("server") {
            let c = get_option(&server_config, "caster_listen")
                        .ok_or(ConfigError::Nothing)
                        .and_then(parse_socketaddr);
            match c {
                Ok(addr) => { config.caster = addr }
                Err(ConfigError::Invalid(e)) => {
                    println!("Invalid caster listen address: {}.", e);
                }
                Err(_) => { }
            }

            let c = get_option(&server_config, "watcher_listen")
                        .ok_or(ConfigError::Nothing)
                        .and_then(parse_socketaddr);
            match c {
                Ok(addr) => { config.watcher = addr }
                Err(ConfigError::Invalid(e)) => {
                    println!("Invalid watcher listen address: {}.", e);
                }
                Err(_) => { }
            }
        }

        return Ok(config);
    }
}
