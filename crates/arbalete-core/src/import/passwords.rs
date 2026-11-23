//! Reads a Chromium browser profile's saved passwords, decrypted (decision 0015). The app hands
//! them to Arbalete's password manager.

use super::Copy;
use super::cookies::Error;
use super::safe_storage::Key;
use rusqlite::Connection;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

/// A password saved for a site's sign-in form.
#[derive(Clone, PartialEq, Eq)]
pub struct Password {
    /// The page of the form it was saved on.
    pub url: String,
    pub username: String,
    pub password: String,
}

/// Never the user name or the password.
impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Password({})", self.url)
    }
}

/// The passwords saved for websites' forms in the profile, on the device and for the browser's
/// account alike, each sign-in once. Left out: sites the user chose never to save for, HTTP
/// authentication, apps' and federated sign-ins, which have no password.
pub fn read(profile: &Path, key: &Key) -> Result<Vec<Password>, Error> {
    let (mut passwords, mut undecrypted) = (Vec::new(), 0);
    let mut seen = HashSet::new();
    for file in ["Login Data", "Login Data For Account"] {
        let Some(copy) = Copy::of(&profile.join(file)).map_err(Error::Io)? else {
            continue;
        };
        let db = Connection::open(copy.file()).map_err(Error::Sqlite)?;
        let mut query = db
            .prepare(
                "SELECT origin_url, username_value, password_value, signon_realm FROM logins
                 WHERE blacklisted_by_user = 0 AND scheme = 0 AND length(password_value) > 0
                 AND (origin_url LIKE 'https://%' OR origin_url LIKE 'http://%')",
            )
            .map_err(Error::Sqlite)?;
        let mut rows = query.query([]).map_err(Error::Sqlite)?;
        while let Some(row) = rows.next().map_err(Error::Sqlite)? {
            let (url, username, realm): (String, String, String) =
                (row.get(0)?, row.get(1)?, row.get(3)?);
            let encrypted: Vec<u8> = row.get(2)?;
            let Some(password) = key
                .decrypt(&encrypted)
                .and_then(|p| String::from_utf8(p).ok())
            else {
                undecrypted += 1;
                continue;
            };
            if !password.is_empty() && seen.insert((realm, username.clone())) {
                passwords.push(Password {
                    url,
                    username,
                    password,
                });
            }
        }
    }
    if passwords.is_empty() && undecrypted > 0 {
        return Err(Error::Key);
    }
    Ok(passwords)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::TestDir;

    /// The part of Chromium's `logins` table the import reads.
    const SCHEMA: &str = "
        CREATE TABLE logins (origin_url VARCHAR NOT NULL, action_url VARCHAR,
            username_element VARCHAR, username_value VARCHAR, password_element VARCHAR,
            password_value BLOB, signon_realm VARCHAR NOT NULL, date_created INTEGER NOT NULL,
            blacklisted_by_user INTEGER NOT NULL, scheme INTEGER NOT NULL,
            federation_url VARCHAR, id INTEGER PRIMARY KEY AUTOINCREMENT);";

    fn database(dir: &TestDir, file: &str) -> Connection {
        let db = Connection::open(dir.0.join(file)).unwrap();
        db.execute_batch(SCHEMA).unwrap();
        db
    }

    fn insert(db: &Connection, url: &str, realm: &str, user: &str, password: &[u8], never: bool) {
        db.execute(
            "INSERT INTO logins (origin_url, username_value, password_value, signon_realm,
                date_created, blacklisted_by_user, scheme) VALUES (?1, ?2, ?3, ?4, 1, ?5, 0)",
            rusqlite::params![url, user, password, realm, never],
        )
        .unwrap();
    }

    #[test]
    fn reads_the_passwords_of_both_stores() {
        let dir = TestDir::new("passwords");
        let key = Key::new(b"password");
        assert!(read(&dir.0, &key).unwrap().is_empty());
        let device = database(&dir, "Login Data");
        let secret = key.encrypt(b"hunter2");
        insert(
            &device,
            "https://a.test/login",
            "https://a.test/",
            "ann",
            &secret,
            false,
        );
        insert(
            &device,
            "https://never.test/",
            "https://never.test/",
            "",
            b"",
            true,
        );
        insert(
            &device,
            "android://hash@app/",
            "android://hash@app/",
            "a",
            &secret,
            false,
        );
        device
            .execute(
                "INSERT INTO logins (origin_url, username_value, password_value, signon_realm,
                    date_created, blacklisted_by_user, scheme)
                 VALUES ('https://basic.test/', 'b', ?1, 'https://basic.test/Realm', 1, 0, 1)",
                [&secret],
            )
            .unwrap();
        let account = database(&dir, "Login Data For Account");
        insert(
            &account,
            "https://a.test/",
            "https://a.test/",
            "ann",
            &secret,
            false,
        );
        insert(
            &account,
            "http://b.test/",
            "http://b.test/",
            "bob",
            &secret,
            false,
        );
        let passwords = read(&dir.0, &key).unwrap();
        let password = |url: &str, username: &str| Password {
            url: url.to_string(),
            username: username.to_string(),
            password: "hunter2".to_string(),
        };
        assert_eq!(
            passwords,
            [
                password("https://a.test/login", "ann"),
                password("http://b.test/", "bob")
            ]
        );
        assert_eq!(
            format!("{:?}", passwords[0]),
            "Password(https://a.test/login)"
        );
        assert!(matches!(read(&dir.0, &Key::new(b"other")), Err(Error::Key)));
    }
}
