use errors::Error;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::net::UnixStream;

#[derive(Debug)]
pub struct ConStream {
    unix: Option<UnixStream>,
    tcp: Option<TcpStream>,
}

impl ConStream {
    /// Create stream from TcpStream.
    pub fn new_tcp(stream: TcpStream) -> ConStream {
        ConStream {
            unix: None,
            tcp: Some(stream),
        }
    }

    /// Create stream from UnixStream.
    pub fn new_unix(stream: UnixStream) -> ConStream {
        ConStream {
            unix: Some(stream),
            tcp: None,
        }
    }

    /// Try to clone underlayed streams (if any).
    pub fn try_clone(&self) -> Result<ConStream, Error> {
        if let Some(ref s) = self.unix {
            return Ok(ConStream {
                unix: Some(s.try_clone()?),
                tcp: None,
            });
        }
        if let Some(ref s) = self.tcp {
            return Ok(ConStream {
                unix: None,
                tcp: Some(s.try_clone()?),
            });
        }
        Err(Error::Empty)
    }

    /// 'set_nonblocking()' impl.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<(), Error> {
        if let Some(ref s) = self.unix {
            s.set_nonblocking(nonblocking)?;
            return Ok(());
        }
        if let Some(ref s) = self.tcp {
            s.set_nonblocking(nonblocking)?;
            return Ok(());
        }
        Err(Error::Empty)
    }

    /// 'shutdown()' impl.
    pub fn shutdown(&self, how: Shutdown) -> Result<(), Error> {
        if let Some(ref s) = self.unix {
            s.shutdown(how)?;
            return Ok(());
        }
        if let Some(ref s) = self.tcp {
            s.shutdown(how)?;
            return Ok(());
        }
        Err(Error::Empty)
    }
}

impl Write for ConStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        if let Some(ref mut s) = self.unix {
            return s.write(buf);
        }
        if let Some(ref mut s) = self.tcp {
            return s.write(buf);
        }
        Ok(0)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        if let Some(ref mut s) = self.unix {
            return s.flush();
        }
        if let Some(ref mut s) = self.tcp {
            return s.flush();
        }
        Ok(())
    }
}

impl Read for ConStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        if let Some(ref mut s) = self.unix {
            return s.read(buf);
        }
        if let Some(ref mut s) = self.tcp {
            return s.read(buf);
        }
        Ok(0)
    }
}

// -----------------------------
// --- --- --- Tests --- --- ---
// -----------------------------
#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::thread;
    use std::time;
    use std::fs;
    use stream::*;

    fn setup_tcp_listener() {
        thread::spawn(move || {
            let _listener = TcpListener::bind("127.0.0.1:1234").unwrap();
            thread::sleep(time::Duration::from_millis(200));
        });
        thread::sleep(time::Duration::from_millis(100));
    }

    fn setup_unix_listener() {
        thread::spawn(move || {
            let _listener = match UnixListener::bind("/tmp/con-test.sock") {
                Ok(l) => l,
                Err(err) => {
                    if err.kind() == io::ErrorKind::AddrInUse {
                        fs::remove_file("/tmp/con-test.sock").unwrap();
                        UnixListener::bind("/tmp/con-test.sock").unwrap()
                    } else {
                        panic!(err);
                    }
                }
            };
            thread::sleep(time::Duration::from_millis(200));
        });
        thread::sleep(time::Duration::from_millis(100));
    }

    #[test]
    fn tcp_creating_and_cloning() {
        setup_tcp_listener();

        let tcp_stream = TcpStream::connect("127.0.0.1:1234").unwrap();
        let stream = ConStream::new_tcp(tcp_stream);
        match stream.tcp {
            Some(_) => assert!(true),
            None => assert!(false),
        }
        match stream.unix {
            Some(_) => assert!(false),
            None => assert!(true),
        }

        let cloned_stream = stream.try_clone().unwrap();
        match cloned_stream.tcp {
            Some(_) => assert!(true),
            None => assert!(false),
        }
        match cloned_stream.unix {
            Some(_) => assert!(false),
            None => assert!(true),
        }
    }

    #[test]
    fn unix_creating_and_cloning() {
        setup_unix_listener();

        let unix_stream = UnixStream::connect("/tmp/con-test.sock").unwrap();
        let stream = ConStream::new_unix(unix_stream);
        match stream.tcp {
            Some(_) => assert!(false),
            None => assert!(true),
        }
        match stream.unix {
            Some(_) => assert!(true),
            None => assert!(false),
        }

        let cloned_stream = stream.try_clone().unwrap();
        match cloned_stream.tcp {
            Some(_) => assert!(false),
            None => assert!(true),
        }
        match cloned_stream.unix {
            Some(_) => assert!(true),
            None => assert!(false),
        }
    }
}
