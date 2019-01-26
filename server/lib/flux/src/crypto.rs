use byteorder::{LittleEndian, WriteBytesExt};
use libsodium_sys;
use ctor::ctor;

pub const MAC_SIZE: usize = libsodium_sys::crypto_aead_chacha20poly1305_IETF_ABYTES as usize;
pub const KEY_SIZE: usize = libsodium_sys::crypto_aead_chacha20poly1305_IETF_KEYBYTES as usize;
pub const NONCE_SIZE: usize = libsodium_sys::crypto_aead_chacha20poly1305_IETF_NPUBBYTES as usize;

const NONCE_OFFSET: usize = NONCE_SIZE - 8;

/// Initialize the sodium infrastructure
#[ctor]
fn INIT_SODIUM() {
    unsafe {
        if libsodium_sys::sodium_init() < 0 {
            panic!("Cryptography initialization failed")
        }
    }
}

#[inline]
fn nonce_to_bytes(nonce: u64) -> [u8; NONCE_SIZE] {
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    (&mut nonce_bytes[NONCE_OFFSET..])
        .write_u64::<LittleEndian>(nonce)
        .expect("Error creating nonce");
    nonce_bytes
}

/// Encrypts the provided plain text into the cipher buffer. The encrypted message size will be the plain
/// text size plus the MAC size (24 bytes). The function will fail if the cipher slice is not large enough.
///
/// The additional data, nonce and key must match those used during encryption, the decryption will fail
/// otherwise.
#[inline]
pub fn encrypt(
    cipher: &mut [u8],
    plain: &[u8],
    additional_data: &[u8],
    nonce: u64,
    key: &[u8; KEY_SIZE],
) -> bool {
    let nonce_bytes = nonce_to_bytes(nonce);

    if cipher.len() != plain.len() + MAC_SIZE {
        panic!(
            "Encryption: cipher data length ({}) must be plain data length ({}) + MAC size ({})",
            cipher.len(),
            plain.len(),
            MAC_SIZE
        )
    }

    unsafe {
        let result = libsodium_sys::crypto_aead_chacha20poly1305_ietf_encrypt(
            cipher.as_mut_ptr(),
            ::std::ptr::null_mut(),
            plain.as_ptr(),
            plain.len() as u64,
            additional_data.as_ptr(),
            additional_data.len() as u64,
            ::std::ptr::null(),
            nonce_bytes.as_ptr(),
            key.as_ptr(),
        );

        result >= 0
    }
}

/// Decrypts the provided ciphertext into the plain buffer. The decoded message size is equal to the cipher
/// text length minus the MAC (24 bytes). The function will fail if the sizes do not match.
///
/// The additional data, nonce and key must match those used during encryption, the decryption will fail
/// otherwise.
#[inline]
pub fn decrypt(
    plain: &mut [u8],
    cipher: &[u8],
    additional_data: &[u8],
    nonce: u64,
    key: &[u8; KEY_SIZE],
) -> bool {
    let nonce_bytes = nonce_to_bytes(nonce);

    if cipher.len() != plain.len() + MAC_SIZE {
        panic!(
            "Decryption: cipher data length ({}) must be plain data length ({}) + MAC size ({})",
            cipher.len(),
            plain.len(),
            MAC_SIZE
        )
    }

    unsafe {
        let result = libsodium_sys::crypto_aead_chacha20poly1305_ietf_decrypt(
            plain.as_mut_ptr(),
            ::std::ptr::null_mut(),
            ::std::ptr::null_mut(),
            cipher.as_ptr(),
            cipher.len() as u64,
            additional_data.as_ptr(),
            additional_data.len() as u64,
            nonce_bytes.as_ptr(),
            key.as_ptr(),
        );

        result >= 0
    }
}

/// Fills the provided buffer with cryptographically secure random bytes
#[inline]
pub fn random_bytes(out: &mut [u8]) {
    unsafe {
        libsodium_sys::randombytes_buf(out.as_mut_ptr() as *mut ::std::ffi::c_void, out.len());
    }
}
