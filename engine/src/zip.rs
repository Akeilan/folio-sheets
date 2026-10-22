//! Minimal from-scratch ZIP reader/writer (stored + deflate), enough for OpenRaster.

use crate::deflate::{crc32, deflate, inflate_sized};

struct CentralEntry {
    name: Vec<u8>,
    crc: u32,
    csize: u32,
    usize_: u32,
    method: u16,
    offset: u32,
}

pub struct ZipWriter {
    buf: Vec<u8>,
    entries: Vec<CentralEntry>,
}

impl ZipWriter {
    pub fn new() -> Self {
        ZipWriter { buf: Vec::new(), entries: Vec::new() }
    }

    pub fn add(&mut self, name: &str, data: &[u8], compress: bool) {
        let crc = crc32(data);
        let mut method = 0u16;
        let mut body: Vec<u8>;
        if compress {
            body = deflate(data, 6);
            method = 8;
            if body.len() >= data.len() {
                body = data.to_vec();
                method = 0;
            }
        } else {
            body = data.to_vec();
        }
        let offset = self.buf.len() as u32;
        let nb = name.as_bytes();
        let b = &mut self.buf;
        b.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        b.extend_from_slice(&20u16.to_le_bytes());
        b.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 names
        b.extend_from_slice(&method.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // time
        b.extend_from_slice(&0x0021u16.to_le_bytes()); // date 1980-01-01
        b.extend_from_slice(&crc.to_le_bytes());
        b.extend_from_slice(&(body.len() as u32).to_le_bytes());
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(nb);
        b.extend_from_slice(&body);
        self.entries.push(CentralEntry {
            name: nb.to_vec(),
            crc,
            csize: body.len() as u32,
            usize_: data.len() as u32,
            method,
            offset,
        });
    }

    pub fn finish(mut self) -> Vec<u8> {
        let cd_start = self.buf.len() as u32;
        for e in &self.entries {
            let b = &mut self.buf;
            b.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            b.extend_from_slice(&20u16.to_le_bytes());
            b.extend_from_slice(&20u16.to_le_bytes());
            b.extend_from_slice(&0x0800u16.to_le_bytes());
            b.extend_from_slice(&e.method.to_le_bytes());
            b.extend_from_slice(&0u16.to_le_bytes());
            b.extend_from_slice(&0x0021u16.to_le_bytes());
            b.extend_from_slice(&e.crc.to_le_bytes());
            b.extend_from_slice(&e.csize.to_le_bytes());
            b.extend_from_slice(&e.usize_.to_le_bytes());
            b.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            b.extend_from_slice(&[0; 2 + 2 + 2 + 2 + 4]); // extra, comment, disk, int attr, ext attr
            b.extend_from_slice(&e.offset.to_le_bytes());
            b.extend_from_slice(&e.name);
        }
        let cd_size = self.buf.len() as u32 - cd_start;
        let n = self.entries.len() as u16;
        let b = &mut self.buf;
        b.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0, 0]);
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
        b.extend_from_slice(&cd_size.to_le_bytes());
        b.extend_from_slice(&cd_start.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        self.buf
    }
}

pub struct ZipReader<'a> {
    data: &'a [u8],
    entries: Vec<CentralEntry>,
}

fn u16le(d: &[u8], i: usize) -> usize {
    u16::from_le_bytes([d[i], d[i + 1]]) as usize
}
fn u32le(d: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]])
}

impl<'a> ZipReader<'a> {
    pub fn open(data: &'a [u8]) -> Result<Self, String> {
        if data.len() < 22 {
            return Err("ZIP: file too short".into());
        }
        let lo = data.len().saturating_sub(22 + 65535);
        let mut eocd = None;
        let mut i = data.len() - 22;
        loop {
            if u32le(data, i) == 0x0605_4b50 {
                eocd = Some(i);
                break;
            }
            if i == lo {
                break;
            }
            i -= 1;
        }
        let e = eocd.ok_or("ZIP: end of central directory not found")?;
        let count = u16le(data, e + 10);
        let mut p = u32le(data, e + 16) as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            if p + 46 > data.len() || u32le(data, p) != 0x0201_4b50 {
                return Err("ZIP: corrupt central directory".into());
            }
            let method = u16le(data, p + 10) as u16;
            let crc = u32le(data, p + 16);
            let csize = u32le(data, p + 20);
            let usize_ = u32le(data, p + 24);
            let nl = u16le(data, p + 28);
            let xl = u16le(data, p + 30);
            let cl = u16le(data, p + 32);
            let offset = u32le(data, p + 42);
            if p + 46 + nl > data.len() {
                return Err("ZIP: corrupt entry name".into());
            }
            let name = data[p + 46..p + 46 + nl].to_vec();
            entries.push(CentralEntry { name, crc, csize, usize_, method, offset });
            p += 46 + nl + xl + cl;
        }
        Ok(ZipReader { data, entries })
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| String::from_utf8_lossy(&e.name).into_owned()).collect()
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let e = self
            .entries
            .iter()
            .find(|e| e.name == name.as_bytes())
            .ok_or_else(|| format!("ZIP: entry '{name}' not found"))?;
        let lp = e.offset as usize;
        if lp + 30 > self.data.len() || u32le(self.data, lp) != 0x0403_4b50 {
            return Err("ZIP: corrupt local header".into());
        }
        let start = lp + 30 + u16le(self.data, lp + 26) + u16le(self.data, lp + 28);
        let end = start + e.csize as usize;
        if end > self.data.len() {
            return Err("ZIP: entry truncated".into());
        }
        let raw = &self.data[start..end];
        let out = match e.method {
            0 => raw.to_vec(),
            8 => inflate_sized(raw, e.usize_ as usize)?,
            m => return Err(format!("ZIP: unsupported compression method {m}")),
        };
        if crc32(&out) != e.crc {
            return Err(format!("ZIP: checksum mismatch in '{name}'"));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let mut z = ZipWriter::new();
        z.add("mimetype", b"image/openraster", false);
        let big: Vec<u8> = (0..100_000).map(|i| (i % 251) as u8).collect();
        z.add("data/a.bin", &big, true);
        let bytes = z.finish();
        let r = ZipReader::open(&bytes).unwrap();
        assert_eq!(r.read("mimetype").unwrap(), b"image/openraster");
        assert_eq!(r.read("data/a.bin").unwrap(), big);
    }
}
