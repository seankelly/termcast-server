use std::default::Default;
use std::fs::File;
use std::io::Read;
use std::io::Error as IoError;
use std::net::SocketAddr;
use std::str;

use toml;

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

impl TermcastConfig {
    pub fn from_config(config_file_path: &str) -> Result<Self, IoError> {
        let mut config = TermcastConfig::default();

        let mut config_file = try!(File::open(&config_file_path));
        let mut contents = Vec::new();
        let _bytes_read = try!(config_file.read_to_end(&mut contents));
        let contents_str = str::from_utf8(&contents).unwrap();
        let mut parser = toml::Parser::new(&contents_str);
        let options = parser.parse().unwrap();
        println!("{:?}", options);
        if let Some(server_config) = options.get("server") {
            if let Some(caster_listen) = get_option(&server_config, "caster_listen") {
                config.caster = caster_listen.parse().unwrap();
            }
            if let Some(watcher_listen) = get_option(&server_config, "watcher_listen") {
                config.watcher = watcher_listen.parse().unwrap();
            }
        }

        return Ok(config);
    }
}
