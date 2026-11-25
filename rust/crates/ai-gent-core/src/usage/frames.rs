//! Bounded raw frame reading; typed validation remains with each journal.
use super::{MAX_RECORD, Result, corrupt, io_error, limit};
use std::io::{BufRead, Read, Seek, SeekFrom};

const BUFFER: usize = 64 * 1024;

/// Preserve whole-file size and final-newline error precedence before parsing frames.
/// Scan the same checked handle, then rewind; no journal-sized allocation.
pub(super) fn preflight<R: Read + Seek>(reader: &mut R, max: u64) -> Result<()> {
    let mut buffer = vec![0; BUFFER];
    let mut total = 0_u64;
    let mut last = None;
    loop {
        let remaining = max.checked_sub(total).ok_or_else(limit)?;
        let count =
            usize::try_from(remaining.saturating_add(1).min(BUFFER as u64)).map_err(|_| limit())?;
        let n = match reader.read(&mut buffer[..count]) {
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            other => other.map_err(io_error)?,
        };
        if n == 0 {
            break;
        }
        total = total.checked_add(n as u64).ok_or_else(limit)?;
        if total > max {
            return Err(limit());
        }
        last = Some(buffer[n - 1]);
    }
    if last.is_some_and(|byte| byte != b'\n') {
        return Err(corrupt());
    }
    reader.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(())
}

pub(super) struct Frames<R> {
    reader: R,
    line: Vec<u8>,
    total: u64,
    max: u64,
}
impl<R: BufRead> Frames<R> {
    #[cfg(test)]
    pub(super) fn new(reader: R) -> Self {
        Self::with_limit(reader, u64::MAX)
    }
    pub(super) fn with_limit(reader: R, max: u64) -> Self {
        Self {
            reader,
            line: Vec::new(),
            total: 0,
            max,
        }
    }

    /// Borrow one literal line, excluding newline, just as legacy decoders did.
    pub(super) fn next(&mut self) -> Result<Option<&[u8]>> {
        self.line.clear();
        loop {
            let bytes = match self.reader.fill_buf() {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                other => other.map_err(io_error)?,
            };
            if bytes.is_empty() {
                return if self.line.is_empty() {
                    Ok(None)
                } else {
                    Err(corrupt())
                };
            }
            let newline = bytes.iter().position(|byte| *byte == b'\n');
            let count = newline.unwrap_or(bytes.len());
            let consumed = count + usize::from(newline.is_some());
            self.total = self.total.checked_add(consumed as u64).ok_or_else(limit)?;
            if self.total > self.max {
                return Err(limit());
            }
            let length = self.line.len().checked_add(count).ok_or_else(limit)?;
            if length > MAX_RECORD {
                return Err(limit());
            }
            self.line
                .reserve_exact(length.saturating_sub(self.line.len()));
            self.line.extend_from_slice(&bytes[..count]);
            self.reader.consume(consumed);
            if newline.is_some() {
                return Ok(Some(&self.line));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, BufReader, Cursor};

    #[test]
    fn chunk_boundaries_preserve_literal_lines_and_empty_frames() {
        let input = "α\n\n{\"é\":1}\n".as_bytes();
        for capacity in 1..=input.len() {
            let mut frames = Frames::new(BufReader::with_capacity(capacity, Cursor::new(input)));
            assert_eq!(frames.next().unwrap().unwrap(), "α".as_bytes());
            assert_eq!(frames.next().unwrap().unwrap(), b"");
            assert_eq!(frames.next().unwrap().unwrap(), "{\"é\":1}".as_bytes());
            assert!(frames.next().unwrap().is_none());
        }
    }

    #[test]
    fn exact_legacy_line_limit_excludes_newline() {
        for length in [MAX_RECORD - 1, MAX_RECORD, MAX_RECORD + 1] {
            let mut input = vec![b' '; length];
            input.push(b'\n');
            let mut frames = Frames::new(BufReader::with_capacity(997, Cursor::new(input)));
            if length > MAX_RECORD {
                assert_eq!(frames.next().unwrap_err().code, "limit_exceeded");
            } else {
                assert_eq!(frames.next().unwrap().unwrap().len(), length);
                assert!(frames.next().unwrap().is_none());
            }
            assert!(frames.line.capacity() <= MAX_RECORD);
        }
    }

    #[test]
    fn generated_infinite_line_is_rejected_with_bounded_consumption() {
        let mut frames = Frames::new(BufReader::with_capacity(4096, io::repeat(b'x')));
        assert_eq!(frames.next().unwrap_err().code, "limit_exceeded");
        assert_eq!(frames.line.len(), MAX_RECORD);
        assert!(frames.line.capacity() <= MAX_RECORD);
        // Only one buffered chunk beyond the accepted maximum can have been requested.
        assert_eq!(frames.reader.buffer().len(), 4096);
    }

    #[test]
    fn preflight_limits_and_tail_checks_precede_parsing_and_rewind() {
        for (input, max, error) in [
            (b"bad\n".as_slice(), 3, Some("limit_exceeded")),
            (b"bad\nx".as_slice(), 99, Some("evidence_corrupt")),
            (b"bad\nx".as_slice(), 4, Some("limit_exceeded")),
            (b"bad\n".as_slice(), 4, None),
            (b"".as_slice(), 0, None),
        ] {
            let mut cursor = Cursor::new(input);
            let result = preflight(&mut cursor, max);
            if let Some(code) = error {
                assert_eq!(result.unwrap_err().code, code);
            } else {
                result.unwrap();
                assert_eq!(cursor.position(), 0);
            }
        }
    }

    #[test]
    fn empty_stream_and_torn_frame_are_distinct() {
        assert!(Frames::new(Cursor::new(b"")).next().unwrap().is_none());
        assert_eq!(
            Frames::new(Cursor::new(b"x")).next().unwrap_err().code,
            "evidence_corrupt"
        );
    }

    struct Faulty {
        cursor: Cursor<Vec<u8>>,
        read_error: bool,
        seek_error: bool,
        interrupted: bool,
    }
    impl Read for Faulty {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.interrupted {
                self.interrupted = false;
                return Err(io::ErrorKind::Interrupted.into());
            }
            if self.read_error {
                return Err(io::Error::other("private fixture path"));
            }
            self.cursor.read(output)
        }
    }
    impl Seek for Faulty {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            if self.seek_error {
                return Err(io::Error::other("private fixture path"));
            }
            self.cursor.seek(from)
        }
    }
    #[test]
    fn io_errors_are_sanitized_and_interrupted_reads_retry() {
        for (read_error, seek_error) in [(true, false), (false, true), (false, false)] {
            let mut input = Faulty {
                cursor: Cursor::new(b"x\n".to_vec()),
                read_error,
                seek_error,
                interrupted: true,
            };
            let result = preflight(&mut input, 10);
            if read_error || seek_error {
                let error = result.unwrap_err();
                assert_eq!(error.code, "io_failure");
                assert!(!error.to_string().contains("private"));
            } else {
                result.unwrap();
            }
        }
        let input = Faulty {
            cursor: Cursor::new(Vec::new()),
            read_error: true,
            seek_error: false,
            interrupted: true,
        };
        assert_eq!(
            Frames::new(BufReader::new(input)).next().unwrap_err().code,
            "io_failure"
        );
    }
}

#[cfg(test)]
mod growth_tests {
    use super::*;
    use std::io::{self, BufReader, Cursor};
    struct Growing(Cursor<Vec<u8>>);
    impl Read for Growing {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.0.read(bytes)
        }
    }
    impl Seek for Growing {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.0 = Cursor::new(b"x\ny\nz\n".to_vec());
            self.0.seek(position)
        }
    }
    #[test]
    fn growth_after_preflight_cannot_escape_decode_pass_limit() {
        let mut input = Growing(Cursor::new(b"x\n".to_vec()));
        preflight(&mut input, 4).unwrap();
        let mut frames = Frames::with_limit(BufReader::new(input.take(5)), 4);
        assert_eq!(frames.next().unwrap().unwrap(), b"x");
        assert_eq!(frames.next().unwrap().unwrap(), b"y");
        assert_eq!(frames.next().unwrap_err().code, "limit_exceeded");
    }
}
