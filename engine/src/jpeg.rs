//! From-scratch JPEG codec.
//! Decoder: baseline + extended + progressive Huffman, any sampling factors, restart markers,
//! grayscale / YCbCr / RGB / CMYK / YCCK, and EXIF orientation.
//! Encoder: baseline YCbCr with 4:4:4 (high quality) or 4:2:0 sampling and standard tables.

use crate::png::Image;

const ZZ: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61,
    54, 47, 55, 62, 63,
];

pub fn is_jpeg(d: &[u8]) -> bool {
    d.len() > 3 && d[0] == 0xFF && d[1] == 0xD8 && d[2] == 0xFF
}

// ------------------------------------------------------------------ Huffman

#[derive(Clone)]
struct Huff {
    fast: Vec<u16>, // 9-bit lookup: (len << 8) | value, 0 = slow path
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: Vec<u8>,
}

impl Huff {
    fn new(bits: &[u8; 16], vals: &[u8]) -> Result<Huff, String> {
        let mut h = Huff { fast: vec![0; 512], maxcode: [-1; 18], valptr: [0; 17], mincode: [0; 17], vals: vals.to_vec() };
        let mut code = 0i32;
        let mut k = 0usize;
        for len in 1..=16usize {
            let n = bits[len - 1] as usize;
            h.valptr[len] = k as i32;
            h.mincode[len] = code;
            for _ in 0..n {
                if k >= vals.len() {
                    return Err("JPEG: bad Huffman table".into());
                }
                if len <= 9 {
                    let shift = 9 - len;
                    let base = (code as usize) << shift;
                    for j in 0..(1usize << shift) {
                        if base + j < 512 {
                            h.fast[base + j] = ((len as u16) << 8) | vals[k] as u16;
                        }
                    }
                }
                code += 1;
                k += 1;
            }
            h.maxcode[len] = if n > 0 { code - 1 } else { -1 };
            code <<= 1;
        }
        h.maxcode[17] = i32::MAX;
        Ok(h)
    }
}

struct BitReader<'a> {
    d: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
    marker: bool,
}

impl<'a> BitReader<'a> {
    fn new(d: &'a [u8]) -> Self {
        BitReader { d, pos: 0, buf: 0, cnt: 0, marker: false }
    }
    #[inline]
    fn fill(&mut self) {
        while self.cnt <= 24 {
            let mut byte = 0u32;
            if !self.marker && self.pos < self.d.len() {
                let b = self.d[self.pos];
                if b == 0xFF {
                    let nb = if self.pos + 1 < self.d.len() { self.d[self.pos + 1] } else { 0xD9 };
                    if nb == 0 {
                        self.pos += 2;
                        byte = 0xFF;
                    } else {
                        self.marker = true;
                    }
                } else {
                    self.pos += 1;
                    byte = b as u32;
                }
            }
            self.buf |= byte << (24 - self.cnt);
            self.cnt += 8;
        }
    }
    #[inline]
    fn get(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        self.fill();
        let v = self.buf >> (32 - n);
        self.buf <<= n;
        self.cnt -= n;
        v
    }
    #[inline]
    fn bit(&mut self) -> u32 {
        self.get(1)
    }
    #[inline]
    fn extend(&mut self, s: u32) -> i32 {
        if s == 0 {
            return 0;
        }
        let v = self.get(s) as i32;
        if v < (1 << (s - 1)) {
            v - (1 << s) + 1
        } else {
            v
        }
    }
    #[inline]
    fn decode(&mut self, h: &Huff) -> Result<u8, String> {
        self.fill();
        let e = h.fast[(self.buf >> 23) as usize];
        if e != 0 {
            let len = (e >> 8) as u32;
            self.buf <<= len;
            self.cnt -= len;
            return Ok(e as u8);
        }
        let mut code = 0i32;
        for len in 1..=16usize {
            code = (code << 1) | self.get(1) as i32;
            if code <= h.maxcode[len] {
                let idx = h.valptr[len] + code - h.mincode[len];
                return h.vals.get(idx as usize).copied().ok_or_else(|| "JPEG: bad Huffman code".to_string());
            }
        }
        Err("JPEG: bad Huffman code".into())
    }
    fn restart(&mut self) {
        self.buf = 0;
        self.cnt = 0;
        // skip to and past the RSTn marker
        loop {
            if self.pos + 1 >= self.d.len() {
                break;
            }
            if self.d[self.pos] == 0xFF && (0xD0..=0xD7).contains(&self.d[self.pos + 1]) {
                self.pos += 2;
                break;
            }
            self.pos += 1;
        }
        self.marker = false;
    }
}

// ------------------------------------------------------------------ decoder

struct Comp {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    bw: usize, // blocks per line (MCU padded)
    bh: usize,
    coefs: Vec<i32>,
    td: usize,
    ta: usize,
    pred: i32,
}

fn be16(d: &[u8], i: usize) -> usize {
    ((d[i] as usize) << 8) | d[i + 1] as usize
}

pub fn decode(d: &[u8]) -> Result<Image, String> {
    if !is_jpeg(d) {
        return Err("not a JPEG file".into());
    }
    let mut qt = [[0u16; 64]; 4];
    let mut dc: Vec<Option<Huff>> = vec![None, None, None, None];
    let mut ac: Vec<Option<Huff>> = vec![None, None, None, None];
    let mut comps: Vec<Comp> = Vec::new();
    let (mut w, mut h) = (0usize, 0usize);
    let mut progressive = false;
    let mut restart = 0usize;
    let mut adobe: Option<u8> = None;
    let mut jfif = false;
    let mut orientation = 1u16;
    let (mut hmax, mut vmax, mut mcux, mut mcuy) = (1, 1, 0, 0);
    let mut pos = 2;
    let mut frame_seen = false;
    let mut eobrun: u32;
    while pos + 4 <= d.len() {
        if d[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let m = d[pos + 1];
        if m == 0xFF || m == 0x01 || (0xD0..=0xD7).contains(&m) {
            pos += if m == 0xFF { 1 } else { 2 };
            continue;
        }
        if m == 0xD9 {
            break;
        }
        let len = be16(d, pos + 2);
        let seg_end = (pos + 2 + len).min(d.len());
        let s = &d[(pos + 4).min(seg_end)..seg_end];
        match m {
            0xDB => {
                let mut i = 0;
                while i < s.len() {
                    let pq = s[i] >> 4;
                    let tq = (s[i] & 3) as usize;
                    i += 1;
                    for k in 0..64 {
                        let v = if pq == 0 {
                            let v = *s.get(i).unwrap_or(&1) as u16;
                            i += 1;
                            v
                        } else {
                            let v = ((*s.get(i).unwrap_or(&0) as u16) << 8) | *s.get(i + 1).unwrap_or(&1) as u16;
                            i += 2;
                            v
                        };
                        qt[tq][ZZ[k]] = v;
                    }
                }
            }
            0xC4 => {
                let mut i = 0;
                while i + 17 <= s.len() {
                    let tc = s[i] >> 4;
                    let th = (s[i] & 3) as usize;
                    let mut bits = [0u8; 16];
                    bits.copy_from_slice(&s[i + 1..i + 17]);
                    let n: usize = bits.iter().map(|&b| b as usize).sum();
                    if i + 17 + n > s.len() {
                        return Err("JPEG: truncated Huffman table".into());
                    }
                    let t = Huff::new(&bits, &s[i + 17..i + 17 + n])?;
                    if tc == 0 {
                        dc[th] = Some(t);
                    } else {
                        ac[th] = Some(t);
                    }
                    i += 17 + n;
                }
            }
            0xC0 | 0xC1 | 0xC2 => {
                progressive = m == 0xC2;
                if s.len() < 6 {
                    return Err("JPEG: bad frame header".into());
                }
                if s[0] != 8 {
                    return Err("JPEG: only 8-bit JPEGs are supported".into());
                }
                h = be16(s, 1);
                w = be16(s, 3);
                let n = s[5] as usize;
                if w == 0 || h == 0 {
                    return Err("JPEG: image has no size (DNL not supported)".into());
                }
                if w > 32768 || h > 32768 {
                    return Err("JPEG: image is too large".into());
                }
                comps.clear();
                for c in 0..n {
                    let b = 6 + c * 3;
                    if b + 2 >= s.len() {
                        return Err("JPEG: bad frame header".into());
                    }
                    let hv = s[b + 1];
                    comps.push(Comp {
                        id: s[b],
                        h: ((hv >> 4) as usize).clamp(1, 4),
                        v: ((hv & 15) as usize).clamp(1, 4),
                        tq: (s[b + 2] & 3) as usize,
                        bw: 0,
                        bh: 0,
                        coefs: Vec::new(),
                        td: 0,
                        ta: 0,
                        pred: 0,
                    });
                }
                hmax = comps.iter().map(|c| c.h).max().unwrap_or(1);
                vmax = comps.iter().map(|c| c.v).max().unwrap_or(1);
                mcux = (w + 8 * hmax - 1) / (8 * hmax);
                mcuy = (h + 8 * vmax - 1) / (8 * vmax);
                for c in comps.iter_mut() {
                    c.bw = mcux * c.h;
                    c.bh = mcuy * c.v;
                    c.coefs = vec![0; c.bw * c.bh * 64];
                }
                frame_seen = true;
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => {
                return Err("JPEG: lossless and arithmetic-coded JPEGs are not supported".into());
            }
            0xDD => restart = be16(s, 0),
            0xE0 => {
                if s.starts_with(b"JFIF\0") {
                    jfif = true;
                }
            }
            0xE1 => {
                if s.starts_with(b"Exif\0\0") {
                    orientation = exif_orientation(&s[6..]).unwrap_or(1);
                }
            }
            0xEE => {
                if s.starts_with(b"Adobe") && s.len() >= 12 {
                    adobe = Some(s[11]);
                }
            }
            0xDA => {
                if !frame_seen {
                    return Err("JPEG: scan before frame header".into());
                }
                let ns = s[0] as usize;
                let mut scomp: Vec<usize> = Vec::new();
                for i in 0..ns {
                    let cid = s[1 + i * 2];
                    let t = s[2 + i * 2];
                    let ci = comps.iter().position(|c| c.id == cid).ok_or("JPEG: scan references unknown component")?;
                    comps[ci].td = (t >> 4) as usize & 3;
                    comps[ci].ta = (t & 15) as usize & 3;
                    scomp.push(ci);
                }
                let p = 1 + ns * 2;
                let ss = s[p] as usize;
                let se = (s[p + 1] as usize).min(63);
                let ah = (s[p + 2] >> 4) as u32;
                let al = (s[p + 2] & 15) as u32;
                // entropy-coded data runs until a non-RST marker
                let start = seg_end;
                let mut end = start;
                while end + 1 < d.len() {
                    if d[end] == 0xFF {
                        let nb = d[end + 1];
                        if nb != 0 && !(0xD0..=0xD7).contains(&nb) && nb != 0xFF {
                            break;
                        }
                    }
                    end += 1;
                }
                if end + 1 >= d.len() {
                    end = d.len();
                }
                let mut br = BitReader::new(&d[start..end]);
                for c in comps.iter_mut() {
                    c.pred = 0;
                }
                eobrun = 0;
                let single = scomp.len() == 1;
                let (units_x, units_y) = if single {
                    let c = &comps[scomp[0]];
                    let cw = (w * c.h + hmax - 1) / hmax;
                    let ch = (h * c.v + vmax - 1) / vmax;
                    ((cw + 7) / 8, (ch + 7) / 8)
                } else {
                    (mcux, mcuy)
                };
                let total = units_x * units_y;
                let mut done = 0usize;
                'units: for uy in 0..units_y {
                    for ux in 0..units_x {
                        if restart > 0 && done > 0 && done % restart == 0 {
                            br.restart();
                            for c in comps.iter_mut() {
                                c.pred = 0;
                            }
                            eobrun = 0;
                        }
                        let res = (|| -> Result<(), String> {
                            for &ci in &scomp {
                                let (bh_, bv_) = if single { (1, 1) } else { (comps[ci].h, comps[ci].v) };
                                for by in 0..bv_ {
                                    for bx in 0..bh_ {
                                        let (x, y) = if single { (ux, uy) } else { (ux * comps[ci].h + bx, uy * comps[ci].v + by) };
                                        let c = &mut comps[ci];
                                        let off = (y * c.bw + x) * 64;
                                        let blk = &mut c.coefs[off..off + 64];
                                        if !progressive {
                                            let dct = dc[c.td].as_ref().ok_or("JPEG: missing DC table")?;
                                            let act = ac[c.ta].as_ref().ok_or("JPEG: missing AC table")?;
                                            let t = br.decode(dct)? as u32;
                                            c.pred += br.extend(t);
                                            blk[0] = c.pred;
                                            let mut k = 1;
                                            while k < 64 {
                                                let rs = br.decode(act)? as u32;
                                                let r = rs >> 4;
                                                let s_ = rs & 15;
                                                if s_ == 0 {
                                                    if r == 15 {
                                                        k += 16;
                                                        continue;
                                                    }
                                                    break;
                                                }
                                                k += r as usize;
                                                if k > 63 {
                                                    break;
                                                }
                                                blk[ZZ[k]] = br.extend(s_);
                                                k += 1;
                                            }
                                        } else if ss == 0 {
                                            if ah == 0 {
                                                let dct = dc[c.td].as_ref().ok_or("JPEG: missing DC table")?;
                                                let t = br.decode(dct)? as u32;
                                                c.pred += br.extend(t);
                                                blk[0] = c.pred << al;
                                            } else if br.bit() == 1 {
                                                blk[0] |= 1 << al;
                                            }
                                        } else if ah == 0 {
                                            if eobrun > 0 {
                                                eobrun -= 1;
                                                continue;
                                            }
                                            let act = ac[c.ta].as_ref().ok_or("JPEG: missing AC table")?;
                                            let mut k = ss;
                                            while k <= se {
                                                let rs = br.decode(act)? as u32;
                                                let r = rs >> 4;
                                                let s_ = rs & 15;
                                                if s_ == 0 {
                                                    if r < 15 {
                                                        eobrun = (1 << r) - 1;
                                                        if r > 0 {
                                                            eobrun += br.get(r);
                                                        }
                                                        break;
                                                    }
                                                    k += 16;
                                                    continue;
                                                }
                                                k += r as usize;
                                                if k > 63 {
                                                    break;
                                                }
                                                blk[ZZ[k]] = br.extend(s_) * (1 << al);
                                                k += 1;
                                            }
                                        } else {
                                            let act = ac[c.ta].as_ref().ok_or("JPEG: missing AC table")?;
                                            refine_ac(&mut br, act, blk, ss, se, al, &mut eobrun)?;
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })();
                        if res.is_err() {
                            // corrupt or truncated data: keep what decoded so far
                            break 'units;
                        }
                        done += 1;
                    }
                }
                let _ = total;
                pos = end;
                continue;
            }
            _ => {}
        }
        pos = seg_end;
    }
    if !frame_seen {
        return Err("JPEG: no image data".into());
    }
    // dequantise + IDCT into component planes
    let mut planes: Vec<(Vec<u8>, usize, usize)> = Vec::with_capacity(comps.len());
    let cosines = idct_table();
    for c in &comps {
        let pw = c.bw * 8;
        let ph = c.bh * 8;
        let mut plane = vec![0u8; pw * ph];
        let q = &qt[c.tq];
        let mut tmp = [0f32; 64];
        let mut out = [0u8; 64];
        for by in 0..c.bh {
            for bx in 0..c.bw {
                let off = (by * c.bw + bx) * 64;
                for k in 0..64 {
                    tmp[k] = (c.coefs[off + k] * q[k] as i32) as f32;
                }
                idct8x8(&tmp, &mut out, &cosines);
                for y in 0..8 {
                    let o = (by * 8 + y) * pw + bx * 8;
                    plane[o..o + 8].copy_from_slice(&out[y * 8..y * 8 + 8]);
                }
            }
        }
        // the meaningful width of this component
        let cw = (w * c.h + hmax - 1) / hmax;
        let ch = (h * c.v + vmax - 1) / vmax;
        planes.push((plane, pw, (cw << 16) | ch.min(0xffff)));
        let _ = ph;
    }
    // upsample + colour convert
    let mut px = vec![0u8; w * h * 4];
    let n = comps.len();
    let sample = |ci: usize, x: usize, y: usize| -> f32 {
        let (plane, pw, packed) = &planes[ci];
        let cw = packed >> 16;
        let ch = packed & 0xffff;
        let c = &comps[ci];
        if c.h == hmax && c.v == vmax {
            return plane[y * pw + x] as f32;
        }
        let fx = ((x as f32 + 0.5) * c.h as f32 / hmax as f32 - 0.5).clamp(0.0, (cw.max(1) - 1) as f32);
        let fy = ((y as f32 + 0.5) * c.v as f32 / vmax as f32 - 0.5).clamp(0.0, (ch.max(1) - 1) as f32);
        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(cw.max(1) - 1);
        let y1 = (y0 + 1).min(ch.max(1) - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let a = plane[y0 * pw + x0] as f32;
        let b = plane[y0 * pw + x1] as f32;
        let cc = plane[y1 * pw + x0] as f32;
        let dd = plane[y1 * pw + x1] as f32;
        (a * (1.0 - tx) + b * tx) * (1.0 - ty) + (cc * (1.0 - tx) + dd * tx) * ty
    };
    let ids: Vec<u8> = comps.iter().map(|c| c.id).collect();
    let rgb_ids = n == 3 && ids == [b'R', b'G', b'B'];
    let transform = match adobe {
        Some(t) => t,
        None => {
            if n == 3 && !rgb_ids {
                1
            } else {
                0
            }
        }
    };
    let _ = jfif;
    let cl = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            match n {
                1 => {
                    let g = sample(0, x, y) as u8;
                    px[o..o + 4].copy_from_slice(&[g, g, g, 255]);
                }
                3 => {
                    let (a, b, c) = (sample(0, x, y), sample(1, x, y), sample(2, x, y));
                    if transform == 0 {
                        px[o..o + 4].copy_from_slice(&[cl(a), cl(b), cl(c), 255]);
                    } else {
                        let (cb, cr) = (b - 128.0, c - 128.0);
                        px[o..o + 4].copy_from_slice(&[cl(a + 1.402 * cr), cl(a - 0.344136 * cb - 0.714136 * cr), cl(a + 1.772 * cb), 255]);
                    }
                }
                4 => {
                    let (a, b, c, k) = (sample(0, x, y), sample(1, x, y), sample(2, x, y), sample(3, x, y));
                    let (cc, mm, yy) = if transform == 2 {
                        let (cb, cr) = (b - 128.0, c - 128.0);
                        (255.0 - (a + 1.402 * cr), 255.0 - (a - 0.344136 * cb - 0.714136 * cr), 255.0 - (a + 1.772 * cb))
                    } else {
                        (a, b, c)
                    };
                    // Adobe writes inverted CMYK
                    let (cc, mm, yy, k) = if adobe.is_some() { (255.0 - cc, 255.0 - mm, 255.0 - yy, 255.0 - k) } else { (cc, mm, yy, k) };
                    let f = |v: f32| cl((255.0 - v) * (255.0 - k) / 255.0);
                    px[o..o + 4].copy_from_slice(&[f(cc), f(mm), f(yy), 255]);
                }
                _ => {
                    let g = sample(0, x, y) as u8;
                    px[o..o + 4].copy_from_slice(&[g, g, g, 255]);
                }
            }
        }
    }
    Ok(apply_orientation(Image { w, h, px }, orientation))
}

fn refine_ac(br: &mut BitReader, act: &Huff, blk: &mut [i32], ss: usize, se: usize, al: u32, eobrun: &mut u32) -> Result<(), String> {
    let p1: i32 = 1 << al;
    let m1: i32 = -1 << al;
    let mut k = ss;
    if *eobrun == 0 {
        while k <= se {
            let rs = br.decode(act)? as u32;
            let mut r = (rs >> 4) as i32;
            let s = rs & 15;
            let mut val = 0i32;
            if s == 0 {
                if r < 15 {
                    *eobrun = 1 << r;
                    if r > 0 {
                        *eobrun += br.get(r as u32);
                    }
                    break;
                }
            } else {
                val = if br.bit() == 1 { p1 } else { m1 };
            }
            while k <= se {
                let z = ZZ[k];
                if blk[z] != 0 {
                    if br.bit() == 1 && (blk[z] & p1) == 0 {
                        blk[z] += if blk[z] >= 0 { p1 } else { m1 };
                    }
                } else {
                    if r == 0 {
                        if val != 0 {
                            blk[z] = val;
                        }
                        k += 1;
                        break;
                    }
                    r -= 1;
                }
                k += 1;
            }
        }
    }
    if *eobrun > 0 {
        while k <= se {
            let z = ZZ[k];
            if blk[z] != 0 && br.bit() == 1 && (blk[z] & p1) == 0 {
                blk[z] += if blk[z] >= 0 { p1 } else { m1 };
            }
            k += 1;
        }
        *eobrun -= 1;
    }
    Ok(())
}

fn idct_table() -> [[f32; 8]; 8] {
    let mut t = [[0f32; 8]; 8];
    for x in 0..8 {
        for u in 0..8 {
            let cu = if u == 0 { std::f32::consts::FRAC_1_SQRT_2 } else { 1.0 };
            t[x][u] = cu * (((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI) / 16.0).cos() / 2.0;
        }
    }
    t
}

fn idct8x8(input: &[f32; 64], out: &mut [u8; 64], t: &[[f32; 8]; 8]) {
    let mut tmp = [0f32; 64];
    // rows: for each v (row of coefficients), transform over u
    for v in 0..8 {
        let row = &input[v * 8..v * 8 + 8];
        if row[1..].iter().all(|&c| c == 0.0) {
            let dcv = row[0] * t[0][0];
            for x in 0..8 {
                tmp[v * 8 + x] = dcv;
            }
            continue;
        }
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += t[x][u] * row[u];
            }
            tmp[v * 8 + x] = s;
        }
    }
    for x in 0..8 {
        for y in 0..8 {
            let mut s = 0.0;
            for v in 0..8 {
                s += t[y][v] * tmp[v * 8 + x];
            }
            out[y * 8 + x] = (s + 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn exif_orientation(t: &[u8]) -> Option<u16> {
    if t.len() < 8 {
        return None;
    }
    let le = &t[..2] == b"II";
    let r16 = |i: usize| -> Option<u16> {
        let b = t.get(i..i + 2)?;
        Some(if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
    };
    let r32 = |i: usize| -> Option<u32> {
        let b = t.get(i..i + 4)?;
        Some(if le { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) } else { u32::from_be_bytes([b[0], b[1], b[2], b[3]]) })
    };
    let ifd = r32(4)? as usize;
    let n = r16(ifd)? as usize;
    for i in 0..n {
        let e = ifd + 2 + i * 12;
        if r16(e)? == 0x0112 {
            let v = r16(e + 8)?;
            return if (1..=8).contains(&v) { Some(v) } else { None };
        }
    }
    None
}

fn apply_orientation(img: Image, o: u16) -> Image {
    if o <= 1 || o > 8 {
        return img;
    }
    let (w, h) = (img.w, img.h);
    let swap = o >= 5;
    let (nw, nh) = if swap { (h, w) } else { (w, h) };
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = match o {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (h - 1 - y, x),
                7 => (h - 1 - y, w - 1 - x),
                _ => (y, w - 1 - x),
            };
            let s = (y * w + x) * 4;
            let d = (dy * nw + dx) * 4;
            out[d..d + 4].copy_from_slice(&img.px[s..s + 4]);
        }
    }
    Image { w: nw, h: nh, px: out }
}

// ------------------------------------------------------------------ encoder

const STD_LUM_Q: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80,
    62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95,
    98, 112, 100, 103, 99,
];
const STD_CHR_Q: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99,
];
const DC_LUM_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUM_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const DC_CHR_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHR_VALS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUM_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUM_VALS: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32,
    0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16,
    0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45,
    0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94,
    0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
    0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8,
    0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
const AC_CHR_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHR_VALS: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81,
    0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34,
    0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44,
    0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92,
    0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
    0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
    0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

struct EncTable {
    code: [u16; 256],
    len: [u8; 256],
}

fn enc_table(bits: &[u8; 16], vals: &[u8]) -> EncTable {
    let mut t = EncTable { code: [0; 256], len: [0; 256] };
    let mut code = 0u16;
    let mut k = 0;
    for l in 1..=16u8 {
        for _ in 0..bits[l as usize - 1] {
            t.code[vals[k] as usize] = code;
            t.len[vals[k] as usize] = l;
            code += 1;
            k += 1;
        }
        code <<= 1;
    }
    t
}

struct BitWriter {
    out: Vec<u8>,
    buf: u32,
    cnt: u32,
}

impl BitWriter {
    fn put(&mut self, v: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.buf = (self.buf << n) | (v & ((1 << n) - 1));
        self.cnt += n;
        while self.cnt >= 8 {
            let b = (self.buf >> (self.cnt - 8)) as u8;
            self.out.push(b);
            if b == 0xFF {
                self.out.push(0);
            }
            self.cnt -= 8;
        }
    }
    fn flush(&mut self) {
        if self.cnt > 0 {
            let pad = 8 - self.cnt;
            self.put((1 << pad) - 1, pad);
        }
    }
}

fn scaled_q(base: &[u8; 64], quality: u32) -> [u8; 64] {
    let q = quality.clamp(1, 100);
    let scale = if q < 50 { 5000 / q } else { 200 - q * 2 };
    let mut out = [0u8; 64];
    for i in 0..64 {
        out[i] = ((base[i] as u32 * scale + 50) / 100).clamp(1, 255) as u8;
    }
    out
}

fn fdct(block: &[f32; 64], out: &mut [f32; 64], t: &[[f32; 8]; 8]) {
    // forward DCT is the transpose of the inverse basis
    let mut tmp = [0f32; 64];
    for y in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += t[x][u] * block[y * 8 + x];
            }
            tmp[y * 8 + u] = s;
        }
    }
    for u in 0..8 {
        for v in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += t[y][v] * tmp[y * 8 + u];
            }
            out[v * 8 + u] = s;
        }
    }
}

fn bit_size(v: i32) -> u32 {
    32 - (v.unsigned_abs()).leading_zeros()
}

/// Encode straight RGBA (alpha must already be flattened) as a baseline JPEG.
pub fn encode(w: usize, h: usize, rgba: &[u8], quality: u32) -> Vec<u8> {
    let sub = quality < 90; // 4:2:0 for smaller files, 4:4:4 keeps coloured text crisp
    let ql = scaled_q(&STD_LUM_Q, quality);
    let qc = scaled_q(&STD_CHR_Q, quality);
    let mut out = Vec::with_capacity(w * h / 2 + 1024);
    out.extend_from_slice(&[0xFF, 0xD8]);
    // JFIF
    out.extend_from_slice(&[0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 1, 0, 96, 0, 96, 0, 0]);
    // DQT
    out.extend_from_slice(&[0xFF, 0xDB, 0, 132, 0]);
    for k in 0..64 {
        out.push(ql[ZZ[k]]);
    }
    out.push(1);
    for k in 0..64 {
        out.push(qc[ZZ[k]]);
    }
    // SOF0
    let samp_y = if sub { 0x22 } else { 0x11 };
    out.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8, (h >> 8) as u8, h as u8, (w >> 8) as u8, w as u8, 3, 1, samp_y, 0, 2, 0x11, 1, 3, 0x11, 1]);
    // DHT
    let tables: [(u8, &[u8; 16], &[u8]); 4] = [
        (0x00, &DC_LUM_BITS, &DC_LUM_VALS),
        (0x10, &AC_LUM_BITS, &AC_LUM_VALS),
        (0x01, &DC_CHR_BITS, &DC_CHR_VALS),
        (0x11, &AC_CHR_BITS, &AC_CHR_VALS),
    ];
    for (id, bits, vals) in tables.iter() {
        let len = 2 + 1 + 16 + vals.len();
        out.extend_from_slice(&[0xFF, 0xC4, (len >> 8) as u8, len as u8, *id]);
        out.extend_from_slice(*bits);
        out.extend_from_slice(vals);
    }
    // SOS
    out.extend_from_slice(&[0xFF, 0xDA, 0, 12, 3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    let dcl = enc_table(&DC_LUM_BITS, &DC_LUM_VALS);
    let acl = enc_table(&AC_LUM_BITS, &AC_LUM_VALS);
    let dcc = enc_table(&DC_CHR_BITS, &DC_CHR_VALS);
    let acc = enc_table(&AC_CHR_BITS, &AC_CHR_VALS);
    let t = idct_table();
    // colour convert to full-res planes
    let n = w * h;
    let mut yp = vec![0f32; n];
    let mut cbp = vec![0f32; n];
    let mut crp = vec![0f32; n];
    for i in 0..n {
        let (r, g, b) = (rgba[i * 4] as f32, rgba[i * 4 + 1] as f32, rgba[i * 4 + 2] as f32);
        yp[i] = 0.299 * r + 0.587 * g + 0.114 * b;
        cbp[i] = -0.168736 * r - 0.331264 * g + 0.5 * b + 128.0;
        crp[i] = 0.5 * r - 0.418688 * g - 0.081312 * b + 128.0;
    }
    let mut bw = BitWriter { out, buf: 0, cnt: 0 };
    let mut preds = [0i32; 3];
    let mcu = if sub { 16 } else { 8 };
    let mut blk = [0f32; 64];
    let mut coef = [0f32; 64];
    let mut encode_block = |bw: &mut BitWriter, blk: &[f32; 64], q: &[u8; 64], dct: &EncTable, act: &EncTable, pred: &mut i32| {
        fdct(blk, &mut coef, &t);
        let mut zz = [0i32; 64];
        for k in 0..64 {
            zz[k] = (coef[ZZ[k]] / q[ZZ[k]] as f32).round() as i32;
        }
        let diff = zz[0] - *pred;
        *pred = zz[0];
        let s = bit_size(diff);
        bw.put(dct.code[s as usize] as u32, dct.len[s as usize] as u32);
        if s > 0 {
            let v = if diff < 0 { diff - 1 } else { diff };
            bw.put(v as u32, s);
        }
        let mut run = 0;
        for k in 1..64 {
            let v = zz[k];
            if v == 0 {
                run += 1;
                continue;
            }
            while run > 15 {
                bw.put(act.code[0xF0] as u32, act.len[0xF0] as u32);
                run -= 16;
            }
            let s = bit_size(v);
            let sym = (run << 4) | s as usize;
            bw.put(act.code[sym] as u32, act.len[sym] as u32);
            let vv = if v < 0 { v - 1 } else { v };
            bw.put(vv as u32, s);
            run = 0;
        }
        if run > 0 {
            bw.put(act.code[0] as u32, act.len[0] as u32);
        }
    };
    let get = |p: &Vec<f32>, x: usize, y: usize| -> f32 { p[y.min(h - 1) * w + x.min(w - 1)] };
    for my in (0..h).step_by(mcu) {
        for mx in (0..w).step_by(mcu) {
            let ny = if sub { 2 } else { 1 };
            for by in 0..ny {
                for bx in 0..ny {
                    for y in 0..8 {
                        for x in 0..8 {
                            blk[y * 8 + x] = get(&yp, mx + bx * 8 + x, my + by * 8 + y) - 128.0;
                        }
                    }
                    encode_block(&mut bw, &blk, &ql, &dcl, &acl, &mut preds[0]);
                }
            }
            for (ci, plane) in [&cbp, &crp].iter().enumerate() {
                for y in 0..8 {
                    for x in 0..8 {
                        blk[y * 8 + x] = if sub {
                            let (sx, sy) = (mx + x * 2, my + y * 2);
                            (get(plane, sx, sy) + get(plane, sx + 1, sy) + get(plane, sx, sy + 1) + get(plane, sx + 1, sy + 1)) / 4.0 - 128.0
                        } else {
                            get(plane, mx + x, my + y) - 128.0
                        };
                    }
                }
                encode_block(&mut bw, &blk, &qc, &dcc, &acc, &mut preds[ci + 1]);
            }
        }
    }
    bw.flush();
    let mut out = bw.out;
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_close() {
        let (w, h) = (45, 31);
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                px[o..o + 4].copy_from_slice(&[(x * 5) as u8, (y * 8) as u8, 128, 255]);
            }
        }
        for q in [75, 95] {
            let e = encode(w, h, &px, q);
            let d = decode(&e).unwrap();
            assert_eq!((d.w, d.h), (w, h));
            let err: f64 = px.iter().zip(d.px.iter()).map(|(a, b)| (*a as f64 - *b as f64).abs()).sum::<f64>() / px.len() as f64;
            assert!(err < 4.0, "q{q} mean error {err}");
        }
    }
}
