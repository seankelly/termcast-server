extern crate crypto;

use self::crypto::digest::Digest;
use self::crypto::sha2::Sha256;
use std::collections::HashMap;
use std::collections::hash_map::Entry;


pub struct CasterAuth {
    login: HashMap<String, String>,
}

impl CasterAuth {
    pub fn new() -> Self {
        CasterAuth {
            login: HashMap::new(),
        }
    }

    // Given a name and password, check the list of accounts. If the name is not registered,
    // register it. If the name is registered, check the password; if the password does not match
    // then return an error.
    pub fn login(&mut self, name: &String, password: &String) -> Result<(), ()> {
        let name = name.clone();

        // Hash the password to get everything to the same length.
        let hashed = CasterAuth::hash_password(password);
        let hashed_password = hashed.clone();

        let mut hash_entry = self.login.entry(name).or_insert(hashed);
        let mut diff = 0;
        for bytes in hashed_password.bytes().zip(hash_entry.bytes()) {
            let (byte_input, byte_entry) = bytes;
            diff |= byte_input ^ byte_entry;
        }

        if diff == 0 {
            Ok(())
        }
        else {
            Err(())
        }
    }

    fn hash_password(password: &String) -> String {
        let mut sha256 = Sha256::new();
        sha256.input(password.as_bytes());
        sha256.result_str()
    }
}
