// SPDX-License-Identifier: MPL-2.0

/// Size of the captured window content, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    /// Length of a tightly packed BGRA8 buffer of this size.
    pub fn bytes(self) -> usize {
        self.row_bytes() * self.height as usize
    }

    pub(crate) fn row_bytes(self) -> usize {
        self.width as usize * 4
    }
}

/// One captured frame, BGRA8, tightly packed rows.
pub struct Frame {
    pub id: u64,
    pub captured_at_ms: u64,
    pub size: Size,
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("id", &self.id)
            .field("size", &self.size)
            .field("bytes", &self.bgra.len())
            .finish()
    }
}

pub(crate) fn pack_rows(src: &[u8], row_pitch: usize, size: Size) -> Option<Vec<u8>> {
    let row = size.row_bytes();
    if row_pitch < row {
        return None;
    }
    let height = size.height as usize;
    if height == 0 || size.width == 0 {
        return Some(Vec::new());
    }
    let span = row_pitch.checked_mul(height - 1)?.checked_add(row)?;
    if src.len() < span {
        return None;
    }

    let mut packed = Vec::with_capacity(row * height);
    for y in 0..height {
        let start = y * row_pitch;
        packed.extend_from_slice(&src[start..start + row]);
    }
    Some(packed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> Size {
        Size { width, height }
    }

    #[test]
    fn padding_between_rows_is_dropped() {
        let src = vec![
            1, 1, 1, 1, 2, 2, 2, 2, 9, 9, 9, 9, // row 0 then 4 bytes of padding
            3, 3, 3, 3, 4, 4, 4, 4, 9, 9, 9, 9, // row 1 then padding
        ];

        let packed = pack_rows(&src, 12, size(2, 2)).expect("a padded buffer is valid");

        assert_eq!(packed, vec![1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4]);
    }

    #[test]
    fn an_unpadded_buffer_is_copied_whole() {
        let src: Vec<u8> = (0..16).collect();

        let packed = pack_rows(&src, 8, size(2, 2)).expect("pitch equal to the row is valid");

        assert_eq!(packed, src);
    }

    #[test]
    fn the_last_row_need_not_carry_its_padding() {
        let src = vec![1, 1, 1, 1, 9, 9, 9, 9, 2, 2, 2, 2];

        let packed =
            pack_rows(&src, 8, size(1, 2)).expect("a buffer ending on the last row is valid");

        assert_eq!(packed, vec![1, 1, 1, 1, 2, 2, 2, 2]);
    }

    #[test]
    fn a_pitch_narrower_than_a_row_is_refused() {
        let src = vec![0u8; 64];

        assert!(
            pack_rows(&src, 4, size(2, 2)).is_none(),
            "a pitch that cannot hold a row would read across rows"
        );
    }

    #[test]
    fn a_buffer_shorter_than_the_image_is_refused() {
        let src = vec![0u8; 15];

        assert!(
            pack_rows(&src, 8, size(2, 2)).is_none(),
            "one byte short must not panic and must not return a truncated frame"
        );
    }

    #[test]
    fn an_empty_size_yields_an_empty_frame() {
        assert_eq!(pack_rows(&[], 0, size(0, 0)), Some(Vec::new()));
    }
}
