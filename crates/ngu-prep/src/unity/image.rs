//! Decodes Unity texture data to 8-bit RGBA, top row first.

type Result<T> = std::result::Result<T, String>;

/// Texture format numbers used by Unity 2019.4.
pub const ALPHA8: i32 = 1;
pub const ARGB4444: i32 = 2;
pub const RGB24: i32 = 3;
pub const RGBA32: i32 = 4;
pub const RGB565: i32 = 7;
pub const DXT1: i32 = 10;
pub const DXT5: i32 = 12;
pub const BC7: i32 = 25;

/// Decoded picture: RGBA bytes plus whether the source had no alpha channel.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub opaque: bool,
}

/// Pillow's expansion for 16-bit colour: each channel scaled to 0..=255, rounded down.
fn expand565(v: u16) -> [u8; 3] {
    let scale = |value: u16, max: u16| (u32::from(value) * 255 / u32::from(max)) as u8;
    [
        scale((v >> 11) & 31, 31),
        scale((v >> 5) & 63, 63),
        scale(v & 31, 31),
    ]
}

fn expand565_full(v: u16) -> [u8; 3] {
    let (r, g, b) = ((v >> 11) & 31, (v >> 5) & 63, v & 31);
    [
        ((r << 3) | (r >> 2)) as u8,
        ((g << 2) | (g >> 4)) as u8,
        ((b << 3) | (b >> 2)) as u8,
    ]
}

fn bc1_colours(block: &[u8], four_colours: bool) -> [[u8; 4]; 4] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (expand565_full(c0), expand565_full(c1));
    let mix = |x: u8, y: u8, wx: u32, wy: u32, div: u32| {
        ((u32::from(x) * wx + u32::from(y) * wy) / div) as u8
    };
    let mut out = [[0; 4]; 4];
    out[0] = [a[0], a[1], a[2], 255];
    out[1] = [b[0], b[1], b[2], 255];
    if c0 > c1 || four_colours {
        out[2] = [
            mix(a[0], b[0], 2, 1, 3),
            mix(a[1], b[1], 2, 1, 3),
            mix(a[2], b[2], 2, 1, 3),
            255,
        ];
        out[3] = [
            mix(a[0], b[0], 1, 2, 3),
            mix(a[1], b[1], 1, 2, 3),
            mix(a[2], b[2], 1, 2, 3),
            255,
        ];
    } else {
        out[2] = [
            mix(a[0], b[0], 1, 1, 2),
            mix(a[1], b[1], 1, 1, 2),
            mix(a[2], b[2], 1, 1, 2),
            255,
        ];
        out[3] = [0, 0, 0, 0];
    }
    out
}

fn bc1_block(block: &[u8], four_colours: bool, out: &mut [[u8; 4]; 16]) {
    let colours = bc1_colours(block, four_colours);
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    for (i, pixel) in out.iter_mut().enumerate() {
        *pixel = colours[((bits >> (2 * i)) & 3) as usize];
    }
}

fn bc3_alpha(block: &[u8]) -> [u8; 16] {
    let (a0, a1) = (u32::from(block[0]), u32::from(block[1]));
    let mut table = [0u32; 8];
    table[0] = a0;
    table[1] = a1;
    if a0 > a1 {
        for i in 1..7 {
            table[i + 1] = ((7 - i as u32) * a0 + i as u32 * a1) / 7;
        }
    } else {
        for i in 1..5 {
            table[i + 1] = ((5 - i as u32) * a0 + i as u32 * a1) / 5;
        }
        table[6] = 0;
        table[7] = 255;
    }
    let mut bits = 0u64;
    for (i, byte) in block[2..8].iter().enumerate() {
        bits |= u64::from(*byte) << (8 * i);
    }
    let mut out = [0; 16];
    for (i, value) in out.iter_mut().enumerate() {
        *value = table[((bits >> (3 * i)) & 7) as usize] as u8;
    }
    out
}

fn blocks(
    width: usize,
    height: usize,
    size: usize,
    data: &[u8],
    mut each: impl FnMut(&[u8], &mut [[u8; 4]; 16]),
) -> Result<Vec<u8>> {
    let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
    if data.len() < bw * bh * size {
        return Err("Texture data is shorter than its size.".into());
    }
    let mut rgba = vec![0; width * height * 4];
    let mut pixels = [[0u8; 4]; 16];
    for by in 0..bh {
        for bx in 0..bw {
            each(&data[(by * bw + bx) * size..][..size], &mut pixels);
            for (i, pixel) in pixels.iter().enumerate() {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x < width && y < height {
                    rgba[(y * width + x) * 4..][..4].copy_from_slice(pixel);
                }
            }
        }
    }
    Ok(rgba)
}

fn flip(rgba: Vec<u8>, width: usize, height: usize) -> Vec<u8> {
    let row = width * 4;
    let mut out = Vec::with_capacity(rgba.len());
    for y in (0..height).rev() {
        out.extend_from_slice(&rgba[y * row..][..row]);
    }
    out
}

/// Decodes `data` of the given Unity texture format.
pub fn decode(format: i32, width: usize, height: usize, data: &[u8]) -> Result<Picture> {
    let need = |count: usize| {
        if data.len() < width * height * count {
            Err("Texture data is shorter than its size.".to_string())
        } else {
            Ok(())
        }
    };
    let mut opaque = false;
    let rgba = match format {
        ALPHA8 => {
            need(1)?;
            data[..width * height]
                .iter()
                .flat_map(|a| [0, 0, 0, *a])
                .collect()
        }
        ARGB4444 => {
            need(2)?;
            data[..width * height * 2]
                .chunks_exact(2)
                .flat_map(|c| {
                    let v = u16::from_le_bytes([c[0], c[1]]);
                    let n = |shift: u32| (((v >> shift) & 15) * 17) as u8;
                    [n(8), n(4), n(0), n(12)]
                })
                .collect()
        }
        RGB24 => {
            need(3)?;
            opaque = true;
            data[..width * height * 3]
                .chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect()
        }
        RGBA32 => {
            need(4)?;
            data[..width * height * 4].to_vec()
        }
        RGB565 => {
            need(2)?;
            opaque = true;
            data[..width * height * 2]
                .chunks_exact(2)
                .flat_map(|c| {
                    let [r, g, b] = expand565(u16::from_le_bytes([c[0], c[1]]));
                    [r, g, b, 255]
                })
                .collect()
        }
        DXT1 => blocks(width, height, 8, data, |block, out| {
            bc1_block(block, false, out)
        })?,
        DXT5 => blocks(width, height, 16, data, |block, out| {
            let alpha = bc3_alpha(&block[..8]);
            bc1_block(&block[8..], true, out);
            for (pixel, a) in out.iter_mut().zip(alpha) {
                pixel[3] = a;
            }
        })?,
        BC7 => {
            let (bw, bh) = (width.div_ceil(4), height.div_ceil(4));
            if data.len() < bw * bh * 16 {
                return Err("Texture data is shorter than its size.".into());
            }
            blocks(width, height, 16, data, |block, out| {
                let mut decoded = [0u8; 64];
                bcdec_rs::bc7(block, &mut decoded, 16);
                for (i, pixel) in out.iter_mut().enumerate() {
                    pixel.copy_from_slice(&decoded[i * 4..i * 4 + 4]);
                }
            })?
        }
        other => return Err(format!("Texture format {other} is not supported.")),
    };
    Ok(Picture {
        width,
        height,
        rgba: flip(rgba, width, height),
        opaque,
    })
}

/// Encodes a picture as PNG: RGB when the source had no alpha, RGBA otherwise.
pub fn png_bytes(picture: &Picture) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, picture.width as u32, picture.height as u32);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    encoder.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
    let data: Vec<u8>;
    if picture.opaque {
        encoder.set_color(png::ColorType::Rgb);
        data = picture
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
    } else {
        encoder.set_color(png::ColorType::Rgba);
        data = picture.rgba.clone();
    }
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_flipped_so_the_top_comes_first() {
        // Two rows of one RGBA32 pixel: Unity stores the bottom row first.
        let picture = decode(RGBA32, 1, 2, &[1, 1, 1, 255, 2, 2, 2, 255]).unwrap();
        assert_eq!(&picture.rgba[..4], &[2, 2, 2, 255]);
    }

    #[test]
    fn rgb565_scales_each_channel_down_like_pillow() {
        // Pure white, then the value used by a real texture (r 14, g 36, b 23).
        let picture = decode(RGB565, 2, 1, &[0xff, 0xff, 0x97, 0x74]).unwrap();
        assert_eq!(&picture.rgba[..4], &[255, 255, 255, 255]);
        assert_eq!(&picture.rgba[4..], &[115, 145, 189, 255]);
        assert!(picture.opaque);
    }

    #[test]
    fn dxt1_block_with_two_equal_colours_is_flat() {
        let mut block = vec![0xff, 0xff, 0xff, 0xff];
        block.extend([0; 4]);
        let picture = decode(DXT1, 4, 4, &block).unwrap();
        assert!(picture.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
    }

    #[test]
    fn short_data_and_unknown_formats_are_errors() {
        assert!(decode(DXT5, 4, 4, &[0; 8]).is_err());
        assert!(decode(99, 1, 1, &[0; 4]).is_err());
    }

    #[test]
    fn png_has_three_channels_for_opaque_sources() {
        let picture = decode(RGB24, 1, 1, &[1, 2, 3]).unwrap();
        let bytes = png_bytes(&picture).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        assert_eq!(bytes[25], 2); // PNG colour type: RGB
    }
}
