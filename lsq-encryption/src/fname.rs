//! encryption/decryption for file name
use md5::{Digest, Md5};
use unicode_normalization::UnicodeNormalization;

use crate::{Error, Result};

pub fn md5_digest(raw: &[u8]) -> [u8; 16] {
    let mut hasher = Md5::new();
    hasher.update(raw);
    let digest = hasher.finalize();
    digest.into()
}

pub fn md5_hexdigest(raw: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(raw);
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

pub fn encrypt_filename(fname: &str, encryption_key: &[u8; 32]) -> Result<String> {
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Nonce};

    if fname.is_empty() {
        return Err(Error::InvalidArg);
    }
    let fname = fname.nfc().collect::<String>();

    let nonce = Nonce::from([0u8; 12]);
    let cipher = ChaCha20Poly1305::new(encryption_key.into());

    let ciphertext = cipher
        .encrypt(&nonce, fname.as_bytes())
        .map_err(|_| Error::Encrypt)?;
    Ok("e.".to_string() + &hex::encode(ciphertext.as_slice()))
}

pub fn decrypt_filename(encrypted_fname: &str, encryption_key: &[u8; 32]) -> Result<String> {
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{ChaCha20Poly1305, Nonce};

    // e.<encrypted><16 byte auth_tag>
    // also reject empty fname
    if !encrypted_fname.starts_with("e.") || encrypted_fname.len() <= 36 {
        return Err(Error::InvalidArg);
    }

    let ciphertext =
        hex::decode(&encrypted_fname.as_bytes()[2..]).map_err(|_| Error::InvalidArg)?;
    let nonce = Nonce::from([0u8; 12]);
    let cipher = ChaCha20Poly1305::new(encryption_key.into());

    let plaintext = cipher
        .decrypt(&nonce, &ciphertext[..])
        .map_err(|_| Error::Decrypt)?;

    let plaintext = String::from_utf8(plaintext).unwrap();
    Ok(plaintext.nfc().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistency() {
        let key = "AGE-SECRET-KEY-1KZEJZYUPL49REUU985PT673PZWSA85HGSE2Z7ZPRRRQX9MJF8DXQRRA7J0";
        let raw_key = crate::to_raw_x25519_key(key).unwrap();
        let encrypted = encrypt_filename("pages/contents.md", &raw_key).unwrap();

        assert_eq!(
            encrypted,
            "e.6dd3a5340dd904be0e509ff824c32cdc1db108166bf58a4a8f3f5299651282ffca"
        );
    }

    #[test]
    fn test_fname_encryption() {
        let key = [8u8; 32];
        let fname = "logseq/config.edn";
        let encrypted_fname = encrypt_filename(fname, &key).unwrap();
        println!("{:?} => {:?}", fname, encrypted_fname);
        let decrypted_fname = decrypt_filename(&encrypted_fname, &key).unwrap();
        assert_eq!(fname, decrypted_fname);
    }

    #[test]
    fn test_fname_encryption_unicode_normalization() {
        let key = "AGE-SECRET-KEY-1KZEJZYUPL49REUU985PT673PZWSA85HGSE2Z7ZPRRRQX9MJF8DXQRRA7J0";
        let enc_key = crate::to_raw_x25519_key(key).unwrap();

        let s1 = "プ"; // 12501, 12442
        let s2 = "プ"; // 12503

        assert_ne!(s1, s2, "s1 != s2");

        let es1 = encrypt_filename(s1, &enc_key).unwrap();
        let es2 = encrypt_filename(s2, &enc_key).unwrap();

        assert_eq!(es1, es2, "es1 == es2");
    }

    #[test]
    fn test_fname_encryption_empty() {
        let key = [8u8; 32];
        let fname = "";
        assert!(encrypt_filename(fname, &key).is_err());
    }
}
