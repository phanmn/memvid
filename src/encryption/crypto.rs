use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::encryption::constants::{
    ARGON2_ITERATIONS, ARGON2_MEMORY_KIB, ARGON2_PARALLELISM, KEY_SIZE, NONCE_SIZE, SALT_SIZE,
};
use crate::encryption::error::EncryptionError;

pub fn derive_key(
    password: &[u8],
    salt: &[u8; SALT_SIZE],
) -> Result<[u8; KEY_SIZE], EncryptionError> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_SIZE),
    )
    .map_err(|e| EncryptionError::KeyDerivation {
        reason: e.to_string(),
    })?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_SIZE];
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| EncryptionError::KeyDerivation {
            reason: e.to_string(),
        })?;

    Ok(key)
}

pub fn encrypt(
    plaintext: &[u8],
    key: &[u8; KEY_SIZE],
    nonce: &[u8; NONCE_SIZE],
) -> Result<Vec<u8>, EncryptionError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| EncryptionError::CipherInit {
        reason: e.to_string(),
    })?;
    cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|e| EncryptionError::Encryption {
            reason: e.to_string(),
        })
}

pub fn decrypt(
    ciphertext: &[u8],
    key: &[u8; KEY_SIZE],
    nonce: &[u8; NONCE_SIZE],
) -> Result<Vec<u8>, EncryptionError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| EncryptionError::CipherInit {
        reason: e.to_string(),
    })?;
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|e| EncryptionError::Decryption {
            reason: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trip() {
        let password = b"test-password-123";
        let salt = [0xABu8; SALT_SIZE];
        let key = derive_key(password, &salt).unwrap();
        let nonce = [0x01u8; NONCE_SIZE];
        let plaintext = b"Hello, memvid encryption!";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        assert_ne!(&ciphertext[..], plaintext);

        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(&decrypted[..], plaintext);
    }

    #[test]
    fn wrong_key_fails_decrypt() {
        let salt = [0xABu8; SALT_SIZE];
        let key = derive_key(b"correct-password", &salt).unwrap();
        let wrong_key = derive_key(b"wrong-password", &salt).unwrap();
        let nonce = [0x01u8; NONCE_SIZE];
        let plaintext = b"secret data";

        let ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        let result = decrypt(&ciphertext, &wrong_key, &nonce);
        assert!(result.is_err());
    }

    #[test]
    fn empty_payload_round_trip() {
        let salt = [0x42u8; SALT_SIZE];
        let key = derive_key(b"password", &salt).unwrap();
        let nonce = [0x02u8; NONCE_SIZE];

        let ciphertext = encrypt(b"", &key, &nonce).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn large_payload_round_trip() {
        let salt = [0x99u8; SALT_SIZE];
        let key = derive_key(b"password", &salt).unwrap();
        let nonce = [0x03u8; NONCE_SIZE];
        let plaintext = vec![0xFFu8; 128 * 1024]; // 128KB

        let ciphertext = encrypt(&plaintext, &key, &nonce).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn key_derivation_deterministic() {
        let password = b"deterministic-test";
        let salt = [0x11u8; SALT_SIZE];

        let key1 = derive_key(password, &salt).unwrap();
        let key2 = derive_key(password, &salt).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn different_salts_different_keys() {
        let password = b"same-password";
        let salt_a = [0x11u8; SALT_SIZE];
        let salt_b = [0x22u8; SALT_SIZE];

        let key_a = derive_key(password, &salt_a).unwrap();
        let key_b = derive_key(password, &salt_b).unwrap();
        assert_ne!(key_a, key_b);
    }

    #[test]
    fn corrupted_ciphertext_fails() {
        let salt = [0xCCu8; SALT_SIZE];
        let key = derive_key(b"password", &salt).unwrap();
        let nonce = [0x04u8; NONCE_SIZE];
        let plaintext = b"tamper test data";

        let mut ciphertext = encrypt(plaintext, &key, &nonce).unwrap();
        if let Some(byte) = ciphertext.get_mut(0) {
            *byte ^= 0xFF;
        }
        let result = decrypt(&ciphertext, &key, &nonce);
        assert!(result.is_err());
    }
}
