extern crate mio;
#[macro_use]
extern crate log;

use mio::*;
use mio::tcp::TcpListener;
use mio::tcp::TcpStream;
use std::collections::HashMap;
use std::rc::Rc;


const CASTER: Token = Token(0);
const WATCHER: Token = Token(1);


enum Client {
    Watcher(Watcher),
    Caster(Caster),
}

struct Caster {
    sock: NonBlock<TcpStream>,
    watchers: Vec<Rc<Watcher>>,
}

struct Watcher {
    sock: NonBlock<TcpStream>,
}

struct Termcastd {
    listen_caster: NonBlock<TcpListener>,
    listen_watcher: NonBlock<TcpListener>,
    clients: HashMap<Token, Client>,
    casters: Vec<Caster>,
    watchers: Vec<Watcher>,
    next_token_id: usize,
    number_watching: u32,
    number_casting: u32,
}


impl Termcastd {
    fn caster_menu(&mut self, watcher: &mut Watcher) {
        let menu_header = format!(
            "{}\n ## Termcast\n ## {} sessions available. {} watchers connected.\n",
            "", self.number_casting, self.number_watching);
        let menu_header_bytes = menu_header.as_bytes();
        watcher.sock.write_slice(&menu_header_bytes);
    }
}

impl Handler for Termcastd {
    type Timeout = ();
    type Message = ();

    fn readable(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token, hint: ReadHint) {
        match token {
            CASTER => {
                if let Ok(opt) = self.listen_caster.accept() {
                    if let Some(sock) = opt {
                        let mut caster = Caster {
                            sock: sock,
                            watchers: Vec::new(),
                        };
                        self.number_casting += 1;
                        self.casters.push(caster);
                        let idx = self.casters.len() - 1;
                        let token = Token(self.next_token_id);
                        self.next_token_id += 1;
                        event_loop.register(&self.casters[idx].sock, token);
                    }
                }
                else {
                }
            },
            WATCHER => {
                if let Ok(opt) = self.listen_watcher.accept() {
                    if let Some(sock) = opt {
                        let mut watcher = Watcher {
                            sock: sock,
                        };
                        self.number_watching += 1;
                        self.caster_menu(&mut watcher);
                        self.watchers.push(watcher);
                        let idx = self.watchers.len() - 1;
                        let token = Token(self.next_token_id);
                        self.next_token_id += 1;
                        event_loop.register(&self.watchers[idx].sock, token);
                    }
                }
                else {
                }
            },
            _ => {},
        }
    }
}


fn main() {
    let caster_addr = "127.0.0.1:31337".parse().unwrap();
    let listen_caster = tcp::listen(&caster_addr).unwrap();

    let watcher_addr = "127.0.0.1:2300".parse().unwrap();
    let listen_watcher = tcp::listen(&watcher_addr).unwrap();

    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&listen_caster, CASTER).unwrap();
    event_loop.register(&listen_watcher, WATCHER).unwrap();

    let mut termcastd = Termcastd {
        listen_caster: listen_caster,
        listen_watcher: listen_watcher,
        clients: HashMap::new(),
        casters: Vec::new(),
        watchers: Vec::new(),
        next_token_id: 2,
        number_watching: 0,
        number_casting: 0,
    };
    event_loop.run(&mut termcastd).unwrap();
}
