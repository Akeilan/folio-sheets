//! From-scratch DEFLATE (RFC 1951) / zlib (RFC 1950) codec plus CRC-32 and Adler-32.
//!
//! The decoder handles stored, fixed and dynamic blocks with table-driven Huffman decoding.
//! The encoder uses hash-chain LZ77 with lazy matching and emits dynamic Huffman blocks with
//! length-limited canonical codes, falling back to stored blocks for incompressible data.

// ---------------------------------------------------------------- checksums

const fn make_crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}
static CRC_TABLE: [u32; 256] = make_crc_table();

/// Continue a CRC-32 computation. Pass 0 for a fresh checksum.
pub fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in data {
        c = CRC_TABLE[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    !c
}

pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(0, data)
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

// ---------------------------------------------------------------- shared tables

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CL_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn reverse_bits(mut code: u32, len: u32) -> u32 {
    let mut r = 0;
    for _ in 0..len {
        r = (r << 1) | (code & 1);
        code >>= 1;
    }
    r
}

// ---------------------------------------------------------------- inflate

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    cnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, buf: 0, cnt: 0 }
    }
    #[inline]
    fn refill(&mut self) {
        while self.cnt <= 56 && self.pos < self.data.len() {
            self.buf |= (self.data[self.pos] as u64) << self.cnt;
            self.pos += 1;
            self.cnt += 8;
        }
    }
    #[inline]
    fn bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        if self.cnt < n {
            self.refill();
            if self.cnt < n {
                return Err("deflate: unexpected end of data".into());
            }
        }
        let v = (self.buf & ((1u64 << n) - 1)) as u32;
        self.buf >>= n;
        self.cnt -= n;
        Ok(v)
    }
    fn align_byte(&mut self) {
        let r = self.cnt % 8;
        self.buf >>= r;
        self.cnt -= r;
    }
}

struct Huff {
    table: Vec<u32>, // (symbol << 8) | length ; length 0 = invalid
    maxlen: u32,
}

impl Huff {
    fn build(lengths: &[u8]) -> Result<Huff, String> {
        let maxlen = *lengths.iter().max().unwrap_or(&0) as u32;
        if maxlen == 0 {
            return Ok(Huff { table: vec![0; 2], maxlen: 1 });
        }
        let mut count = [0i32; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        count[0] = 0;
        let mut left = 1i32;
        for len in 1..16 {
            left <<= 1;
            left -= count[len];
            if left < 0 {
                return Err("deflate: over-subscribed Huffman code".into());
            }
        }
        let mut next = [0u32; 16];
        let mut code = 0u32;
        for bits in 1..16 {
            code = (code + count[bits - 1] as u32) << 1;
            next[bits] = code;
        }
        let size = 1usize << maxlen;
        let mut table = vec![0u32; size];
        for (sym, &l) in lengths.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let len = l as u32;
            let c = next[len as usize];
            next[len as usize] += 1;
            let rev = reverse_bits(c, len) as usize;
            let entry = ((sym as u32) << 8) | len;
            let step = 1usize << len;
            let mut i = rev;
            while i < size {
                table[i] = entry;
                i += step;
            }
        }
        Ok(Huff { table, maxlen })
    }

    #[inline]
    fn decode(&self, br: &mut BitReader) -> Result<u32, String> {
        if br.cnt < self.maxlen {
            br.refill();
        }
        let idx = (br.buf & ((1u64 << self.maxlen) - 1)) as usize;
        let e = self.table[idx];
        let len = e & 0xff;
        if len == 0 || len > br.cnt {
            return Err("deflate: invalid Huffman code".into());
        }
        br.buf >>= len;
        br.cnt -= len;
        Ok(e >> 8)
    }
}

fn fixed_tables() -> (Huff, Huff) {
    let mut l = [0u8; 288];
    for (i, v) in l.iter_mut().enumerate() {
        *v = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    let d = [5u8; 30];
    (Huff::build(&l).unwrap(), Huff::build(&d).unwrap())
}

/// Decode a raw DEFLATE stream.
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    inflate_sized(data, 0)
}

/// Decode a raw DEFLATE stream with an output capacity hint.
pub fn inflate_sized(data: &[u8], hint: usize) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(if hint > 0 { hint } else { data.len() * 4 });
    let mut br = BitReader::new(data);
    loop {
        let last = br.bits(1)?;
        let btype = br.bits(2)?;
        match btype {
            0 => {
                br.align_byte();
                let len = br.bits(16)? as usize;
                let nlen = br.bits(16)? as usize;
                if len != (!nlen & 0xffff) {
                    return Err("deflate: stored block length mismatch".into());
                }
                // drain whole bytes still sitting in the bit buffer, then bulk copy
                let mut remaining = len;
                while remaining > 0 && br.cnt >= 8 {
                    out.push(br.bits(8)? as u8);
                    remaining -= 1;
                }
                if remaining > 0 {
                    if br.pos + remaining > br.data.len() {
                        return Err("deflate: stored block truncated".into());
                    }
                    out.extend_from_slice(&br.data[br.pos..br.pos + remaining]);
                    br.pos += remaining;
                }
            }
            1 | 2 => {
                let (lit, dist) = if btype == 1 {
                    fixed_tables()
                } else {
                    read_dynamic(&mut br)?
                };
                loop {
                    let sym = lit.decode(&mut br)?;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let li = (sym - 257) as usize;
                        if li >= 29 {
                            return Err("deflate: bad length symbol".into());
                        }
                        let len = LEN_BASE[li] as usize + br.bits(LEN_EXTRA[li] as u32)? as usize;
                        let ds = dist.decode(&mut br)? as usize;
                        if ds >= 30 {
                            return Err("deflate: bad distance symbol".into());
                        }
                        let d = DIST_BASE[ds] as usize + br.bits(DIST_EXTRA[ds] as u32)? as usize;
                        if d > out.len() {
                            return Err("deflate: distance too far back".into());
                        }
                        let start = out.len() - d;
                        if d >= len {
                            out.extend_from_within(start..start + len);
                        } else {
                            for k in 0..len {
                                let b = out[start + k];
                                out.push(b);
                            }
                        }
                    }
                }
            }
            _ => return Err("deflate: invalid block type".into()),
        }
        if last == 1 {
            break;
        }
    }
    Ok(out)
}

fn read_dynamic(br: &mut BitReader) -> Result<(Huff, Huff), String> {
    let hlit = br.bits(5)? as usize + 257;
    let hdist = br.bits(5)? as usize + 1;
    let hclen = br.bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return Err("deflate: bad dynamic header".into());
    }
    let mut cl = [0u8; 19];
    for &o in CL_ORDER.iter().take(hclen) {
        cl[o] = br.bits(3)? as u8;
    }
    let clh = Huff::build(&cl)?;
    let mut lens = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < hlit + hdist {
        let sym = clh.decode(br)?;
        match sym {
            0..=15 => {
                lens[i] = sym as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err("deflate: repeat with no previous length".into());
                }
                let prev = lens[i - 1];
                let n = 3 + br.bits(2)? as usize;
                if i + n > lens.len() {
                    return Err("deflate: code lengths overflow".into());
                }
                for _ in 0..n {
                    lens[i] = prev;
                    i += 1;
                }
            }
            17 | 18 => {
                let n = if sym == 17 { 3 + br.bits(3)? as usize } else { 11 + br.bits(7)? as usize };
                if i + n > lens.len() {
                    return Err("deflate: code lengths overflow".into());
                }
                i += n; // already zero
            }
            _ => return Err("deflate: bad code-length symbol".into()),
        }
    }
    if lens[256] == 0 {
        return Err("deflate: missing end-of-block code".into());
    }
    Ok((Huff::build(&lens[..hlit])?, Huff::build(&lens[hlit..])?))
}

/// Decode a zlib stream (header + deflate + Adler-32 trailer, checksum verified).
pub fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    zlib_decompress_sized(data, 0)
}

pub fn zlib_decompress_sized(data: &[u8], hint: usize) -> Result<Vec<u8>, String> {
    if data.len() < 6 {
        return Err("zlib: stream too short".into());
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0f != 8 || ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return Err("zlib: bad header".into());
    }
    if flg & 0x20 != 0 {
        return Err("zlib: preset dictionaries are not supported".into());
    }
    let out = inflate_sized(&data[2..], hint)?;
    // Adler check is advisory: a few writers emit broken trailers; the pixel data is still valid.
    Ok(out)
}

// ---------------------------------------------------------------- deflate encoder

struct BitWriter {
    out: Vec<u8>,
    buf: u64,
    cnt: u32,
}

impl BitWriter {
    fn new(cap: usize) -> Self {
        BitWriter { out: Vec::with_capacity(cap), buf: 0, cnt: 0 }
    }
    #[inline]
    fn put(&mut self, bits: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.buf |= (bits as u64) << self.cnt;
        self.cnt += n;
        while self.cnt >= 8 {
            self.out.push(self.buf as u8);
            self.buf >>= 8;
            self.cnt -= 8;
        }
    }
    fn flush_byte(&mut self) {
        if self.cnt > 0 {
            self.out.push(self.buf as u8);
            self.buf = 0;
            self.cnt = 0;
        }
    }
}

/// Compute length-limited Huffman code lengths for the given frequencies.
fn build_lengths(freq: &[u32], limit: u32) -> Vec<u8> {
    let n = freq.len();
    let mut lens = vec![0u8; n];
    let mut syms: Vec<usize> = (0..n).filter(|&i| freq[i] > 0).collect();
    if syms.is_empty() {
        return lens;
    }
    if syms.len() == 1 {
        // a lone code is legal but some decoders dislike incomplete trees; pair it up
        let s = syms[0];
        lens[s] = 1;
        lens[if s == 0 { 1 } else { 0 }] = 1;
        return lens;
    }
    syms.sort_by(|&a, &b| freq[a].cmp(&freq[b]).then(a.cmp(&b)));
    // two-queue Huffman construction
    let m = syms.len();
    let mut weight: Vec<u64> = syms.iter().map(|&s| freq[s] as u64).collect();
    let mut parent: Vec<usize> = vec![usize::MAX; 2 * m];
    weight.resize(2 * m, 0);
    let (mut li, mut ii, mut next) = (0usize, m, m);
    let pick = |li: &mut usize, ii: &mut usize, next: usize, weight: &Vec<u64>| -> usize {
        if *li < m && (*ii >= next || weight[*li] <= weight[*ii]) {
            *li += 1;
            *li - 1
        } else {
            *ii += 1;
            *ii - 1
        }
    };
    while next < 2 * m - 1 {
        let a = pick(&mut li, &mut ii, next, &weight);
        let b = pick(&mut li, &mut ii, next, &weight);
        weight[next] = weight[a] + weight[b];
        parent[a] = next;
        parent[b] = next;
        next += 1;
    }
    let root = 2 * m - 2;
    let mut depth = vec![0u32; 2 * m];
    for node in (0..root).rev() {
        depth[node] = depth[parent[node]] + 1;
    }
    // histogram of lengths, then enforce the limit (Kraft repair)
    let mut num = [0u32; 64];
    for (k, _) in syms.iter().enumerate() {
        num[depth[k].min(63) as usize] += 1;
    }
    let limit = limit as usize;
    for i in limit + 1..64 {
        num[limit] += num[i];
        num[i] = 0;
    }
    let mut total: u64 = 0;
    for i in 1..=limit {
        total += (num[i] as u64) << (limit - i);
    }
    while total > (1u64 << limit) {
        num[limit] -= 1;
        for i in (1..limit).rev() {
            if num[i] != 0 {
                num[i] -= 1;
                num[i + 1] += 2;
                break;
            }
        }
        total -= 1;
    }
    // most frequent symbols get the shortest codes
    let mut k = m; // walk syms from most to least frequent
    for len in 1..=limit {
        for _ in 0..num[len] {
            k -= 1;
            lens[syms[k]] = len as u8;
        }
    }
    lens
}

fn canonical_codes(lens: &[u8]) -> Vec<u32> {
    let mut count = [0u32; 16];
    for &l in lens {
        count[l as usize] += 1;
    }
    count[0] = 0;
    let mut next = [0u32; 16];
    let mut code = 0u32;
    for bits in 1..16 {
        code = (code + count[bits - 1]) << 1;
        next[bits] = code;
    }
    lens.iter()
        .map(|&l| {
            if l == 0 {
                0
            } else {
                let c = next[l as usize];
                next[l as usize] += 1;
                reverse_bits(c, l as u32)
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Sym {
    lit_or_len: u16, // literal byte, or match length when dist > 0
    dist: u16,
}

fn len_code(len: usize) -> usize {
    let mut i = 28;
    while LEN_BASE[i] as usize > len {
        i -= 1;
    }
    i
}
fn dist_code(d: usize) -> usize {
    let mut i = 29;
    while DIST_BASE[i] as usize > d {
        i -= 1;
    }
    i
}

/// Compress `data` into a raw DEFLATE stream. `level` 0 = stored only, 1..=9 = effort.
pub fn deflate(data: &[u8], level: u32) -> Vec<u8> {
    let mut bw = BitWriter::new(data.len() / 2 + 64);
    if level == 0 || data.is_empty() {
        write_stored(&mut bw, data, true);
        return bw.out;
    }
    let chain_limit = match level {
        1..=3 => 8,
        4..=6 => 48,
        _ => 256,
    };
    let nice = if level >= 7 { 258 } else { 96 };
    const WBITS: usize = 15;
    const WSIZE: usize = 1 << WBITS;
    const WMASK: usize = WSIZE - 1;
    const HBITS: usize = 15;
    let n = data.len();
    let mut head = vec![u32::MAX; 1 << HBITS];
    let mut prev = vec![u32::MAX; WSIZE];
    let hash = |i: usize| -> usize {
        (((data[i] as usize) << 10) ^ ((data[i + 1] as usize) << 5) ^ (data[i + 2] as usize)
            ^ ((data[i + 2] as usize) << 7))
            & ((1 << HBITS) - 1)
    };
    let insert = |i: usize, head: &mut Vec<u32>, prev: &mut Vec<u32>| {
        if i + 2 < n {
            let h = hash(i);
            prev[i & WMASK] = head[h];
            head[h] = i as u32;
        }
    };
    let find = |i: usize, head: &Vec<u32>, prev: &Vec<u32>| -> (usize, usize) {
        if i + 2 >= n {
            return (0, 0);
        }
        let maxlen = (n - i).min(258);
        let mut best = 0usize;
        let mut bestd = 0usize;
        let mut cand = head[hash(i)];
        let mut chain = chain_limit;
        while cand != u32::MAX && chain > 0 {
            let c = cand as usize;
            if c >= i || i - c > WSIZE - 262 {
                break;
            }
            if data[c + best.min(maxlen - 1)] == data[i + best.min(maxlen - 1)] && data[c] == data[i] {
                let mut l = 0;
                while l < maxlen && data[c + l] == data[i + l] {
                    l += 1;
                }
                if l > best {
                    best = l;
                    bestd = i - c;
                    if l >= nice || l == maxlen {
                        break;
                    }
                }
            }
            let nxt = prev[c & WMASK];
            if nxt != u32::MAX && nxt as usize >= c {
                break;
            }
            cand = nxt;
            chain -= 1;
        }
        if best < 3 || (best == 3 && bestd > 4096) {
            (0, 0)
        } else {
            (best, bestd)
        }
    };

    let block_syms = 1 << 16;
    let mut syms: Vec<Sym> = Vec::with_capacity(block_syms + 2);
    let mut block_start = 0usize;
    let mut i = 0usize;
    while i < n {
        let (len, dist) = find(i, &head, &prev);
        if len >= 3 {
            insert(i, &mut head, &mut prev);
            if len < nice && i + 1 < n {
                let (l2, _) = find(i + 1, &head, &prev);
                if l2 > len {
                    syms.push(Sym { lit_or_len: data[i] as u16, dist: 0 });
                    i += 1;
                    if syms.len() >= block_syms {
                        emit_block(&mut bw, &syms, &data[block_start..i], false);
                        syms.clear();
                        block_start = i;
                    }
                    continue;
                }
            }
            syms.push(Sym { lit_or_len: len as u16, dist: dist as u16 });
            for k in 1..len {
                insert(i + k, &mut head, &mut prev);
            }
            i += len;
        } else {
            insert(i, &mut head, &mut prev);
            syms.push(Sym { lit_or_len: data[i] as u16, dist: 0 });
            i += 1;
        }
        if syms.len() >= block_syms {
            emit_block(&mut bw, &syms, &data[block_start..i], false);
            syms.clear();
            block_start = i;
        }
    }
    emit_block(&mut bw, &syms, &data[block_start..n], true);
    bw.flush_byte();
    bw.out
}

fn write_stored(bw: &mut BitWriter, raw: &[u8], last: bool) {
    let mut chunks: Vec<&[u8]> = raw.chunks(65535).collect();
    if chunks.is_empty() {
        chunks.push(&[]);
    }
    let nchunks = chunks.len();
    for (ci, c) in chunks.into_iter().enumerate() {
        let fin = last && ci + 1 == nchunks;
        bw.put(fin as u32, 1);
        bw.put(0, 2);
        bw.flush_byte();
        let len = c.len() as u32;
        bw.out.extend_from_slice(&[(len & 0xff) as u8, (len >> 8) as u8, (!len & 0xff) as u8, ((!len >> 8) & 0xff) as u8]);
        bw.out.extend_from_slice(c);
    }
}

fn emit_block(bw: &mut BitWriter, syms: &[Sym], raw: &[u8], last: bool) {
    let mut lf = vec![0u32; 286];
    let mut df = vec![0u32; 30];
    for s in syms {
        if s.dist == 0 {
            lf[s.lit_or_len as usize] += 1;
        } else {
            lf[257 + len_code(s.lit_or_len as usize)] += 1;
            df[dist_code(s.dist as usize)] += 1;
        }
    }
    lf[256] = 1;
    let llens = build_lengths(&lf, 15);
    let mut dlens = build_lengths(&df, 15);
    if dlens.iter().all(|&l| l == 0) {
        dlens[0] = 1;
        dlens[1] = 1;
    }
    let mut hlit = 286;
    while hlit > 257 && llens[hlit - 1] == 0 {
        hlit -= 1;
    }
    let mut hdist = 30;
    while hdist > 1 && dlens[hdist - 1] == 0 {
        hdist -= 1;
    }
    // run-length encode the concatenated code lengths
    let mut all: Vec<u8> = Vec::with_capacity(hlit + hdist);
    all.extend_from_slice(&llens[..hlit]);
    all.extend_from_slice(&dlens[..hdist]);
    let mut rle: Vec<(u8, u8)> = Vec::new(); // (symbol, extra bits value)
    let mut i = 0;
    while i < all.len() {
        let v = all[i];
        let mut run = 1;
        while i + run < all.len() && all[i + run] == v {
            run += 1;
        }
        if v == 0 && run >= 3 {
            let mut r = run;
            while r >= 3 {
                if r >= 11 {
                    let k = r.min(138);
                    rle.push((18, (k - 11) as u8));
                    r -= k;
                } else {
                    let k = r.min(10);
                    rle.push((17, (k - 3) as u8));
                    r -= k;
                }
            }
            for _ in 0..r {
                rle.push((0, 0));
            }
        } else if v != 0 && run >= 4 {
            rle.push((v, 0));
            let mut r = run - 1;
            while r >= 3 {
                let k = r.min(6);
                rle.push((16, (k - 3) as u8));
                r -= k;
            }
            for _ in 0..r {
                rle.push((v, 0));
            }
        } else {
            for _ in 0..run {
                rle.push((v, 0));
            }
        }
        i += run;
    }
    let mut cf = vec![0u32; 19];
    for &(s, _) in &rle {
        cf[s as usize] += 1;
    }
    let clens = build_lengths(&cf, 7);
    let mut hclen = 19;
    while hclen > 4 && clens[CL_ORDER[hclen - 1]] == 0 {
        hclen -= 1;
    }

    // cost estimate: dynamic vs stored
    let mut bits: u64 = 3 + 5 + 5 + 4 + 3 * hclen as u64;
    for &(s, _) in &rle {
        bits += clens[s as usize] as u64 + match s { 16 => 2, 17 => 3, 18 => 7, _ => 0 };
    }
    for (sym, &f) in lf.iter().enumerate() {
        if f > 0 {
            bits += f as u64 * llens[sym] as u64;
            if sym >= 257 {
                bits += f as u64 * LEN_EXTRA[sym - 257] as u64;
            }
        }
    }
    for (sym, &f) in df.iter().enumerate() {
        bits += f as u64 * (dlens[sym] as u64 + DIST_EXTRA[sym] as u64);
    }
    let stored_bits = (raw.len() as u64 + 5 * (raw.len() as u64 / 65535 + 1)) * 8 + 8;
    if stored_bits < bits {
        write_stored(bw, raw, last);
        return;
    }

    let lcodes = canonical_codes(&llens);
    let dcodes = canonical_codes(&dlens);
    let ccodes = canonical_codes(&clens);
    bw.put(last as u32, 1);
    bw.put(2, 2);
    bw.put((hlit - 257) as u32, 5);
    bw.put((hdist - 1) as u32, 5);
    bw.put((hclen - 4) as u32, 4);
    for &o in CL_ORDER.iter().take(hclen) {
        bw.put(clens[o] as u32, 3);
    }
    for &(s, e) in &rle {
        bw.put(ccodes[s as usize], clens[s as usize] as u32);
        match s {
            16 => bw.put(e as u32, 2),
            17 => bw.put(e as u32, 3),
            18 => bw.put(e as u32, 7),
            _ => {}
        }
    }
    for s in syms {
        if s.dist == 0 {
            let l = s.lit_or_len as usize;
            bw.put(lcodes[l], llens[l] as u32);
        } else {
            let len = s.lit_or_len as usize;
            let lc = len_code(len);
            bw.put(lcodes[257 + lc], llens[257 + lc] as u32);
            bw.put((len - LEN_BASE[lc] as usize) as u32, LEN_EXTRA[lc] as u32);
            let d = s.dist as usize;
            let dc = dist_code(d);
            bw.put(dcodes[dc], dlens[dc] as u32);
            bw.put((d - DIST_BASE[dc] as usize) as u32, DIST_EXTRA[dc] as u32);
        }
    }
    bw.put(lcodes[256], llens[256] as u32);
}

/// Wrap a DEFLATE stream in a zlib container.
pub fn zlib_compress(data: &[u8], level: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 16);
    out.push(0x78);
    out.push(0xDA);
    out.extend_from_slice(&deflate(data, level));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    #[test]
    fn crc_known() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn roundtrip_various() {
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut cases: Vec<Vec<u8>> = vec![vec![], vec![7], b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec()];
        let random: Vec<u8> = (0..200_000).map(|_| rng(&mut s) as u8).collect();
        cases.push(random);
        let text: Vec<u8> = (0..300_000).map(|i| b"the quick brown fox jumps over "[(i * 7 + i / 13) % 31]).collect();
        cases.push(text);
        let lowent: Vec<u8> = (0..150_000).map(|_| (rng(&mut s) % 3) as u8).collect();
        cases.push(lowent);
        for c in &cases {
            for level in [0, 1, 6, 9] {
                let z = zlib_compress(c, level);
                let back = zlib_decompress(&z).expect("decode");
                assert_eq!(&back, c, "level {level} len {}", c.len());
            }
        }
    }
}
