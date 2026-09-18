// SPDX-License-Identifier: MPL-2.0
use crate::VisionError;

pub fn png_from_bgra(width: u32, height: u32, bgra: &[u8]) -> Result<Vec<u8>, VisionError> {
    let expected = (width as usize) * (height as usize) * 4;
    if width == 0 || height == 0 || bgra.len() != expected {
        return Err(VisionError::Asset(format!(
            "a {width}x{height} crop needs {expected} bytes, got {}",
            bgra.len()
        )));
    }

    let rgba: Vec<u8> = bgra
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
        .collect();

    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| VisionError::Asset(format!("png header could not be written: {error}")))?;
    writer
        .write_image_data(&rgba)
        .map_err(|error| VisionError::Asset(format!("png data could not be written: {error}")))?;
    writer
        .finish()
        .map_err(|error| VisionError::Asset(format!("png could not be finished: {error}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::png_to_gray;

    #[test]
    fn a_crop_survives_the_round_trip_the_bundle_loader_makes() {
        let bgra = [
            0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 128, 128, 128, 255,
        ];

        let gray = png_to_gray(&png_from_bgra(2, 2, &bgra).expect("encodes")).expect("decodes");

        assert_eq!((gray.width, gray.height), (2, 2));
        assert_eq!(
            gray.pixels,
            vec![76, 149, 28, 128],
            "blue and red swapped on the way out would read as a different template"
        );
    }

    #[test]
    fn a_buffer_that_does_not_match_its_size_is_refused() {
        let error = png_from_bgra(2, 2, &[0; 12]).expect_err("one pixel short");

        assert!(matches!(error, VisionError::Asset(message) if message.contains("16 bytes")));
    }

    #[test]
    fn an_empty_crop_is_refused_rather_than_written() {
        assert!(png_from_bgra(0, 4, &[]).is_err());
    }
}
