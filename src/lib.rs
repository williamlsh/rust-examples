pub mod blocking_client;
pub mod client;

mod db;
use db::{Db, DbDropGuard};

mod frame;
pub use frame::Frame;

mod cmd;
pub use cmd::Command;

mod connection;
pub use connection::Connection;

mod parse;
use parse::{Parse, ParseError};

mod shutdown;
use shutdown::Shutdown;

pub mod server;

mod buffer;
pub use buffer::{buffer, Buffer};

/// Default port that a redis sever listens on.
///
/// Used if no port is specified.
pub const DEFAULT_PORT: u16 = 6379;

/// Error returned by most functions.
///
/// When writing a real application, one might want to consider a specialized
/// error handling crate or defining an error type as an `enum` of causes.
/// However, for our example, using a boxed `std::error::Error` is sufficient.
///
/// For performance reasons, boxing is avoided in any hot path. Fox example, in
/// `parse`, a custom error `enum` is defined. This is because the error is hit
/// and handled during normal execution when a partial frame is received on a socket.
/// `std::error::Error` is implemented for `parse::Error` which allows it to be converted
/// to `Box<dyn std::error::Error>`.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A specialized `Result` type for mini-redis operations.
///
/// This is defined as a convenience.
pub type Result<T> = std::result::Result<T, Error>;
