use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;

use nix::unistd::pipe;

/// Create a pair of file descriptors for inter-process communication.
///
/// The receiver is meant for the child process, while the sender is for the parent.
pub(crate) fn pipe_channel() -> Result<(PipeSender, RawPipeReceiver), std::io::Error> {
    let (pipe_in, pipe_out) = pipe()?;
    let receiver = RawPipeReceiver::new(pipe_in.as_raw_fd(), pipe_out.as_raw_fd());
    let sender = PipeSender::new(pipe_in, pipe_out);
    Ok((sender, receiver))
}

pub(crate) struct RawPipeReceiver {
    pub(crate) raw_fd_in: RawFd,
    pub(crate) raw_fd_out: RawFd,
}

impl RawPipeReceiver {
    fn new(raw_fd_in: RawFd, raw_fd_out: RawFd) -> Self {
        Self {
            raw_fd_in,
            raw_fd_out,
        }
    }
}

pub(crate) struct PipeReceiver {
    fd: OwnedFd,
}

impl PipeReceiver {
    pub(crate) fn new(fd_in: RawFd, fd_out: RawFd) -> Self {
        // drop sender
        unsafe { OwnedFd::from_raw_fd(fd_out) };
        Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd_in) },
        }
    }

    pub(crate) fn wait_until_closed(&self) -> Result<(), std::io::Error> {
        let mut buf = [0_u8; 1];
        nix::unistd::read(self.fd.as_raw_fd(), &mut buf)?;
        Ok(())
    }
}

impl From<RawPipeReceiver> for PipeReceiver {
    fn from(other: RawPipeReceiver) -> Self {
        Self::new(other.raw_fd_in, other.raw_fd_out)
    }
}

pub(crate) struct PipeSender {
    #[allow(dead_code)]
    fd_in: OwnedFd,
    #[allow(dead_code)]
    fd_out: OwnedFd,
}

impl PipeSender {
    pub(crate) fn new(fd_in: OwnedFd, fd_out: OwnedFd) -> Self {
        // drop receiver
        Self { fd_in, fd_out }
    }

    pub(crate) fn close(mut self) -> Result<(), std::io::Error> {
        nix::unistd::write(&mut self.fd_out, &[1])?;
        Ok(())
    }
}
