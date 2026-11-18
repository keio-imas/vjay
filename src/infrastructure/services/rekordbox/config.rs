use base64::Engine;
use blowfish::{
    Blowfish,
    cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vjay_application::ports::PortResult;

#[derive(Clone)]
pub struct RekordboxPaths {
    pub database: PathBuf,
    pub options: PathBuf,
}
impl RekordboxPaths {
    pub fn discover() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        #[cfg(target_os = "macos")]
        {
            Self {
                database: home.join("Library/Pioneer/rekordbox/master.db"),
                options: home.join(
                    "Library/Application Support/Pioneer/rekordboxAgent/storage/options.json",
                ),
            }
        }
        #[cfg(target_os = "windows")]
        {
            let root = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Roaming"));
            Self {
                database: root.join("Pioneer/rekordbox/master.db"),
                options: root.join("Pioneer/rekordboxAgent/storage/options.json"),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self {
                database: home.join(".Pioneer/rekordbox/master.db"),
                options: home.join(".Pioneer/rekordboxAgent/storage/options.json"),
            }
        }
    }
}
pub(super) fn credentials(options: &Path) -> PortResult<(PathBuf, String)> {
    let bytes = fs::read(options).map_err(|e| format!("Cannot read rekordbox options: {e}"))?;
    let options: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid rekordbox options JSON")?;
    let entries = options["options"]
        .as_array()
        .ok_or("Missing rekordbox options")?;
    let get = |name: &str| {
        entries
            .iter()
            .find(|v| v[0].as_str() == Some(name))
            .and_then(|v| v[1].as_str())
    };
    let path = PathBuf::from(get("db-path").ok_or("Database path is missing from options")?);
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(get("dp").ok_or("Database password is missing from options")?)
        .map_err(|_| "Invalid encrypted database password")?;
    if bytes.is_empty() || !bytes.len().is_multiple_of(8) {
        return Err("Invalid encrypted database password length".into());
    }
    let cipher: Blowfish =
        Blowfish::new_from_slice(b"ZOwUlUZYqe9Rdm6j").map_err(|_| "Invalid password cipher")?;
    for block in bytes.as_chunks_mut::<8>().0 {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    let padding = *bytes.last().ok_or("Empty password")? as usize;
    let padded = (1..=8).contains(&padding)
        && bytes[bytes.len() - padding..]
            .iter()
            .all(|b| *b as usize == padding);
    if padded {
        bytes.truncate(bytes.len() - padding);
    } else if bytes.len() != 64 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Err("Database password padding is invalid".into());
    }
    // Legacy rekordbox writes a full 64-byte hexadecimal key without padding.
    let password = String::from_utf8(bytes)
        .map_err(|_| "Database password is not UTF-8")?
        .trim()
        .to_owned();
    Ok((path, password))
}

#[cfg(test)]
mod tests {
    use super::*;
    use blowfish::cipher::BlockEncrypt;
    #[test]
    fn options_support_padded_and_legacy_unpadded_keys() {
        let expected = "0123456789abcdef".repeat(4);
        for padded in [false, true] {
            let mut encrypted = expected.as_bytes().to_vec();
            if padded {
                encrypted.extend_from_slice(&[8; 8]);
            }
            let cipher: Blowfish = Blowfish::new_from_slice(b"ZOwUlUZYqe9Rdm6j").unwrap();
            for block in encrypted.as_chunks_mut::<8>().0 {
                cipher.encrypt_block(GenericArray::from_mut_slice(block));
            }
            let path = std::env::temp_dir()
                .join(format!("vjay-options-{}-{padded}.json", std::process::id()));
            let json = serde_json::json!({"options":[["db-path","/fixture/master.db"],["dp",base64::engine::general_purpose::STANDARD.encode(encrypted)]]});
            fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
            let (database, key) = credentials(&path).unwrap();
            assert_eq!(database, PathBuf::from("/fixture/master.db"));
            assert_eq!(key, expected);
            fs::remove_file(path).unwrap();
        }
    }
}
