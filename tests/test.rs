extern crate mio;
extern crate termcastd;

use termcastd::config::TermcastConfig;
use termcastd::TermcastServer;

use mio::tcp::{TcpListener, TcpStream};

#[test]
fn listen() {
    let config = TermcastConfig {
        caster: "127.0.0.1:0".parse().unwrap(),
        watcher: "127.0.0.1:0".parse().unwrap(),
    };

    assert!(TermcastServer::new(config).is_ok(), "Can bind both ports.");
}

#[test]
fn bind_taken() {
    let sock = "127.0.0.1:0".parse().unwrap();
    let l = TcpListener::bind(&sock).unwrap();

    let config = TermcastConfig {
        caster: l.local_addr().unwrap(),
        watcher: "127.0.0.1:0".parse().unwrap(),
    };

    let tc = TermcastServer::new(config);
    assert!(tc.is_err(), "Error returned when port in use.");
}
