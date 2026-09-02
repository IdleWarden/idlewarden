// SPDX-License-Identifier: MPL-2.0

/// An 8-bit greyscale buffer, tightly packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gray {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Gray {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Gray> {
        if pixels.len() != (width as usize) * (height as usize) {
            return None;
        }
        Some(Gray {
            width,
            height,
            pixels,
        })
    }

    /// BT.601 luma from tightly packed BGRA8, which is what capture delivers.
    pub fn from_bgra(width: u32, height: u32, bgra: &[u8]) -> Option<Gray> {
        let count = (width as usize).checked_mul(height as usize)?;
        if bgra.len() < count * 4 {
            return None;
        }
        let pixels = bgra[..count * 4]
            .chunks_exact(4)
            .map(|px| {
                let (b, g, r) = (px[0] as u32, px[1] as u32, px[2] as u32);
                ((77 * r + 150 * g + 29 * b) >> 8) as u8
            })
            .collect();
        Gray::new(width, height, pixels)
    }

    pub fn at(&self, x: u32, y: u32) -> u8 {
        self.pixels[(y as usize) * (self.width as usize) + x as usize]
    }

    pub fn crop(&self, x: u32, y: u32, width: u32, height: u32) -> Option<Gray> {
        if width == 0 || height == 0 || x + width > self.width || y + height > self.height {
            return None;
        }
        let mut pixels = Vec::with_capacity((width as usize) * (height as usize));
        for row in y..y + height {
            let start = (row as usize) * (self.width as usize) + x as usize;
            pixels.extend_from_slice(&self.pixels[start..start + width as usize]);
        }
        Gray::new(width, height, pixels)
    }

    /// Nearest-neighbour resample. Templates are small and flat UI art, so the
    /// cost of anything better is not repaid.
    pub fn resized(&self, width: u32, height: u32) -> Option<Gray> {
        if width == 0 || height == 0 || self.width == 0 || self.height == 0 {
            return None;
        }
        let mut pixels = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            let src_y = (y as u64 * self.height as u64 / height as u64) as u32;
            for x in 0..width {
                let src_x = (x as u64 * self.width as u64 / width as u64) as u32;
                pixels.push(self.at(src_x.min(self.width - 1), src_y.min(self.height - 1)));
            }
        }
        Gray::new(width, height, pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mismatched_buffer_is_refused() {
        assert!(Gray::new(2, 2, vec![0; 3]).is_none());
    }

    #[test]
    fn bgra_is_read_in_the_right_channel_order() {
        let blue = Gray::from_bgra(1, 1, &[255, 0, 0, 255]).expect("valid");
        let green = Gray::from_bgra(1, 1, &[0, 255, 0, 255]).expect("valid");
        let red = Gray::from_bgra(1, 1, &[0, 0, 255, 255]).expect("valid");

        assert!(
            green.at(0, 0) > red.at(0, 0) && red.at(0, 0) > blue.at(0, 0),
            "green must weigh most and blue least: got g={} r={} b={}",
            green.at(0, 0),
            red.at(0, 0),
            blue.at(0, 0)
        );
    }

    #[test]
    fn a_short_bgra_buffer_is_refused_rather_than_read_past_its_end() {
        assert!(Gray::from_bgra(2, 2, &[0; 12]).is_none());
    }

    #[test]
    fn cropping_takes_the_requested_rectangle() {
        let gray = Gray::new(3, 3, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).expect("valid");

        let crop = gray.crop(1, 1, 2, 2).expect("inside the buffer");

        assert_eq!(crop.pixels, vec![5, 6, 8, 9]);
    }

    #[test]
    fn cropping_outside_the_buffer_is_refused() {
        let gray = Gray::new(2, 2, vec![0; 4]).expect("valid");

        assert!(gray.crop(1, 1, 2, 2).is_none());
        assert!(gray.crop(0, 0, 0, 1).is_none());
    }

    #[test]
    fn resizing_keeps_the_corners() {
        let gray = Gray::new(2, 2, vec![10, 20, 30, 40]).expect("valid");

        let bigger = gray.resized(4, 4).expect("valid");

        assert_eq!(bigger.at(0, 0), 10);
        assert_eq!(bigger.at(3, 0), 20);
        assert_eq!(bigger.at(0, 3), 30);
        assert_eq!(bigger.at(3, 3), 40);
    }
}
