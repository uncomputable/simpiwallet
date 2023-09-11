use std::{fmt, io};

use elements_miniscript as miniscript;
use miniscript::bitcoin;
use miniscript::elements;

pub enum Error {
    Cli(lexopt::Error),
    Simplicity(simplicity::Error),
    HumanEncoding(simplicity::human_encoding::ErrorSet),
    Miniscript(miniscript::Error),
    Json(serde_json::Error),
    IO(io::Error),
    Bip32(bitcoin::bip32::Error),
    Rpc(jsonrpc::Error),
    Http(jsonrpc::simple_http::Error),
    NotEnoughFunds,
    CouldNotSatisfy,
    CouldNotParse(String),
    AssemblyOutOfBounds,
    UnknownAssembly(simplicity::Cmr),
}

impl Error {
    pub fn missing_value(value: &str) -> Self {
        lexopt::Error::MissingValue {
            option: Some(value.into()),
        }
        .into()
    }

    pub fn unknown_command(command: &str) -> Self {
        lexopt::Error::UnexpectedOption(command.into()).into()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Cli(error) => write!(f, "{}", error),
            Error::Simplicity(error) => write!(f, "{}", error),
            Error::HumanEncoding(error) => write!(f, "{}", error),
            Error::Miniscript(error) => write!(f, "{}", error),
            Error::Json(error) => write!(f, "{}", error),
            Error::IO(error) => write!(f, "{}", error),
            Error::Bip32(error) => write!(f, "{}", error),
            Error::Rpc(error) => write!(f, "{}", error),
            Error::Http(error) => write!(f, "{}", error),
            Error::NotEnoughFunds => write!(f, "Not enough funds"),
            Error::CouldNotSatisfy => write!(f, "Could not satisfy"),
            Error::CouldNotParse(error) => write!(f, "Could not parse: {}", error),
            Error::AssemblyOutOfBounds => write!(f, "Assembly fragment is out of bounds"),
            Error::UnknownAssembly(cmr) => {
                write!(f, "Unknown assembly fragment (not imported): {}", cmr)
            }
        }
    }
}

impl From<lexopt::Error> for Error {
    fn from(error: lexopt::Error) -> Self {
        Error::Cli(error)
    }
}

impl From<simplicity::Error> for Error {
    fn from(error: simplicity::Error) -> Self {
        Error::Simplicity(error)
    }
}

impl From<simplicity::human_encoding::ErrorSet> for Error {
    fn from(error: simplicity::human_encoding::ErrorSet) -> Self {
        Error::HumanEncoding(error)
    }
}

impl From<elements_miniscript::Error> for Error {
    fn from(error: elements_miniscript::Error) -> Self {
        Error::Miniscript(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Json(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IO(error)
    }
}

impl From<bitcoin::bip32::Error> for Error {
    fn from(error: bitcoin::bip32::Error) -> Self {
        Error::Bip32(error)
    }
}

impl From<jsonrpc::Error> for Error {
    fn from(error: jsonrpc::Error) -> Self {
        Error::Rpc(error)
    }
}

impl From<jsonrpc::simple_http::Error> for Error {
    fn from(error: jsonrpc::simple_http::Error) -> Self {
        Error::Http(error)
    }
}

impl From<elements::AddressError> for Error {
    fn from(error: elements::AddressError) -> Self {
        Error::CouldNotParse(error.to_string())
    }
}

impl From<bitcoin::amount::ParseAmountError> for Error {
    fn from(error: bitcoin::amount::ParseAmountError) -> Self {
        Error::CouldNotParse(error.to_string())
    }
}
