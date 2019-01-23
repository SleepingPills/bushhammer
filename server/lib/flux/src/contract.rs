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

pub const SECRET_KEY_SIZE: usize = 32;

pub mod key_serde {
    use super::SECRET_KEY_SIZE;
    use base64;
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::encode_config(bytes, base64::STANDARD_NO_PAD))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; SECRET_KEY_SIZE], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <&str>::deserialize(deserializer)?;
        let mut output = [0; SECRET_KEY_SIZE];
        base64::decode_config_slice(s, base64::STANDARD_NO_PAD, &mut output[..])
            .map_err(de::Error::custom)?;
        Ok(output)
    }
}
