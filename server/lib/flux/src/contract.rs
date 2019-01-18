use std::io::{Error, Read, Write};
use byteorder::{BigEndian, ReadBytesExt};
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

    #[inline]
    pub fn write<W: Write>(&self, stream: W) -> Result<(), Error> {
        // Write out the private data into a temp buffer
//        {
//            let data_slice = &mut private_data[..];
//            data_slice.write_u64::<BigEndian>(user.id).unwrap();
//            data_slice.write_all(&client_key).unwrap();
//            data_slice.write_all(&server_key).unwrap();
//        }
        unimplemented!()
    }
}
