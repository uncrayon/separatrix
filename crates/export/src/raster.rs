//! PNG images: the point layer of a large figure, and decoding a PNG the UI
//! sends for the clipboard.
//!
//! The point layer covers the plot box exactly, at `dpi` (300 for print),
//! with a transparent background. Dots follow the screen's rule: discs of
//! radius 2.6 px (1.3 px above 300 iterates), or squares of half-side
//! 0.75 px above 2,000 (`fillRect` on screen). Disc edges are
//! anti-aliased by coverage.

use std::io::Cursor;

/// One trajectory's dots, in figure pixels (y down).
#[derive(Debug, Clone, PartialEq)]
pub struct Dots {
    /// Centres.
    pub points: Vec<(f64, f64)>,
    /// Radius (or half-side of a square), CSS px.
    pub radius_px: f64,
    /// Squares instead of discs.
    pub square: bool,
    /// `#rrggbb`.
    pub color: String,
}

fn parse_hex(c: &str) -> [u8; 3] {
    let h = c.trim_start_matches('#');
    let v = u32::from_str_radix(h, 16).unwrap_or(0);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// Paints `alpha` of `rgb` over the pixel (source-over, straight alpha).
fn blend(img: &mut [u8], i: usize, rgb: [u8; 3], alpha: f64) {
    if alpha <= 0.0 {
        return;
    }
    let a = alpha.min(1.0);
    let da = f64::from(img[i + 3]) / 255.0;
    let oa = a + da * (1.0 - a);
    for c in 0..3 {
        let s = f64::from(rgb[c]);
        let d = f64::from(img[i + c]);
        let v = if oa > 0.0 {
            (s * a + d * da * (1.0 - a)) / oa
        } else {
            0.0
        };
        img[i + c] = v.round().clamp(0.0, 255.0) as u8;
    }
    img[i + 3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Size in device pixels of a `w` × `h` CSS-pixel box at `dpi`.
pub fn device_size(w: f64, h: f64, dpi: f64) -> (u32, u32) {
    let k = dpi / 96.0;
    (
        (w * k).round().clamp(1.0, 20000.0) as u32,
        (h * k).round().clamp(1.0, 20000.0) as u32,
    )
}

/// The dots as an RGBA image of the `w` × `h` (CSS px) plot box at `dpi`.
pub fn render_rgba(w: f64, h: f64, dpi: f64, layers: &[Dots]) -> (Vec<u8>, u32, u32) {
    let (iw, ih) = device_size(w, h, dpi);
    let k = f64::from(iw) / w;
    let mut img = vec![0u8; iw as usize * ih as usize * 4];
    for layer in layers {
        let rgb = parse_hex(&layer.color);
        let r = layer.radius_px * k;
        for &(cx, cy) in &layer.points {
            let (cx, cy) = (cx * k, cy * k);
            if !cx.is_finite() || !cy.is_finite() {
                continue;
            }
            let x0 = (cx - r - 1.0).floor().max(0.0) as i64;
            let x1 = (cx + r + 1.0).ceil().min(f64::from(iw)) as i64;
            let y0 = (cy - r - 1.0).floor().max(0.0) as i64;
            let y1 = (cy + r + 1.0).ceil().min(f64::from(ih)) as i64;
            for py in y0..y1 {
                for px in x0..x1 {
                    let (fx, fy) = (px as f64 + 0.5, py as f64 + 0.5);
                    let cover = if layer.square {
                        let ox = ((cx + r).min(fx + 0.5) - (cx - r).max(fx - 0.5)).max(0.0);
                        let oy = ((cy + r).min(fy + 0.5) - (cy - r).max(fy - 0.5)).max(0.0);
                        ox * oy
                    } else {
                        let d = (fx - cx).hypot(fy - cy);
                        (r + 0.5 - d).clamp(0.0, 1.0)
                    };
                    let i = (py as usize * iw as usize + px as usize) * 4;
                    blend(&mut img, i, rgb, cover);
                }
            }
        }
    }
    (img, iw, ih)
}

/// RGBA pixels as a PNG with its physical size (`pHYs`) set to `dpi`.
pub fn encode_png(rgba: &[u8], w: u32, h: u32, dpi: f64) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(Cursor::new(&mut out), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let ppm = (dpi / 0.0254).round() as u32;
        enc.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
        let mut writer = enc.write_header().expect("PNG header to memory");
        writer.write_image_data(rgba).expect("PNG data to memory");
    }
    out
}

/// The point layer of a figure: a transparent PNG of the plot box.
pub fn points_png(w: f64, h: f64, dpi: f64, layers: &[Dots]) -> Vec<u8> {
    let (rgba, iw, ih) = render_rgba(w, h, dpi, layers);
    encode_png(&rgba, iw, ih, dpi)
}

/// Decodes a PNG (as the UI sends for the clipboard) to 8-bit RGBA.
pub fn decode_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut dec = png::Decoder::new(Cursor::new(bytes));
    dec.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = dec
        .read_info()
        .map_err(|e| format!("Not a PNG image: {e}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or("The PNG image is too large")?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("The PNG image is damaged: {e}"))?;
    buf.truncate(info.buffer_size());
    let (w, h) = (info.width, info.height);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("Indexed PNG images aren't supported".into()),
    };
    Ok((rgba, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_round_trip_through_png() {
        let layers = [
            Dots {
                points: vec![(10.0, 10.0)],
                radius_px: 2.6,
                square: false,
                color: "#0072b2".into(),
            },
            Dots {
                points: vec![(30.0, 20.0)],
                radius_px: 0.75,
                square: true,
                color: "#d55e00".into(),
            },
        ];
        let png = points_png(40.0, 30.0, 300.0, &layers);
        let (rgba, w, h) = decode_rgba(&png).unwrap();
        assert_eq!((w, h), (125, 94));
        let px = |x: f64, y: f64| {
            let k = 300.0 / 96.0;
            let i = ((y * k) as usize * w as usize + (x * k) as usize) * 4;
            [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
        };
        assert_eq!(px(10.0, 10.0), [0x00, 0x72, 0xb2, 255]);
        assert_eq!(px(30.0, 20.0), [0xd5, 0x5e, 0x00, 255]);
        assert_eq!(px(20.0, 5.0)[3], 0, "transparent background");
        // 300 dpi is 11811 px per metre.
        let phys = png.windows(4).position(|w| w == b"pHYs").unwrap();
        let ppm = u32::from_be_bytes(png[phys + 4..phys + 8].try_into().unwrap());
        assert_eq!(ppm, 11811);
        assert!(decode_rgba(b"not a png").is_err());
    }
}
