use std::error::{self, Error as StdError};
use std::io;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    ClientNotFound,
    Mutex,
    Empty,
    IO(io::Error),
}

impl Error {
    pub fn description(&self) -> &str {
        match self {
            Error::ClientNotFound => "Client not found.",
            Error::IO(err) => err.description(),
            _ => "Unknown error.",
        }
    }

    pub fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Some error in AridaCore... todo: write normal dbg messages")
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IO(error)
    }
}
