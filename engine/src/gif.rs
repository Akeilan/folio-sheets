//! From-scratch GIF codec. Reads the first frame (composited on a transparent logical
//! screen, honouring transparency and interlacing). Writes GIF89a with exact colours when
//! the picture has 256 or fewer, otherwise median-cut quantisation with Floyd–Steinberg
//! dithering. Pixels under 50% alpha become the transparent colour.

use crate::png::Image;
use std::collections::HashMap;

pub fn is_gif(d: &[u8]) -> bool {
    d.len() >= 6 && (&d[..6] == b"GIF87a" || &d[..6] == b"GIF89a")
}

fn lzw_decode(data: &[u8], min_code: u8, max_out: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(max_out);
    if !(1..=11).contains(&min_code) {
        return out;
    }
    let clear = 1usize << min_code;
    let eoi = clear + 1;
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let mut length = vec![0u16; 4096];
    for i in 0..clear {
        suffix[i] = i as u8;
        length[i] = 1;
    }
    let mut size = min_code as u32 + 1;
    let mut next = eoi + 1;
    let mut prev: Option<usize> = None;
    let (mut buf, mut cnt, mut pos) = (0u32, 0u32, 0usize);
    let mut stack: Vec<u8> = Vec::with_capacity(4096);
    loop {
        while cnt < size {
            if pos >= data.len() {
                return out;
            }
            buf |= (data[pos] as u32) << cnt;
            pos += 1;
            cnt += 8;
        }
        let code = (buf & ((1 << size) - 1)) as usize;
        buf >>= size;
        cnt -= size;
        if code == clear {
            size = min_code as u32 + 1;
            next = eoi + 1;
            prev = None;
            continue;
        }
        if code == eoi {
            break;
        }
        let first;
        if code < next {
            // emit code's string
            stack.clear();
            let mut c = code;
            while c >= clear {
                stack.push(suffix[c]);
                c = prefix[c] as usize;
            }
            stack.push(suffix[c]);
            first = suffix[c];
            out.extend(stack.iter().rev());
        } else if code == next {
            let Some(p) = prev else { return out };
            stack.clear();
            let mut c = p;
            while c >= clear {
                stack.push(suffix[c]);
                c = prefix[c] as usize;
            }
            stack.push(suffix[c]);
            first = suffix[c];
            out.extend(stack.iter().rev());
            out.push(first);
        } else {
            return out; // corrupt
        }
        if let Some(p) = prev {
            if next < 4096 {
                prefix[next] = p as u16;
                suffix[next] = first;
                length[next] = length[p] + 1;
                next += 1;
                if next == (1 << size) && size < 12 {
                    size += 1;
                }
            }
        }
        prev = Some(code);
        if out.len() >= max_out {
            break;
        }
    }
    out
}

pub struct Frames {
    pub w: usize,
    pub h: usize,
    /// Fully composited frames (what a viewer shows), with their durations in ms.
    pub frames: Vec<(Vec<u8>, u32)>,
    /// NETSCAPE loop count (0 = forever); None = no loop extension, plays once.
    pub loop_count: Option<u16>,
    /// True when frames were dropped to stay within memory limits.
    pub truncated: bool,
}

const MAX_FRAME_BYTES: usize = 1 << 30;

/// Decode every frame, applying GIF disposal so each frame is a complete picture.
pub fn decode_frames(d: &[u8]) -> Result<Frames, String> {
    if !is_gif(d) || d.len() < 13 {
        return Err("not a GIF file".into());
    }
    let mut sw = u16::from_le_bytes([d[6], d[7]]) as usize;
    let mut sh = u16::from_le_bytes([d[8], d[9]]) as usize;
    let flags = d[10];
    let mut pos = 13;
    let mut gct: Vec<[u8; 3]> = Vec::new();
    if flags & 0x80 != 0 {
        let n = 2usize << (flags & 7);
        for i in 0..n {
            let o = pos + i * 3;
            if o + 3 > d.len() {
                break;
            }
            gct.push([d[o], d[o + 1], d[o + 2]]);
        }
        pos += n * 3;
    }
    // some writers leave the logical screen at 0x0; size it from the first frame instead
    if sw == 0 || sh == 0 {
        let mut p = pos;
        while p + 10 <= d.len() {
            if d[p] == 0x2C {
                sw = sw.max(u16::from_le_bytes([d[p + 1], d[p + 2]]) as usize + u16::from_le_bytes([d[p + 5], d[p + 6]]) as usize);
                sh = sh.max(u16::from_le_bytes([d[p + 3], d[p + 4]]) as usize + u16::from_le_bytes([d[p + 7], d[p + 8]]) as usize);
                break;
            }
            p += 1;
        }
    }
    let (w, h) = (sw.max(1), sh.max(1));
    if w > 32768 || h > 32768 {
        return Err("GIF: image is too large".into());
    }
    let mut out = Frames { w, h, frames: Vec::new(), loop_count: None, truncated: false };
    let mut canvas = vec![0u8; w * h * 4];
    let (mut transparent, mut disposal, mut delay): (Option<u8>, u8, u32) = (None, 0, 0);
    while pos < d.len() {
        match d[pos] {
            0x21 => {
                let label = *d.get(pos + 1).unwrap_or(&0);
                pos += 2;
                if label == 0xF9 && pos + 5 < d.len() && d[pos] >= 4 {
                    let packed = d[pos + 1];
                    disposal = (packed >> 2) & 7;
                    delay = u16::from_le_bytes([d[pos + 2], d[pos + 3]]) as u32 * 10;
                    transparent = if packed & 1 != 0 { Some(d[pos + 4]) } else { None };
                } else if label == 0xFF && pos + 12 <= d.len() && d[pos] == 11 && (&d[pos + 1..pos + 12] == b"NETSCAPE2.0" || &d[pos + 1..pos + 12] == b"ANIMEXTS1.0") {
                    let sub = pos + 12;
                    if sub + 3 < d.len() && d[sub] >= 3 && d[sub + 1] == 1 {
                        out.loop_count = Some(u16::from_le_bytes([d[sub + 2], d[sub + 3]]));
                    }
                }
                while pos < d.len() && d[pos] != 0 {
                    pos += d[pos] as usize + 1;
                }
                pos += 1;
            }
            0x2C => {
                if pos + 10 > d.len() {
                    break;
                }
                let ix = u16::from_le_bytes([d[pos + 1], d[pos + 2]]) as usize;
                let iy = u16::from_le_bytes([d[pos + 3], d[pos + 4]]) as usize;
                let iw = u16::from_le_bytes([d[pos + 5], d[pos + 6]]) as usize;
                let ih = u16::from_le_bytes([d[pos + 7], d[pos + 8]]) as usize;
                let f = d[pos + 9];
                pos += 10;
                let mut ct = gct.clone();
                if f & 0x80 != 0 {
                    let n = 2usize << (f & 7);
                    ct.clear();
                    for i in 0..n {
                        let o = pos + i * 3;
                        if o + 3 > d.len() {
                            break;
                        }
                        ct.push([d[o], d[o + 1], d[o + 2]]);
                    }
                    pos += n * 3;
                }
                let interlaced = f & 0x40 != 0;
                let min_code = *d.get(pos).unwrap_or(&8);
                pos += 1;
                let mut data = Vec::new();
                while pos < d.len() && d[pos] != 0 {
                    let n = d[pos] as usize;
                    let end = (pos + 1 + n).min(d.len());
                    data.extend_from_slice(&d[pos + 1..end]);
                    pos = end;
                }
                pos += 1;
                if (out.frames.len() + 1) * w * h * 4 > MAX_FRAME_BYTES {
                    out.truncated = true;
                    break;
                }
                let saved = if disposal == 3 { Some(canvas.clone()) } else { None };
                let idx = lzw_decode(&data, min_code, iw * ih);
                let rows: Vec<usize> = if interlaced {
                    let mut r = Vec::with_capacity(ih);
                    for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
                        let mut y = start;
                        while y < ih {
                            r.push(y);
                            y += step;
                        }
                    }
                    r
                } else {
                    (0..ih).collect()
                };
                for (i, &ci) in idx.iter().enumerate() {
                    let row = i / iw.max(1);
                    if row >= rows.len() {
                        break;
                    }
                    let y = iy + rows[row];
                    let x = ix + i % iw.max(1);
                    if Some(ci) == transparent || ci as usize >= ct.len() || x >= w || y >= h {
                        continue;
                    }
                    let c = ct[ci as usize];
                    let o = (y * w + x) * 4;
                    canvas[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                }
                // browsers show 0-10 ms frames at 100 ms; keep what people actually see
                let shown = if delay <= 10 { 100 } else { delay };
                out.frames.push((canvas.clone(), shown));
                match disposal {
                    2 => {
                        for y in iy..(iy + ih).min(h) {
                            for x in ix..(ix + iw).min(w) {
                                let o = (y * w + x) * 4;
                                canvas[o..o + 4].copy_from_slice(&[0, 0, 0, 0]);
                            }
                        }
                    }
                    3 => {
                        if let Some(sv) = saved {
                            canvas = sv;
                        }
                    }
                    _ => {}
                }
                transparent = None;
                disposal = 0;
                delay = 0;
            }
            0x3B => break,
            _ => pos += 1,
        }
    }
    if out.frames.is_empty() {
        return Err("GIF: no image found".into());
    }
    Ok(out)
}

/// First frame only (used by Paste from file and other single-picture paths).
pub fn decode(d: &[u8]) -> Result<Image, String> {
    let mut f = decode_frames(d)?;
    let (px, _) = f.frames.swap_remove(0);
    Ok(Image { w: f.w, h: f.h, px })
}

// ------------------------------------------------------------------ encoder

fn median_cut(colors: &[([u8; 3], u32)], max: usize) -> Vec<[u8; 3]> {
    let mut boxes: Vec<Vec<([u8; 3], u32)>> = vec![colors.to_vec()];
    while boxes.len() < max {
        // split the box with the largest (range × population)
        let mut best = None;
        let mut best_score = 0u64;
        for (i, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            let mut lo = [255u8; 3];
            let mut hi = [0u8; 3];
            let mut pop = 0u64;
            for (c, n) in b {
                for k in 0..3 {
                    lo[k] = lo[k].min(c[k]);
                    hi[k] = hi[k].max(c[k]);
                }
                pop += *n as u64;
            }
            let range = (0..3).map(|k| (hi[k] - lo[k]) as u64).max().unwrap();
            let score = range * range * (pop as f64).sqrt() as u64 + range;
            if score > best_score {
                best_score = score;
                best = Some(i);
            }
        }
        let Some(i) = best else { break };
        let mut b = boxes.swap_remove(i);
        let mut lo = [255u8; 3];
        let mut hi = [0u8; 3];
        for (c, _) in &b {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        let ch = (0..3).max_by_key(|&k| hi[k] - lo[k]).unwrap();
        b.sort_by_key(|(c, _)| c[ch]);
        let total: u64 = b.iter().map(|(_, n)| *n as u64).sum();
        let mut acc = 0u64;
        let mut cut = 1;
        for (j, (_, n)) in b.iter().enumerate() {
            acc += *n as u64;
            if acc * 2 >= total {
                cut = (j + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let mut s = [0u64; 3];
            let mut t = 0u64;
            for (c, n) in b {
                for k in 0..3 {
                    s[k] += c[k] as u64 * *n as u64;
                }
                t += *n as u64;
            }
            [(s[0] / t) as u8, (s[1] / t) as u8, (s[2] / t) as u8]
        })
        .collect()
}

fn lzw_encode(idx: &[u8], min_code: u8) -> Vec<u8> {
    let clear = 1u32 << min_code;
    let eoi = clear + 1;
    let mut out = Vec::new();
    let (mut buf, mut cnt) = (0u32, 0u32);
    let mut size = min_code as u32 + 1;
    let put = |code: u32, size: u32, out: &mut Vec<u8>, buf: &mut u32, cnt: &mut u32| {
        *buf |= code << *cnt;
        *cnt += size;
        while *cnt >= 8 {
            out.push(*buf as u8);
            *buf >>= 8;
            *cnt -= 8;
        }
    };
    let mut dict: HashMap<(u32, u8), u32> = HashMap::new();
    let mut next = eoi + 1;
    put(clear, size, &mut out, &mut buf, &mut cnt);
    let mut cur: Option<u32> = None;
    for &k in idx {
        match cur {
            None => cur = Some(k as u32),
            Some(c) => {
                if let Some(&code) = dict.get(&(c, k)) {
                    cur = Some(code);
                } else {
                    put(c, size, &mut out, &mut buf, &mut cnt);
                    if next < 4096 {
                        dict.insert((c, k), next);
                        next += 1;
                        if next > (1 << size) && size < 12 {
                            size += 1;
                        }
                    } else {
                        put(clear, size, &mut out, &mut buf, &mut cnt);
                        dict.clear();
                        next = eoi + 1;
                        size = min_code as u32 + 1;
                    }
                    cur = Some(k as u32);
                }
            }
        }
    }
    if let Some(c) = cur {
        put(c, size, &mut out, &mut buf, &mut cnt);
        // the decoder adds one more entry after reading the final code
        if next < 4096 && next + 1 > (1 << size) && size < 12 {
            size += 1;
        }
    }
    put(eoi, size, &mut out, &mut buf, &mut cnt);
    if cnt > 0 {
        out.push(buf as u8);
    }
    out
}

/// Build a palette and index buffer for a block of pixels; `None` = transparent.
fn index_block(pixels: &[Option<[u8; 3]>], bw: usize) -> (Vec<[u8; 3]>, Vec<u8>, Option<u8>) {
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    let mut has_transparent = false;
    for p in pixels {
        match p {
            Some(c) => *counts.entry(*c).or_insert(0) += 1,
            None => has_transparent = true,
        }
    }
    let cap = if has_transparent { 255 } else { 256 };
    let exact = counts.len() <= cap;
    let mut palette: Vec<[u8; 3]> = if exact {
        let mut v: Vec<_> = counts.keys().copied().collect();
        v.sort();
        v
    } else {
        let mut v: Vec<([u8; 3], u32)> = counts.iter().map(|(c, n)| (*c, *n)).collect();
        v.sort();
        median_cut(&v, cap)
    };
    if palette.is_empty() {
        palette.push([0, 0, 0]);
    }
    let opaque_len = palette.len();
    let trans_idx = if has_transparent {
        palette.push([0, 0, 0]);
        Some((palette.len() - 1) as u8)
    } else {
        None
    };
    let n = pixels.len();
    let bh = if bw == 0 { 0 } else { n / bw };
    let mut idx = vec![0u8; n];
    if exact {
        let map: HashMap<[u8; 3], u8> = palette[..opaque_len].iter().enumerate().map(|(i, c)| (*c, i as u8)).collect();
        for (i, p) in pixels.iter().enumerate() {
            idx[i] = match p {
                Some(c) => map[c],
                None => trans_idx.unwrap(),
            };
        }
    } else {
        // nearest colour with a cache, Floyd–Steinberg error diffusion
        let mut cache: HashMap<[u8; 3], u8> = HashMap::new();
        let mut nearest = |c: [u8; 3]| -> u8 {
            if let Some(&v) = cache.get(&c) {
                return v;
            }
            let mut best = 0;
            let mut bd = i32::MAX;
            for (i, p) in palette[..opaque_len].iter().enumerate() {
                let d: i32 = (0..3)
                    .map(|k| {
                        let e = p[k] as i32 - c[k] as i32;
                        e * e
                    })
                    .sum();
                if d < bd {
                    bd = d;
                    best = i;
                }
            }
            cache.insert(c, best as u8);
            best as u8
        };
        let mut err = vec![[0f32; 3]; (bw + 2) * 2];
        for y in 0..bh {
            let (cur_row, next_row) = if y % 2 == 0 { (0, bw + 2) } else { (bw + 2, 0) };
            for v in err[next_row..next_row + bw + 2].iter_mut() {
                *v = [0.0; 3];
            }
            for x in 0..bw {
                let i = y * bw + x;
                let Some(p) = pixels[i] else {
                    idx[i] = trans_idx.unwrap();
                    continue;
                };
                let e = err[cur_row + x + 1];
                let c = [
                    (p[0] as f32 + e[0]).round().clamp(0.0, 255.0) as u8,
                    (p[1] as f32 + e[1]).round().clamp(0.0, 255.0) as u8,
                    (p[2] as f32 + e[2]).round().clamp(0.0, 255.0) as u8,
                ];
                let q = nearest(c);
                idx[i] = q;
                let pc = palette[q as usize];
                let dd = [c[0] as f32 - pc[0] as f32, c[1] as f32 - pc[1] as f32, c[2] as f32 - pc[2] as f32];
                for k in 0..3 {
                    err[cur_row + x + 2][k] += dd[k] * 7.0 / 16.0;
                    err[next_row + x][k] += dd[k] * 3.0 / 16.0;
                    err[next_row + x + 1][k] += dd[k] * 5.0 / 16.0;
                    err[next_row + x + 2][k] += dd[k] * 1.0 / 16.0;
                }
            }
        }
    }
    (palette, idx, trans_idx)
}

fn write_header(out: &mut Vec<u8>, w: usize, h: usize) {
    out.extend_from_slice(b"GIF89a");
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.push(0x70); // no global colour table; every frame carries its own
    out.push(0);
    out.push(0);
}

#[allow(clippy::too_many_arguments)]
fn write_frame(out: &mut Vec<u8>, x: usize, y: usize, rw: usize, rh: usize, pixels: &[Option<[u8; 3]>], disposal: u8, delay_ms: u32) {
    let (palette, idx, trans) = index_block(pixels, rw);
    let mut bits = 1u8;
    while (1usize << bits) < palette.len() {
        bits += 1;
    }
    let cs = ((delay_ms + 5) / 10).min(65535) as u16;
    let packed = (disposal << 2) | trans.is_some() as u8;
    out.extend_from_slice(&[0x21, 0xF9, 4, packed]);
    out.extend_from_slice(&cs.to_le_bytes());
    out.extend_from_slice(&[trans.unwrap_or(0), 0]);
    out.push(0x2C);
    out.extend_from_slice(&(x as u16).to_le_bytes());
    out.extend_from_slice(&(y as u16).to_le_bytes());
    out.extend_from_slice(&(rw as u16).to_le_bytes());
    out.extend_from_slice(&(rh as u16).to_le_bytes());
    out.push(0x80 | (bits - 1));
    for i in 0..(1usize << bits) {
        out.extend_from_slice(&palette.get(i).copied().unwrap_or([0, 0, 0]));
    }
    let min_code = bits.max(2);
    out.push(min_code);
    let z = lzw_encode(&idx, min_code);
    for c in z.chunks(255) {
        out.push(c.len() as u8);
        out.extend_from_slice(c);
    }
    out.push(0);
}

#[inline]
fn opaque(px: &[u8], i: usize) -> Option<[u8; 3]> {
    if px[i * 4 + 3] >= 128 {
        Some([px[i * 4], px[i * 4 + 1], px[i * 4 + 2]])
    } else {
        None
    }
}

/// Single still picture. Pixels under 50% alpha become transparent.
pub fn encode(w: usize, h: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, w, h);
    let pixels: Vec<Option<[u8; 3]>> = (0..w * h).map(|i| opaque(rgba, i)).collect();
    write_frame(&mut out, 0, 0, w, h, &pixels, 0, 0);
    out.push(0x3B);
    out
}

/// Animated GIF from full frames (bottom layer first). Each frame stores only the
/// rectangle that changed; frames that uncover transparency clear themselves afterwards.
pub fn encode_animation(w: usize, h: usize, frames: &[(&[u8], u32)], loop_count: Option<u16>) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, w, h);
    // NETSCAPE looping extension (absent = play once)
    if let Some(lc) = loop_count {
        out.extend_from_slice(&[0x21, 0xFF, 11]);
        out.extend_from_slice(b"NETSCAPE2.0");
        out.extend_from_slice(&[3, 1]);
        out.extend_from_slice(&lc.to_le_bytes());
        out.push(0);
    }
    let n = w * h;
    let count = frames.len();
    // does frame i leave behind a pixel that frame i+1 needs to be transparent?
    let clear_after: Vec<bool> = (0..count)
        .map(|i| {
            i + 1 < count && (0..n).any(|p| frames[i].0[p * 4 + 3] >= 128 && frames[i + 1].0[p * 4 + 3] < 128)
        })
        .collect();
    for i in 0..count {
        let (px, delay) = (frames[i].0, frames[i].1);
        let base: Option<&[u8]> = if i == 0 || clear_after[i - 1] { None } else { Some(frames[i - 1].0) };
        let changed = |p: usize| -> bool {
            let cur = opaque(px, p);
            match base {
                None => cur.is_some(),
                Some(b) => cur != opaque(b, p),
            }
        };
        let (x0, y0, x1, y1) = if clear_after[i] {
            (0, 0, w, h)
        } else {
            let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
            for y in 0..h {
                for x in 0..w {
                    if changed(y * w + x) {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x + 1);
                        y1 = y1.max(y + 1);
                    }
                }
            }
            if x1 == 0 {
                (0, 0, 1, 1) // identical frame: keep its timing with a 1-pixel no-op
            } else {
                (x0, y0, x1, y1)
            }
        };
        let (rw, rh) = (x1 - x0, y1 - y0);
        let mut block: Vec<Option<[u8; 3]>> = Vec::with_capacity(rw * rh);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = y * w + x;
                block.push(if changed(p) { opaque(px, p) } else { None });
            }
        }
        write_frame(&mut out, x0, y0, rw, rh, &block, if clear_after[i] { 2 } else { 1 }, delay);
    }
    out.push(0x3B);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_exact() {
        let (w, h) = (61, 37);
        let mut px = vec![0u8; w * h * 4];
        for (i, p) in px.chunks_exact_mut(4).enumerate() {
            let c = (i * 7 % 200) as u8;
            if i % 13 == 0 {
                p.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                p.copy_from_slice(&[c, 255 - c, c / 2, 255]);
            }
        }
        let e = encode(w, h, &px);
        let d = decode(&e).unwrap();
        assert_eq!(d.px, px);
    }
    #[test]
    fn many_colors_quantised() {
        let (w, h) = (128, 96);
        let px: Vec<u8> = (0..w * h).flat_map(|i| [(i % 256) as u8, (i / 128 * 2) as u8, (i * 3 % 256) as u8, 255]).collect();
        let e = encode(w, h, &px);
        let d = decode(&e).unwrap();
        let err: f64 = px.iter().zip(d.px.iter()).map(|(a, b)| (*a as f64 - *b as f64).abs()).sum::<f64>() / px.len() as f64;
        assert!(err < 12.0, "{err}");
    }
    #[test]
    fn animation_roundtrip_with_transparency() {
        let (w, h) = (40, 30);
        let mut frames: Vec<Vec<u8>> = Vec::new();
        for f in 0..5usize {
            let mut px = vec![0u8; w * h * 4];
            // moving opaque square on a transparent background: forces clear-after frames
            for y in 5..15 {
                for x in (f * 5)..(f * 5 + 10) {
                    let o = (y * w + x) * 4;
                    px[o..o + 4].copy_from_slice(&[200, (f * 40) as u8, 30, 255]);
                }
            }
            // a static stripe that never changes
            for x in 0..w {
                let o = (25 * w + x) * 4;
                px[o..o + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
            frames.push(px);
        }
        let refs: Vec<(&[u8], u32)> = frames.iter().enumerate().map(|(i, f)| (f.as_slice(), 50 + i as u32 * 10)).collect();
        let e = encode_animation(w, h, &refs, Some(0));
        let d = decode_frames(&e).unwrap();
        assert_eq!(d.frames.len(), 5);
        assert_eq!(d.loop_count, Some(0));
        for (i, (px, delay)) in d.frames.iter().enumerate() {
            assert_eq!(px, &frames[i], "frame {i}");
            assert_eq!(*delay, 50 + i as u32 * 10);
        }
    }
}
