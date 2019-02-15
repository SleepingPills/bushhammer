/// Shared infrastructure pertaining to the Server Session, that is an authenticated game server connected
/// to the master server.
pub mod server {
    use crate::encoding::base64;
    use serde::{de, Deserialize, Deserializer};
    use serde_derive::{Deserialize, Serialize};
    use std::ops::{Deref, DerefMut};

    const SESSION_KEY_SIZE: usize = 32;

    #[derive(Serialize, Deserialize, Clone)]
    pub struct SessionKey(
        #[serde(
            serialize_with = "base64::serialize",
            deserialize_with = "deserialize_b64_key"
        )]
        [u8; SESSION_KEY_SIZE],
    );

    #[inline]
    fn deserialize_b64_key<'de, D>(deserializer: D) -> Result<[u8; SESSION_KEY_SIZE], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <&str>::deserialize(deserializer)?;
        let decoded_raw = base64::decode(s).map_err(de::Error::custom)?;
        let mut decoded = [0u8; SESSION_KEY_SIZE];

        for (i, &byte) in decoded_raw.iter().enumerate() {
            decoded[i] = byte;
        }

        Ok(decoded)
    }

    impl SessionKey {
        pub const SIZE: usize = SESSION_KEY_SIZE;

        #[inline]
        pub fn new(key: [u8; Self::SIZE]) -> SessionKey {
            SessionKey(key)
        }
    }

    impl Deref for SessionKey {
        type Target = [u8; SessionKey::SIZE];

        #[inline]
        fn deref(&self) -> &[u8; SessionKey::SIZE] {
            &self.0
        }
    }

    impl DerefMut for SessionKey {
        #[inline]
        fn deref_mut(&mut self) -> &mut [u8; SessionKey::SIZE] {
            &mut self.0
        }
    }
}

/// Shared infrastructure pertaining to the User Session, that is an authenticated user connected to a
/// game server.
pub mod user {
    use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
    use std::io::{Error, Read, Write};
    use std::mem;

    /// Private data part (visible only to the server) of the connection token.
    pub struct PrivateData {
        pub user_id: u64,
        pub server_key: [u8; 32],
        pub client_key: [u8; 32],
    }

    impl PrivateData {
        pub const SIZE: usize = 72;

        /// Parse the supplied stream as a private data structure.
        #[inline]
        pub fn read<R: Read>(mut stream: R) -> Result<PrivateData, Error> {
            let mut instance = unsafe { mem::uninitialized::<PrivateData>() };

            instance.user_id = stream.read_u64::<BigEndian>()?;
            stream.read_exact(&mut instance.server_key)?;
            stream.read_exact(&mut instance.client_key)?;

            Ok(instance)
        }

        /// Write the private data to the supplied stream.
        #[inline]
        pub fn write<W: Write>(&self, mut stream: W) -> Result<(), Error> {
            stream.write_u64::<BigEndian>(self.user_id)?;
            stream.write_all(&self.client_key)?;
            stream.write_all(&self.server_key).map_err(Into::into)
        }

        /// Construct the additional encryption data.
        #[inline]
        pub fn additional_data(version: &[u8], protocol: u16, expires: u64) -> Result<[u8; 26], Error> {
            let mut additional_data = [0u8; 26];
            let mut additional_data_slice = &mut additional_data[..];

            additional_data_slice.write_all(version)?;
            additional_data_slice.write_u16::<LittleEndian>(protocol)?;
            additional_data_slice.write_u64::<LittleEndian>(expires)?;

            Ok(additional_data)
        }
    }
}
