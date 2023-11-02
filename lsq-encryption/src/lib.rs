//! Age-encryption impl from rage-wasm
use std::mem;
use std::{
    io::{Read, Write},
    iter,
};

use age::{
    armor::{ArmoredReader, ArmoredWriter, Format},
    x25519, Decryptor, Encryptor,
};
use secrecy::{ExposeSecret, Secret};

pub use crate::error::{Error, Result};
pub use crate::fname::{decrypt_filename, encrypt_filename, md5_digest, md5_hexdigest};

pub mod error;
mod fname;

pub const AGE_MAGIC: &[u8] = b"age-encryption.org/";
pub const ARMORED_BEGIN_MARKER: &[u8] = b"-----BEGIN AGE ENCRYPTED FILE-----";

/// Generate (secret, public) key pairs
pub fn keygen() -> (String, String) {
    let secret = x25519::Identity::generate();
    let public = secret.to_public();
    (
        secret.to_string().expose_secret().clone(),
        public.to_string(),
    )
}

pub fn encrypt_with_x25519(public_key: &str, data: &[u8], armor: bool) -> Result<Box<[u8]>> {
    let key: x25519::Recipient = public_key.parse().map_err(|_| Error::ParseKey)?;
    let recipients = vec![Box::new(key) as Box<dyn age::Recipient + Send>];
    let encryptor = Encryptor::with_recipients(recipients).expect("not empty; qed");
    let mut output = vec![];
    let format = if armor {
        Format::AsciiArmor
    } else {
        Format::Binary
    };
    let armor = ArmoredWriter::wrap_output(&mut output, format)?;
    let mut writer = encryptor.wrap_output(armor).map_err(|_| Error::Encrypt)?;
    writer.write_all(data)?;
    writer.finish().and_then(|armor| armor.finish())?;
    Ok(output.into_boxed_slice())
}

pub fn decrypt_with_x25519(secret_key: &str, data: &[u8]) -> Result<Box<[u8]>> {
    let identity: x25519::Identity = secret_key.parse().map_err(|_| Error::ParseKey)?;
    let armor = ArmoredReader::new(data);
    let decryptor = match Decryptor::new(armor)? {
        Decryptor::Recipients(d) => d,
        _ => return Err(Error::Decrypt),
    };
    let mut decrypted = vec![];
    let mut reader = decryptor.decrypt(iter::once(&identity as &dyn age::Identity))?;
    reader.read_to_end(&mut decrypted)?;
    Ok(decrypted.into_boxed_slice())
}

pub fn encrypt_with_user_passphrase(
    passphrase: &str,
    data: &[u8],
    armor: bool,
) -> Result<Box<[u8]>> {
    let encryptor = Encryptor::with_user_passphrase(Secret::new(passphrase.to_owned()));
    let mut output = vec![];
    let format = if armor {
        Format::AsciiArmor
    } else {
        Format::Binary
    };
    let armor = ArmoredWriter::wrap_output(&mut output, format)?;
    let mut writer = encryptor.wrap_output(armor)?;
    writer.write_all(data)?;
    writer.finish().and_then(|armor| armor.finish())?;
    Ok(output.into_boxed_slice())
}

pub fn decrypt_with_user_passphrase(passphrase: &str, data: &[u8]) -> Result<Box<[u8]>> {
    let armor = ArmoredReader::new(data);
    let decryptor = match age::Decryptor::new(armor)? {
        age::Decryptor::Passphrase(d) => d,
        _ => return Err(Error::Decrypt),
    };
    let mut decrypted = vec![];
    // NOTE: use bigger max work factor here
    let mut reader = decryptor.decrypt(&Secret::new(passphrase.to_owned()), Some(100))?;
    reader.read_to_end(&mut decrypted)?;
    Ok(decrypted.into_boxed_slice())
}

/// Unsafe export private x25519 key as bytes
pub fn to_raw_x25519_key(secret_key: &str) -> Result<[u8; 32]> {
    use x25519_dalek::StaticSecret;

    let identity: x25519::Identity = secret_key.parse().map_err(|_| Error::ParseKey)?;
    let secret: &StaticSecret = unsafe { mem::transmute(&identity) };

    Ok(secret.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keygen() {
        let keys = keygen();
        println!("=> {:?}", keys);

        let raw = b"hello world";

        let encrypted = encrypt_with_x25519(&keys.1, raw, false).unwrap();
        let decrypted = decrypt_with_x25519(&keys.0, &encrypted).unwrap();

        assert_eq!(&*decrypted, &raw[..]);
    }

    #[test]
    fn encryption_size_expansion() {
        let keys = keygen();

        for i in [0, 1, 2, 3, 10, 100, 200, 300, 400, 500, 1000, 2000, 10000] {
            let content = vec![0u8; i];

            let encrypted = encrypt_with_x25519(&keys.1, &content, true).unwrap();
            let encrypted2 = encrypt_with_x25519(&keys.1, &content, false).unwrap();

            println!(
                "size={} before={} after={}",
                i,
                encrypted.len(),
                encrypted2.len()
            );
        }
    }
}
