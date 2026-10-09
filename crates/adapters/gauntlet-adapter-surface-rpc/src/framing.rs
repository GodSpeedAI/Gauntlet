//! Bounded newline framing. A line longer than the limit is never buffered:
//! the reader refuses before it would grow the buffer past the limit.

use std::io::{self, BufRead};

/// The outcome of reading one frame from the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Frame {
    /// A line (terminator excluded) now sits in the caller's buffer.
    Line,
    /// The line holds more than `max` bytes. Nothing past the limit was kept.
    TooLong,
    /// The input is exhausted and no partial line is pending.
    End,
}

/// Reads one `\n`-terminated line into `out`, holding at most `max` bytes of
/// content. A final line without a terminator is returned as a line.
pub(crate) fn read_frame<R: BufRead>(
    input: &mut R,
    max: usize,
    out: &mut Vec<u8>,
) -> io::Result<Frame> {
    out.clear();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(if out.is_empty() {
                Frame::End
            } else {
                Frame::Line
            });
        }
        let (take, terminated) = match available.iter().position(|&byte| byte == b'\n') {
            Some(index) => (index, true),
            None => (available.len(), false),
        };
        if out.len() + take > max {
            return Ok(Frame::TooLong);
        }
        out.extend_from_slice(&available[..take]);
        if terminated {
            input.consume(take + 1);
            return Ok(Frame::Line);
        }
        let consumed = available.len();
        input.consume(consumed);
    }
}
