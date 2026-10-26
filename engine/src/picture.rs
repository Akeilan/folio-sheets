//! Pictures placed on a sheet: the file bytes as they are kept in the workbook, their pixel
//! size, and (decoded once, when first drawn) premultiplied ARGB for cairo. PNG, JPEG, GIF and
//! BMP are read with the program's own decoders.

use std::cell::OnceCell;

#[derive(Debug)]
pub struct Picture {
    pub bytes: Vec<u8>,
    pub ext: &'static str,
    pub w: u32,
    pub h: u32,
    argb: OnceCell<Option<Vec<u8>>>,
}

impl PartialEq for Picture {
    fn eq(&self, o: &Self) -> bool {
        self.bytes == o.bytes
    }
}

pub fn sniff(b: &[u8]) -> Option<&'static str> {
    if crate::png::is_png(b) {
        Some("png")
    } else if crate::jpeg::is_jpeg(b) {
        Some("jpeg")
    } else if crate::gif::is_gif(b) {
        Some("gif")
    } else if crate::bmp::is_bmp(b) {
        Some("bmp")
    } else {
        None
    }
}

pub fn content_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn decode(b: &[u8]) -> Option<crate::png::Image> {
    match sniff(b)? {
        "png" => crate::png::decode(b).ok(),
        "jpeg" => crate::jpeg::decode(b).ok(),
        "gif" => crate::gif::decode(b).ok(),
        _ => crate::bmp::decode(b).ok(),
    }
}

impl Picture {
    /// A picture from file bytes; None when it isn't one we can read.
    pub fn new(bytes: Vec<u8>) -> Option<Picture> {
        let ext = sniff(&bytes)?;
        let img = decode(&bytes)?;
        if img.w == 0 || img.h == 0 {
            return None;
        }
        let p = Picture { w: img.w as u32, h: img.h as u32, ext, bytes, argb: OnceCell::new() };
        let _ = p.argb.set(Some(to_argb(&img.px)));
        Some(p)
    }

    /// Kept as read: decoded the first time it is drawn.
    pub fn lazy(bytes: Vec<u8>) -> Picture {
        let ext = sniff(&bytes).unwrap_or("png");
        Picture { w: 0, h: 0, ext, bytes, argb: OnceCell::new() }
    }

    /// Premultiplied ARGB32 rows (cairo's native order), with the size.
    pub fn argb(&self) -> Option<(&[u8], u32, u32)> {
        let px = self.argb.get_or_init(|| decode(&self.bytes).map(|i| to_argb(&i.px)));
        let px = px.as_deref()?;
        let (w, h) = if self.w > 0 { (self.w, self.h) } else {
            let i = decode(&self.bytes)?;
            (i.w as u32, i.h as u32)
        };
        if px.len() != (w * h * 4) as usize {
            return None;
        }
        Some((px, w, h))
    }

    /// The pixel size, decoding if needed.
    pub fn size(&self) -> Option<(u32, u32)> {
        if self.w > 0 {
            return Some((self.w, self.h));
        }
        decode(&self.bytes).map(|i| (i.w as u32, i.h as u32))
    }
}

fn to_argb(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for p in rgba.chunks_exact(4) {
        let a = p[3] as u32;
        let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
        // little-endian ARGB32: B, G, R, A in memory
        out.extend_from_slice(&[m(p[2]), m(p[1]), m(p[0]), p[3]]);
    }
    out
}
