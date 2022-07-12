use std::time::Duration;

use bytes::Bytes;
use tracing::debug;

use crate::cmd::{Connection, Db, Frame, Parse, ParseError};

/// Set `key` to hold the string `value`.
///
/// if `key` already holds a value, it's overwritten, regardless of its type.
/// Any previous time to live associated with the key is discarded on successful
/// SET operation.
///
/// # Options
///
/// Currently, the following options are supported:
///
/// * EX `seconds` -- Set the specified expire time, in seconds.
/// * PX `milliseconds` -- Set the specified expire time, in milliseconds.
#[derive(Debug)]
pub struct Set {
    /// the lookup key.
    key: String,

    /// the value to be stored.
    value: Bytes,

    /// when to expire the key.
    expire: Option<Duration>,
}

impl Set {
    /// Create a new `Set` command which sets `key` to `value`.
    ///
    /// If `expire` is `Some`, the value should expire after the specified duration.
    pub fn new(key: impl ToString, value: Bytes, expire: Option<Duration>) -> Set {
        Set {
            key: key.to_string(),
            value,
            expire,
        }
    }

    /// Get the key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Get the value.
    pub fn value(&self) -> &Bytes {
        &self.value
    }

    // Get the expire.
    pub fn expire(&self) -> Option<Duration> {
        self.expire
    }

    /// Parse a `Set` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from the
    /// socket.
    ///
    /// The `SET` string has already been consumed.
    ///
    /// # Returns
    ///
    /// Returns the `Set` value on success. If the frame is malformed, `Err` is returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing at least 3 entries.
    ///
    /// ```text
    /// SET key value [EX seconds|PX milliseconds]
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Set> {
        use ParseError::EndOfStream;

        // Read the key to set. This is a required field.
        let key = parse.next_string()?;

        // Read the value to set. This is a required field.
        let value = parse.next_bytes()?;

        // The expiration is optional. If nothing else follows, then it's `None`.
        let mut expire = None;

        // Attempt to parse another string.
        match parse.next_string() {
            Ok(s) if s.to_uppercase() == "EX" => {
                // An expiration is specified in seconds. The next value is an integer.
                let secs = parse.next_int()?;
                expire = Some(Duration::from_secs(secs));
            }
            Ok(s) if s.to_uppercase() == "PX" => {
                // An expiration is specified in milliseconds. The next value is
                // an integer.
                let ms = parse.next_int()?;
                expire = Some(Duration::from_millis(ms));
            }
            // Currently, mini-redis does not support any of the other SET options.
            // An error here results in the connection being terminated. Other connections
            // will continue to operate normally.
            Ok(_) => return Err("currently `SET` only supports the expiration option".into()),
            // The `EndOfStream` error indicates there's no further data to parse.
            // In this case, it's a normal runtime situation and indicates there're no specified
            // `SET` options.
            Err(EndOfStream) => {}
            // All other errors are bubbled up, resulting in the connection being terminated.
            Err(err) => return Err(err.into()),
        }

        Ok(Set { key, value, expire })
    }

    /// Apply `Set` command to the specified `Db` instance.
    ///
    /// The response is written to `dst`. This's called by the server in order to execute
    /// a received command.
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> crate::Result<()> {
        // Set the value in the shared database state.
        db.set(self.key, self.value, self.expire);

        // Create a success response and write it to `dst`.
        let response = Frame::Simple("OK".to_string());
        debug!(?response);
        dst.write_frame(&response).await?;

        Ok(())
    }

    /// Converts the command into an equivalent `Frame`.
    ///
    /// This is called by the client when encoding a `Set` command to send
    /// to the server.
    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("set".as_bytes()));
        frame.push_bulk(Bytes::from(self.key.into_bytes()));
        frame.push_bulk(self.value);
        if let Some(ms) = self.expire {
            // Expirations in Redis protocol can be specified in two ways
            // 1. SET key value EX seconds
            // 2. SET key value PX milliseconds
            // We use the second option because it allows greater precision
            // and src/bin/cli.rs parses the expiration argument as milliseconds
            // in duration_from_ms_str()
            frame.push_bulk(Bytes::from("px".as_bytes()));
            frame.push_int(ms.as_millis() as u64);
        }
        frame
    }
}
