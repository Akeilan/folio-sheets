//! From-scratch PNG decoder (all colour types, bit depths 1-16, Adam7) and encoder
//! (palette when it fits, otherwise RGB/RGBA; adaptive per-row filtering).

use crate::deflate::{crc32_update, zlib_compress, zlib_decompress_sized};
use std::collections::HashMap;

pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>, // straight RGBA8
}

const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

pub fn is_png(data: &[u8]) -> bool {
    data.len() >= 8 && data[..8] == SIG
}

fn be32(d: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]])
}

pub fn decode(data: &[u8]) -> Result<Image, String> {
    if !is_png(data) {
        return Err("not a PNG file".into());
    }
    let mut pos = 8;
    let (mut w, mut h, mut depth, mut ctype, mut interlace) = (0usize, 0usize, 0u8, 0u8, 0u8);
    let mut have_ihdr = false;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut trns_gray: Option<u16> = None;
    let mut trns_rgb: Option<(u16, u16, u16)> = None;
    let mut idat: Vec<u8> = Vec::new();
    while pos + 12 <= data.len() {
        let len = be32(data, pos) as usize;
        let ty = &data[pos + 4..pos + 8];
        let start = pos + 8;
        if start + len + 4 > data.len() {
            if ty == b"IDAT" {
                idat.extend_from_slice(&data[start..data.len().min(start + len)]);
            }
            break;
        }
        let body = &data[start..start + len];
        match ty {
            b"IHDR" => {
                if len < 13 {
                    return Err("PNG: bad IHDR".into());
                }
                w = be32(body, 0) as usize;
                h = be32(body, 4) as usize;
                depth = body[8];
                ctype = body[9];
                interlace = body[12];
                have_ihdr = true;
            }
            b"PLTE" => {
                palette = body.chunks_exact(3).map(|c| [c[0], c[1], c[2], 255]).collect();
            }
            b"tRNS" => match ctype {
                3 => {
                    for (i, &a) in body.iter().enumerate() {
                        if i < palette.len() {
                            palette[i][3] = a;
                        }
                    }
                }
                0 if len >= 2 => trns_gray = Some(u16::from_be_bytes([body[0], body[1]])),
                2 if len >= 6 => {
                    trns_rgb = Some((
                        u16::from_be_bytes([body[0], body[1]]),
                        u16::from_be_bytes([body[2], body[3]]),
                        u16::from_be_bytes([body[4], body[5]]),
                    ))
                }
                _ => {}
            },
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos = start + len + 4;
    }
    if !have_ihdr || w == 0 || h == 0 {
        return Err("PNG: missing or empty header".into());
    }
    if w > 32768 || h > 32768 {
        return Err("PNG: image is too large".into());
    }
    let channels = match ctype {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return Err("PNG: unsupported colour type".into()),
    };
    if ![1, 2, 4, 8, 16].contains(&depth) {
        return Err("PNG: unsupported bit depth".into());
    }
    let bpp_bits = channels * depth as usize;
    let fbpp = (bpp_bits / 8).max(1);
    let raw = zlib_decompress_sized(&idat, h * (w * bpp_bits / 8 + 2))?;
    let mut px = vec![0u8; w * h * 4];
    let passes: Vec<(usize, usize, usize, usize)> = if interlace == 1 {
        vec![(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
    } else {
        vec![(0, 0, 1, 1)]
    };
    let mut rp = 0usize;
    for (x0, y0, dx, dy) in passes {
        if x0 >= w || y0 >= h {
            continue;
        }
        let pw = (w - x0 + dx - 1) / dx;
        let ph = (h - y0 + dy - 1) / dy;
        let rowbytes = (pw * bpp_bits + 7) / 8;
        let mut prev = vec![0u8; rowbytes];
        let mut cur = vec![0u8; rowbytes];
        for r in 0..ph {
            if rp + 1 + rowbytes > raw.len() {
                // truncated: keep what decoded so far
                return Ok(Image { w, h, px });
            }
            let ft = raw[rp];
            cur.copy_from_slice(&raw[rp + 1..rp + 1 + rowbytes]);
            rp += 1 + rowbytes;
            unfilter(ft, &mut cur, &prev, fbpp)?;
            let y = y0 + r * dy;
            for i in 0..pw {
                let x = x0 + i * dx;
                let o = (y * w + x) * 4;
                let rgba = sample(&cur, i, ctype, depth, &palette, trns_gray, trns_rgb);
                px[o..o + 4].copy_from_slice(&rgba);
            }
            std::mem::swap(&mut prev, &mut cur);
        }
    }
    Ok(Image { w, h, px })
}

fn unfilter(ft: u8, cur: &mut [u8], prev: &[u8], bpp: usize) -> Result<(), String> {
    let n = cur.len();
    match ft {
        0 => {}
        1 => {
            for i in bpp..n {
                cur[i] = cur[i].wrapping_add(cur[i - bpp]);
            }
        }
        2 => {
            for i in 0..n {
                cur[i] = cur[i].wrapping_add(prev[i]);
            }
        }
        3 => {
            for i in 0..n {
                let a = if i >= bpp { cur[i - bpp] as u16 } else { 0 };
                cur[i] = cur[i].wrapping_add(((a + prev[i] as u16) / 2) as u8);
            }
        }
        4 => {
            for i in 0..n {
                let a = if i >= bpp { cur[i - bpp] } else { 0 };
                let c = if i >= bpp { prev[i - bpp] } else { 0 };
                cur[i] = cur[i].wrapping_add(paeth(a, prev[i], c));
            }
        }
        _ => return Err("PNG: bad filter type".into()),
    }
    Ok(())
}

#[inline]
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let pa = (p - a as i16).abs();
    let pb = (p - b as i16).abs();
    let pc = (p - c as i16).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn read_val(row: &[u8], idx: usize, depth: u8) -> u16 {
    match depth {
        16 => u16::from_be_bytes([row[idx * 2], row[idx * 2 + 1]]),
        8 => row[idx] as u16,
        d => {
            let bit = idx * d as usize;
            let byte = row[bit / 8];
            let shift = 8 - d as usize - (bit % 8);
            ((byte >> shift) & ((1u8 << d) - 1)) as u16
        }
    }
}

fn to8(v: u16, depth: u8) -> u8 {
    match depth {
        16 => (v >> 8) as u8,
        8 => v as u8,
        4 => (v * 17) as u8,
        2 => (v * 85) as u8,
        _ => (v * 255) as u8,
    }
}

fn sample(row: &[u8], i: usize, ctype: u8, depth: u8, pal: &[[u8; 4]], tg: Option<u16>, trgb: Option<(u16, u16, u16)>) -> [u8; 4] {
    match ctype {
        0 => {
            let v = read_val(row, i, depth);
            let g = to8(v, depth);
            let a = if tg == Some(v) { 0 } else { 255 };
            [g, g, g, a]
        }
        2 => {
            let r = read_val(row, i * 3, depth);
            let g = read_val(row, i * 3 + 1, depth);
            let b = read_val(row, i * 3 + 2, depth);
            let a = if trgb == Some((r, g, b)) { 0 } else { 255 };
            [to8(r, depth), to8(g, depth), to8(b, depth), a]
        }
        3 => {
            let v = read_val(row, i, depth) as usize;
            if v < pal.len() {
                pal[v]
            } else {
                [0, 0, 0, 255]
            }
        }
        4 => {
            let g = to8(read_val(row, i * 2, depth), depth);
            [g, g, g, to8(read_val(row, i * 2 + 1, depth), depth)]
        }
        _ => [
            to8(read_val(row, i * 4, depth), depth),
            to8(read_val(row, i * 4 + 1, depth), depth),
            to8(read_val(row, i * 4 + 2, depth), depth),
            to8(read_val(row, i * 4 + 3, depth), depth),
        ],
    }
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(body);
    let crc = crc32_update(crc32_update(0, ty), body);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Encode straight RGBA8 pixels. Picks the smallest suitable colour type automatically.
/// Fully transparent pixels are normalised to (0,0,0,0) so they compress well and never
/// leak hidden colour.
pub fn encode(w: usize, h: usize, rgba: &[u8], level: u32) -> Vec<u8> {
    let mut opaque = true;
    let mut palette: Vec<u32> = Vec::new();
    let mut index: HashMap<u32, u8> = HashMap::new();
    let mut palette_ok = true;
    for p in rgba.chunks_exact(4) {
        let key = if p[3] == 0 { 0 } else { u32::from_le_bytes([p[0], p[1], p[2], p[3]]) };
        if p[3] != 255 {
            opaque = false;
        }
        if palette_ok && !index.contains_key(&key) {
            if palette.len() == 256 {
                palette_ok = false;
            } else {
                index.insert(key, palette.len() as u8);
                palette.push(key);
            }
        }
    }
    // tiny images: indexed overhead is not worth it; big flat images benefit hugely
    let use_palette = palette_ok && w * h >= 64;
    let (ctype, bpp, rowlen) = if use_palette {
        (3u8, 1usize, w)
    } else if opaque {
        (2u8, 3usize, w * 3)
    } else {
        (6u8, 4usize, w * 4)
    };
    if use_palette {
        // sort palette so transparent entries come first (shorter tRNS chunk)
        let mut order: Vec<u32> = palette.clone();
        order.sort_by_key(|&c| (c >> 24) as u8 == 255);
        index.clear();
        for (i, &c) in order.iter().enumerate() {
            index.insert(c, i as u8);
        }
        palette = order;
    }
    let mut raw: Vec<u8> = Vec::with_capacity(h * (rowlen + 1));
    let mut prev = vec![0u8; rowlen];
    let mut row = vec![0u8; rowlen];
    let mut cand = vec![0u8; rowlen];
    let mut best = vec![0u8; rowlen];
    for y in 0..h {
        let src = &rgba[y * w * 4..(y + 1) * w * 4];
        for x in 0..w {
            let p = &src[x * 4..x * 4 + 4];
            match ctype {
                3 => {
                    let key = if p[3] == 0 { 0 } else { u32::from_le_bytes([p[0], p[1], p[2], p[3]]) };
                    row[x] = index[&key];
                }
                2 => row[x * 3..x * 3 + 3].copy_from_slice(&p[..3]),
                _ => {
                    if p[3] == 0 {
                        row[x * 4..x * 4 + 4].copy_from_slice(&[0, 0, 0, 0]);
                    } else {
                        row[x * 4..x * 4 + 4].copy_from_slice(p);
                    }
                }
            }
        }
        if ctype == 3 {
            raw.push(0);
            raw.extend_from_slice(&row);
        } else {
            // adaptive filter: minimum sum of absolute signed differences
            let mut best_ft = 0u8;
            let mut best_score = u64::MAX;
            for ft in 0..5u8 {
                for i in 0..rowlen {
                    let a = if i >= bpp { row[i - bpp] } else { 0 };
                    let b = prev[i];
                    let c = if i >= bpp { prev[i - bpp] } else { 0 };
                    let pred = match ft {
                        0 => 0,
                        1 => a,
                        2 => b,
                        3 => ((a as u16 + b as u16) / 2) as u8,
                        _ => paeth(a, b, c),
                    };
                    cand[i] = row[i].wrapping_sub(pred);
                }
                let score: u64 = cand.iter().map(|&v| (v as i8).unsigned_abs() as u64).sum();
                if score < best_score {
                    best_score = score;
                    best_ft = ft;
                    best.copy_from_slice(&cand);
                }
            }
            raw.push(best_ft);
            raw.extend_from_slice(&best);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    let z = zlib_compress(&raw, level);
    let mut out = Vec::with_capacity(z.len() + 1024);
    out.extend_from_slice(&SIG);
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, ctype, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    if ctype == 3 {
        let mut plte = Vec::with_capacity(palette.len() * 3);
        let mut trns = Vec::new();
        for &c in &palette {
            let b = c.to_le_bytes();
            plte.extend_from_slice(&b[..3]);
            if b[3] != 255 {
                trns.push(b[3]);
            }
        }
        chunk(&mut out, b"PLTE", &plte);
        if !trns.is_empty() {
            chunk(&mut out, b"tRNS", &trns);
        }
    }
    for part in z.chunks(1 << 18) {
        chunk(&mut out, b"IDAT", part);
    }
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_rgba_and_palette() {
        let (w, h) = (37, 23);
        let mut px = vec![0u8; w * h * 4];
        for (i, p) in px.chunks_exact_mut(4).enumerate() {
            p.copy_from_slice(&[(i * 7) as u8, (i * 3) as u8, (i * 11) as u8, (i * 5) as u8]);
            if p[3] == 0 {
                p.copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        let enc = encode(w, h, &px, 6);
        let dec = decode(&enc).unwrap();
        assert_eq!(dec.px, px);
        // flat image → palette path
        let flat: Vec<u8> = (0..w * h).flat_map(|i| if i % 2 == 0 { [255, 0, 0, 255] } else { [0, 0, 0, 0] }).collect();
        let enc = encode(w, h, &flat, 6);
        assert_eq!(decode(&enc).unwrap().px, flat);
    }
}
