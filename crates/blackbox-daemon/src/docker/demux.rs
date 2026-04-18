#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StreamKind {
    Stdout,
    Stderr,
    /// stdin or unknown stream type
    Other,
}

#[allow(dead_code)]
/// Parse Docker's 8-byte multiplexed stream header.
/// Layout: [stream_type(1), 0, 0, 0, payload_size(4 BE)]
/// Returns (StreamKind, payload_size_bytes) or None for malformed headers.
pub fn parse_header(header: &[u8; 8]) -> Option<(StreamKind, u32)> {
    let kind = match header[0] {
        1 => StreamKind::Stdout,
        2 => StreamKind::Stderr,
        _ => StreamKind::Other,
    };
    let size = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    Some((kind, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stdout_frame() {
        let header: [u8; 8] = [1, 0, 0, 0, 0, 0, 0, 5];
        let (kind, size) = parse_header(&header).unwrap();
        assert_eq!(kind, StreamKind::Stdout);
        assert_eq!(size, 5);
    }

    #[test]
    fn parses_stderr_frame() {
        let header: [u8; 8] = [2, 0, 0, 0, 0, 0, 0, 12];
        let (kind, size) = parse_header(&header).unwrap();
        assert_eq!(kind, StreamKind::Stderr);
        assert_eq!(size, 12);
    }

    #[test]
    fn parses_unknown_stream_type() {
        let header: [u8; 8] = [3, 0, 0, 0, 0, 0, 0, 1];
        let (kind, _) = parse_header(&header).unwrap();
        assert_eq!(kind, StreamKind::Other);
    }
}
