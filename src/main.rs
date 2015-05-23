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
const CASTERS_PER_SCREEN: usize = 16;
const MENU_CHOICES: [&'static str; 16] = ["a", "b", "c", "d", "e", "f", "g",
                                          "h", "i", "j", "k", "l", "m", "n",
                                          "o", "p"];


struct Caster {
    sock: NonBlock<TcpStream>,
    token: Token,
    watchers: Vec<Rc<Watcher>>,
}

struct Watcher {
    offset: usize,
    sock: NonBlock<TcpStream>,
    token: Token,
    state: WatcherState,
}

struct Termcastd {
    listen_caster: NonBlock<TcpListener>,
    listen_watcher: NonBlock<TcpListener>,
    clients: HashMap<Token, Client>,
    watchers: HashMap<Token, Watcher>,
    casters: HashMap<Token, Caster>,
    next_token_id: usize,
    number_watching: u32,
    number_casting: u32,
}


enum TermcastdMessage {
    CasterDisconnected(Token),
    WatcherDisconnected(Token),
}

#[derive(Clone, Copy, Debug)]
enum Client {
    Caster,
    Watcher,
}

enum WatcherState {
    Connecting,
    Disconnecting,
    MainMenu,
    Watching,
}

impl Watcher {
    fn show_menu(&mut self, casters: &HashMap<Token, Caster>, number_casting: &u32, number_watching: &u32) {
        fn caster_menu_entry(choice: &'static str, caster: &Caster) -> String {
            let _caster = caster;
            format!(" {}) {}", choice, "caster")
        }

        self.state = WatcherState::MainMenu;

        let menu_header = format!(
            "{}{}\n ## Termcast\n ## {} sessions available. {} watchers connected.\n\n",
            term::clear_screen(), term::reset_cursor(),
            number_casting, number_watching);

        // If the offset is too high, reset it to the last page.
        if self.offset > casters.len() {
            let num_casters = casters.len();
            let page_length = MENU_CHOICES.len();
            let pages = num_casters / page_length;
            let new_offset = pages * page_length;
            self.offset = if num_casters % num_casters != 0 { new_offset }
                          else { new_offset - 1 };
        }

        let menu_choices: Vec<String> = casters.values()
                    .skip(self.offset)
                    .take(CASTERS_PER_SCREEN)
                    .zip(MENU_CHOICES.iter())
                    .map(|c| {
                        let (caster, choice) = c;
                        caster_menu_entry(choice, caster)
                    })
                    .collect();
        let menu_header_bytes = menu_header.as_bytes();
        let mut menu = menu_choices.connect("\n");
        menu.push_str("\n");
        let menu_bytes = menu.as_bytes();
        let res = self.sock.write_slice(&menu_header_bytes);
        let res = self.sock.write_slice(&menu_bytes);
    }
}

impl Termcastd {

    fn next_token(&mut self) -> Token {
        let token = Token(self.next_token_id);
        self.next_token_id += 1;
        return token;
    }

    fn read_caster(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) {
    }

    fn read_watcher(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) {
        let mut watcher = self.watchers.get_mut(&token).unwrap();
        let mut bytes_received = [0u8; 128];
        if let Ok(res) = watcher.sock.read_slice(&mut bytes_received) {
            if let Some(num_bytes) = res {
                let each_byte = 0..num_bytes;
                let channel = event_loop.channel();
                for (_offset, byte) in each_byte.zip(bytes_received.iter()) {
                    match watcher.state {
                        WatcherState::Watching => {
                            // Pressing 'q' while watching returns the watcher to the main menu.
                            if *byte == 113 {
                                // This will reset the state back to the main menu.
                                watcher.show_menu(&self.casters, &self.number_casting, &self.number_watching);
                            }
                        },
                        WatcherState::MainMenu => {
                            match *byte {
                                97...112 => { // a...p
                                    // a = 97.
                                    let page_offset = *byte as usize - 97;
                                    // Check if the entry picked is still valid.
                                    if watcher.offset + page_offset <= self.casters.len() {
                                    }
                                    else {
                                        watcher.show_menu(&self.casters, &self.number_casting, &self.number_watching);
                                    }
                                    watcher.state = WatcherState::Watching;
                                }
                                113 => { // q
                                    watcher.state = WatcherState::Disconnecting;
                                    channel.send(TermcastdMessage::WatcherDisconnected(token));
                                    return;
                                },
                                _ => {},
                            }
                        },
                        WatcherState::Connecting => {},
                        WatcherState::Disconnecting => { return },
                    }
                }
            }
            else {
                return;
            }
        }
        else {
            return;
        }
    }

    fn handle_disconnect(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) {
        if let Entry::Occupied(client) = self.clients.entry(token) {
            match client.get() {
                &Client::Caster => {
                    if let Entry::Occupied(caster_entry) = self.casters.entry(token) {
                        {
                            let caster = caster_entry.get();
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
                        }
                        caster_entry.remove();
                    }
                },
                &Client::Watcher => {
                    if let Entry::Occupied(watcher_entry) = self.watchers.entry(token) {
                        {
                            let watcher = watcher_entry.get();
                            let res = event_loop.deregister(&watcher.sock);
                        }
                        self.number_watching -= 1;
                        watcher_entry.remove();
                    }
                },
            }
            client.remove();
        }
        else {
            panic!("Couldn't find token {:?} in self.clients", token);
        }
    }

    fn new_caster(&mut self, event_loop: &mut EventLoop<Termcastd>) {
        if let Ok(opt) = self.listen_caster.accept() {
            if let Some(sock) = opt {
                let token = self.next_token();
                let caster = Caster {
                    sock: sock,
                    token: token,
                    watchers: Vec::new(),
                };
                let res = event_loop.register_opt(
                    &caster.sock,
                    token,
                    Interest::all(),
                    PollOpt::edge(),
                );
                if res.is_ok() {
                    self.number_casting += 1;
                    let client = Client::Caster;
                    self.clients.insert(token, client);
                    self.casters.insert(token, caster);
                }
            }
        }
    }

    fn new_watcher(&mut self, event_loop: &mut EventLoop<Termcastd>) {
        if let Ok(opt) = self.listen_watcher.accept() {
            if let Some(sock) = opt {
                let token = self.next_token();
                let mut watcher = Watcher {
                    offset: 0,
                    sock: sock,
                    token: token,
                    state: WatcherState::Connecting,
                };
                let res = event_loop.register_opt(
                    &watcher.sock,
                    token,
                    Interest::all(),
                    PollOpt::edge(),
                );
                if res.is_ok() {
                    self.number_watching += 1;
                    watcher.show_menu(&self.casters, &self.number_casting, &self.number_watching);
                    let client = Client::Watcher;
                    self.clients.insert(token, client);
                    self.watchers.insert(token, watcher);
                }
            }
        }
    }
}

impl Handler for Termcastd {
    type Timeout = ();
    type Message = TermcastdMessage;

    fn readable(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token, hint: ReadHint) {
        match token {
            CASTER => {
                self.new_caster(event_loop);
            },
            WATCHER => {
                self.new_watcher(event_loop);
            },
            _ => {
                let client = {
                    *self.clients.get(&token).expect("Expected to find token.")
                };
                match (hint.is_data(), hint.is_hup(), hint.is_error(), client) {
                    (true, false, false, Client::Caster) => {
                        self.read_caster(event_loop, token);
                    },
                    (true, false, false, Client::Watcher) => {
                        self.read_watcher(event_loop, token);
                    },
                    (_, true, false, _) => {
                        self.handle_disconnect(event_loop, token);
                    },
                    (_, _, true, _) => {},
                    (false, false, false, _) => {},
                };
            },
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Termcastd>, message: TermcastdMessage) {
        match message {
            TermcastdMessage::CasterDisconnected(token) => {
            },
            TermcastdMessage::WatcherDisconnected(token) => {
                self.handle_disconnect(event_loop, token);
            },
        }
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
        casters: HashMap::new(),
        watchers: HashMap::new(),
        next_token_id: 2,
        number_watching: 0,
        number_casting: 0,
    };
    event_loop.run(&mut termcastd).unwrap();
}
