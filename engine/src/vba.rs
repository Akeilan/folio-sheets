//! Macros in .xlsm workbooks (SH-24): the VBA project is kept as it is (never run); this reads
//! enough of it to list the macros, as Excel's Macro dialog does. The project is an OLE compound
//! file ([MS-CFB]) whose "dir" and module streams are compressed ([MS-OVBA] 2.4.1).

/// A macro: a public Sub without arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct Macro {
    pub module: String,
    pub name: String,
    /// standard module (true) or a sheet/workbook/class module
    pub standard: bool,
}

struct Cfb<'a> {
    data: &'a [u8],
    ssz: usize,
    fat: Vec<u32>,
    minifat: Vec<u32>,
    mini: Vec<u8>,
    cutoff: u64,
    dir: Vec<Entry>,
}

#[derive(Clone)]
struct Entry {
    name: String,
    #[allow(dead_code)]
    kind: u8,
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

const END: u32 = 0xFFFF_FFFE;
const FREE: u32 = 0xFFFF_FFFF;

fn u16le(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn u32le(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

impl<'a> Cfb<'a> {
    fn parse(data: &'a [u8]) -> Option<Cfb<'a>> {
        if data.len() < 512 || data[..8] != [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1] {
            return None;
        }
        let shift = u16le(data, 0x1E)? as u32;
        if !(7..=16).contains(&shift) {
            return None;
        }
        let ssz = 1usize << shift;
        let mut cfb = Cfb { data, ssz, fat: vec![], minifat: vec![], mini: vec![], cutoff: u32le(data, 0x38)? as u64, dir: vec![] };
        // the FAT's sectors: 109 in the header, the rest in the DIFAT chain
        let mut fat_secs: Vec<u32> = (0..109).filter_map(|i| u32le(data, 0x4C + i * 4)).filter(|&s| s != FREE && s != END).collect();
        let mut difat = u32le(data, 0x44)?;
        let mut guard = 0;
        while difat != END && difat != FREE && guard < 10_000 {
            let sec = cfb.sector(difat)?;
            let n = ssz / 4 - 1;
            for i in 0..n {
                let s = u32le(sec, i * 4)?;
                if s != FREE && s != END {
                    fat_secs.push(s);
                }
            }
            difat = u32le(sec, n * 4)?;
            guard += 1;
        }
        for s in fat_secs {
            let sec = cfb.sector(s)?;
            for i in 0..ssz / 4 {
                cfb.fat.push(u32le(sec, i * 4)?);
            }
        }
        // directory
        let dir_bytes = cfb.chain(u32le(data, 0x30)?)?;
        for e in dir_bytes.chunks(128) {
            if e.len() < 128 {
                break;
            }
            let nlen = (u16le(e, 64)? as usize).min(64);
            let units: Vec<u16> = (0..nlen.saturating_sub(2) / 2).filter_map(|i| u16le(e, i * 2)).collect();
            cfb.dir.push(Entry {
                name: String::from_utf16_lossy(&units),
                kind: e[66],
                left: u32le(e, 68)?,
                right: u32le(e, 72)?,
                child: u32le(e, 76)?,
                start: u32le(e, 116)?,
                size: u32le(e, 120)? as u64,
            });
        }
        // the mini stream (in the root entry) and its FAT
        if let Some(root) = cfb.dir.first().cloned() {
            let mut m = cfb.chain(root.start).unwrap_or_default();
            m.truncate(root.size as usize);
            cfb.mini = m;
        }
        if let Some(first) = u32le(data, 0x3C) {
            if first != END && first != FREE {
                let mf = cfb.chain(first).unwrap_or_default();
                cfb.minifat = (0..mf.len() / 4).filter_map(|i| u32le(&mf, i * 4)).collect();
            }
        }
        Some(cfb)
    }

    fn sector(&self, s: u32) -> Option<&'a [u8]> {
        let o = (s as usize + 1) * self.ssz;
        self.data.get(o..o + self.ssz)
    }

    fn chain(&self, mut s: u32) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut guard = 0;
        while s != END && s != FREE {
            out.extend_from_slice(self.sector(s)?);
            s = *self.fat.get(s as usize)?;
            guard += 1;
            if guard > 1_000_000 {
                return None;
            }
        }
        Some(out)
    }

    fn stream(&self, e: &Entry) -> Option<Vec<u8>> {
        if e.size < self.cutoff {
            let mut out = Vec::new();
            let mut s = e.start;
            let mut guard = 0;
            while s != END && s != FREE && out.len() < e.size as usize {
                let o = s as usize * 64;
                out.extend_from_slice(self.mini.get(o..o + 64)?);
                s = *self.minifat.get(s as usize)?;
                guard += 1;
                if guard > 1_000_000 {
                    return None;
                }
            }
            out.truncate(e.size as usize);
            Some(out)
        } else {
            let mut out = self.chain(e.start)?;
            out.truncate(e.size as usize);
            Some(out)
        }
    }

    /// Entries directly inside storage `parent` (a red-black tree through left/right siblings).
    fn children(&self, parent: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![self.dir.get(parent).map(|e| e.child).unwrap_or(FREE)];
        let mut guard = 0;
        while let Some(i) = stack.pop() {
            if i == FREE || i as usize >= self.dir.len() || guard > 100_000 {
                continue;
            }
            guard += 1;
            let e = &self.dir[i as usize];
            out.push(i as usize);
            stack.push(e.left);
            stack.push(e.right);
        }
        out
    }

    fn find(&self, parent: usize, name: &str) -> Option<usize> {
        self.children(parent).into_iter().find(|&i| self.dir[i].name.eq_ignore_ascii_case(name))
    }
}

/// [MS-OVBA] 2.4.1 decompression.
pub fn decompress(src: &[u8]) -> Option<Vec<u8>> {
    if src.first() != Some(&1) {
        return None;
    }
    let mut out: Vec<u8> = Vec::new();
    let mut i = 1;
    while i + 2 <= src.len() {
        let header = u16le(src, i)? as usize;
        let size = (header & 0x0FFF) + 3;
        let compressed = header & 0x8000 != 0;
        let end = (i + size).min(src.len());
        let start_out = out.len();
        i += 2;
        if !compressed {
            out.extend_from_slice(src.get(i..(i + 4096).min(src.len()))?);
            i += 4096;
            continue;
        }
        while i < end {
            let flags = src[i];
            i += 1;
            for bit in 0..8 {
                if i >= end {
                    break;
                }
                if flags & (1 << bit) == 0 {
                    out.push(src[i]);
                    i += 1;
                } else {
                    let token = u16le(src, i)? as usize;
                    i += 2;
                    let done = out.len() - start_out;
                    let mut bits = 4;
                    while (1usize << bits) < done && bits < 12 {
                        bits += 1;
                    }
                    let len_mask = 0xFFFF >> bits;
                    let len = (token & len_mask) + 3;
                    let off = (token >> (16 - bits)) + 1;
                    if off > out.len() {
                        return None;
                    }
                    let from = out.len() - off;
                    for k in 0..len {
                        let b = out[from + k];
                        out.push(b);
                    }
                }
            }
        }
        i = end;
    }
    Some(out)
}

struct ModuleInfo {
    name: String,
    stream: String,
    offset: u32,
    standard: bool,
}

fn read_dir(dir: &[u8]) -> (Vec<ModuleInfo>, u16) {
    let mut mods = Vec::new();
    let mut codepage = 1252u16;
    let mut i = 0;
    let mut cur: Option<ModuleInfo> = None;
    while i + 6 <= dir.len() {
        let id = u16le(dir, i).unwrap_or(0);
        if id == 0x0009 {
            // PROJECTVERSION: its size field doesn't count the version numbers
            i += 12;
            continue;
        }
        let size = u32le(dir, i + 2).unwrap_or(0) as usize;
        let data = match dir.get(i + 6..i + 6 + size) {
            Some(d) => d,
            None => break,
        };
        i += 6 + size;
        match id {
            0x0003 if size == 2 => codepage = u16le(data, 0).unwrap_or(1252),
            0x0019 => {
                if let Some(m) = cur.take() {
                    mods.push(m);
                }
                cur = Some(ModuleInfo { name: decode(data, codepage), stream: decode(data, codepage), offset: 0, standard: true });
            }
            0x0047 => {
                // MODULENAMEUNICODE
                if let Some(m) = cur.as_mut() {
                    let units: Vec<u16> = (0..data.len() / 2).filter_map(|k| u16le(data, k * 2)).collect();
                    m.name = String::from_utf16_lossy(&units);
                }
            }
            0x001A => {
                if let Some(m) = cur.as_mut() {
                    m.stream = decode(data, codepage);
                }
            }
            0x0032 => {
                if let Some(m) = cur.as_mut() {
                    let units: Vec<u16> = (0..data.len() / 2).filter_map(|k| u16le(data, k * 2)).collect();
                    m.stream = String::from_utf16_lossy(&units);
                }
            }
            0x0031 => {
                if let Some(m) = cur.as_mut() {
                    m.offset = u32le(data, 0).unwrap_or(0);
                }
            }
            0x0022 => {
                if let Some(m) = cur.as_mut() {
                    m.standard = false;
                }
            }
            0x002B => {
                if let Some(m) = cur.take() {
                    mods.push(m);
                }
            }
            _ => {}
        }
    }
    if let Some(m) = cur.take() {
        mods.push(m);
    }
    (mods, codepage)
}

fn decode(b: &[u8], _codepage: u16) -> String {
    // Western code pages: bytes below 0x80 are ASCII; the rest as Latin-1 (close to 1252)
    b.iter().map(|&c| c as char).collect()
}

/// The macros of a vbaProject.bin, in module order. None if it can't be read.
pub fn macros(project: &[u8]) -> Option<Vec<Macro>> {
    let cfb = Cfb::parse(project)?;
    let vba = cfb.find(0, "VBA")?;
    let dir_e = cfb.find(vba, "dir")?;
    let dir = decompress(&cfb.stream(&cfb.dir[dir_e].clone())?)?;
    let (mods, cp) = read_dir(&dir);
    let mut out = Vec::new();
    for m in mods {
        let e = match cfb.find(vba, &m.stream) {
            Some(e) => e,
            None => continue,
        };
        let raw = match cfb.stream(&cfb.dir[e].clone()) {
            Some(r) => r,
            None => continue,
        };
        let src = match raw.get(m.offset as usize..).and_then(decompress) {
            Some(s) => s,
            None => continue,
        };
        let text = decode(&src, cp);
        for name in public_subs(&text) {
            out.push(Macro { module: m.name.clone(), name, standard: m.standard });
        }
    }
    Some(out)
}

/// Names of the Subs a user can run from the Macro dialog: not Private, no parameters.
pub fn public_subs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut joined = String::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        // continued lines
        if let Some(stripped) = line.strip_suffix(" _") {
            joined.push_str(stripped);
            joined.push(' ');
            continue;
        }
        joined.push_str(line);
        let l = joined.trim().to_string();
        joined.clear();
        let lower = l.to_ascii_lowercase();
        let rest = if let Some(r) = lower.strip_prefix("public sub ") {
            r
        } else if let Some(r) = lower.strip_prefix("sub ") {
            r
        } else {
            continue;
        };
        let off = l.len() - rest.len();
        let name_end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
        if name_end == 0 {
            continue;
        }
        let after = rest[name_end..].trim_start();
        let no_args = after.starts_with("()") || after.is_empty();
        if no_args {
            out.push(l[off..off + name_end].to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subs_a_user_can_run() {
        let src = "Attribute VB_Name = \"Module1\"\r\nSub Hello()\r\n  MsgBox 1\r\nEnd Sub\r\nPrivate Sub Hidden()\r\nEnd Sub\r\nPublic Sub Tidy()\r\nEnd Sub\r\nSub WithArgs(x As Long)\r\nEnd Sub\r\nFunction F()\r\nEnd Function\r\nPublic Sub Long_ _\r\n  ()\r\nEnd Sub\r\n";
        assert_eq!(public_subs(src), vec!["Hello", "Tidy", "Long_"]);
    }

    #[test]
    fn decompresses_the_spec_example() {
        // [MS-OVBA] 3.2.3: "#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa"
        let c: Vec<u8> = vec![
            0x01, 0x2F, 0xB0, 0x00, 0x23, 0x61, 0x61, 0x61, 0x62, 0x63, 0x64, 0x65, 0x82, 0x66, 0x00, 0x70, 0x61, 0x67, 0x68,
            0x69, 0x6A, 0x01, 0x38, 0x08, 0x61, 0x6B, 0x6C, 0x00, 0x30, 0x6D, 0x6E, 0x6F, 0x70, 0x06, 0x71, 0x02, 0x70, 0x04,
            0x10, 0x72, 0x73, 0x74, 0x75, 0x76, 0x10, 0x77, 0x78, 0x79, 0x7A, 0x00, 0x3C,
        ];
        let d = decompress(&c).unwrap();
        assert_eq!(String::from_utf8(d).unwrap(), "#aaabcdefaaaaghijaaaaaklaaamnopqaaaaaaaaaaaarstuvwxyzaaa");
    }

    #[test]
    fn not_a_project() {
        assert!(macros(b"not an OLE file").is_none());
    }
}
