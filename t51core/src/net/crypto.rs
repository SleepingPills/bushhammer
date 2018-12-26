use byteorder::{LittleEndian, WriteBytesExt};
use libsodium_sys;

pub const MAC_SIZE: usize = libsodium_sys::crypto_aead_chacha20poly1305_IETF_ABYTES as usize;
pub const KEY_SIZE: usize = libsodium_sys::crypto_aead_chacha20poly1305_IETF_KEYBYTES as usize;
pub const NONCE_SIZE: usize = libsodium_sys::crypto_aead_chacha20poly1305_IETF_NPUBBYTES as usize;

const NONCE_OFFSET: usize = NONCE_SIZE - 8;

#[inline]
fn nonce_to_bytes(nonce: u64) -> [u8; NONCE_SIZE] {
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    (&mut nonce_bytes[NONCE_OFFSET..])
        .write_u64::<LittleEndian>(nonce)
        .expect("Error creating nonce");
    nonce_bytes
}

#[inline]
pub fn encrypt(
    cipher: &mut [u8],
    mac: &mut [u8; MAC_SIZE],
    plain: &[u8],
    additional_data: &[u8],
    nonce: u64,
    key: &[u8; KEY_SIZE],
) -> bool {
    let nonce_bytes = nonce_to_bytes(nonce);

    unsafe {
        let result = libsodium_sys::crypto_aead_chacha20poly1305_ietf_encrypt_detached(
            cipher.as_mut_ptr(),
            mac.as_mut_ptr(),
            ::std::ptr::null_mut(),
            plain.as_ptr(),
            plain.len() as u64,
            additional_data.as_ptr(),
            additional_data.len() as u64,
            ::std::ptr::null(),
            nonce_bytes.as_ptr(),
            key.as_ptr(),
        );
        result != -1
    }
}

#[inline]
pub fn decrypt(
    plain: &mut [u8],
    cipher: &[u8],
    mac: &[u8; MAC_SIZE],
    additional_data: &[u8],
    nonce: u64,
    key: &[u8; KEY_SIZE],
) -> bool {
    let nonce_bytes = nonce_to_bytes(nonce);

    unsafe {
        let result = libsodium_sys::crypto_aead_chacha20poly1305_ietf_decrypt_detached(
            plain.as_mut_ptr(),
            ::std::ptr::null_mut(),
            cipher.as_ptr(),
            cipher.len() as u64,
            mac.as_ptr(),
            additional_data.as_ptr(),
            additional_data.len() as u64,
            nonce_bytes.as_ptr(),
            key.as_ptr(),
        );
        result != -1
    }
}
