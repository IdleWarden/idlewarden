// SPDX-License-Identifier: MPL-2.0
use crate::gray::Gray;
use crate::VisionError;

/// Largest template we will decode, in pixels.
///
/// Plugin assets arrive from the registry, so they are untrusted input. A
/// declared size is checked before any buffer is allocated for it.
const MAX_PIXELS: u64 = 16_000_000;

/// Decode a PNG template into the greyscale buffer matching uses.
pub fn png_to_gray(bytes: &[u8]) -> Result<Gray, VisionError> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());

    let mut reader = decoder
        .read_info()
        .map_err(|error| VisionError::Ocr(format!("template is not a readable png: {error}")))?;

    let info = reader.info();
    let (width, height) = (info.width, info.height);
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(VisionError::Ocr(format!(
            "template declares {width}x{height}, which is beyond anything a game UI needs"
        )));
    }

    let size = reader
        .output_buffer_size()
        .ok_or_else(|| VisionError::Ocr("template declares an unusable size".to_owned()))?;
    let mut buffer = vec![0u8; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|error| VisionError::Ocr(format!("template could not be decoded: {error}")))?;

    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => {
            return Err(VisionError::Ocr(format!(
                "template uses an unsupported colour type: {other:?}"
            )))
        }
    };

    let pixels = buffer[..frame.buffer_size()]
        .chunks_exact(channels)
        .map(|pixel| match channels {
            1 | 2 => pixel[0],
            _ => {
                let (r, g, b) = (pixel[0] as u32, pixel[1] as u32, pixel[2] as u32);
                ((77 * r + 150 * g + 29 * b) >> 8) as u8
            }
        })
        .collect();

    Gray::new(width, height, pixels)
        .ok_or_else(|| VisionError::Ocr("decoded template does not match its own size".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(width: u32, height: u32, colour: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(colour);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("header");
        writer.write_image_data(data).expect("data");
        writer.finish().expect("finish");
        out
    }

    #[test]
    fn a_greyscale_png_keeps_its_values() {
        let png = encode(2, 2, png::ColorType::Grayscale, &[10, 20, 30, 40]);

        let gray = png_to_gray(&png).expect("decodes");

        assert_eq!((gray.width, gray.height), (2, 2));
        assert_eq!(gray.pixels, vec![10, 20, 30, 40]);
    }

    #[test]
    fn colour_channels_are_weighted_the_same_way_as_captured_frames() {
        let png = encode(
            3,
            1,
            png::ColorType::Rgb,
            &[255, 0, 0, 0, 255, 0, 0, 0, 255],
        );

        let gray = png_to_gray(&png).expect("decodes");

        assert!(
            gray.at(1, 0) > gray.at(0, 0) && gray.at(0, 0) > gray.at(2, 0),
            "green must weigh most and blue least, matching Gray::from_bgra: {:?}",
            gray.pixels
        );
    }

    #[test]
    fn an_alpha_channel_is_dropped_rather_than_darkening_the_template() {
        let opaque = encode(1, 1, png::ColorType::Rgb, &[200, 200, 200]);
        let transparent = encode(1, 1, png::ColorType::Rgba, &[200, 200, 200, 0]);

        let a = png_to_gray(&opaque).expect("decodes");
        let b = png_to_gray(&transparent).expect("decodes");

        assert_eq!(
            a.pixels, b.pixels,
            "correlation works on luma; letting alpha bleed in would change the score"
        );
    }

    #[test]
    fn something_that_is_not_a_png_is_refused() {
        let error = png_to_gray(b"certainly not a png").expect_err("refused");

        assert!(matches!(error, VisionError::Ocr(message) if message.contains("readable png")));
    }

    #[test]
    fn a_truncated_png_is_refused_rather_than_half_decoded() {
        let png = encode(4, 4, png::ColorType::Grayscale, &[128; 16]);

        let error = png_to_gray(&png[..png.len() / 2]).expect_err("refused");

        assert!(matches!(error, VisionError::Ocr(_)));
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn an_absurd_declared_size_is_refused_before_allocating_for_it() {
        let mut png = encode(1, 1, png::ColorType::Grayscale, &[0]);
        png[16..20].copy_from_slice(&40_000u32.to_be_bytes());
        png[20..24].copy_from_slice(&40_000u32.to_be_bytes());
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());

        let error = png_to_gray(&png).expect_err("refused");

        assert!(
            matches!(error, VisionError::Ocr(message) if message.contains("beyond anything")),
            "an untrusted asset must not be able to ask for a 6 GB allocation"
        );
    }
}
