extern crate mio;
#[macro_use]
extern crate log;

use mio::*;
use mio::tcp::TcpListener;
use mio::tcp::TcpStream;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;


const CASTER: Token = Token(0);
const WATCHER: Token = Token(1);

struct Caster {
    sock: NonBlock<TcpStream>,
    token: Token,
    watchers: Vec<Rc<Watcher>>,
}

struct Watcher {
    offset: usize,
    sock: NonBlock<TcpStream>,
    token: Token,
}

struct Termcastd {
    listen_caster: NonBlock<TcpListener>,
    listen_watcher: NonBlock<TcpListener>,
    clients: HashMap<Token, Client>,
    next_token_id: usize,
    number_watching: u32,
    number_casting: u32,
}


enum TermcastdMessage {
    CasterDisconnected(Token),
}

enum Client {
    Casting(Caster),
    Watching(Watcher),
}


impl Termcastd {
    fn caster_menu(&mut self, watcher: &mut Watcher) {
        let menu_header = format!(
            "{}{}\n ## Termcast\n ## {} sessions available. {} watchers connected.\n\n",
            term::clear_screen(), term::reset_cursor(),
            self.number_casting, self.number_watching);
        let menu_choices: Vec<String> = self.clients.iter()
                   .filter(|client| {
                       let (t, c) = *client;
                       match c {
                           &Client::Casting(ref C) => true, _ => false
                       }
                   })
                   .skip(watcher.offset)
                   .enumerate()
                   .map(|c| "caster".to_string())
                   .collect();
        let menu_header_bytes = menu_header.as_bytes();
        let menu = menu_choices.connect("n");
        let menu_bytes = menu.as_bytes();
        let res = watcher.sock.write_slice(&menu_header_bytes);
        let res = watcher.sock.write_slice(&menu_bytes);
    }

    fn next_token(&mut self) -> Token {
        let token = Token(self.next_token_id);
        self.next_token_id += 1;
        return token;
    }

    fn handle_disconnect(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) {
        if let Entry::Occupied(client) = self.clients.entry(token) {
            match client.get() {
                &Client::Casting(ref caster) => {
                    let res = event_loop.deregister(&caster.sock);
                    self.number_casting -= 1;
                    let channel = event_loop.channel();
                    // To not have to do a mutable borrow, send a message to
                    // reset these watchers back to the main menu. Everything
                    // will be dropped after the end of the match when the
                    // entry is removed.
                    for watcher in caster.watchers.iter() {
                        let res = channel.send(TermcastdMessage::CasterDisconnected(watcher.token));
                    }
                },
                &Client::Watching(ref watcher) => {
                    let res = event_loop.deregister(&watcher.sock);
                    self.number_watching -= 1;
                },
            }
            client.remove();
        }
        else {
            panic!("Couldn't find token {:?} in self.clients", token);
        }
    }
}

impl Handler for Termcastd {
    type Timeout = ();
    type Message = TermcastdMessage;

    fn readable(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token, hint: ReadHint) {
        match token {
            CASTER => {
                if let Ok(opt) = self.listen_caster.accept() {
                    if let Some(sock) = opt {
                        let token = self.next_token();
                        let caster = Caster {
                            sock: sock,
                            token: token,
                            watchers: Vec::new(),
                        };
                        self.number_casting += 1;
                        let res = event_loop.register_opt(
                            &caster.sock,
                            token,
                            Interest::all(),
                            PollOpt::edge(),
                        );
                        let client = Client::Casting(caster);
                        self.clients.insert(token, client);
                    }
                }
                else {
                }
            },
            WATCHER => {
                if let Ok(opt) = self.listen_watcher.accept() {
                    if let Some(sock) = opt {
                        let token = self.next_token();
                        let mut watcher = Watcher {
                            offset: 0,
                            sock: sock,
                            token: token,
                        };
                        self.number_watching += 1;
                        self.caster_menu(&mut watcher);
                        let res = event_loop.register_opt(
                            &watcher.sock,
                            token,
                            Interest::all(),
                            PollOpt::edge(),
                        );
                        let client = Client::Watching(watcher);
                        self.clients.insert(token, client);
                    }
                }
                else {
                }
            },
            _ => {
                match (hint.is_data(), hint.is_hup(), hint.is_error()) {
                    (true, false, false) => {},
                    (_, true, false) => {
                        self.handle_disconnect(event_loop, token);
                    },
                    (_, _, true) => {},
                    (false, false, false) => {},
                };
            },
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Termcastd>, message: TermcastdMessage) {
        println!("Got message");
    }
}

mod term {
    pub fn clear_screen() -> &'static str { "\x1b[2J" }
    pub fn reset_cursor() -> &'static str { "\x1b[H" }
}


fn main() {
    println!("Listening on caster port.");
    let caster_addr = "127.0.0.1:31337".parse().unwrap();
    let listen_caster = tcp::listen(&caster_addr).unwrap();

    println!("Listening on watcher port.");
    let watcher_addr = "127.0.0.1:2300".parse().unwrap();
    let listen_watcher = tcp::listen(&watcher_addr).unwrap();

    println!("Registering listeners with event loop.");
    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&listen_caster, CASTER).unwrap();
    event_loop.register(&listen_watcher, WATCHER).unwrap();

    let mut termcastd = Termcastd {
        listen_caster: listen_caster,
        listen_watcher: listen_watcher,
        clients: HashMap::new(),
        next_token_id: 2,
        number_watching: 0,
        number_casting: 0,
    };
    event_loop.run(&mut termcastd).unwrap();
}
