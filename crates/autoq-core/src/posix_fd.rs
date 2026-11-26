#![cfg(target_os = "linux")]
//! RAII POSIX file-descriptor & socket wrapper isolating all Linux `libc` `unsafe` calls.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const AF_BLUETOOTH: libc::c_int = 31;
const BTPROTO_RFCOMM: libc::c_int = 3;

#[repr(C)]
struct SockAddrRc {
    rc_family: libc::sa_family_t,
    rc_bdaddr: [u8; 6],
    rc_channel: u8,
}

/// Safe RAII wrapper around `std::os::fd::OwnedFd` (closes descriptor automatically on drop).
pub(crate) struct PosixFd(OwnedFd);

impl PosixFd {
    pub fn open(path: &str, flags: libc::c_int) -> io::Result<Self> {
        let cpath = CString::new(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let raw = unsafe { libc::open(cpath.as_ptr(), flags) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a newly opened, valid file descriptor checked >= 0.
        let owned = unsafe { OwnedFd::from_raw_fd(raw) };
        Ok(Self(owned))
    }

    pub fn open_rfcomm_socket(timeout_ms: i32) -> io::Result<Self> {
        let raw = unsafe { libc::socket(AF_BLUETOOTH, libc::SOCK_STREAM, BTPROTO_RFCOMM) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `raw` is a newly created socket descriptor checked >= 0.
        let owned = unsafe { OwnedFd::from_raw_fd(raw) };
        let fd = Self(owned);
        let tv = libc::timeval {
            tv_sec: (timeout_ms / 1000) as libc::time_t,
            tv_usec: ((timeout_ms % 1000) * 1000) as libc::suseconds_t,
        };
        unsafe {
            libc::setsockopt(
                fd.0.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDTIMEO,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
            libc::setsockopt(
                fd.0.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &tv as *const _ as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
        }
        Ok(fd)
    }

    pub fn connect_rfcomm(&self, bdaddr: [u8; 6], channel: u8) -> io::Result<()> {
        let addr = SockAddrRc {
            rc_family: AF_BLUETOOTH as libc::sa_family_t,
            rc_bdaddr: bdaddr,
            rc_channel: channel,
        };
        let rc = unsafe {
            libc::connect(
                self.0.as_raw_fd(),
                &addr as *const SockAddrRc as *const libc::sockaddr,
                std::mem::size_of::<SockAddrRc>() as libc::socklen_t,
            )
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn configure_raw_serial_115200(&self) {
        let raw = self.0.as_raw_fd();
        unsafe {
            let mut tio: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(raw, &mut tio) == 0 {
                libc::cfmakeraw(&mut tio);
                libc::cfsetispeed(&mut tio, libc::B115200);
                libc::cfsetospeed(&mut tio, libc::B115200);
                tio.c_cflag |= libc::CLOCAL | libc::CREAD;
                libc::tcsetattr(raw, libc::TCSANOW, &tio);
            }
            libc::tcflush(raw, libc::TCIOFLUSH);
        }
    }

    pub fn wait_readable(&self, timeout_ms: i32) -> bool {
        let mut pfd = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        rc > 0 && (pfd.revents & libc::POLLIN) != 0
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let n = unsafe {
            libc::read(
                self.0.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    pub fn write(&self, data: &[u8]) -> io::Result<usize> {
        let n = unsafe {
            libc::write(
                self.0.as_raw_fd(),
                data.as_ptr() as *const libc::c_void,
                data.len(),
            )
        };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    pub fn send(&self, data: &[u8]) -> io::Result<usize> {
        let n = unsafe {
            libc::send(
                self.0.as_raw_fd(),
                data.as_ptr() as *const libc::c_void,
                data.len(),
                0,
            )
        };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        let n = unsafe {
            libc::recv(
                self.0.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
                0,
            )
        };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    pub fn ioctl_hid_set_feature(&self, data: &[u8]) -> io::Result<()> {
        if data.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty feature report"));
        }
        let req = ((3u32 << 30) | ((data.len() as u32) << 16) | ((b'H' as u32) << 8) | 0x06)
            as libc::Ioctl;
        let rc = unsafe { libc::ioctl(self.0.as_raw_fd(), req, data.as_ptr()) };
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn ioctl_hid_get_feature(&self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty feature buffer"));
        }
        let req = ((3u32 << 30) | ((buf.len() as u32) << 16) | ((b'H' as u32) << 8) | 0x07)
            as libc::Ioctl;
        let rc = unsafe { libc::ioctl(self.0.as_raw_fd(), req, buf.as_mut_ptr()) };
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(rc as usize)
        }
    }
}
