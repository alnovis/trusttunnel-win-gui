//! At-rest encryption of the settings file.
//!
//! The password is NEVER stored. The file holds only a random salt, the Argon2
//! parameters, a random nonce, and the AEAD ciphertext. A wrong password fails
//! the Poly1305 tag on decrypt -- the tag itself is the password verifier, so
//! no separate hash is kept.
//!
//! password + salt --Argon2id--> 32-byte key --XChaCha20-Poly1305--> ciphertext
//!
//! Pure Rust (argon2 / chacha20poly1305 / getrandom / zeroize): no C deps, so
//! it cross-compiles for Windows 7.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"TTS1";
const VERSION: u8 = 1;
const KDF_ARGON2ID: u8 = 1;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
const HEADER_LEN: usize = 4 + 1 + 1 + 12 + SALT_LEN + NONCE_LEN; // magic+ver+kdf+params(3*u32)+salt+nonce

/// Argon2id cost parameters, stored in the file so they can be raised later
/// without breaking existing vaults.
#[derive(Debug, Clone, Copy)]
pub struct KdfParams {
    pub m_cost: u32, // KiB
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        // ~64 MiB, 3 passes: expensive enough to slow offline brute force of a
        // copied file, fast enough for an interactive unlock (~sub-second).
        Self {
            m_cost: 65536,
            t_cost: 3,
            p_cost: 1,
        }
    }
}

/// An unlocked vault: the derived key plus the salt/params needed to re-seal on
/// save. Holds live key material -- zeroized on drop.
pub struct Vault {
    key: Zeroizing<[u8; KEY_LEN]>,
    salt: [u8; SALT_LEN],
    params: KdfParams,
}

impl Vault {
    /// First run: pick a fresh salt, derive the key from `password`.
    pub fn create(password: &str) -> Result<Vault, String> {
        let params = KdfParams::default();
        let mut salt = [0u8; SALT_LEN];
        rand_bytes(&mut salt)?;
        let key = derive_key(password, &salt, &params)?;
        Ok(Vault { key, salt, params })
    }

    /// Open an existing blob with `password`; returns the vault and the
    /// decrypted plaintext. A wrong password yields an error (tag mismatch).
    pub fn open(blob: &[u8], password: &str) -> Result<(Vault, Vec<u8>), String> {
        if blob.len() < HEADER_LEN {
            return Err("settings file is corrupt (too short)".into());
        }
        if &blob[0..4] != MAGIC {
            return Err("settings file is corrupt (bad magic)".into());
        }
        if blob[4] != VERSION || blob[5] != KDF_ARGON2ID {
            return Err("unsupported settings file version".into());
        }
        let m_cost = u32::from_le_bytes(blob[6..10].try_into().unwrap());
        let t_cost = u32::from_le_bytes(blob[10..14].try_into().unwrap());
        let p_cost = u32::from_le_bytes(blob[14..18].try_into().unwrap());
        let params = KdfParams {
            m_cost,
            t_cost,
            p_cost,
        };
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&blob[18..18 + SALT_LEN]);
        let nonce_off = 18 + SALT_LEN;
        let ct_off = nonce_off + NONCE_LEN;
        let nonce = &blob[nonce_off..ct_off];
        let ciphertext = &blob[ct_off..];

        let key = derive_key(password, &salt, &params)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(key.as_slice()).map_err(|_| "key setup failed")?;
        let plaintext = cipher
            .decrypt(XNonce::from_slice(nonce), ciphertext)
            .map_err(|_| "wrong password".to_string())?;

        Ok((Vault { key, salt, params }, plaintext))
    }

    /// Serialize `plaintext` into a fresh blob (new random nonce each time).
    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; NONCE_LEN];
        rand_bytes(&mut nonce)?;
        let cipher = XChaCha20Poly1305::new_from_slice(self.key.as_slice())
            .map_err(|_| "key setup failed")?;
        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext)
            .map_err(|_| "encrypt failed")?;

        let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(KDF_ARGON2ID);
        out.extend_from_slice(&self.params.m_cost.to_le_bytes());
        out.extend_from_slice(&self.params.t_cost.to_le_bytes());
        out.extend_from_slice(&self.params.p_cost.to_le_bytes());
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

}

fn derive_key(
    password: &str,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    let p = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(KEY_LEN))
        .map_err(|e| format!("bad kdf params: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password.as_bytes(), salt, key.as_mut_slice())
        .map_err(|e| format!("key derivation failed: {e}"))?;
    Ok(key)
}

fn rand_bytes(buf: &mut [u8]) -> Result<(), String> {
    getrandom::getrandom(buf).map_err(|e| format!("rng failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cheap params so tests stay fast.
    fn fast() -> Vault {
        let params = KdfParams {
            m_cost: 256,
            t_cost: 1,
            p_cost: 1,
        };
        let salt = [7u8; SALT_LEN];
        let key = derive_key("pw", &salt, &params).unwrap();
        Vault { key, salt, params }
    }

    #[test]
    fn roundtrip() {
        let v = fast();
        let blob = v.seal(b"hello secret").unwrap();
        // Re-open with the same password (salt/params come from the blob).
        let (_v2, pt) = Vault::open(&blob, "pw").unwrap();
        assert_eq!(pt, b"hello secret");
    }

    #[test]
    fn wrong_password_fails() {
        let v = fast();
        let blob = v.seal(b"data").unwrap();
        // Not unwrap_err(): that would require Vault: Debug (it holds key bytes).
        let err = match Vault::open(&blob, "nope") {
            Err(e) => e,
            Ok(_) => panic!("wrong password should not open"),
        };
        assert!(err.contains("wrong password"));
    }

    #[test]
    fn tamper_fails() {
        let v = fast();
        let mut blob = v.seal(b"data").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xFF; // flip a ciphertext byte
        assert!(Vault::open(&blob, "pw").is_err());
    }

    #[test]
    fn full_create_open_cycle() {
        let v = Vault::create("correct horse").unwrap();
        let blob = v.seal(b"cfg").unwrap();
        let (_v2, pt) = Vault::open(&blob, "correct horse").unwrap();
        assert_eq!(pt, b"cfg");
        assert!(Vault::open(&blob, "wrong").is_err());
    }
}
