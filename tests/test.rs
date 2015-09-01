extern crate mio;
extern crate termcastd;

use std::thread;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::mpsc::channel;

use termcastd::config::TermcastConfig;
use termcastd::TermcastServer;
use termcastd::TermcastdMessage;

use mio::Sender;
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

#[test]
fn threaded_termcastd() {
    let (_thd, ev_channel, _caster_addr, _watcher_addr) = termcastd_thread();

    ev_channel.send(TermcastdMessage::Quit).unwrap();
}

#[test]
fn one_caster_log_in() {
    let (_thd, ev_channel, caster_addr, _watcher_addr) = termcastd_thread();

    let mut _caster = caster_login(&caster_addr, "name", "pass");

    ev_channel.send(TermcastdMessage::Quit).unwrap();
}

#[test]
fn caster_log_in_fail() {
    let (_thd, ev_channel, caster_addr, _watcher_addr) = termcastd_thread();

    let mut caster = make_caster(&caster_addr);
    caster.write("hello\n".as_bytes()).unwrap();

    let mut buf = [0; 128];
    let res = caster.read(&mut buf);
    assert!(res.is_err());

    ev_channel.send(TermcastdMessage::Quit).unwrap();
}


fn make_termcastd() -> TermcastServer {
    let config = TermcastConfig {
        caster: "127.0.0.1:0".parse().unwrap(),
        watcher: "127.0.0.1:0".parse().unwrap(),
    };

    TermcastServer::new(config).unwrap()
}

fn termcastd_thread() -> (thread::JoinHandle<()>, Sender<TermcastdMessage>, SocketAddr, SocketAddr) {
    let (tx, rx) = channel();

    let thd = thread::spawn(move || {
        let mut tc = make_termcastd();
        let (caster_addr, watcher_addr) = tc.get_socket_addrs().unwrap();
        tx.send((tc.get_channel(), caster_addr, watcher_addr)).unwrap();
        tc.run();
    });

    let (ev_channel, caster_addr, watcher_addr) = rx.recv().unwrap();

    return (thd, ev_channel, caster_addr, watcher_addr);
}

fn make_caster(addr: &SocketAddr) -> TcpStream {
    TcpStream::connect(addr).unwrap()
}

fn caster_login(addr: &SocketAddr, name: &str, password: &str) -> TcpStream {
    let mut stream = make_caster(addr);
    stream.write_fmt(format_args!("hello {} {}\n", name, password)).unwrap();
    return stream;
}
