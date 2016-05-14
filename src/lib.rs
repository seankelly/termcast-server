extern crate chrono;
extern crate core;
extern crate mio;
#[macro_use]
extern crate log;
extern crate sodiumoxide;
extern crate toml;

pub mod config;

mod auth;
mod caster;
mod duration;
mod ring;
mod term;
mod watcher;

use chrono::{DateTime, UTC};
use mio::*;
use std::io::{Error, ErrorKind};
use std::io::Read;
use std::io::Write;
use mio::tcp::TcpListener;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::SocketAddr;

use auth::CasterAuth;
use caster::{Caster, CasterMenuEntry};
use duration::relative_duration_format;
use config::TermcastConfig;
use watcher::{Watcher, WatcherAction, WatcherState};


const CASTER: Token = Token(0);
const WATCHER: Token = Token(1);
const CASTERS_PER_SCREEN: usize = 16;
const MENU_CHOICES: [&'static str; 16] = ["a", "b", "c", "d", "e", "f", "g",
                                          "h", "i", "j", "k", "l", "m", "n",
                                          "o", "p"];


struct MenuView {
    motd: String,
    caster_entries: Vec<CasterMenuEntry>,
    total_watchers: usize,
}

struct Termcastd {
    listen_caster: TcpListener,
    listen_watcher: TcpListener,
    clients: HashMap<Token, Client>,
    watchers: HashMap<Token, Watcher>,
    casters: HashMap<Token, Caster>,
    caster_auth: CasterAuth,
    next_token_id: usize,
    motd: String,
}

pub struct TermcastServer {
    termcastd: Termcastd,
    config: TermcastConfig,
    event_loop: EventLoop<Termcastd>,
}


pub enum TermcastdMessage {
    CasterDisconnected(Token),
    WatcherDisconnected(Token),
    Quit,
}

#[derive(Clone, Copy, Debug)]
enum Client {
    Caster,
    Watcher,
}


impl MenuView {
    fn render(&self, offset: usize) -> (String, Option<usize>) {
        fn caster_menu_entry(now: &DateTime<UTC>, choice: &'static str,
                             caster: &CasterMenuEntry) -> String {
            format!(" {}) {} (idle {}, connected {}, {} watching, {} bytes)\r\n",
                    choice, caster.name(),
                    relative_duration_format(&now, caster.last_byte_received()),
                    relative_duration_format(&now, caster.connected_when()),
                    caster.num_watchers(),
                    caster.buffer_size())
        }

        let num_casters = self.caster_entries.len();
        // If the offset is too high, reset it to the last page.
        let actual_offset = if offset < num_casters {
            offset
        }
        else {
            let page_length = MENU_CHOICES.len();
            let pages = num_casters / page_length;
            if num_casters == 0 || num_casters % page_length != 0 {
                pages * page_length
            }
            else {
                (pages - 1) * page_length
            }
        };

        let menu_header = format!(
            concat!(
                "{}{}",
                "\r\n",
                " ## Termcast\r\n",
                " ## {} sessions available. {} watchers connected.\r\n\r\n",
            ),
            term::clear_screen(), term::reset_cursor(),
            num_casters, self.total_watchers);

        let mut menu = String::with_capacity(80*24);
        menu.push_str(&menu_header);

        let now = UTC::now();
        let caster_choices = self.caster_entries.iter()
                    .skip(offset)
                    .take(CASTERS_PER_SCREEN);
        for c in caster_choices.zip(&MENU_CHOICES) {
            let (caster, choice) = c;
            menu.push_str(&caster_menu_entry(&now, choice, caster));
        }

        let menu_footer = concat!(
            "\r\n",
            "Watch which session? ('q' quits)",
            " ",
        );
        menu.push_str(&menu_footer);

        if actual_offset != offset {
            return (menu, None);
        }
        else {
            return (menu, Some(actual_offset));
        }
    }

    fn get_offset_token(&self, offset: usize) -> Option<Token> {
        self.caster_entries.get(offset)
                           .map(|entry| entry.token())
    }
}

impl Termcastd {
    fn new(listen_caster: TcpListener, listen_watcher: TcpListener) -> Self {
        Termcastd {
            listen_caster: listen_caster,
            listen_watcher: listen_watcher,
            clients: HashMap::new(),
            casters: HashMap::new(),
            caster_auth: CasterAuth::new(),
            watchers: HashMap::new(),
            next_token_id: 2,
            motd: String::from(""),
        }
    }

    fn next_token(&mut self) -> Token {
        let token = Token(self.next_token_id);
        self.next_token_id += 1;
        return token;
    }

    fn menu_view(&self) -> MenuView {
        let valid_casters = self.casters.values()
            .filter_map(|c| c.menu_entry());

        let view = MenuView {
            motd: self.motd.clone(),
            caster_entries: valid_casters.collect(),
            total_watchers: self.watchers.len(),
        };
        return view;
    }

    // Section for Caster and Watcher functions.
    ////////////////////////////////////
    fn handle_disconnect(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) {
        if let Entry::Occupied(client) = self.clients.entry(token) {
            match client.get() {
                &Client::Caster => {
                    if let Entry::Occupied(caster_entry) = self.casters.entry(token) {
                        {
                            let caster = caster_entry.get();
                            let res = event_loop.deregister(caster.socket());
                            let channel = event_loop.channel();
                            // To not have to do a mutable borrow, send a message to
                            // reset these watchers back to the main menu. Everything
                            // will be dropped after the end of the match when the
                            // entry is removed.
                            for watcher in caster.each_watcher() {
                                let res = channel.send(TermcastdMessage::CasterDisconnected(watcher.token()));
                            }
                        }
                        caster_entry.remove();
                    }
                },
                &Client::Watcher => {
                    if let Entry::Occupied(watcher_entry) = self.watchers.entry(token) {
                        {
                            let watcher = watcher_entry.get();
                            let res = event_loop.deregister(watcher.sock());
                        }
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

    // Section for Caster functions.
    ////////////////////////////////////
    fn new_caster(&mut self, event_loop: &mut EventLoop<Termcastd>) {
        if let Ok(opt) = self.listen_caster.accept() {
            if let Some(sock) = opt {
                let token = self.next_token();
                let caster = Caster::new(token, sock);
                let res = event_loop.register_opt(
                    caster.socket(),
                    token,
                    EventSet::all(),
                    PollOpt::edge(),
                );
                if res.is_ok() {
                    let client = Client::Caster;
                    self.clients.insert(token, client);
                    self.casters.insert(token, caster);
                }
            }
        }
    }

    /// Wrapper function for handling caster input. The caster will be removed if an error is
    /// returned from the wrapped function. Any watchers of that caster will be reset back to the
    /// main menu.
    fn read_caster(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) {
        if let Err(watchers) = self.caster_input(event_loop, token) {
            for watcher in &watchers {
                self.reset_watcher(*watcher);
            }

            let _ = self.casters.remove(&token);
        }
    }

    /// Actual method to interface between the Caster method to parse the input and Termcastd. If
    /// there is an error, then the watchers for that caster will be grouped together and sent up
    /// the call chain to have them reset.
    fn caster_input(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token) -> Result<(), Vec<Token>> {
        if let Some(caster) = self.casters.get_mut(&token) {
            if let Err(_) = caster.input(&mut self.caster_auth) {
                // Assemble all of the watchers, stuff them in a vector, and send it up to have
                // those watchers reset to the main menu.
                let watchers = caster.each_watcher()
                    .map(|w| w.token()).collect();

                event_loop.deregister(caster.socket());

                return Err(watchers);
            }
        }
        else {
            // Got an event for a token with no matching socket.
            return Err(Vec::new());
        }
        Ok(())
    }

    // Section for Watcher functions.
    ////////////////////////////////////
    fn new_watcher(&mut self, event_loop: &mut EventLoop<Termcastd>) -> Result<(), Error> {
        match self.listen_watcher.accept() {
            Ok(Some(sock)) => {
                let token = self.next_token();
                let watcher = Watcher::new(token, sock);

                try!(event_loop.register_opt(
                    watcher.sock(),
                    token,
                    EventSet::all(),
                    PollOpt::edge(),
                ));

                self.clients.insert(token, Client::Watcher);
                self.watchers.insert(token, watcher);

                let menu_view = self.menu_view();

                let watcher_init = self.watchers.get_mut(&token)
                    .ok_or(Error::new(ErrorKind::NotFound, ""))
                    .and_then(|w| {
                        w.write(&term::disable_linemode())
                            .map_err(|_err| event_loop.deregister(w.sock()))
                            .map_err(|_| Error::new(ErrorKind::Other, ""))
                            .map(|_| w)
                    })
                    .and_then(|w| {
                        w.write(&term::disable_local_echo())
                            .map_err(|_err| event_loop.deregister(w.sock()))
                            .map_err(|_| Error::new(ErrorKind::Other, ""))
                            .map(|_| w)
                    })
                    .and_then(|w| {
                        w.state = WatcherState::MainMenu;
                        w.send_menu(&menu_view)
                            .map_err(|_err| event_loop.deregister(w.sock()))
                            .map_err(|_| Error::new(ErrorKind::Other, ""))
                            .map(|_| w)
                    });

                // Success does not need to return anything.
                watcher_init.map(|_| ())
            }
            Ok(None) => {
                Ok(())
            },
            Err(err) => {
                Err(err)
            },
        }
    }

    /// Wrapper function for when the casters structure needs to be modified.
    fn read_watcher(&mut self, token: Token) {
        match self.watcher_input(token) {
            Ok(WatcherAction::Exit) => {
                let _ = self.watchers.remove(&token);
            },
            Ok(_) => {},
            Err(_) => {}
        }
    }

    /// Handle actions affecting the watcher in this method. Actions that affect casters will be
    /// sent up the call chain.
    fn watcher_input(&mut self, token: Token) -> Result<WatcherAction, ()> {
        let menu_view = self.menu_view();
        if let Some(mut watcher) = self.watchers.get_mut(&token) {
            loop {
                match watcher.parse_input(&menu_view) {
                    // The watcher returns the overall offset. Check that offset points to a valid
                    // caster. If it does, move the watcher to watch that caster. If it does not,
                    // refresh the menu for that watcher.
                    WatcherAction::Watch(offset) => {
                        let caster_token = menu_view.get_offset_token(offset);
                        if caster_token.is_none() {
                            watcher.send_menu(&menu_view);
                            continue;
                        }
                        let caster = self.casters.get_mut(&caster_token.unwrap());
                        if caster.is_none() {
                            // Huh...
                            // TODO: Add log statement here.
                            watcher.send_menu(&menu_view);
                            continue;
                        }
                        let caster = caster.unwrap();
                        let watcherlite = watcher.caster_copy();
                        if watcherlite.is_err() {
                            watcher.send_menu(&menu_view);
                            continue;
                        }
                        let watcherlite = watcherlite.unwrap();
                        caster.add_watcher(watcherlite);
                        watcher.state = WatcherState::Watching(caster.token());
                    },
                    WatcherAction::StopWatching => {
                        if let WatcherState::Watching(caster_token) = watcher.state {
                            watcher.state = WatcherState::MainMenu;
                            if let Some(caster) = self.casters.get_mut(&caster_token) {
                                caster.remove_watcher(watcher.token());
                            }
                            else {
                                // Huh...
                                // TODO: Add log statement here.
                                continue;
                            }
                            // FIXME: This is a now stale menu view.
                            watcher.send_menu(&menu_view);
                        }
                    },
                    WatcherAction::Exit => {
                        return Ok(WatcherAction::Exit);
                    },
                    WatcherAction::Nothing => { break },
                }
            }
        }
        else {
            // Got an event for a token with no matching socket.
        }

        Ok(WatcherAction::Nothing)
    }

    fn reset_watcher(&mut self, token: Token) {
        let menu_view = self.menu_view();
        self.watchers.get_mut(&token)
                     .and_then(|w| {
                         w.state = WatcherState::MainMenu;
                         w.send_menu(&menu_view).err()
                     });
    }
}

impl Handler for Termcastd {
    type Timeout = ();
    type Message = TermcastdMessage;

    fn ready(&mut self, event_loop: &mut EventLoop<Termcastd>, token: Token, event: EventSet) {
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
                match (event.is_readable(), event.is_hup(), event.is_error(), client) {
                    (true, false, false, Client::Caster) => {
                        self.read_caster(event_loop, token);
                    },
                    (true, false, false, Client::Watcher) => {
                        self.read_watcher(token);
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
                self.reset_watcher(token);
            },
            TermcastdMessage::WatcherDisconnected(token) => {
                self.handle_disconnect(event_loop, token);
            },
            TermcastdMessage::Quit => {
                event_loop.shutdown();
            }
        }
    }
}


impl TermcastServer {
    pub fn new(config: TermcastConfig) -> Result<Self, Error> {
        let listen_caster = try!(TcpListener::bind(&config.caster));
        let listen_watcher = try!(TcpListener::bind(&config.watcher));
        let termcastd = Termcastd::new(listen_caster, listen_watcher);
        let mut event_loop = EventLoop::new().unwrap();
        event_loop.register(&termcastd.listen_caster, CASTER).unwrap();
        event_loop.register(&termcastd.listen_watcher, WATCHER).unwrap();

        Ok(TermcastServer {
            termcastd: termcastd,
            config: config,
            event_loop: event_loop,
        })
    }

    pub fn run(&mut self) {
        self.event_loop.run(&mut self.termcastd).unwrap();
    }

    pub fn get_channel(&mut self) -> Sender<TermcastdMessage> {
        self.event_loop.channel()
    }

    pub fn get_socket_addrs(&self) -> Result<(SocketAddr, SocketAddr), Error> {
        let caster_addr = try!(self.termcastd.listen_caster.local_addr());
        let watcher_addr = try!(self.termcastd.listen_watcher.local_addr());
        Ok((caster_addr, watcher_addr))
    }
}
