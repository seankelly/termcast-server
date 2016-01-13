use mio::Token;
use mio::tcp::TcpStream;
use std::io::Error;
use std::io::Read;
use std::io::Write;

use super::MenuView;


pub struct Watcher {
    pub state: WatcherState,
    offset: usize,
    sock: TcpStream,
    input_buffer: [u8; 128],
    token: Token,
}

pub struct WatcherLite {
    sock: TcpStream,
    token: Token,
}

pub enum WatcherState {
    Connecting,
    Disconnecting,
    MainMenu,
    Watching(Token),
}

pub enum WatcherAction {
    Exit,
    Nothing,
    StopWatching,
    Watch(usize),
}


impl Watcher {
    pub fn new(token: Token, sock: TcpStream) -> Self {
        Watcher {
            offset: 0,
            sock: sock,
            input_buffer: [0; 128],
            token: token,
            state: WatcherState::Connecting,
        }
    }

    pub fn parse_input(&mut self, menu_view: &MenuView) -> WatcherAction {
        while let Ok(num_bytes) = self.sock.read(&mut self.input_buffer) {
            let each_byte = 0..num_bytes;
            for (_offset, byte) in each_byte.zip(self.input_buffer.iter()) {
                match self.state {
                    WatcherState::Watching(_) => {
                        // Pressing 'q' while watching returns the watcher to the main menu.
                        if *byte == b'q' {
                            // This will reset the state back to the main menu.
                            return WatcherAction::StopWatching;
                        }
                    },
                    WatcherState::MainMenu => {
                        match *byte {
                            b'a'...b'p' => {
                                let page_offset = (*byte - b'a') as usize;
                                let caster_offset = self.offset + page_offset;
                                return WatcherAction::Watch(caster_offset);
                            }
                            b'q' => {
                                self.state = WatcherState::Disconnecting;
                                return WatcherAction::Exit;
                            },
                            // Any other character, refresh the menu.
                            _ => {
                                // TODO: Replace rest of this block with the following line.
                                //self.send_menu(&menu_view);
                                let (menu, fixed_offset) = menu_view.render(self.offset);
                                if let Some(offset) = fixed_offset {
                                    self.offset = offset;
                                }
                                self.sock.write(&menu.as_bytes());
                            },
                        }
                    },
                    WatcherState::Connecting => {},
                    WatcherState::Disconnecting => { return WatcherAction::Nothing },
                }
            }
        }

        return WatcherAction::Nothing;
    }

    pub fn send_menu(&mut self, menu_view: &MenuView) -> Result<usize, Error> {
        let (menu, fixed_offset) = menu_view.render(self.offset);
        if let Some(offset) = fixed_offset {
            self.offset = offset;
        }
        self.sock.write(&menu.as_bytes())
    }

    pub fn caster_copy(&mut self) -> Result<WatcherLite, Error> {
        let socket = try!(self.sock.try_clone());
        let lite = WatcherLite {
            sock: socket,
            token: self.token,
        };
        Ok(lite)
    }

    pub fn sock(&self) -> &TcpStream {
        &self.sock
    }

    pub fn token(&self) -> Token {
        self.token
    }
}

impl Write for Watcher {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.sock.write(buf)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.sock.flush()
    }
}

impl WatcherLite {
    pub fn token(&self) -> Token {
        self.token
    }
}

impl Write for WatcherLite {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.sock.write(buf)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.sock.flush()
    }
}
