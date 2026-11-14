//! Decrypts what a Chromium browser encrypts with its key in the macOS keychain (Chromium's
//! `os_crypt`): its cookies, and its saved passwords alike. The key is a password the browser
//! keeps as the generic password `<Name> Safe Storage`, which the app reads (decision 0012).

use aes::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};

/// The keychain item holding each Chromium browser's key, by the name `browsers` gives it. Arc's
/// and Dia's are named after them too.
pub fn service(browser: &str) -> Option<&'static str> {
    Some(match browser {
        "Arc" => "Arc Safe Storage",
        "Chrome" => "Chrome Safe Storage",
        "Edge" => "Microsoft Edge Safe Storage",
        "Brave" => "Brave Safe Storage",
        "Vivaldi" => "Vivaldi Safe Storage",
        "Opera" => "Opera Safe Storage",
        "Dia" => "Dia Safe Storage",
        _ => return None,
    })
}

/// A browser's key, derived from the password in its keychain item.
pub struct Key([u8; 16]);

impl Key {
    pub fn new(password: &[u8]) -> Self {
        Self(pbkdf2::pbkdf2_hmac_array::<sha1::Sha1, 16>(
            password,
            b"saltysalt",
            1003,
        ))
    }

    /// The clear text of a value encrypted with this key: `v10`, then AES-128-CBC with an IV of
    /// spaces. `None` when it is not one, or not this key's.
    pub fn decrypt(&self, encrypted: &[u8]) -> Option<Vec<u8>> {
        let data = encrypted.strip_prefix(b"v10")?;
        let decryptor = cbc::Decryptor::<aes::Aes128>::new(&self.0.into(), &[b' '; 16].into());
        let mut buffer = data.to_vec();
        let clear = decryptor.decrypt_padded::<Pkcs7>(&mut buffer).ok()?;
        Some(clear.to_vec())
    }

    /// Encrypts as the browser does, for tests.
    #[cfg(test)]
    pub fn encrypt(&self, clear: &[u8]) -> Vec<u8> {
        use aes::cipher::BlockModeEncrypt;
        let encryptor = cbc::Encryptor::<aes::Aes128>::new(&self.0.into(), &[b' '; 16].into());
        let mut buffer = [clear, &[0; 16]].concat();
        let encrypted = encryptor
            .encrypt_padded::<Pkcs7>(&mut buffer, clear.len())
            .unwrap();
        [b"v10".as_slice(), encrypted].concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypts_what_the_same_key_encrypted() {
        let key = Key::new(b"password");
        let encrypted = key.encrypt(b"session=1");
        assert_eq!(
            key.decrypt(&encrypted).as_deref(),
            Some(b"session=1".as_slice())
        );
        assert_ne!(
            Key::new(b"other").decrypt(&encrypted).as_deref(),
            Some(b"session=1".as_slice())
        );
        assert_eq!(key.decrypt(b"v11whatever"), None);
    }

    #[test]
    fn decrypts_what_chromium_encrypts() {
        // `openssl enc -aes-128-cbc -iv 2020…20` of "session=1", with the key Python's
        // `hashlib.pbkdf2_hmac("sha1", b"password", b"saltysalt", 1003, 16)` derives.
        let encrypted = [
            b"v10".as_slice(),
            &[
                0x10, 0xca, 0x22, 0x12, 0x29, 0xdc, 0x46, 0x90, 0xd7, 0xe5, 0x7a, 0xa1, 0xc2, 0x41,
                0xe7, 0x89,
            ],
        ]
        .concat();
        let clear = Key::new(b"password").decrypt(&encrypted);
        assert_eq!(clear.as_deref(), Some(b"session=1".as_slice()));
    }
}
