extern crate mio;

use mio::*;
use mio::tcp::TcpListener;


const CASTER: Token = Token(0);
const WATCHER: Token = Token(1);


struct Termcastd (NonBlock<TcpListener>);


impl Handler for Termcastd {
    type Timeout = ();
    type Message = ();
}


fn main() {
    let addr = "127.0.0.1:31337".parse().unwrap();
    let server = tcp::listen(&addr).unwrap();

    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&server, CASTER).unwrap();

    let watcher_addr = "127.0.0.1:2300".parse().unwrap();
    let watcher_server = tcp::listen(&watcher_addr).unwrap();
    event_loop.register(&watcher_server, WATCHER).unwrap();

    event_loop.run(&mut Termcastd(server)).unwrap();
}
