use image::{imageops::FilterType, ImageBuffer, Rgba};

pub const MAX_INPUT_BYTES: u64 = 64 << 20;
pub const MAX_INPUT_SIDE: u32 = 8192;

#[derive(Clone, Debug)]
pub struct Rgba8 {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn fmt(format: u32) -> Result<gctex::TextureFormat, String> {
    match format {
        0..=6 | 14 => gctex::TextureFormat::from_u32(format).ok_or_else(|| "E:tex.format".into()),
        _ => Err("E:tex.format".into()),
    }
}

pub fn decode(data: &[u8], width: u32, height: u32, format: u32) -> Result<Rgba8, String> {
    let f = fmt(format)?;
    if data.len() < gctex::compute_image_size(f, width, height) as usize {
        return Err("E:file.format".into());
    }
    let pixels = match decode_direct(data, width, height, format) {
        Some(pixels) => pixels,
        None => gctex::decode(data, width, height, f, &[], 0),
    };
    Ok(Rgba8 { width, height, pixels })
}

fn decode_direct(data: &[u8], width: u32, height: u32, format: u32) -> Option<Vec<u8>> {
    fn c3(v: u16) -> u8 { ((v << 5) | (v << 2) | (v >> 1)) as u8 }
    fn c4(v: u16) -> u8 { (v * 17) as u8 }
    fn c5(v: u16) -> u8 { ((v << 3) | (v >> 2)) as u8 }
    fn c6(v: u16) -> u8 { ((v << 2) | (v >> 4)) as u8 }
    let (bw, bh, block_bytes) = match format {
        0 => (8, 8, 32),
        1 | 2 => (8, 4, 32),
        3..=5 => (4, 4, 32),
        6 => (4, 4, 64),
        _ => return None,
    };
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w.checked_mul(h)?.checked_mul(4)?];
    let mut offset = 0;
    for by in 0..h.div_ceil(bh) {
        for bx in 0..w.div_ceil(bw) {
            let block = data.get(offset..offset + block_bytes)?;
            offset += block_bytes;
            for py in 0..bh {
                for px in 0..bw {
                    let (x, y) = (bx * bw + px, by * bh + py);
                    if x >= w || y >= h { continue; }
                    let i = py * bw + px;
                    let be16 = || u16::from_be_bytes([block[2 * i], block[2 * i + 1]]);
                    let rgba = match format {
                        0 => {
                            let b = block[i / 2] as u16;
                            let c = c4(if i % 2 == 0 { b >> 4 } else { b & 15 });
                            [c, c, c, c]
                        }
                        1 => { let c = block[i]; [c, c, c, c] }
                        2 => { let b = block[i] as u16; let c = c4(b & 15); [c, c, c, c4(b >> 4)] }
                        3 => { let c = block[2 * i + 1]; [c, c, c, block[2 * i]] }
                        4 => { let v = be16(); [c5(v >> 11), c6((v >> 5) & 63), c5(v & 31), 255] }
                        5 => {
                            let v = be16();
                            if v & 0x8000 != 0 {
                                [c5((v >> 10) & 31), c5((v >> 5) & 31), c5(v & 31), 255]
                            } else {
                                [c4((v >> 8) & 15), c4((v >> 4) & 15), c4(v & 15), c3((v >> 12) & 7)]
                            }
                        }
                        _ => [block[2 * i + 1], block[32 + 2 * i], block[32 + 2 * i + 1], block[2 * i]],
                    };
                    out[(y * w + x) * 4..][..4].copy_from_slice(&rgba);
                }
            }
        }
    }
    Some(out)
}

fn half(src: &Rgba8) -> Rgba8 {
    let (w, h) = ((src.width / 2).max(1), (src.height / 2).max(1));
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            let mut wsum = 0u32;
            let mut rgb = [0u32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let sx = (x * 2 + dx).min(src.width - 1);
                let sy = (y * 2 + dy).min(src.height - 1);
                let p = ((sy * src.width + sx) * 4) as usize;
                let a = src.pixels[p + 3] as u32;
                for c in 0..3 {
                    rgb[c] += src.pixels[p + c] as u32 * a;
                    acc[c] += src.pixels[p + c] as u32;
                }
                acc[3] += a;
                wsum += a;
            }
            let o = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                out[o + c] = if wsum > 0 { ((rgb[c] + wsum / 2) / wsum) as u8 } else { ((acc[c] + 2) / 4) as u8 };
            }
            out[o + 3] = ((acc[3] + 2) / 4) as u8;
        }
    }
    Rgba8 { width: w, height: h, pixels: out }
}

pub fn encode(img: &Rgba8, format: u32, images: u32) -> Result<Vec<u8>, String> {
    let f = fmt(format)?;
    let mut out = Vec::new();
    let mut level = img.clone();
    for i in 0..images.max(1) {
        if i > 0 {
            level = half(&level);
        }
        out.extend(gctex::encode(f, &level.pixels, level.width, level.height));
    }
    Ok(out)
}

pub fn resize(img: &Rgba8, width: u32, height: u32) -> Rgba8 {
    if img.width == width && img.height == height {
        return img.clone();
    }
    let mut buf: ImageBuffer<Rgba<f32>, Vec<f32>> = ImageBuffer::new(img.width, img.height);
    for (i, p) in buf.pixels_mut().enumerate() {
        let s = &img.pixels[i * 4..i * 4 + 4];
        let a = s[3] as f32 / 255.0;
        *p = Rgba([s[0] as f32 / 255.0 * a, s[1] as f32 / 255.0 * a, s[2] as f32 / 255.0 * a, a]);
    }
    let shrinking = width < img.width || height < img.height;
    let filter = if shrinking { FilterType::Lanczos3 } else { FilterType::CatmullRom };
    let out = image::imageops::resize(&buf, width, height, filter);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for p in out.pixels() {
        let a = p[3].clamp(0.0, 1.0);
        for c in 0..3 {
            let v = if a > 1e-6 { p[c] / a } else { 0.0 };
            pixels.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        pixels.push((a * 255.0).round() as u8);
    }
    Rgba8 { width, height, pixels }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Recolor {
    pub hue: f32,
    pub saturation: f32,
    pub brightness: f32,
}

impl Recolor {
    pub fn is_identity(&self) -> bool {
        self.hue.abs() < 0.01 && self.saturation.abs() < 0.01 && self.brightness.abs() < 0.01
    }
    pub fn clamped(self) -> Self {
        let c = |v: f32, m: f32| if v.is_finite() { v.clamp(-m, m) } else { 0.0 };
        Recolor { hue: c(self.hue, 180.0), saturation: c(self.saturation, 100.0), brightness: c(self.brightness, 100.0) }
    }
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (r + m, g + m, b + m)
}

pub fn recolor(img: &Rgba8, rc: Recolor) -> Rgba8 {
    let rc = rc.clamped();
    if rc.is_identity() {
        return img.clone();
    }
    let sat = 1.0 + rc.saturation / 100.0;
    let br = rc.brightness / 100.0;
    let mut out = img.pixels.clone();
    for px in out.chunks_exact_mut(4) {
        let (h, s, v) = rgb_to_hsv(px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0);
        let (mut r, mut g, mut b) = hsv_to_rgb(h + rc.hue, s, v);
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        for c in [&mut r, &mut g, &mut b] {
            *c = luma + (*c - luma) * sat;
            *c = if br >= 0.0 { *c + (1.0 - *c) * br } else { *c * (1.0 + br) };
        }
        px[0] = (r.clamp(0.0, 1.0) * 255.0).round() as u8;
        px[1] = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
        px[2] = (b.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    Rgba8 { width: img.width, height: img.height, pixels: out }
}

pub fn png(img: &Rgba8) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::write_buffer_with_format(&mut out, &img.pixels, img.width, img.height, image::ExtendedColorType::Rgba8, image::ImageFormat::Png)
        .map_err(|_| "E:image.encode")?;
    Ok(out.into_inner())
}

pub fn data_url(img: &Rgba8) -> Result<String, String> {
    use base64::Engine;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png(img)?)))
}

pub fn load(bytes: &[u8]) -> Result<Rgba8, String> {
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("E:image.tooLarge".into());
    }
    let format = image::guess_format(bytes).map_err(|_| "E:image.unsupported")?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("E:image.unsupported".into());
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_INPUT_SIDE);
    limits.max_image_height = Some(MAX_INPUT_SIDE);
    limits.max_alloc = Some(512 << 20);
    reader.limits(limits);
    let img = reader.decode().map_err(|_| "E:image.decode")?.to_rgba8();
    Ok(Rgba8 { width: img.width(), height: img.height(), pixels: img.into_raw() })
}

pub fn mean_abs_diff(a: &Rgba8, b: &Rgba8) -> f64 {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let sum: u64 = a.pixels.iter().zip(&b.pixels).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum();
    sum as f64 / a.pixels.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> Rgba8 {
        let mut p = Vec::new();
        for y in 0..h {
            for x in 0..w {
                p.extend_from_slice(&[(x * 255 / w) as u8, (y * 255 / h) as u8, 128, 255]);
            }
        }
        Rgba8 { width: w, height: h, pixels: p }
    }

    #[test]
    fn encode_sizes_match_gctex_mip_sizes() {
        let img = gradient(64, 32);
        for (format, images) in [(14u32, 1u32), (14, 3), (1, 1), (3, 2), (4, 1), (5, 4), (6, 1)] {
            let enc = encode(&img, format, images).unwrap();
            assert_eq!(enc.len(), crate::brres::data_len(format, 64, 32, images).unwrap(), "fmt {format} x{images}");
        }
    }

    #[test]
    fn cmpr_round_trip_is_close() {
        let img = gradient(64, 64);
        let enc = encode(&img, 14, 1).unwrap();
        let dec = decode(&enc, 64, 64, 14).unwrap();
        assert!(mean_abs_diff(&img, &dec) < 6.0);
        assert!(dec.pixels.iter().any(|&b| b != 0), "CMPR decode must not be blank");
    }

    #[test]
    fn direct_formats_decode_known_texels() {
        let block = |head: &[u8], len: usize| { let mut b = vec![0u8; len]; b[..head.len()].copy_from_slice(head); b };
        let px = |format: u32, w: u32, h: u32, data: &[u8]| decode(data, w, h, format).unwrap().pixels[..8].to_vec();
        assert_eq!(px(0, 8, 8, &block(&[0xf0], 32)), [255, 255, 255, 255, 0, 0, 0, 0], "I4");
        assert_eq!(px(1, 8, 4, &block(&[0x80, 0x10], 32)), [128, 128, 128, 128, 16, 16, 16, 16], "I8");
        assert_eq!(px(2, 8, 4, &block(&[0xf3], 32)), [51, 51, 51, 255, 0, 0, 0, 0], "IA4");
        assert_eq!(px(3, 4, 4, &block(&[0x40, 0x80], 32)), [128, 128, 128, 64, 0, 0, 0, 0], "IA8");
        assert_eq!(px(4, 4, 4, &block(&[0xf8, 0x00], 32)), [255, 0, 0, 255, 0, 0, 0, 255], "RGB565");
        assert_eq!(px(5, 4, 4, &block(&[0xfc, 0x00, 0x70, 0xf0], 32)), [255, 0, 0, 255, 0, 255, 0, 255], "RGB5A3");
        let mut rgba8 = vec![0u8; 64];
        rgba8[..2].copy_from_slice(&[0x80, 0x10]);
        rgba8[32..34].copy_from_slice(&[0x20, 0x30]);
        assert_eq!(px(6, 4, 4, &rgba8), [16, 32, 48, 128, 0, 0, 0, 0], "RGBA8");
        assert_eq!(decode(&block(&[0xfc, 0x00], 32), 3, 2, 5).unwrap().pixels.len(), 3 * 2 * 4);
    }

    #[cfg(windows)]
    #[test]
    fn direct_formats_match_gctex() {
        let mut seed = 0x1234_5678u32;
        for format in 0..=6u32 {
            for (w, h) in [(8u32, 8u32), (16, 12), (5, 3), (64, 32)] {
                let f = gctex::TextureFormat::from_u32(format).unwrap();
                let data: Vec<u8> = (0..gctex::compute_image_size(f, w, h)).map(|_| { seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223); (seed >> 24) as u8 }).collect();
                let ours = decode_direct(&data, w, h, format).unwrap();
                let theirs = gctex::decode(&data, w, h, f, &[], 0);
                let first = ours.iter().zip(&theirs).position(|(a, b)| a != b);
                assert!(first.is_none() && ours.len() == theirs.len(), "format {format} {w}x{h}: first difference at byte {first:?}");
            }
        }
    }

    #[test]
    fn recolor_hue_180_turns_red_cyan_and_keeps_alpha() {
        let img = Rgba8 { width: 1, height: 1, pixels: vec![255, 0, 0, 77] };
        let out = recolor(&img, Recolor { hue: 180.0, saturation: 0.0, brightness: 0.0 });
        assert_eq!(out.pixels, vec![0, 255, 255, 77]);
        let dark = recolor(&img, Recolor { hue: 0.0, saturation: 0.0, brightness: -100.0 });
        assert_eq!(dark.pixels, vec![0, 0, 0, 77]);
        let gray = recolor(&img, Recolor { hue: 0.0, saturation: -100.0, brightness: 0.0 });
        assert_eq!(gray.pixels, vec![76, 76, 76, 77]);
    }

    #[test]
    fn resize_exact_dimensions_and_png_load_round_trip() {
        let img = gradient(300, 170);
        let r = resize(&img, 256, 128);
        assert_eq!((r.width, r.height, r.pixels.len()), (256, 128, 256 * 128 * 4));
        let loaded = load(&png(&r).unwrap()).unwrap();
        assert_eq!(loaded.pixels, r.pixels);
        assert!(load(b"GIF89a....").is_err());
    }
}
