use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Write;

use bincode::decode_from_slice;
use bincode::encode_into_std_write;
use bincode::error::DecodeError;
use bincode::error::EncodeError;
use bincode::Decode;
use bincode::Encode;

use crate::MAX_MESSAGE_SIZE;

#[derive(Decode, Encode, Debug)]
pub(crate) enum IpcMessage {
    Send(BroadcastPayload),
    Receive,
    Wait,
}

pub(crate) type BroadcastPayload = Vec<u8>;

pub(crate) trait IpcEncodeDecode {
    fn encode(&self, writer: &mut impl Write) -> Result<(), EncodeError>;
    fn decode<R>(reader: &mut BufReader<R>) -> Result<Self, DecodeError>
    where
        Self: Sized,
        BufReader<R>: BufRead,
        R: Read;
}

impl<T: Encode + Decode> IpcEncodeDecode for T {
    fn encode(&self, writer: &mut impl Write) -> Result<(), EncodeError> {
        encode_into_std_write(self, writer, bincode_config())?;
        Ok(())
    }

    fn decode<R>(reader: &mut BufReader<R>) -> Result<Self, DecodeError>
    where
        BufReader<R>: BufRead,
        R: Read,
    {
        let (object, n): (Self, usize) = decode_from_slice(reader.buffer(), bincode_config())?;
        reader.consume(n);
        Ok(object)
    }
}

const fn bincode_config() -> bincode::config::Configuration<
    bincode::config::LittleEndian,
    bincode::config::Fixint,
    bincode::config::Limit<MAX_MESSAGE_SIZE>,
> {
    bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding()
        .with_limit::<MAX_MESSAGE_SIZE>()
}
