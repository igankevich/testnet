use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;

use bincode::error::DecodeError;
use mio::unix::SourceFd;
use mio::Interest;
use mio::Poll;
use mio::Token;

use crate::IpcEncodeDecode;
use crate::IpcMessage;

pub(crate) struct IpcClient {
    reader: BufReader<File>,
    writer: BufWriter<File>,
}

impl IpcClient {
    pub(crate) fn new(in_fd: OwnedFd, out_fd: OwnedFd) -> Self {
        Self {
            reader: BufReader::with_capacity(MAX_MESSAGE_SIZE, in_fd.into()),
            writer: BufWriter::with_capacity(MAX_MESSAGE_SIZE, out_fd.into()),
        }
    }

    pub(crate) fn fill_buf(&mut self) -> Result<(), std::io::Error> {
        self.reader.fill_buf()?;
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> Result<bool, std::io::Error> {
        self.writer.flush()?;
        Ok(self.writer.buffer().is_empty())
    }

    pub(crate) fn recv(&mut self) -> Result<Option<IpcMessage>, std::io::Error> {
        match IpcMessage::decode(&mut self.reader) {
            Ok(message) => Ok(Some(message)),
            Err(DecodeError::UnexpectedEnd { .. }) => Ok(None),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }

    pub(crate) fn send(&mut self, message: &IpcMessage) -> Result<(), std::io::Error> {
        message
            .encode(&mut self.writer)
            .map_err(std::io::Error::other)?;
        Ok(())
    }

    pub(crate) fn send_finalize(
        &mut self,
        writer_token: Token,
        poll: &mut Poll,
    ) -> Result<(), std::io::Error> {
        if !self.flush()? {
            poll.registry().reregister(
                &mut SourceFd(&self.writer.get_ref().as_raw_fd()),
                writer_token,
                Interest::WRITABLE,
            )?;
        }
        Ok(())
    }

    pub(crate) fn output_raw_fd(&self) -> RawFd {
        self.writer.get_ref().as_raw_fd()
    }
}

pub(crate) const MAX_MESSAGE_SIZE: usize = 4096 * 16;
