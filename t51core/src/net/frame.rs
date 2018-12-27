use crate::net::buffer::Buffer;
use crate::net::crypto;
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io;
use std::io::{Read, Write};
use std::mem;

pub struct ConnectionToken {
    pub class: u8,
    pub version: [u8; 16],
    pub protocol: u64,
    pub created: u64,
    pub expires: u64,
    pub sequence: u64,
    pub data: PrivateData,
}

impl ConnectionToken {
    pub const CLASS: u8 = 0;
    pub const SIZE: usize = 49 + PrivateData::SIZE + crypto::MAC_SIZE;

    pub fn deserialize(buffer: &mut Buffer, secret_key: &[u8; 32]) -> io::Result<ConnectionToken> {
        // Bail out immediately in case there isn't enough data in the buffer.
        if buffer.len() < Self::SIZE {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        // Get the slice for the available data.
        let mut stream = buffer.read_slice();

        // Parse the data into the token structure.
        let mut instance = unsafe { mem::uninitialized::<ConnectionToken>() };

        if stream.read_u8()? != Self::CLASS {
            return Err(io::ErrorKind::InvalidData.into());
        }

        instance.class = Self::CLASS;
        stream.read_exact(&mut instance.version)?;
        instance.protocol = stream.read_u64::<BigEndian>()?;
        instance.created = stream.read_u64::<BigEndian>()?;
        instance.expires = stream.read_u64::<BigEndian>()?;
        instance.sequence = stream.read_u64::<BigEndian>()?;

        // Extract out the encrypted private data part.
        let mut plain = [0u8; PrivateData::SIZE];

        // Construct the additional data used for the encryption.
        let mut additional_data = [0u8; 32];
        {
            let mut additional_data_slice = &mut additional_data[..];

            additional_data_slice.write_all(&instance.version)?;
            additional_data_slice.write_u64::<LittleEndian>(instance.protocol)?;
            additional_data_slice.write_u64::<LittleEndian>(instance.created)?;
        }

        // Decrypt the cipher into the plain data.
        if !crypto::decrypt(
            &mut plain,
            &stream[..PrivateData::SIZE + crypto::MAC_SIZE],
            &additional_data,
            instance.sequence,
            &secret_key,
        ) {
            return Err(io::ErrorKind::InvalidData.into());
        }

        // Deserialize the private data part.
        instance.data = PrivateData::deserialize(&plain[..])?;

        buffer.move_head(Self::SIZE);
        Ok(instance)
    }
}

pub struct PrivateData {
    pub client_id: u64,
    pub server_key: [u8; 32],
    pub client_key: [u8; 32],
}

impl PrivateData {
    pub const SIZE: usize = 72;

    fn deserialize<R: Read>(mut stream: R) -> io::Result<PrivateData> {
        let mut instance = unsafe { mem::uninitialized::<PrivateData>() };

        instance.client_id = stream.read_u64::<BigEndian>()?;
        stream.read_exact(&mut instance.server_key)?;
        stream.read_exact(&mut instance.client_key)?;

        Ok(instance)
    }
}

pub struct Header {
    pub class: u8,
    pub sequence: u64,
    pub size: u16,
}

impl Header {
    pub const CLASS: u8 = 1;
    pub const SIZE: usize = 11;

    pub fn deserialize(buffer: &mut Buffer) -> io::Result<Header> {
        if buffer.len() < Self::SIZE {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        let mut stream = buffer.read_slice();

        if stream.read_u8()? != Self::CLASS {
            return Err(io::ErrorKind::InvalidData.into());
        }

        Ok(Header {
            class: Self::CLASS,
            sequence: stream.read_u64::<BigEndian>()?,
            size: stream.read_u16::<BigEndian>()?,
        })
    }
}
