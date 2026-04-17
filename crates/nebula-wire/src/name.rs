//! DNS names (RFC 1035 §3.1 / §4.1.4).
//!
//! M1 adds full name compression: the decoder follows pointers with cycle detection, and
//! the encoder emits pointers when the suffix has already been written.

use std::collections::HashMap;

use crate::{ParseError, MAX_LABEL_LEN, MAX_NAME_LEN};

/// A fully-qualified domain name, represented as its sequence of labels.
///
/// The root zone `.` is the empty label sequence. Labels are stored lowercased-by-choice
/// at the call site; equality comparison is case-sensitive by design — callers wanting
/// case-insensitive matching should use [`Name::eq_ignore_ascii_case`].
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    labels: Vec<Vec<u8>>,
}

impl Name {
    /// Root name (`.`).
    #[must_use]
    pub fn root() -> Self {
        Self { labels: Vec::new() }
    }

    /// Borrowed view over the labels.
    #[must_use]
    pub fn labels(&self) -> &[Vec<u8>] {
        &self.labels
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.labels.is_empty()
    }

    /// Case-insensitive comparison per RFC 1035 §2.3.3.
    #[must_use]
    pub fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        if self.labels.len() != other.labels.len() {
            return false;
        }
        self.labels
            .iter()
            .zip(other.labels.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    }

    /// Encoded wire length (uncompressed — including the final zero-length terminator).
    #[must_use]
    pub fn wire_len(&self) -> usize {
        self.labels.iter().map(|l| 1 + l.len()).sum::<usize>() + 1
    }

    /// Produce a lower-cased copy. Compression lookup uses this form.
    #[must_use]
    pub fn to_ascii_lowercase(&self) -> Self {
        Self {
            labels: self
                .labels
                .iter()
                .map(|l| l.iter().map(u8::to_ascii_lowercase).collect())
                .collect(),
        }
    }

    /// Build from a dotted ASCII representation.
    pub fn from_ascii(s: &str) -> Result<Self, ParseError> {
        let s = s.trim_end_matches('.');
        if s.is_empty() {
            return Ok(Self::root());
        }
        let mut labels = Vec::new();
        let mut total = 1; // final zero
        for part in s.split('.') {
            let bytes = part.as_bytes();
            if bytes.is_empty() || bytes.len() > MAX_LABEL_LEN {
                return Err(ParseError::LabelTooLong { len: bytes.len() });
            }
            total += 1 + bytes.len();
            if total > MAX_NAME_LEN {
                return Err(ParseError::NameTooLong { len: total });
            }
            labels.push(bytes.to_vec());
        }
        Ok(Self { labels })
    }

    /// Encode uncompressed into `out`. Returns bytes written.
    ///
    /// For compressed output use [`Name::encode_compressed`] with an encode context.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, ParseError> {
        let need = self.wire_len();
        if out.len() < need {
            return Err(ParseError::OutputBufferTooSmall {
                need,
                have: out.len(),
            });
        }
        let mut pos = 0;
        for label in &self.labels {
            out[pos] = u8::try_from(label.len())
                .map_err(|_| ParseError::LabelTooLong { len: label.len() })?;
            pos += 1;
            out[pos..pos + label.len()].copy_from_slice(label);
            pos += label.len();
        }
        out[pos] = 0;
        Ok(pos + 1)
    }

    /// Decode a name starting at `offset` in `buf`. Follows compression pointers with
    /// a visited-set to reject pointer cycles (a classic CVE in older DNS software).
    ///
    /// Returns the decoded name and the offset just past the last consumed byte of the
    /// *non-compressed* portion — callers use this to advance through the message when
    /// the name is immediately followed by more data.
    pub fn decode(buf: &[u8], offset: usize) -> Result<(Self, usize), ParseError> {
        let mut pos = offset;
        let mut labels = Vec::new();
        let mut total_len = 0usize;
        let mut jumped = false;
        let mut advance_end = offset; // where the caller resumes after this name

        // Cap the number of label/pointer hops to prevent unbounded traversal even when
        // the buffer is pathologically structured.
        let mut hops = 0usize;
        const MAX_HOPS: usize = 128;

        loop {
            if hops >= MAX_HOPS {
                return Err(ParseError::InvalidCompression("too many pointer hops"));
            }
            hops += 1;
            if pos >= buf.len() {
                return Err(ParseError::UnexpectedEof {
                    offset: pos,
                    context: "name",
                });
            }
            let b = buf[pos];
            match b & 0xC0 {
                0x00 => {
                    let len = usize::from(b & 0x3F);
                    if len == 0 {
                        pos += 1;
                        if !jumped {
                            advance_end = pos;
                        }
                        break;
                    }
                    if len > MAX_LABEL_LEN {
                        return Err(ParseError::LabelTooLong { len });
                    }
                    if pos + 1 + len > buf.len() {
                        return Err(ParseError::UnexpectedEof {
                            offset: pos + 1,
                            context: "label bytes",
                        });
                    }
                    total_len += 1 + len;
                    if total_len > MAX_NAME_LEN {
                        return Err(ParseError::NameTooLong { len: total_len });
                    }
                    labels.push(buf[pos + 1..pos + 1 + len].to_vec());
                    pos += 1 + len;
                    if !jumped {
                        advance_end = pos;
                    }
                }
                0xC0 => {
                    if pos + 1 >= buf.len() {
                        return Err(ParseError::UnexpectedEof {
                            offset: pos + 1,
                            context: "pointer low byte",
                        });
                    }
                    let target = ((usize::from(b & 0x3F)) << 8) | usize::from(buf[pos + 1]);
                    // A pointer must always point backwards (RFC 1035 §4.1.4).
                    if target >= pos {
                        return Err(ParseError::InvalidCompression(
                            "pointer does not point backwards",
                        ));
                    }
                    if !jumped {
                        advance_end = pos + 2;
                    }
                    jumped = true;
                    pos = target;
                }
                // 0x40 and 0x80 are reserved forms (RFC 6891 extended label was removed).
                _ => return Err(ParseError::InvalidCompression("reserved label form")),
            }
        }

        Ok((Self { labels }, advance_end))
    }
}

/// Encode context that tracks label offsets so repeated suffixes become pointers.
/// RFC 1035 §4.1.4: a pointer is the 14-bit offset of a previously-written name suffix,
/// encoded in the top two bits as `0b11`. Only offsets below 0x4000 (16 KiB) are
/// representable — we record that limit and fall back to uncompressed when it's hit.
#[derive(Debug, Default)]
pub struct EncodeCtx {
    // Map of lowercased suffix labels (as one owned Vec<Vec<u8>>) to the offset in the
    // output buffer where that suffix begins.
    suffixes: HashMap<Vec<Vec<u8>>, u16>,
}

impl EncodeCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Write `name` into `out` starting at `absolute_offset`, emitting compression
    /// pointers for any suffix seen previously in this context.
    ///
    /// Returns bytes written in the output slice.
    pub fn write_name(
        &mut self,
        name: &Name,
        out: &mut [u8],
        absolute_offset: usize,
    ) -> Result<usize, ParseError> {
        let mut written = 0;
        let mut suffix = name.labels.clone();
        loop {
            if suffix.is_empty() {
                // Root terminator.
                if written + 1 > out.len() {
                    return Err(ParseError::OutputBufferTooSmall {
                        need: written + 1,
                        have: out.len(),
                    });
                }
                out[written] = 0;
                written += 1;
                return Ok(written);
            }
            // Case-insensitive lookup: the key is always lowercased.
            let key: Vec<Vec<u8>> = suffix
                .iter()
                .map(|l| l.iter().map(u8::to_ascii_lowercase).collect())
                .collect();
            if let Some(&ptr) = self.suffixes.get(&key) {
                if written + 2 > out.len() {
                    return Err(ParseError::OutputBufferTooSmall {
                        need: written + 2,
                        have: out.len(),
                    });
                }
                out[written] = 0xC0 | ((ptr >> 8) as u8 & 0x3F);
                out[written + 1] = (ptr & 0xFF) as u8;
                written += 2;
                return Ok(written);
            }
            // Record the suffix's offset (if it fits in a pointer) and emit the label.
            let this_suffix_offset = absolute_offset + written;
            if this_suffix_offset < 0x4000 {
                self.suffixes.insert(
                    key,
                    u16::try_from(this_suffix_offset).expect("checked above"),
                );
            }
            let label = &suffix[0];
            let needed = 1 + label.len();
            if written + needed > out.len() {
                return Err(ParseError::OutputBufferTooSmall {
                    need: written + needed,
                    have: out.len(),
                });
            }
            out[written] = u8::try_from(label.len())
                .map_err(|_| ParseError::LabelTooLong { len: label.len() })?;
            out[written + 1..written + 1 + label.len()].copy_from_slice(label);
            written += needed;
            suffix.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_roundtrip() {
        let n = Name::root();
        let mut buf = [0u8; 1];
        let len = n.encode(&mut buf).unwrap();
        assert_eq!(len, 1);
        assert_eq!(buf, [0]);
        let (decoded, end) = Name::decode(&buf, 0).unwrap();
        assert_eq!(decoded, n);
        assert_eq!(end, 1);
    }

    #[test]
    fn simple_name_roundtrip() {
        let n = Name::from_ascii("example.com").unwrap();
        let mut buf = vec![0u8; n.wire_len()];
        n.encode(&mut buf).unwrap();
        let (decoded, end) = Name::decode(&buf, 0).unwrap();
        assert_eq!(decoded, n);
        assert_eq!(end, buf.len());
    }

    #[test]
    fn trailing_dot_is_tolerated() {
        let a = Name::from_ascii("example.com.").unwrap();
        let b = Name::from_ascii("example.com").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn oversize_label_rejected() {
        let long = "a".repeat(64);
        assert!(matches!(
            Name::from_ascii(&long),
            Err(ParseError::LabelTooLong { .. })
        ));
    }

    #[test]
    fn empty_middle_label_rejected() {
        assert!(matches!(
            Name::from_ascii("foo..bar"),
            Err(ParseError::LabelTooLong { .. })
        ));
    }

    #[test]
    fn decode_follows_pointer() {
        // Build: [example.com at offset 0][pointer at offset 13 -> offset 0]
        let base = Name::from_ascii("example.com").unwrap();
        let mut buf = vec![0u8; base.wire_len() + 2];
        base.encode(&mut buf[..base.wire_len()]).unwrap();
        // Pointer to offset 0.
        buf[base.wire_len()] = 0xC0;
        buf[base.wire_len() + 1] = 0x00;

        let (decoded, end) = Name::decode(&buf, base.wire_len()).unwrap();
        assert_eq!(decoded, base);
        // Caller resumes right past the pointer (2 bytes).
        assert_eq!(end, base.wire_len() + 2);
    }

    #[test]
    fn pointer_forward_rejected() {
        // Forward-pointing pointer: offset 0 -> offset 5.
        let buf = [0xC0u8, 0x05, 0x00, 0x00, 0x00, 0x00];
        assert!(matches!(
            Name::decode(&buf, 0),
            Err(ParseError::InvalidCompression(_))
        ));
    }

    #[test]
    fn pointer_self_reference_rejected() {
        // A pointer at offset 0 cannot point to offset 0 — "pointer must point backwards"
        // catches the self-reference case, which is the simplest form of a cycle.
        let buf = [0xC0u8, 0x00];
        assert!(matches!(
            Name::decode(&buf, 0),
            Err(ParseError::InvalidCompression(_))
        ));
    }

    #[test]
    fn decode_does_not_panic_on_deep_chain() {
        // A valid backwards chain: the first byte is a root terminator, then many
        // 2-byte pointers each pointing to the previous pointer. The chain is bounded
        // by the hop limit; the important contract is "no panic, no quadratic time".
        let mut buf = vec![0u8; 512];
        buf[0] = 0;
        let mut prev: u16 = 0;
        let mut i = 1;
        while i + 2 <= buf.len() && i < 0x3FFF {
            buf[i] = 0xC0 | u8::try_from((prev >> 8) & 0x3F).unwrap();
            buf[i + 1] = (prev & 0xFF) as u8;
            prev = u16::try_from(i).unwrap();
            i += 2;
        }
        // Starting from the last pointer: the decoder either succeeds or returns
        // `InvalidCompression` (hop-limit). Anything else is a panic, which fails the test.
        let _ = Name::decode(&buf, (i - 2) as usize);
    }

    #[test]
    fn encode_ctx_compresses_shared_suffix() {
        let a = Name::from_ascii("www.example.com").unwrap();
        let b = Name::from_ascii("mail.example.com").unwrap();

        let mut buf = vec![0u8; 64];
        let mut ctx = EncodeCtx::new();
        let n1 = ctx.write_name(&a, &mut buf, 0).unwrap();
        let n2 = ctx.write_name(&b, &mut buf[n1..], n1).unwrap();

        // `a` is fully laid out; `b` should end in a pointer (compressed).
        //   www.example.com = 1+3 + 1+7 + 1+3 + 1 = 17 bytes
        //   mail. then pointer to `example.com` = 1+4 + 2 = 7 bytes
        assert_eq!(n1, 17);
        assert_eq!(n2, 7);

        let total = &buf[..n1 + n2];
        let (decoded_a, pos_a) = Name::decode(total, 0).unwrap();
        let (decoded_b, pos_b) = Name::decode(total, pos_a).unwrap();
        assert_eq!(decoded_a, a);
        assert_eq!(decoded_b, b);
        assert_eq!(pos_b, n1 + n2);
    }

    #[test]
    fn encode_ctx_case_insensitive_suffix_dedup() {
        let a = Name::from_ascii("www.Example.COM").unwrap();
        let b = Name::from_ascii("mail.example.com").unwrap();

        let mut buf = vec![0u8; 64];
        let mut ctx = EncodeCtx::new();
        let n1 = ctx.write_name(&a, &mut buf, 0).unwrap();
        let n2 = ctx.write_name(&b, &mut buf[n1..], n1).unwrap();
        // Same compression savings whether the earlier name was mixed-case.
        assert_eq!(n2, 7);
    }
}
