//! Telegram helpers shared by the interfaces (C#: static methods of
//! `EdInterfaceBase` and private helpers of `EdInterfaceObd`, GPL-3.0).

/// C#: `TransBufferSize`, transmit/receive buffer size of the interfaces.
pub const TRANS_BUFFER_SIZE: usize = 0x800;

/// C#: `TelLengthBmwFast`. Telegram length without checksum, limited to
/// `data.len()`; `None` where C# would index out of range.
pub fn tel_length_bmw_fast(data: &[u8]) -> Option<usize> {
    let mut tel_length = (*data.first()? & 0x3F) as usize;
    if tel_length == 0 {
        // with length byte
        if *data.get(3)? == 0 {
            tel_length = ((*data.get(4)? as usize) << 8) + *data.get(5)? as usize + 6;
        } else {
            tel_length = data[3] as usize + 4;
        }
    } else {
        tel_length += 3;
    }
    Some(tel_length.min(data.len()))
}

/// C#: `CalcChecksumBmwFast`: byte sum over the first `length` bytes.
pub fn calc_checksum_bmw_fast(data: &[u8], length: usize) -> u8 {
    data[..length].iter().fold(0u8, |s, b| s.wrapping_add(*b))
}

/// C#: `CalcChecksumXor`: XOR over the first `length` bytes (DS2, KWP2000*).
pub fn calc_checksum_xor(data: &[u8], length: usize) -> u8 {
    data[..length].iter().fold(0u8, |s, b| s ^ *b)
}

/// C#: `TelLengthKwp2000S`. Telegram length without checksum.
pub fn tel_length_kwp2000s(data: &[u8]) -> usize {
    data[3] as usize + 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmw_fast_lengths() {
        assert_eq!(
            tel_length_bmw_fast(&[0x82, 0x12, 0xF1, 0x1A, 0x80, 0x00]),
            Some(5)
        );
        assert_eq!(
            tel_length_bmw_fast(&[0x80, 0x12, 0xF1, 0x02, 0x1A, 0x80, 0x00]),
            Some(6)
        );
        let mut long = vec![0x80, 0x12, 0xF1, 0x00, 0x01, 0x00];
        assert_eq!(tel_length_bmw_fast(&long), Some(6));
        long.resize(0x200, 0);
        assert_eq!(tel_length_bmw_fast(&long), Some(0x106));
        assert_eq!(tel_length_bmw_fast(&[0x80, 0x12]), None);
    }

    #[test]
    fn checksums() {
        assert_eq!(
            calc_checksum_bmw_fast(&[0x82, 0x12, 0xF1, 0x1A, 0x80], 5),
            0x1F
        );
        assert_eq!(calc_checksum_xor(&[0x12, 0x04, 0x00], 3), 0x16);
        assert_eq!(
            tel_length_kwp2000s(&[0xB8, 0x12, 0xF1, 0x02, 0x1A, 0x80]),
            6
        );
    }
}
