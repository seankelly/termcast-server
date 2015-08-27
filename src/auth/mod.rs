extern crate crypto;

use self::crypto::digest::Digest;
use self::crypto::sha2::Sha256;
use std::collections::HashMap;


pub struct CasterAuth {
    logins: HashMap<String, [u8; 32]>,
}

impl CasterAuth {
    pub fn new() -> Self {
        CasterAuth {
            logins: HashMap::new(),
        }
    }

    // Given a name and password, check the list of accounts. If the name is not registered,
    // register it. If the name is registered, check the password; if the password does not match
    // then return an error.
    pub fn login(&mut self, name: &str, password: &str) -> Result<(), ()> {
        let name = String::from(name);

        // Hash the password to get everything to the same length.
        let mut hashed_password = [0; 32];
        CasterAuth::hash_password(password, &mut hashed_password);
        let hashed = hashed_password.clone();

        let hash_entry = self.logins.entry(name).or_insert(hashed);
        let mut diff = 0;
        for bytes in hashed_password.iter().zip(hash_entry) {
            let (byte_input, byte_entry) = bytes;
            diff |= *byte_input ^ *byte_entry;
        }

        if diff == 0 {
            Ok(())
        }
        else {
            Err(())
        }
    }

    fn hash_password(password: &str, output: &mut [u8]) {
        let mut sha256 = Sha256::new();
        sha256.input(password.as_bytes());
        sha256.result(output);
    }
}

#[cfg(test)]
mod tests {
    use super::CasterAuth;

    #[test]
    fn register() {
        let mut ca = CasterAuth::new();
        let name = "foo";
        let pass = "";
        assert!(ca.login(&name, &pass).is_ok(), "Can register new name.");
        assert_eq!(ca.logins.len(), 1);
    }

    #[test]
    fn register_three() {
        let mut ca = CasterAuth::new();
        let name = "foo1";
        let pass = "pass1";
        assert!(ca.login(&name, &pass).is_ok(), "Can register new name.");

        let name = "foo2";
        let pass = "pass2";
        assert!(ca.login(&name, &pass).is_ok(), "Can register new name.");

        let name = "foo3";
        let pass = "pass3";
        assert!(ca.login(&name, &pass).is_ok(), "Can register new name.");

        assert_eq!(ca.logins.len(), 3);
    }

    #[test]
    fn login() {
        let mut ca = CasterAuth::new();
        let name = "foo";
        let pass = "";
        ca.login(&name, &pass);

        assert!(ca.login(&name, &pass).is_ok(),
                "Logging in works.");
        assert_eq!(ca.logins.len(), 1);
    }

    #[test]
    fn login_fail() {
        let mut ca = CasterAuth::new();
        let name = "foo";
        let pass = "";
        ca.login(&name, &pass);

        let new_pass = "x";
        assert!(ca.login(&name, &new_pass).is_err(),
                "Login fail with wrong password.");
        assert_eq!(ca.logins.len(), 1);
    }
}
