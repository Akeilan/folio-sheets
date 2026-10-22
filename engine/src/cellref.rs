//! A1-style references.

pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;

pub fn col_name(c: u32) -> String {
    let mut n = c + 1;
    let mut s = Vec::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.push(b'A' + r as u8);
        n = (n - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// Column letters (any case) to a 0-based index.
pub fn col_index(letters: &str) -> Option<u32> {
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for ch in letters.bytes() {
        let u = ch.to_ascii_uppercase();
        if !u.is_ascii_uppercase() {
            return None;
        }
        n = n * 26 + (u - b'A' + 1) as u32;
    }
    if n == 0 || n > MAX_COLS {
        return None;
    }
    Some(n - 1)
}

pub fn a1(r: u32, c: u32) -> String {
    format!("{}{}", col_name(c), r + 1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rect {
    pub r1: u32,
    pub c1: u32,
    pub r2: u32,
    pub c2: u32,
}

impl Rect {
    pub fn new(r1: u32, c1: u32, r2: u32, c2: u32) -> Self {
        Rect { r1: r1.min(r2), c1: c1.min(c2), r2: r1.max(r2), c2: c1.max(c2) }
    }
    pub fn cell(r: u32, c: u32) -> Self {
        Rect { r1: r, c1: c, r2: r, c2: c }
    }
    pub fn contains(&self, r: u32, c: u32) -> bool {
        r >= self.r1 && r <= self.r2 && c >= self.c1 && c <= self.c2
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        !(o.r2 < self.r1 || o.r1 > self.r2 || o.c2 < self.c1 || o.c1 > self.c2)
    }
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        if !self.intersects(o) {
            return None;
        }
        Some(Rect { r1: self.r1.max(o.r1), c1: self.c1.max(o.c1), r2: self.r2.min(o.r2), c2: self.c2.min(o.c2) })
    }
    pub fn contains_rect(&self, o: &Rect) -> bool {
        o.r1 >= self.r1 && o.r2 <= self.r2 && o.c1 >= self.c1 && o.c2 <= self.c2
    }
    pub fn rows(&self) -> u32 {
        self.r2 - self.r1 + 1
    }
    pub fn cols(&self) -> u32 {
        self.c2 - self.c1 + 1
    }
    pub fn count(&self) -> u64 {
        self.rows() as u64 * self.cols() as u64
    }
    pub fn whole_cols(&self) -> bool {
        self.r1 == 0 && self.r2 == MAX_ROWS - 1
    }
    pub fn whole_rows(&self) -> bool {
        self.c1 == 0 && self.c2 == MAX_COLS - 1
    }
    pub fn tuple(&self) -> (u32, u32, u32, u32) {
        (self.r1, self.c1, self.r2, self.c2)
    }
    pub fn union(&self, o: &Rect) -> Rect {
        Rect { r1: self.r1.min(o.r1), c1: self.c1.min(o.c1), r2: self.r2.max(o.r2), c2: self.c2.max(o.c2) }
    }
    /// "A1", "A1:B2", "A:C", "3:5"
    pub fn a1(&self) -> String {
        if self.whole_cols() && !self.whole_rows() {
            return format!("{}:{}", col_name(self.c1), col_name(self.c2));
        }
        if self.whole_rows() {
            return format!("{}:{}", self.r1 + 1, self.r2 + 1);
        }
        if self.r1 == self.r2 && self.c1 == self.c2 {
            return a1(self.r1, self.c1);
        }
        format!("{}:{}", a1(self.r1, self.c1), a1(self.r2, self.c2))
    }
}

/// "A1" (with optional $) -> (row, col, row_abs, col_abs)
pub fn parse_cell_abs(s: &str) -> Option<(u32, u32, bool, bool)> {
    let b = s.as_bytes();
    let mut i = 0;
    let ca = i < b.len() && b[i] == b'$';
    if ca {
        i += 1;
    }
    let cs = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == cs {
        return None;
    }
    let col = col_index(&s[cs..i])?;
    let ra = i < b.len() && b[i] == b'$';
    if ra {
        i += 1;
    }
    let rs = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == rs || i != b.len() || b[rs] == b'0' {
        return None;
    }
    let row: u32 = s[rs..i].parse().ok()?;
    if row == 0 || row > MAX_ROWS {
        return None;
    }
    Some((row - 1, col, ra, ca))
}

pub fn parse_cell(s: &str) -> Option<(u32, u32)> {
    parse_cell_abs(s).map(|(r, c, _, _)| (r, c))
}

/// "A1", "B2:C5", "A:C", "3:5" (no sheet prefix).
pub fn parse_range(s: &str) -> Option<Rect> {
    let s = s.trim();
    let (a, b) = match s.find(':') {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    if let Some((r1, c1)) = parse_cell(a) {
        let (r2, c2) = match b {
            Some(b) => parse_cell(b)?,
            None => (r1, c1),
        };
        return Some(Rect::new(r1, c1, r2, c2));
    }
    let b = b?;
    let strip = |x: &str| x.trim_start_matches('$').to_string();
    let (a, b) = (strip(a), strip(b));
    if let (Some(c1), Some(c2)) = (col_index(&a), col_index(&b)) {
        return Some(Rect::new(0, c1, MAX_ROWS - 1, c2));
    }
    if let (Ok(r1), Ok(r2)) = (a.parse::<u32>(), b.parse::<u32>()) {
        if r1 >= 1 && r2 >= 1 && r1 <= MAX_ROWS && r2 <= MAX_ROWS {
            return Some(Rect::new(r1 - 1, 0, r2 - 1, MAX_COLS - 1));
        }
    }
    None
}

/// A sheet name as it must be written in a reference ('quoted' when needed).
pub fn quote_sheet(name: &str) -> String {
    let plain = !name.is_empty()
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        && !name.chars().next().unwrap().is_ascii_digit()
        && parse_cell(name).is_none()
        && !looks_like_r1c1(name)
        && !name.eq_ignore_ascii_case("TRUE")
        && !name.eq_ignore_ascii_case("FALSE");
    if plain {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn looks_like_r1c1(s: &str) -> bool {
    let u = s.to_ascii_uppercase();
    (u.starts_with('R') || u.starts_with('C')) && u[1..].chars().all(|c| c.is_ascii_digit() || c == 'C')
        && u.len() > 1
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(16383), "XFD");
        assert_eq!(col_index("xfd"), Some(16383));
        assert_eq!(parse_range("B2:C5"), Some(Rect::new(1, 1, 4, 2)));
        assert_eq!(parse_range("A:C"), Some(Rect::new(0, 0, MAX_ROWS - 1, 2)));
        assert_eq!(parse_range("3:5"), Some(Rect::new(2, 0, 4, MAX_COLS - 1)));
        assert_eq!(parse_range("zz"), None);
        assert_eq!(quote_sheet("My Sheet"), "'My Sheet'");
        assert_eq!(quote_sheet("Sheet1"), "Sheet1");
        assert_eq!(quote_sheet("A1"), "'A1'");
    }
}
