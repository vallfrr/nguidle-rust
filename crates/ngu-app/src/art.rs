//! The interface files embedded in the program.

use fontdue::{Font, FontSettings};

/// A decoded picture, RGBA, top row first.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn decode(bytes: &[u8]) -> Image {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("embedded picture");
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).expect("embedded picture");
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buffer[..info.buffer_size()]
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        other => panic!("unexpected picture type {other:?}"),
    };
    Image {
        width: info.width,
        height: info.height,
        rgba,
    }
}

/// Everything the setup window draws with.
pub struct Art {
    pub frame: Image,
    pub button: Image,
    pub backdrop: Image,
    pub cat: Image,
    pub title: Font,
    pub body: Font,
}

impl Art {
    pub fn load() -> Art {
        let font =
            |bytes: &[u8]| Font::from_bytes(bytes, FontSettings::default()).expect("embedded font");
        Art {
            frame: decode(include_bytes!("../../../assets/ui/frame.png")),
            button: decode(include_bytes!("../../../assets/ui/button.png")),
            backdrop: decode(include_bytes!("../../../assets/ui/backdrop.png")),
            cat: decode(include_bytes!("../../../assets/ui/cat.png")),
            title: font(include_bytes!("../../../assets/ui/title.ttf")),
            body: font(include_bytes!("../../../assets/ui/body.ttf")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_files_load() {
        let art = Art::load();
        assert_eq!((art.frame.width, art.frame.height), (200, 200));
        assert_eq!((art.backdrop.width, art.backdrop.height), (960, 600));
        assert_eq!(art.frame.rgba.len(), 200 * 200 * 4);
        assert!(art.title.horizontal_line_metrics(20.0).is_some());
    }
}
