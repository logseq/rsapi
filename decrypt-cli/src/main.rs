use clap::{value_parser, Arg, Command};
use std::fs;
use std::io;
use std::path;
use std::result::Result;
#[derive(Debug)]
struct Keys {
    encrypted_secret_key: String,
    _public_key: String,
}

fn read_keys<P: AsRef<path::Path>>(path: P) -> Keys {
    let file_content =
        fs::read_to_string(&path).expect(&format!("Unable to read file: {:?}", path.as_ref()));
    let edn = edn_format::parse_str(&file_content).expect("Failed to read keys-file as edn");
    match edn {
        edn_format::Value::Map(map) => {
            let edn_format::Value::String(encrypted_secret_key) = map
                .get(&edn_format::Value::String(
                    "encrypted-private-key".to_string(),
                ))
                .expect("Failed to get encrypted-private-key")
            else {
                panic!("encrypted_secret_key")
            };
            let edn_format::Value::String(public_key) = map
                .get(&edn_format::Value::String("public-key".to_string()))
                .expect("Failed to get public-key")
            else {
                panic!("public_key")
            };
            Keys {
                encrypted_secret_key: encrypted_secret_key.to_owned(),
                _public_key: public_key.to_owned(),
            }
        }
        _ => {
            panic!("Failed to parse edn")
        }
    }
}

fn decrypt_file<P: AsRef<path::Path>>(
    path: P,
    secret_key: &str,
    filename_encrypt_key: [u8; 32],
    dst: P,
) {
    let file_content = fs::read(&path).unwrap();
    let decrypted_content = lsq_encryption::decrypt_with_x25519(secret_key, &file_content)
        .expect(&format!("Failed to decrypt: {:?}", path.as_ref()));
    let mut dst_path = path::PathBuf::from(dst.as_ref());
    let filename = path
        .as_ref()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let decrypted_filename =
        lsq_encryption::decrypt_filename(&filename, &filename_encrypt_key).unwrap_or(filename);
    dst_path.push(decrypted_filename);
    let parent = dst_path.parent();
    if parent.is_some() {
        fs::create_dir_all(parent.unwrap())
            .expect(&format!("Failed to create dir: {:?}", parent.unwrap()));
    }
    fs::write(&dst_path, decrypted_content).expect(&format!("Failed to write: {:?}", dst_path));
    println!("Generated {:?}", dst_path);
}

fn main() -> Result<(), io::Error> {
    let matches = Command::new("decrypt cli")
        .arg(
            Arg::new("password")
                .help("graph password")
                .long("pwd")
                .required(true),
        )
        .arg(
            Arg::new("dir")
                .help("graph data dir path")
                .long("dir")
                .value_name("PATH")
                .required(true)
                .value_parser(value_parser!(path::PathBuf)),
        )
        .arg(
            Arg::new("dst")
                .help("dir to store decrypted data")
                .long("dst")
                .value_name("PATH")
                .default_value("./decrypted")
                .value_parser(value_parser!(path::PathBuf)),
        )
        .get_matches();
    let passwd = matches.get_one::<String>("password").unwrap();
    let dir = matches.get_one::<path::PathBuf>("dir").unwrap();
    let dst = matches.get_one::<path::PathBuf>("dst").unwrap();
    let mut pathbuf = path::PathBuf::from(dir);
    pathbuf.push("keys.edn");
    let keys_edn_path = pathbuf.to_str().unwrap();
    let keys = read_keys(keys_edn_path);
    println!("keys.edn: {:?}", &keys);
    let secret_key_ = lsq_encryption::decrypt_with_user_passphrase(
        &passwd,
        &keys.encrypted_secret_key.as_bytes(),
    )
    .expect("Failed to decrypt secret_key, wrong password");
    let secret_key = std::str::from_utf8(&secret_key_).unwrap();
    let filename_encrypt_key =
        lsq_encryption::to_raw_x25519_key(secret_key).expect("Failed to get filename encrypt key");
    println!("secret key: {}", secret_key);
    println!("dst dir: {:?}", dst);
    fs::create_dir_all(dst).expect(&format!("Failed to create dir: {:?}", dst));
    let entries = fs::read_dir(dir).expect(&format!("Failed to read dir: {:?}", dir));
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() && path.file_name().unwrap() != "keys.edn" {
                    decrypt_file(path, secret_key, filename_encrypt_key, dst.clone());
                }
            }
            _ => {}
        }
    }

    Ok(())
}
