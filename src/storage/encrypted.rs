use crate::clipboard::types::ClipEntry;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use dirs::home_dir;
use keyring::Entry;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

// ── Constants ─────────────────────────────────────────────────────────────────

const KEYCHAIN_SERVICE: &str = "com.clipwallet.vault";
const KEYCHAIN_USER:    &str = "clipwallet-key";

/// Nonce is 12 bytes for AES-256-GCM
const NONCE_LEN: usize = 12;

// ── Key Management ────────────────────────────────────────────────────────────

/// Retrieve the AES key from macOS Keychain.
/// If no key exists yet, generate one, store it, and return it.
pub fn get_or_create_key() -> anyhow::Result<[u8; 32]> {
    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)?;

    let generate_and_save_key = |entry: &Entry| -> anyhow::Result<[u8; 32]> {
        let key_bytes: [u8; 32] = {
            let k = Aes256Gcm::generate_key(&mut OsRng);
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&k);
            arr
        };
        let hex = hex::encode(key_bytes);
        entry.set_password(&hex)?;
        info!("Generated new AES-256 key — stored in macOS Keychain");
        Ok(key_bytes)
    };

    match entry.get_password() {
        Ok(hex) => {
            // Decode key from hex string
            let bytes = hex::decode(&hex).map_err(|e| {
                anyhow::anyhow!("Keychain key failed to decode: {}", e)
            })?;
            
            // Check key length
            if bytes.len() != 32 {
                anyhow::bail!("Keychain key wrong length: expected 32 bytes, got {}", bytes.len());
            }
            
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            info!("Encryption key loaded from Keychain");
            Ok(key)
        }
        Err(_) => {
            // No key yet — generate a fresh key
            generate_and_save_key(&entry)
        }
    }
}

/// Delete the key from Keychain (used on uninstall)
pub fn delete_key() -> anyhow::Result<()> {
    let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)?;
    match entry.delete_password() {
        Ok(_)  => info!("Encryption key removed from Keychain"),
        Err(_) => info!("No Keychain key found — skipping"),
    }
    Ok(())
}

// ── Vault Directory ───────────────────────────────────────────────────────────

pub fn vault_dir() -> PathBuf {
    home_dir()
        .expect("No home dir")
        .join(".clipwallet")
        .join("vault")
}

// ── Encrypt / Decrypt ─────────────────────────────────────────────────────────

/// Encrypt a ClipEntry and return the ciphertext bytes.
/// Format: [ nonce (12 bytes) | ciphertext ]
pub fn encrypt_entry(entry: &ClipEntry) -> anyhow::Result<Vec<u8>> {
    let raw_key = get_or_create_key()?;
    let key     = Key::<Aes256Gcm>::from_slice(&raw_key);
    let cipher  = Aes256Gcm::new(key);
    let nonce   = Aes256Gcm::generate_nonce(&mut OsRng);

    // Serialise the entry to MessagePack first
    let plaintext = rmp_serde::to_vec(entry)?;

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    // Prepend nonce so we can recover it on decryption
    let mut output = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Decrypt bytes from the vault back into a ClipEntry.
/// Expects format: [ nonce (12 bytes) | ciphertext ]
pub fn decrypt_entry(bytes: &[u8]) -> anyhow::Result<ClipEntry> {
    if bytes.len() <= NONCE_LEN {
        anyhow::bail!("Ciphertext too short to contain nonce");
    }

    let raw_key = get_or_create_key()?;
    let key     = Key::<Aes256Gcm>::from_slice(&raw_key);
    let cipher  = Aes256Gcm::new(key);

    let (nonce_bytes, ciphertext) = bytes.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("Decryption failed — wrong key or corrupt data: {}", e))?;

    let entry = rmp_serde::from_slice::<ClipEntry>(&plaintext)?;
    Ok(entry)
}

// ── Vault File Operations ─────────────────────────────────────────────────────

/// Encrypt and save an entry to ~/.clipwallet/vault/<id>.vlt
pub fn save_to_vault(entry: &ClipEntry) -> anyhow::Result<PathBuf> {
    let dir = vault_dir();
    fs::create_dir_all(&dir)?;

    let ciphertext = encrypt_entry(entry)?;
    let path = dir.join(format!("{}.vlt", entry.id));

    // Atomic write — .tmp then rename
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &ciphertext)?;
    fs::rename(&tmp, &path)?;

    info!("Saved encrypted entry {} → {:?}", entry.id, path);
    Ok(path)
}

/// Load and decrypt a vault entry by its ID
pub fn load_from_vault(id: u64) -> anyhow::Result<ClipEntry> {
    let path = vault_dir().join(format!("{}.vlt", id));
    if !path.exists() {
        anyhow::bail!("Vault entry {} not found", id);
    }
    let bytes = fs::read(&path)?;
    let entry = decrypt_entry(&bytes)?;
    Ok(entry)
}

/// List all entry IDs currently in the vault
pub fn list_vault_ids() -> Vec<u64> {
    let dir = vault_dir();
    if !dir.exists() { return vec![]; }

    fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name();
                    let s = name.to_string_lossy();
                    if s.ends_with(".vlt") {
                        s.trim_end_matches(".vlt").parse::<u64>().ok()
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Delete a single vault entry by ID
pub fn delete_from_vault(id: u64) -> anyhow::Result<()> {
    let path = vault_dir().join(format!("{}.vlt", id));
    if path.exists() {
        fs::remove_file(&path)?;
        info!("Deleted vault entry {}", id);
    } else {
        warn!("Vault entry {} not found for deletion", id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Non-hermetic test: accesses real system Keychain. Run manually with `cargo test -- --ignored`"]
    fn test_keychain_get_or_create_and_corruption() {
        // Ensure starting state is clean
        let _ = delete_key();

        // 1. First call should generate a fresh key
        let key1 = get_or_create_key().expect("Failed to create key");
        assert_eq!(key1.len(), 32);

        // 2. Subsequent call should retrieve the same key
        let key2 = get_or_create_key().expect("Failed to retrieve key");
        assert_eq!(key1, key2);

        // 3. Set a corrupted (non-hex) key in Keychain
        let entry = Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).unwrap();
        entry.set_password("not-hexadecimal").unwrap();

        // 4. Retrieving should fail due to decoding error
        let err = get_or_create_key().unwrap_err();
        assert!(err.to_string().contains("failed to decode") || err.to_string().contains("decode"));

        // 5. Set a key with invalid length (e.g. 16 bytes instead of 32)
        let short_key = [0u8; 16];
        let short_hex = hex::encode(short_key);
        entry.set_password(&short_hex).unwrap();

        // 6. Retrieving should fail due to length mismatch
        let err2 = get_or_create_key().unwrap_err();
        assert!(err2.to_string().contains("wrong length") || err2.to_string().contains("length"));

        // Clean up
        delete_key().unwrap();
    }
}