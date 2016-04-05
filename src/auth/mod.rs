use std::collections::HashMap;

use sodiumoxide::crypto::pwhash;


pub struct CasterAuth {
    logins: HashMap<String, pwhash::HashedPassword>,
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

        let password_bytes = password.as_bytes();
        let pwh = try!(pwhash::pwhash(password_bytes,
                                      pwhash::OPSLIMIT_INTERACTIVE,
                                      pwhash::MEMLIMIT_INTERACTIVE));

        let pwhash_entry = self.logins.entry(name).or_insert(pwh);
        if pwhash::pwhash_verify(&pwhash_entry, password_bytes) {
            Ok(())
        }
        else {
            Err(())
        }
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
