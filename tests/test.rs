extern crate mio;
extern crate termcastd;

use std::thread;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::channel;
use std::time::Duration;
use std::str;

use termcastd::config::TermcastConfig;
use termcastd::TermcastServer;
use termcastd::TermcastdMessage;

use mio::Sender;
use mio::tcp::TcpListener;

#[test]
fn listen() {
    let config = TermcastConfig {
        caster: "127.0.0.1:0".parse().unwrap(),
        watcher: "127.0.0.1:0".parse().unwrap(),
        motd: None,
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
        motd: None,
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

    let mut buf = [0; 128];

    let mut caster = caster_login(&caster_addr, "name", "pass");
    caster.set_read_timeout(Some(Duration::new(1, 0))).unwrap();
    let res = caster.read(&mut buf);
    // This will be an error because the read will timeout because there is nothing to read.
    assert!(res.is_err(), "Logged in successfully.");

    // Test sending the login in three parts.
    let mut caster = connect_timeout(&caster_addr);
    caster.write("hello ".as_bytes()).unwrap();
    caster.write("name ".as_bytes()).unwrap();
    caster.write("pass\n".as_bytes()).unwrap();
    let res = caster.read(&mut buf);
    assert!(res.is_err(), "Three-part log in successful.");

    ev_channel.send(TermcastdMessage::Quit).unwrap();
}

#[test]
fn caster_log_in_fail() {
    let (_thd, ev_channel, caster_addr, _watcher_addr) = termcastd_thread();

    let mut buf = [0; 128];

    // Need more than just "hello\n".
    let mut caster = connect_timeout(&caster_addr);
    caster.write("hello\n".as_bytes()).unwrap();
    let res = caster.read(&mut buf);
    assert!(res.is_ok(), "Missing name fails.");
    assert_eq!(res.unwrap(), 0);

    // Write 1025 bytes without a newline, more than the 1024 byte limit.
    let mut caster = connect_timeout(&caster_addr);
    let input = [32; 1025];
    caster.write(&input).unwrap();
    let res = caster.read(&mut buf);
    assert!(res.is_ok(), "No newline fails.");
    assert_eq!(res.unwrap(), 0);

    // Try a zero-length name.
    let mut caster = connect_timeout(&caster_addr);
    caster.write("hello  \n".as_bytes()).unwrap();
    let res = caster.read(&mut buf);
    assert!(res.is_ok(), "Zero-length name fails.");
    assert_eq!(res.unwrap(), 0);

    // Try a name with a control character in it.
    let mut caster = connect_timeout(&caster_addr);
    caster.write("hello \u{19} \n".as_bytes()).unwrap();
    let res = caster.read(&mut buf);
    assert!(res.is_ok(), "Control character in name fails.");
    assert_eq!(res.unwrap(), 0);

    ev_channel.send(TermcastdMessage::Quit).unwrap();
}

#[test]
fn can_cast() {
    let (_thd, _ev_channel, caster_addr, watcher_addr) = termcastd_thread();

    let mut caster = caster_login(&caster_addr, "caster1", "secret");
    // Write something to keep the value from being optimized away.
    caster.write(&[32]).unwrap();

    let mut watcher = connect(&watcher_addr);
    let mut buf = [0; 2048];
    watcher.set_read_timeout(Some(Duration::new(0, 100))).unwrap();

    // Refresh the main menu a few times until the caster is seen. Set a low limit.
    let max_checks = 10;
    let mut offset;
    let mut loops = 0;
    loop {
        let res = watcher.read(&mut buf);
        // Read timeout, try again.
        if res.is_err() {
            loops += 1;
            continue;
        }
        // Scan for the position of the first '#' in the stream, this indicates the banner. Keep
        // reading from the socket until it's found.
        if let Some(i) = buf.iter().position(|b| *b == 0x23) {
            offset = i;
            let utf8_buf = str::from_utf8(&buf[offset..]).unwrap();
            let found_caster = utf8_buf.find("caster1");
            if found_caster.is_some() {
                break;
            }

            loops += 1;
            if loops > max_checks {
                break;
            }

            watcher.write(&[32]).unwrap();
        }
    }

    assert!(loops <= max_checks);

}


fn make_termcastd() -> TermcastServer {
    let config = TermcastConfig {
        caster: "127.0.0.1:0".parse().unwrap(),
        watcher: "127.0.0.1:0".parse().unwrap(),
        motd: None,
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

    // Connect to the watcher address. Once something is received, termcastd can be considered
    // running. Then return connection details for the tests.
    {
        let mut watcher = connect(&watcher_addr);
        // An 80x24 terminal is 1920 bytes. Round up to fit everything.
        let mut buf = [0; 2048];
        watcher.read(&mut buf).unwrap();
        let quit = ['q' as u8];
        watcher.write(&quit).unwrap();
    }

    return (thd, ev_channel, caster_addr, watcher_addr);
}

fn connect(addr: &SocketAddr) -> TcpStream {
    TcpStream::connect(addr).unwrap()
}

fn connect_timeout(addr: &SocketAddr) -> TcpStream {
    let caster = connect(addr);
    caster.set_read_timeout(Some(Duration::new(1, 0))).unwrap();
    return caster;
}

fn caster_login(addr: &SocketAddr, name: &str, password: &str) -> TcpStream {
    let mut stream = connect(addr);
    stream.write_fmt(format_args!("hello {} {}\n", name, password)).unwrap();
    return stream;
}
