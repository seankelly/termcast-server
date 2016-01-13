use chrono::{DateTime, UTC};
use core::slice::Iter;
use mio::Token;
use mio::tcp::TcpStream;
use std::io::{Error, ErrorKind};
use std::io::Read;
use std::io::Write;
use std::str;

use auth::CasterAuth;
use ring::RingBuffer;
use term;
use watcher::WatcherLite;

pub struct Caster {
    sock: TcpStream,
    token: Token,
    name: Option<String>,
    cast_buffer: RingBuffer,
    watchers: Vec<WatcherLite>,
    connected: DateTime<UTC>,
    last_byte_received: DateTime<UTC>,
}

pub struct CasterMenuEntry {
    token: Token,
    name: String,
    num_watchers: usize,
    buffer_size: usize,
    connected: DateTime<UTC>,
    last_byte_received: DateTime<UTC>,
}


enum AuthResults {
    InvalidLogin,
    InvalidName,
    MissingHello,
    NotEnoughParts,
    TooLong,
    TryAgain,
    Utf8Error,
}


impl Caster {
    pub fn new(token: Token, sock: TcpStream) -> Self {
        Caster {
            sock: sock,
            token: token,
            name: None,
            cast_buffer: RingBuffer::new(90_000),
            watchers: Vec::new(),
            connected: UTC::now(),
            last_byte_received: UTC::now(),
        }
    }

    pub fn input(&mut self, caster_auth: &mut CasterAuth) -> Result<(), ()> {
        let mut bytes_received = [0u8; 1024];
        loop {
            match self.sock.read(&mut bytes_received) {
                Ok(num_bytes) => {
                    self.last_byte_received = UTC::now();
                    // If a name is set then all bytes go straight to the watchers.
                    if self.name.is_some() {
                        self.relay_input(&bytes_received[..num_bytes]);
                    }
                    else {
                        let auth = self.handle_auth(&bytes_received[..num_bytes], caster_auth);
                        match auth {
                            Ok((offset, name)) => {
                                self.name = Some(name);
                                self.relay_input(&bytes_received[offset..num_bytes]);
                            },
                            // Not enough data sent so try again later.
                            Err(AuthResults::TryAgain) => {},
                            Err(_) => return Err(()),
                        }
                    }
                },
                Err(_e) => {
                    break;
                },
            }
        }

        Ok(())
    }

    pub fn menu_entry(&self) -> Option<CasterMenuEntry> {
        if let Some(ref name) = self.name {
            Some(CasterMenuEntry {
                token: self.token,
                name: name.clone(),
                num_watchers: self.watchers.len(),
                buffer_size: self.cast_buffer.len(),
                connected: self.connected,
                last_byte_received: self.last_byte_received,
            })
        }
        else {
            None
        }
    }

    pub fn add_watcher(&mut self, mut watcher: WatcherLite) -> Result<(), Error> {
        try!(watcher.write(term::clear_screen().as_bytes()));
        try!(watcher.write(term::reset_cursor().as_bytes()));
        try!(self.send_buffer(&mut watcher));
        self.watchers.push(watcher);
        Ok(())
    }

    pub fn remove_watcher(&mut self, token: Token) {
        let watcher_idx = self.watchers.iter().position(|w| w.token() == token);
        if let Some(idx) = watcher_idx {
            self.watchers.remove(idx);
        }
    }

    pub fn each_watcher(&self) -> Iter<WatcherLite> {
        self.watchers.iter()
    }

    pub fn socket(&self) -> &TcpStream {
        &self.sock
    }

    pub fn token(&self) -> Token {
        self.token
    }

    // The very first bytes sent should be in utf-8:
    //   hello <name> <password>
    fn handle_auth(&mut self, raw_input: &[u8], caster_auth: &mut CasterAuth) -> Result<(usize, String), AuthResults> {
        // Limit the buffer used for the authentication to 1024 bytes. This is to limit a DoS and
        // reduce the possibility of getting into an unknown state.
        let mut auth_buffer = [0; 1024];

        if raw_input.len() + self.cast_buffer.len() > auth_buffer.len() {
            return Err(AuthResults::TooLong);
        }

        for (idx, byte) in self.cast_buffer.iter().enumerate() {
            auth_buffer[idx] = byte;
        }

        let cb_len = self.cast_buffer.len();
        for (idx, byte) in raw_input.iter().enumerate() {
            auth_buffer[idx+cb_len] = *byte;
        }

        let auth_len = cb_len + raw_input.len();

        // Try to find a newline as that marks the end of the opening message.
        if let Some(newline_idx) = auth_buffer[..auth_len].iter().position(|b| *b == b'\n') {
            // Check for a single trailing \r and skip that too.
            let eol_idx = if newline_idx > 0 && auth_buffer[newline_idx-1] == b'\r' {
                newline_idx - 1
            }
            else {
                newline_idx
            };
            if let Ok(input) = str::from_utf8(&auth_buffer[..eol_idx]) {
                let parts: Vec<&str> = input.splitn(3, ' ').collect();

                if parts.len() < 2 {
                    return Err(AuthResults::NotEnoughParts);
                }
                else if parts[0] != "hello" {
                    return Err(AuthResults::MissingHello);
                }

                let name = parts[1];
                // Valid names must have a length and consist of characters/bytes greater than 32.
                // The splitn above prevents spaces and this check verifies no control codes are in
                // the name.
                if name.len() == 0 {
                    return Err(AuthResults::InvalidName);
                }
                else if name.as_bytes().iter().any(|b| *b < 32) {
                    return Err(AuthResults::InvalidName);
                }
                // Allow the password field to be empty. Default to the empty string.
                let password = if parts.len() >= 3 { parts[2] } else { "" };
                // Would like to use this but can't get the types to quite work out.
                //let password = parts.get(2).unwrap_or("");
                if let Ok(_) = caster_auth.login(&name, &password) {
                    // Determine if there are any remaining bytes in raw_input. Reset the
                    // cast_buffer to contain those bytes.
                    self.cast_buffer.clear();
                    let cast_byte_idx = newline_idx + 1;
                    let offset = if cast_byte_idx > cb_len {
                        // Possibly extra bytes left in the buffer. Need to return the index into
                        // raw_input so the calling function can relay those bytes.
                        cast_byte_idx - cb_len
                    }
                    else {
                        0
                    };
                    return Ok((offset, String::from(name)));
                }
                else {
                    return Err(AuthResults::InvalidLogin);
                }
            }
            else {
                return Err(AuthResults::Utf8Error);
            }
        }
        else {
            // No new line found so add all of the data to the ring buffer. Return an "error"
            // indicating not authenticated yet.
            let res = self.cast_buffer.add_no_wraparound(&raw_input);
            if res.is_err() {
                return Err(AuthResults::TooLong);
            }
            return Err(AuthResults::TryAgain);
        }
    }

    fn relay_input(&mut self, input: &[u8]) {
        self.cast_buffer.add(&input);
        for watcher in self.watchers.iter_mut() {
            let res = watcher.write(&input);
            // Need to notify the watcher has an error.
            if res.is_err() {
            }
        }
    }

    fn send_buffer(&self, watcher: &mut WatcherLite) -> Result<usize, Error> {
        let cast_buffer = self.cast_buffer.clone();
        watcher.write(&cast_buffer)
    }
}

impl CasterMenuEntry {
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn num_watchers(&self) -> usize {
        self.num_watchers
    }

    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn connected_when(&self) -> &DateTime<UTC> {
        &self.connected
    }

    pub fn last_byte_received(&self) -> &DateTime<UTC> {
        &self.last_byte_received
    }

    pub fn token(&self) -> Token {
        self.token
    }
}
