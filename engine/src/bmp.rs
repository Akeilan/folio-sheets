//! From-scratch Windows bitmap codec: reads 1/4/8/16/24/32-bit, BI_RGB, BI_BITFIELDS,
//! RLE4/RLE8, OS/2 and V1-V5 headers, top-down and bottom-up; writes 24-bit or 32-bit
//! (V4 header with an alpha mask, so transparency survives).

use crate::png::Image;

fn u16le(d: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([d[i], d[i + 1]])
}
fn u32le(d: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]])
}

pub fn is_bmp(d: &[u8]) -> bool {
    d.len() > 26 && &d[..2] == b"BM"
}

fn mask_info(m: u32) -> (u32, u32) {
    if m == 0 {
        return (0, 0);
    }
    (m.trailing_zeros(), m.count_ones())
}
fn extract(px: u32, m: u32, sh: u32, bits: u32) -> u8 {
    if bits == 0 {
        return 0;
    }
    let v = (px & m) >> sh;
    let max = if bits >= 32 { u32::MAX } else { (1u32 << bits) - 1 };
    ((v as u64 * 255 + max as u64 / 2) / max as u64) as u8
}

pub fn decode(d: &[u8]) -> Result<Image, String> {
    if !is_bmp(d) {
        return Err("not a BMP file".into());
    }
    let off = u32le(d, 10) as usize;
    let hsz = u32le(d, 14) as usize;
    let (w, h_signed, bpp, comp, clrused);
    if hsz == 12 {
        w = u16le(d, 18) as i64;
        h_signed = u16le(d, 20) as i16 as i64;
        bpp = u16le(d, 24) as u32;
        comp = 0;
        clrused = 0;
    } else {
        if d.len() < 14 + 40 {
            return Err("BMP: truncated header".into());
        }
        w = u32le(d, 18) as i32 as i64;
        h_signed = u32le(d, 22) as i32 as i64;
        bpp = u16le(d, 28) as u32;
        comp = u32le(d, 30);
        clrused = u32le(d, 46) as usize;
    }
    if w <= 0 || h_signed == 0 || w > 32768 || h_signed.abs() > 32768 {
        return Err("BMP: bad dimensions".into());
    }
    let w = w as usize;
    let top_down = h_signed < 0;
    let h = h_signed.unsigned_abs() as usize;
    // channel masks
    let (mut rm, mut gm, mut bm, mut am) = match bpp {
        16 => (0x7c00, 0x03e0, 0x001f, 0),
        _ => (0x00ff_0000u32, 0x0000_ff00, 0x0000_00ff, 0),
    };
    if comp == 3 || comp == 6 {
        let base = if hsz >= 52 { 14 + 40 } else { 14 + hsz };
        if d.len() >= base + 12 {
            rm = u32le(d, base);
            gm = u32le(d, base + 4);
            bm = u32le(d, base + 8);
            if (hsz >= 56 || comp == 6) && d.len() >= base + 16 {
                am = u32le(d, base + 12);
            }
        }
    } else if hsz >= 56 && bpp == 32 && d.len() >= 14 + 56 {
        am = u32le(d, 14 + 52);
    }
    // palette
    let mut pal: Vec<[u8; 4]> = Vec::new();
    if bpp <= 8 {
        let entry = if hsz == 12 { 3 } else { 4 };
        let count = if clrused > 0 { clrused.min(256) } else { 1 << bpp };
        let mut p = 14 + hsz + if comp == 3 && hsz == 40 { 12 } else { 0 };
        for _ in 0..count {
            if p + 3 > d.len() {
                break;
            }
            pal.push([d[p + 2], d[p + 1], d[p], 255]);
            p += entry;
        }
    }
    let mut px = vec![0u8; w * h * 4];
    let put = |px: &mut Vec<u8>, x: usize, row: usize, c: [u8; 4]| {
        if x < w && row < h {
            let y = if top_down { row } else { h - 1 - row };
            let o = (y * w + x) * 4;
            px[o..o + 4].copy_from_slice(&c);
        }
    };
    let palc = |i: usize| -> [u8; 4] { pal.get(i).copied().unwrap_or([0, 0, 0, 255]) };

    if comp == 1 || comp == 2 {
        // RLE8 / RLE4; rows are bottom-up
        let rle4 = comp == 2;
        let (mut x, mut row, mut p) = (0usize, 0usize, off);
        while p + 1 < d.len() {
            let a = d[p] as usize;
            let b = d[p + 1];
            p += 2;
            if a > 0 {
                for k in 0..a {
                    let idx = if rle4 { if k % 2 == 0 { b >> 4 } else { b & 15 } } else { b };
                    put(&mut px, x, row, palc(idx as usize));
                    x += 1;
                }
            } else {
                match b {
                    0 => {
                        x = 0;
                        row += 1;
                    }
                    1 => break,
                    2 => {
                        if p + 1 >= d.len() {
                            break;
                        }
                        x += d[p] as usize;
                        row += d[p + 1] as usize;
                        p += 2;
                    }
                    n => {
                        let n = n as usize;
                        for k in 0..n {
                            let idx = if rle4 {
                                let byte = *d.get(p + k / 2).unwrap_or(&0);
                                if k % 2 == 0 { byte >> 4 } else { byte & 15 }
                            } else {
                                *d.get(p + k).unwrap_or(&0)
                            };
                            put(&mut px, x, row, palc(idx as usize));
                            x += 1;
                        }
                        let bytes = if rle4 { (n + 1) / 2 } else { n };
                        p += (bytes + 1) & !1;
                    }
                }
            }
        }
        // RLE rows are always bottom-up: fix orientation if we treated them as stored-order
        return Ok(Image { w, h, px });
    }

    let stride = ((w * bpp as usize + 31) / 32) * 4;
    let (rs, rb) = mask_info(rm);
    let (gs, gb) = mask_info(gm);
    let (bs, bb) = mask_info(bm);
    let (as_, ab) = mask_info(am);
    let mut any_alpha = false;
    for row in 0..h {
        let rp = off + row * stride;
        if rp + stride > d.len() {
            break;
        }
        let r = &d[rp..rp + stride];
        for x in 0..w {
            let c = match bpp {
                1 => palc(((r[x / 8] >> (7 - x % 8)) & 1) as usize),
                4 => palc(((r[x / 2] >> if x % 2 == 0 { 4 } else { 0 }) & 15) as usize),
                8 => palc(r[x] as usize),
                16 => {
                    let v = u16le(r, x * 2) as u32;
                    [extract(v, rm, rs, rb), extract(v, gm, gs, gb), extract(v, bm, bs, bb), if ab > 0 { extract(v, am, as_, ab) } else { 255 }]
                }
                24 => [r[x * 3 + 2], r[x * 3 + 1], r[x * 3], 255],
                32 => {
                    let v = u32le(r, x * 4);
                    let a = if ab > 0 {
                        extract(v, am, as_, ab)
                    } else {
                        // BI_RGB 32-bit: the 4th byte is officially unused, but many writers
                        // store alpha there. Collect it and decide after the scan.
                        let a = (v >> 24) as u8;
                        if a != 0 {
                            any_alpha = true;
                        }
                        a
                    };
                    [extract(v, rm, rs, rb), extract(v, gm, gs, gb), extract(v, bm, bs, bb), a]
                }
                _ => return Err(format!("BMP: unsupported bit depth {bpp}")),
            };
            put(&mut px, x, row, c);
        }
    }
    if bpp == 32 && ab == 0 && !any_alpha {
        for p in px.chunks_exact_mut(4) {
            p[3] = 255;
        }
    }
    Ok(Image { w, h, px })
}

/// Encode as BMP. `alpha` = true writes 32-bit BGRA with a V4 header; otherwise 24-bit with
/// transparency flattened onto `matte` (r,g,b).
pub fn encode(w: usize, h: usize, rgba: &[u8], alpha: bool, matte: [u8; 3]) -> Vec<u8> {
    let bpp = if alpha { 32 } else { 24 };
    let stride = ((w * bpp + 31) / 32) * 4;
    let hsz = if alpha { 108 } else { 40 };
    let off = 14 + hsz;
    let size = off + stride * h;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(off as u32).to_le_bytes());
    out.extend_from_slice(&(hsz as u32).to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(bpp as u16).to_le_bytes());
    out.extend_from_slice(&(if alpha { 3u32 } else { 0 }).to_le_bytes());
    out.extend_from_slice(&((stride * h) as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    if alpha {
        for m in [0x00ff_0000u32, 0x0000_ff00, 0x0000_00ff, 0xff00_0000] {
            out.extend_from_slice(&m.to_le_bytes());
        }
        out.extend_from_slice(b"BGRs"); // LCS_sRGB, stored little-endian as 'sRGB'
        out.extend_from_slice(&[0u8; 36 + 12]);
    }
    for row in 0..h {
        let y = h - 1 - row;
        let start = out.len();
        for x in 0..w {
            let p = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            if alpha {
                out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            } else {
                let a = p[3] as u32;
                let mix = |c: u8, m: u8| ((c as u32 * a + m as u32 * (255 - a) + 127) / 255) as u8;
                out.extend_from_slice(&[mix(p[2], matte[2]), mix(p[1], matte[1]), mix(p[0], matte[0])]);
            }
        }
        while out.len() - start < stride {
            out.push(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let (w, h) = (5, 3);
        let px: Vec<u8> = (0..w * h * 4).map(|i| (i * 13) as u8).collect();
        let e = encode(w, h, &px, true, [255, 255, 255]);
        assert_eq!(decode(&e).unwrap().px, px);
        let e = encode(w, h, &px, false, [255, 255, 255]);
        let d = decode(&e).unwrap();
        assert!(d.px.chunks_exact(4).all(|p| p[3] == 255));
    }
}
