//! Tables (Format as Table): the model, structured references, the table parts of the
//! file, and the commands (create, resize, convert, options, totals, calculated columns).

use crate::book::{Book, Delta};
use crate::cellref::{a1, col_name, Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, TokKind};
use crate::sheet::{Sheet};
use crate::value::Value;
use crate::xml::{self, Element, Node};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TableCol {
    pub id: u32,
    pub name: String,
    /// sum, average, count, countNums, max, min, stdDev, var, custom
    pub totals_fn: Option<String>,
    pub totals_label: Option<String>,
    pub totals_formula: Option<String>,
    /// the calculated column formula (as typed, without "=")
    pub calc: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub id: u32,
    pub name: String,
    pub rect: Rect,
    pub header: bool,
    pub totals: bool,
    pub cols: Vec<TableCol>,
    pub style: Option<String>,
    pub first_col: bool,
    pub last_col: bool,
    pub row_stripes: bool,
    pub col_stripes: bool,
    /// filter buttons on the header row, and the filters set
    pub filter_button: bool,
    pub filter: Option<crate::book_data::AutoFilter>,
    pub part: Option<String>,
    pub orig: Option<(String, Element)>,
}

impl Table {
    /// The data rows (between the header and the totals).
    pub fn data(&self) -> Rect {
        let r1 = self.rect.r1 + self.header as u32;
        let r2 = if self.totals { self.rect.r2.saturating_sub(1) } else { self.rect.r2 };
        Rect::new(r1, self.rect.c1, r2.max(r1), self.rect.c2)
    }
    pub fn header_row(&self) -> Option<u32> {
        if self.header { Some(self.rect.r1) } else { None }
    }
    pub fn totals_row(&self) -> Option<u32> {
        if self.totals { Some(self.rect.r2) } else { None }
    }
    pub fn col_index(&self, name: &str) -> Option<usize> {
        self.cols.iter().position(|c| c.name.eq_ignore_ascii_case(name))
    }
    /// The range the filter covers (header and data).
    pub fn filter_rect(&self) -> Rect {
        let d = self.data();
        Rect::new(self.rect.r1, self.rect.c1, d.r2, self.rect.c2)
    }
}

// ---------------------------------------------------------------------- structured references

#[derive(Clone, Debug, PartialEq, Default)]
pub struct StructRef {
    pub table: Option<String>,
    pub all: bool,
    pub data: bool,
    pub headers: bool,
    pub totals: bool,
    pub this_row: bool,
    /// first and last column names (None: every column)
    pub cols: Option<(String, String)>,
}

fn unescape_col(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\'' {
            if let Some(n) = it.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn escape_col(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if matches!(c, '[' | ']' | '#' | '\'') {
            out.push('\'');
        }
        out.push(c);
    }
    out
}

/// The bracketed items of "[a],[b]:[c]" at the top level.
fn items(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    let mut esc = false;
    for c in s.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        match c {
            '\'' => {
                cur.push(c);
                esc = true;
            }
            '[' => {
                depth += 1;
                cur.push(c);
            }
            ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn strip(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') { &s[1..s.len() - 1] } else { s }
}

pub fn parse_struct(text: &str) -> Option<StructRef> {
    let i = text.find('[')?;
    let table = if i == 0 { None } else { Some(text[..i].to_string()) };
    let inner = text[i..].trim();
    if !inner.ends_with(']') {
        return None;
    }
    let inner = &inner[1..inner.len() - 1];
    let mut r = StructRef { table, ..Default::default() };
    let col_item = |r: &mut StructRef, it: &str| {
        // [A] or [A]:[B]
        let parts: Vec<&str> = it.splitn(2, "]:[").collect();
        if parts.len() == 2 {
            r.cols = Some((unescape_col(strip(&format!("{}]", parts[0]))), unescape_col(strip(&format!("[{}", parts[1])))));
        } else {
            let n = unescape_col(strip(it));
            r.cols = Some((n.clone(), n));
        }
    };
    let special = |r: &mut StructRef, it: &str| -> bool {
        match strip(it).to_ascii_lowercase().as_str() {
            "#all" => r.all = true,
            "#data" => r.data = true,
            "#headers" => r.headers = true,
            "#totals" => r.totals = true,
            "#this row" => r.this_row = true,
            _ => return false,
        }
        true
    };
    if let Some(rest) = inner.strip_prefix('@') {
        r.this_row = true;
        let rest = rest.trim();
        if !rest.is_empty() {
            if rest.starts_with('[') && rest.contains("]:[") {
                col_item(&mut r, rest);
            } else {
                let n = unescape_col(strip(rest));
                r.cols = Some((n.clone(), n));
            }
        }
        return Some(r);
    }
    if inner.trim().is_empty() {
        r.data = true;
        return Some(r);
    }
    if !inner.contains('[') {
        if !special(&mut r, inner) {
            let n = unescape_col(inner);
            r.cols = Some((n.clone(), n));
        }
        return Some(r);
    }
    for it in items(inner) {
        if !special(&mut r, &it) {
            col_item(&mut r, &it);
        }
    }
    Some(r)
}

pub fn write_struct(r: &StructRef, qualify: Option<&str>, file: bool) -> String {
    let name = r.table.clone().or_else(|| qualify.map(|s| s.to_string())).unwrap_or_default();
    let col = |c: &(String, String)| -> String {
        if c.0.eq_ignore_ascii_case(&c.1) { format!("[{}]", escape_col(&c.0)) } else { format!("[{}]:[{}]", escape_col(&c.0), escape_col(&c.1)) }
    };
    // the short forms the formula bar shows
    if !file && r.this_row && !r.all && !r.data && !r.headers && !r.totals {
        return match &r.cols {
            None => format!("{}[@]", if r.table.is_some() { &name } else { "" }),
            Some(c) => {
                let simple = c.0.eq_ignore_ascii_case(&c.1) && !c.0.chars().any(|x| matches!(x, ' ' | '[' | ']' | '#' | '\'' | ',' | ':' | '@'));
                let tbl = if r.table.is_some() { name.as_str() } else { "" };
                if simple { format!("{}[@{}]", tbl, c.0) } else { format!("{}[@{}]", tbl, col(c)) }
            }
        };
    }
    let mut parts: Vec<String> = Vec::new();
    if r.all { parts.push("[#All]".into()) }
    if r.headers { parts.push("[#Headers]".into()) }
    if r.data { parts.push("[#Data]".into()) }
    if r.totals { parts.push("[#Totals]".into()) }
    if r.this_row { parts.push("[#This Row]".into()) }
    if let Some(c) = &r.cols {
        parts.push(col(c));
    }
    let body = if parts.len() == 1 && r.cols.is_some() && !r.all && !r.headers && !r.data && !r.totals && !r.this_row {
        let c = r.cols.as_ref().unwrap();
        if c.0.eq_ignore_ascii_case(&c.1) { format!("[{}]", escape_col(&c.0)) } else { format!("[{}]", col(c)) }
    } else if parts.len() == 1 && r.cols.is_none() {
        parts[0].clone()
    } else if parts.is_empty() {
        "[]".into()
    } else {
        format!("[{}]", parts.join(","))
    };
    format!("{}{}", name, body)
}

/// Find a table by name in the sheets: (sheet, index).
pub fn find_table(sheets: &[Sheet], name: &str) -> Option<(usize, usize)> {
    for (s, sh) in sheets.iter().enumerate() {
        if let Some(i) = sh.tables.iter().position(|t| t.name.eq_ignore_ascii_case(name)) {
            return Some((s, i));
        }
    }
    None
}

pub fn table_at(sheets: &[Sheet], s: usize, r: u32, c: u32) -> Option<usize> {
    sheets.get(s)?.tables.iter().position(|t| t.rect.contains(r, c))
}

/// The cells a structured reference names: (sheet, range).
pub fn resolve(sheets: &[Sheet], text: &str, here: usize, cell: Option<(u32, u32)>) -> Option<(usize, Rect)> {
    let sr = parse_struct(text)?;
    let (s, ti) = match &sr.table {
        Some(n) => find_table(sheets, n)?,
        None => {
            let (r, c) = cell?;
            (here, table_at(sheets, here, r, c)?)
        }
    };
    let t = &sheets[s].tables[ti];
    let (c1, c2) = match &sr.cols {
        None => (t.rect.c1, t.rect.c2),
        Some((a, b)) => {
            let i = t.col_index(a)? as u32 + t.rect.c1;
            let j = t.col_index(b)? as u32 + t.rect.c1;
            (i.min(j), i.max(j))
        }
    };
    let d = t.data();
    let (r1, r2) = if sr.all {
        (t.rect.r1, t.rect.r2)
    } else if sr.this_row {
        let (r, _) = cell?;
        if r < d.r1 || r > d.r2 {
            // outside the data rows: the whole column (dependencies), or nothing
            if s != here || cell.is_none() {
                return None;
            }
            return Some((s, Rect::new(d.r1, c1, d.r2, c2)));
        }
        (r, r)
    } else {
        let mut lo = u32::MAX;
        let mut hi = 0u32;
        let mut any = false;
        if sr.headers {
            if let Some(h) = t.header_row() {
                lo = lo.min(h);
                hi = hi.max(h);
                any = true;
            } else if !sr.data && !sr.totals {
                return None;
            }
        }
        if sr.data || (!sr.headers && !sr.totals) {
            lo = lo.min(d.r1);
            hi = hi.max(d.r2);
            any = true;
        }
        if sr.totals {
            if let Some(tr) = t.totals_row() {
                lo = lo.min(tr);
                hi = hi.max(tr);
                any = true;
            } else if !sr.data && !sr.headers {
                return None;
            }
        }
        if !any {
            return None;
        }
        (lo, hi)
    };
    Some((s, Rect::new(r1, c1, r2, c2)))
}

/// Rewrite the structured references of a formula with f (None: keep).
pub fn rewrite_structs(text: &str, mut f: impl FnMut(&StructRef) -> Option<String>) -> String {
    let toks = match formula::tokenize(text) {
        Ok(t) => t,
        Err(_) => return text.to_string(),
    };
    if !toks.iter().any(|t| t.kind == TokKind::Struct) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 16);
    for t in &toks {
        if t.kind == TokKind::Struct {
            if let Some(sr) = parse_struct(&t.text) {
                if let Some(n) = f(&sr) {
                    out.push_str(&n);
                    continue;
                }
            }
        }
        out.push_str(&t.text);
    }
    out
}

/// Formula text as the file keeps it: references qualified with the table's name.
pub fn to_file(text: &str, table: Option<&str>) -> String {
    rewrite_structs(text, |sr| Some(write_struct(sr, table, true)))
}

/// Formula text as the formula bar shows it inside table `here` (Table1[[#This Row],[A]] -> [@A]).
pub fn to_display(text: &str, here: Option<&str>) -> String {
    rewrite_structs(text, |sr| {
        let mut x = sr.clone();
        if let (Some(t), Some(h)) = (&sr.table, here) {
            if t.eq_ignore_ascii_case(h) && sr.this_row {
                x.table = None;
            }
        }
        Some(write_struct(&x, None, false))
    })
}

// ---------------------------------------------------------------------- styles

/// What a table style does to one cell.
#[derive(Clone, Debug, Default)]
pub struct TCell {
    pub fill: Option<u32>,
    pub color: Option<u32>,
    pub bold: bool,
    /// left, right, top, bottom: (style, colour)
    pub borders: [Option<(&'static str, u32)>; 4],
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Whole,
    Header,
    Totals,
    RowStripe,
    ColStripe,
    FirstCol,
    LastCol,
}

/// The built-in styles: family (Light/Medium/Dark) and number.
pub fn builtin(name: &str) -> Option<(&'static str, u32)> {
    let n = name.strip_prefix("TableStyle")?;
    for fam in ["Light", "Medium", "Dark"] {
        if let Some(k) = n.strip_prefix(fam) {
            let k: u32 = k.parse().ok()?;
            let max = match fam { "Light" => 21, "Medium" => 28, _ => 11 };
            if k >= 1 && k <= max {
                return Some((match fam { "Light" => "Light", "Medium" => "Medium", _ => "Dark" }, k));
            }
        }
    }
    None
}

fn tint(c: u32, t: f64) -> u32 {
    crate::styles::apply_tint(c, t)
}

impl Book {
    /// The style formatting of cell (r, c) in table t.
    pub fn table_cell(&self, t: &Table, r: u32, c: u32) -> TCell {
        let mut out = TCell::default();
        let style = match &t.style {
            Some(s) => s.clone(),
            None => return out,
        };
        let parts = self.parts_at(t, r, c);
        if let Some((fam, k)) = builtin(&style) {
            let th = &self.styles.theme;
            // the colour of the style's column in the gallery: dark 1 (black), then accents 1..6
            let idx = (k - 1) % 7;
            let base = if idx == 0 { 0x000000 } else { th[3 + idx as usize] };
            let gray = |t: f64| tint(0x000000, t);
            let light = |t: f64| if idx == 0 { gray(t) } else { tint(base, t) };
            let row = (k - 1) / 7;
            let thin = "thin";
            let set_b = |o: &mut TCell, side: usize, st: &'static str, col: u32| o.borders[side] = Some((st, col));
            for p in parts {
                match (fam, row, p) {
                    // Light 1-7: lines at the top and bottom, light bands
                    ("Light", 0, Part::Whole) => {
                        if r == t.rect.r1 { set_b(&mut out, 2, thin, base) }
                        if r == t.rect.r2 { set_b(&mut out, 3, thin, base) }
                        if idx != 0 { out.color = Some(tint(base, -0.25)) }
                    }
                    ("Light", 0, Part::Header) => {
                        out.bold = true;
                        set_b(&mut out, 3, thin, base);
                    }
                    ("Light", 0, Part::RowStripe) | ("Light", 0, Part::ColStripe) => out.fill = Some(light(0.8)),
                    // Light 8-14: coloured header, outlined
                    ("Light", 1, Part::Whole) => {
                        if r == t.rect.r1 { set_b(&mut out, 2, thin, base) }
                        if r == t.rect.r2 { set_b(&mut out, 3, thin, base) }
                        if c == t.rect.c1 { set_b(&mut out, 0, thin, base) }
                        if c == t.rect.c2 { set_b(&mut out, 1, thin, base) }
                    }
                    ("Light", 1, Part::Header) => {
                        out.fill = Some(base);
                        out.color = Some(0xFFFFFF);
                        out.bold = true;
                    }
                    ("Light", 1, Part::RowStripe) => {
                        set_b(&mut out, 2, thin, base);
                        set_b(&mut out, 3, thin, base);
                    }
                    ("Light", 1, Part::ColStripe) => {
                        set_b(&mut out, 0, thin, base);
                        set_b(&mut out, 1, thin, base);
                    }
                    // Light 15-21: a grid
                    ("Light", 2, Part::Whole) => {
                        for side in 0..4 { set_b(&mut out, side, thin, base) }
                    }
                    ("Light", 2, Part::Header) => {
                        out.bold = true;
                        set_b(&mut out, 3, "medium", base);
                    }
                    ("Light", 2, Part::RowStripe) | ("Light", 2, Part::ColStripe) => out.fill = Some(light(0.8)),
                    // Medium 1-7: coloured header, light bands, light lines
                    ("Medium", 0, Part::Whole) => {
                        let l = light(0.4);
                        set_b(&mut out, 2, thin, l);
                        set_b(&mut out, 3, thin, l);
                        if c == t.rect.c1 { set_b(&mut out, 0, thin, l) }
                        if c == t.rect.c2 { set_b(&mut out, 1, thin, l) }
                    }
                    ("Medium", 0, Part::Header) => {
                        out.fill = Some(base);
                        out.color = Some(0xFFFFFF);
                        out.bold = true;
                        out.borders = [None, None, Some((thin, base)), Some((thin, base))];
                    }
                    ("Medium", 0, Part::RowStripe) | ("Medium", 0, Part::ColStripe) => out.fill = Some(light(0.8)),
                    // Medium 8-14: filled, white lines
                    ("Medium", 1, Part::Whole) => {
                        out.fill = Some(light(0.8));
                        for side in 0..4 { set_b(&mut out, side, thin, 0xFFFFFF) }
                    }
                    ("Medium", 1, Part::Header) | ("Medium", 1, Part::FirstCol) | ("Medium", 1, Part::LastCol) => {
                        out.fill = Some(base);
                        out.color = Some(0xFFFFFF);
                        out.bold = true;
                        if p == Part::Header { set_b(&mut out, 3, "thick", 0xFFFFFF) }
                    }
                    ("Medium", 1, Part::RowStripe) | ("Medium", 1, Part::ColStripe) => out.fill = Some(light(0.6)),
                    // Medium 15-21: dark lines, gray bands
                    ("Medium", 2, Part::Whole) => {
                        if r == t.rect.r1 { set_b(&mut out, 2, "medium", 0x000000) }
                        if r == t.rect.r2 { set_b(&mut out, 3, "medium", 0x000000) }
                        set_b(&mut out, 3, thin, 0x000000);
                    }
                    ("Medium", 2, Part::Header) => {
                        out.fill = Some(base);
                        out.color = Some(0xFFFFFF);
                        out.bold = true;
                        set_b(&mut out, 3, "medium", 0x000000);
                    }
                    ("Medium", 2, Part::RowStripe) | ("Medium", 2, Part::ColStripe) => out.fill = Some(gray(0.85)),
                    // Medium 22-28: filled with lines in a light shade
                    ("Medium", 3, Part::Whole) => {
                        out.fill = Some(light(0.8));
                        for side in 0..4 { set_b(&mut out, side, thin, light(0.4)) }
                    }
                    ("Medium", 3, Part::Header) => out.bold = true,
                    ("Medium", 3, Part::RowStripe) | ("Medium", 3, Part::ColStripe) => out.fill = Some(light(0.6)),
                    // Dark 1-7: dark fills, white text
                    ("Dark", 0, Part::Whole) => {
                        out.fill = Some(if idx == 0 { 0x737373 } else { tint(base, -0.25) });
                        out.color = Some(0xFFFFFF);
                    }
                    ("Dark", 0, Part::Header) => {
                        out.fill = Some(0x000000);
                        out.bold = true;
                        set_b(&mut out, 3, "medium", 0xFFFFFF);
                    }
                    ("Dark", 0, Part::RowStripe) | ("Dark", 0, Part::ColStripe) => out.fill = Some(if idx == 0 { 0x404040 } else { tint(base, -0.5) }),
                    ("Dark", 0, Part::FirstCol) | ("Dark", 0, Part::LastCol) => {
                        out.fill = Some(if idx == 0 { 0x404040 } else { tint(base, -0.5) });
                        out.bold = true;
                    }
                    // Dark 8-11: light fills with a black header
                    ("Dark", _, Part::Whole) => out.fill = Some(if idx == 0 { gray(0.65) } else { light(0.6) }),
                    ("Dark", _, Part::Header) => {
                        out.fill = Some(0x000000);
                        out.color = Some(0xFFFFFF);
                        out.bold = true;
                    }
                    ("Dark", _, Part::RowStripe) | ("Dark", _, Part::ColStripe) => out.fill = Some(if idx == 0 { gray(0.5) } else { light(0.4) }),
                    (_, _, Part::Totals) => {
                        out.bold = true;
                        set_b(&mut out, 2, "double", if fam == "Dark" { 0xFFFFFF } else { base });
                        if fam == "Medium" && row == 1 {
                            out.fill = Some(base);
                            out.color = Some(0xFFFFFF);
                        }
                    }
                    (_, _, Part::FirstCol) | (_, _, Part::LastCol) => out.bold = true,
                    _ => {}
                }
            }
            return out;
        }
        // a style of the workbook's own (tableStyles in the styles part)
        if let Some(ts) = self.styles.root.child("tableStyles").and_then(|x| x.elems().find(|e| e.attr("name") == Some(style.as_str()))) {
            let el = |kind: &str| ts.elems().find(|e| e.attr("type") == Some(kind)).and_then(|e| e.attr("dxfId")).and_then(|v| v.parse::<u32>().ok());
            for p in parts {
                let kind = match p {
                    Part::Whole => "wholeTable",
                    Part::Header => "headerRow",
                    Part::Totals => "totalRow",
                    Part::RowStripe => "firstRowStripe",
                    Part::ColStripe => "firstColumnStripe",
                    Part::FirstCol => "firstColumn",
                    Part::LastCol => "lastColumn",
                };
                if let Some(d) = el(kind) {
                    let d = self.dxf(d);
                    if d.fill.is_some() { out.fill = d.fill }
                    if d.color.is_some() { out.color = d.color }
                    if d.bold == Some(true) { out.bold = true }
                    for k in 0..4 {
                        if let Some((st, col)) = &d.borders[k] {
                            out.borders[k] = Some((leak_style(st), *col));
                        }
                    }
                }
            }
        }
        out
    }

    /// The parts of the style that apply to a cell, lowest precedence first.
    fn parts_at(&self, t: &Table, r: u32, c: u32) -> Vec<Part> {
        let mut v = vec![Part::Whole];
        let d = t.data();
        let in_data = r >= d.r1 && r <= d.r2 && !(t.header && r == t.rect.r1) && !(t.totals && r == t.rect.r2);
        if t.col_stripes && in_data && (c - t.rect.c1) % 2 == 0 {
            v.push(Part::ColStripe);
        }
        if t.row_stripes && in_data && (r - d.r1) % 2 == 0 {
            v.push(Part::RowStripe);
        }
        if t.last_col && c == t.rect.c2 {
            v.push(Part::LastCol);
        }
        if t.first_col && c == t.rect.c1 {
            v.push(Part::FirstCol);
        }
        if t.header && r == t.rect.r1 {
            v.push(Part::Header);
        }
        if t.totals && r == t.rect.r2 {
            v.push(Part::Totals);
        }
        v
    }
}

fn leak_style(s: &str) -> &'static str {
    match s {
        "thin" => "thin",
        "medium" => "medium",
        "thick" => "thick",
        "double" => "double",
        "dashed" => "dashed",
        "dotted" => "dotted",
        "hair" => "hair",
        _ => "thin",
    }
}

// ---------------------------------------------------------------------- the file

fn flag(e: &Element, name: &str, default: bool) -> bool {
    match e.attr(name) {
        Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        None => default,
    }
}

pub fn read_table(part: &str, text: &str) -> Option<Table> {
    let (prolog, e) = xml::parse(text).ok()?;
    let rect = crate::cellref::parse_range(e.attr("ref")?)?;
    let header = e.attr("headerRowCount").map(|v| v != "0").unwrap_or(true);
    let totals = e.attr("totalsRowCount").map(|v| v != "0").unwrap_or(false);
    let mut cols = Vec::new();
    if let Some(tc) = e.child("tableColumns") {
        for c in tc.elems() {
            cols.push(TableCol {
                id: c.attr("id").and_then(|v| v.parse().ok()).unwrap_or(cols.len() as u32 + 1),
                name: xml::unescape_ooxml(c.attr("name").unwrap_or("")),
                totals_fn: c.attr("totalsRowFunction").filter(|v| *v != "none").map(|s| s.to_string()),
                totals_label: c.attr("totalsRowLabel").map(|s| s.to_string()),
                totals_formula: c.child("totalsRowFormula").map(|f| formula::from_file_text(&f.text())),
                calc: c.child("calculatedColumnFormula").map(|f| formula::from_file_text(&f.text())),
            });
        }
    }
    let si = e.child("tableStyleInfo");
    let af = e.child("autoFilter");
    let filter = af.map(|a| a.to_string()).and_then(|x| crate::book_data::read_autofilter(&x));
    let name = e.attr("displayName").or_else(|| e.attr("name")).unwrap_or("Table1").to_string();
    Some(Table {
        id: e.attr("id").and_then(|v| v.parse().ok()).unwrap_or(1),
        name,
        rect,
        header,
        totals,
        cols,
        style: si.and_then(|s| s.attr("name")).map(|s| s.to_string()),
        first_col: si.map(|s| flag(s, "showFirstColumn", false)).unwrap_or(false),
        last_col: si.map(|s| flag(s, "showLastColumn", false)).unwrap_or(false),
        row_stripes: si.map(|s| flag(s, "showRowStripes", true)).unwrap_or(false),
        col_stripes: si.map(|s| flag(s, "showColumnStripes", false)).unwrap_or(false),
        filter_button: af.is_some(),
        filter,
        part: Some(part.to_string()),
        orig: Some((prolog, e)),
    })
}

pub fn write_table(t: &Table) -> String {
    let (prolog, mut e) = t.orig.clone().unwrap_or_else(|| (String::new(), Element::with_attrs("table", &[("xmlns", "http://schemas.openxmlformats.org/spreadsheetml/2006/main")])));
    let pfx = match e.name.find(':') {
        Some(k) => e.name[..=k].to_string(),
        None => String::new(),
    };
    let tag = |n: &str| format!("{}{}", pfx, n);
    e.set_attr("id", &t.id.to_string());
    e.set_attr("name", &t.name);
    e.set_attr("displayName", &t.name);
    e.set_attr("ref", &t.rect.a1());
    if t.header { e.remove_attr("headerRowCount") } else { e.set_attr("headerRowCount", "0") }
    if t.totals {
        e.set_attr("totalsRowCount", "1");
        e.remove_attr("totalsRowShown");
    } else {
        e.remove_attr("totalsRowCount");
        e.set_attr("totalsRowShown", "0");
    }
    // keep the attribute order sensible for new tables
    let order = ["autoFilter", "sortState", "tableColumns", "tableStyleInfo", "extLst"];
    e.children.retain(|n| !matches!(n, Node::Elem(x) if matches!(x.local(), "autoFilter" | "tableColumns" | "tableStyleInfo")));
    if t.filter_button && t.header {
        let mut af = t.filter.clone().unwrap_or(crate::book_data::AutoFilter { rect: t.filter_rect(), cols: vec![] });
        af.rect = t.filter_rect();
        let mut s = String::new();
        crate::book_data::write_autofilter(&af, &pfx, &mut s);
        if let Ok((_, a)) = xml::parse(&s) {
            e.insert_ordered(a, &order);
        }
    }
    let mut tc = Element::with_attrs(&tag("tableColumns"), &[("count", &t.cols.len().to_string())]);
    for c in &t.cols {
        let mut x = Element::with_attrs(&tag("tableColumn"), &[("id", &c.id.to_string()), ("name", &escape_name(&c.name))]);
        if let Some(f) = &c.totals_fn {
            x.set_attr("totalsRowFunction", f);
        }
        if let Some(l) = &c.totals_label {
            x.set_attr("totalsRowLabel", l);
        }
        if let Some(f) = &c.calc {
            let mut y = Element::new(&tag("calculatedColumnFormula"));
            y.children.push(Node::Text(formula::to_file_text(&to_file(f, Some(&t.name)))));
            x.push(y);
        }
        if let (Some(f), Some("custom")) = (&c.totals_formula, c.totals_fn.as_deref()) {
            let mut y = Element::new(&tag("totalsRowFormula"));
            y.children.push(Node::Text(formula::to_file_text(&to_file(f, Some(&t.name)))));
            x.push(y);
        }
        tc.push(x);
    }
    e.insert_ordered(tc, &order);
    if let Some(st) = &t.style {
        let si = Element::with_attrs(&tag("tableStyleInfo"), &[
            ("name", st),
            ("showFirstColumn", if t.first_col { "1" } else { "0" }),
            ("showLastColumn", if t.last_col { "1" } else { "0" }),
            ("showRowStripes", if t.row_stripes { "1" } else { "0" }),
            ("showColumnStripes", if t.col_stripes { "1" } else { "0" }),
        ]);
        e.insert_ordered(si, &order);
    }
    xml::document(&prolog, &e)
}

/// A column name as the file keeps it (line breaks as _x000a_).
fn escape_name(s: &str) -> String {
    s.replace('\n', "_x000a_")
}

// ---------------------------------------------------------------------- commands

const SUBTOTAL: [(&str, u32); 8] = [("average", 101), ("count", 103), ("countNums", 102), ("max", 104), ("min", 105),
    ("stdDev", 107), ("sum", 109), ("var", 110)];

pub fn totals_formula(func: &str, col: &str) -> Option<String> {
    let n = SUBTOTAL.iter().find(|x| x.0 == func)?.1;
    Some(format!("SUBTOTAL({},[{}])", n, escape_col(col)))
}

impl Book {
    pub fn tables_on(&self, s: usize) -> &[Table] {
        &self.sheets[s].tables
    }

    pub fn table_at(&self, s: usize, r: u32, c: u32) -> Option<usize> {
        table_at(&self.sheets, s, r, c)
    }

    pub(crate) fn table_names(&self) -> Vec<String> {
        self.sheets.iter().flat_map(|s| s.tables.iter().map(|t| t.name.to_lowercase())).collect()
    }

    pub(crate) fn new_table_name(&self) -> String {
        let used = self.table_names();
        let mut k = 1;
        loop {
            let n = format!("Table{}", k);
            if !used.contains(&n.to_lowercase()) && self.find_name(&n, 0).is_none() {
                return n;
            }
            k += 1;
        }
    }

    pub(crate) fn next_table_id(&self) -> u32 {
        self.sheets.iter().flat_map(|s| s.tables.iter().map(|t| t.id)).max().unwrap_or(0) + 1
    }

    /// Header texts made unique and non-empty, as the original does.
    fn header_names(&self, s: usize, r: u32, c1: u32, c2: u32) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in c1..=c2 {
            let mut t = self.display(s, r, c).trim().to_string();
            if t.is_empty() {
                t = format!("Column{}", c - c1 + 1);
            }
            let base = t.clone();
            let mut k = 2;
            while out.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                t = format!("{}{}", base, k);
                k += 1;
            }
            out.push(t);
        }
        out
    }

    /// Format as Table: a table over rc (with its first row as headers, or a header row added).
    pub fn create_table(&mut self, s: usize, rc: Rect, has_headers: bool, style: &str) -> Result<usize, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("Tables can be made on worksheets only.".into());
        }
        let mut rc = self.sheets[s].clamp_rect(rc);
        if rc.count() == 1 {
            rc = self.current_region(s, rc.r1, rc.c1);
        }
        if rc.rows() > 1_000_000 || rc.cols() > 16384 {
            return Err("The selection is too large.".into());
        }
        let sh = &self.sheets[s];
        if sh.tables.iter().any(|t| t.rect.intersects(&rc)) {
            return Err("A table can't overlap another table.".into());
        }
        if sh.merges.iter().any(|m| m.intersects(&rc)) {
            return Err("A table can't contain merged cells.".into());
        }
        if sh.arrays.values().any(|a| a.intersects(&rc)) || sh.spills.values().any(|a| a.intersects(&rc)) {
            return Err("A table can't contain array formulas or spilled ranges.".into());
        }
        self.begin("Format as Table", s);
        if !has_headers {
            // a header row goes in above the data (the cells below move down)
            if rc.r2 + 1 >= MAX_ROWS {
                self.abort();
                return Err("There's no room for a header row.".into());
            }
            self.commit();
            self.begin_group("Format as Table", s);
            self.insert_cells(s, Rect::new(rc.r1, rc.c1, rc.r1, rc.c2), true)?;
            self.begin("Format as Table", s);
            for c in rc.c1..=rc.c2 {
                let name = format!("Column{}", c - rc.c1 + 1);
                let cell = crate::sheet::Cell { v: Value::string(name), s: self.sheets[s].style_at(rc.r1, c), ..Default::default() };
                self.record(Delta::Cell(s, rc.r1, c, Some(cell)));
            }
            rc.r2 += 1;
        } else if rc.rows() < 2 {
            // a header alone gets an empty data row
            rc.r2 += 1;
        }
        let names = self.header_names(s, rc.r1, rc.c1, rc.c2);
        // header cells must hold their names as text
        for (k, n) in names.iter().enumerate() {
            let c = rc.c1 + k as u32;
            let cur = self.sheets[s].get(rc.r1, c).cloned().unwrap_or_default();
            if cur.f.is_some() || cur.v != Value::string(n.clone()) {
                let cell = crate::sheet::Cell { v: Value::string(n.clone()), s: cur.s, ..Default::default() };
                self.record(Delta::Cell(s, rc.r1, c, Some(cell)));
            }
        }
        let t = Table {
            id: self.next_table_id(),
            name: self.new_table_name(),
            rect: rc,
            header: true,
            totals: false,
            cols: names.into_iter().enumerate().map(|(k, n)| TableCol { id: k as u32 + 1, name: n, ..Default::default() }).collect(),
            style: Some(style.to_string()),
            first_col: false,
            last_col: false,
            row_stripes: true,
            col_stripes: false,
            filter_button: true,
            filter: None,
            part: None,
            orig: None,
        };
        // a sheet AutoFilter over the same cells gives way to the table's
        if let Some(af) = &self.sheets[s].autofilter {
            if af.rect.intersects(&rc) {
                self.record(Delta::Filter(s, None));
            }
        }
        let mut tables = self.sheets[s].tables.clone();
        tables.push(t);
        let i = tables.len() - 1;
        self.record(Delta::Tables(s, tables));
        self.commit();
        if !has_headers {
            self.end_group();
        }
        Ok(i)
    }

    fn set_tables(&mut self, s: usize, tables: Vec<Table>, label: &str) {
        self.begin(label, s);
        self.record(Delta::Tables(s, tables));
        self.commit();
    }

    /// Convert to Range: the cells stay, the table goes (its references become plain ones).
    pub fn convert_table(&mut self, s: usize, i: usize) -> Result<(), String> {
        let t = self.sheets[s].tables.get(i).cloned().ok_or("no such table")?;
        self.begin("Convert to Range", s);
        // structured references become ordinary references
        let sheets = self.sheets.clone();
        for ss in 0..self.sheets.len() {
            let mut edits = Vec::new();
            for ((r, c), cell) in self.sheets[ss].cells.iter() {
                if let Some(f) = &cell.f {
                    let nt = rewrite_structs(&f.text, |sr| {
                        let tn = sr.table.clone().or_else(|| if ss == s && t.rect.contains(*r, *c) { Some(t.name.clone()) } else { None })?;
                        if !tn.eq_ignore_ascii_case(&t.name) {
                            return None;
                        }
                        let (_, rect) = resolve(&sheets, &write_struct(sr, Some(&t.name), true), ss, Some((*r, *c)))?;
                        let pre = if ss == s { String::new() } else { format!("{}!", crate::cellref::quote_sheet(&self.sheets[s].name)) };
                        let one = |r: u32, c: u32| format!("${}${}", col_name(c), r + 1);
                        Some(if rect.count() == 1 { format!("{}{}", pre, one(rect.r1, rect.c1)) } else { format!("{}{}:{}", pre, one(rect.r1, rect.c1), one(rect.r2, rect.c2)) })
                    });
                    if nt != f.text {
                        let mut n = cell.clone();
                        let mut nf = (**f).clone();
                        nf.text = nt;
                        nf.expr = formula::parse(&nf.text).ok();
                        n.f = Some(std::rc::Rc::new(nf));
                        edits.push((*r, *c, n));
                    }
                }
            }
            for (r, c, n) in edits {
                self.record(Delta::Cell(ss, r, c, Some(n)));
            }
        }
        // the look stays as cell formatting
        let mut styled = Vec::new();
        for r in t.rect.r1..=t.rect.r2 {
            for c in t.rect.c1..=t.rect.c2 {
                let tc = self.table_cell(&t, r, c);
                if tc.fill.is_none() && tc.color.is_none() && !tc.bold && tc.borders.iter().all(|b| b.is_none()) {
                    continue;
                }
                let xf = self.sheets[s].style_at(r, c);
                let mut ch = crate::styles::Change::default();
                if let Some(f) = tc.fill {
                    if self.fill_color(self.styles.xf(xf)).is_none() {
                        ch.fill = Some(Some(("solid".into(), Some(f), None)));
                    }
                }
                if tc.color.is_some() || tc.bold {
                    ch.font = Some(crate::styles::FontChange {
                        color: tc.color.map(Some),
                        bold: if tc.bold { Some(true) } else { None },
                        ..Default::default()
                    });
                }
                for (k, side) in ["left", "right", "top", "bottom"].iter().enumerate() {
                    if let Some((st, col)) = tc.borders[k] {
                        ch.border.push((side.to_string(), Some((st.to_string(), Some(col)))));
                    }
                }
                let nx = self.styles.derive(xf, &ch);
                styled.push((r, c, nx));
            }
        }
        for (r, c, nx) in styled {
            let mut cell = self.sheets[s].get(r, c).cloned().unwrap_or_default();
            cell.s = nx;
            self.record(Delta::Cell(s, r, c, Some(cell)));
        }
        let mut tables = self.sheets[s].tables.clone();
        tables.remove(i);
        self.record(Delta::Tables(s, tables));
        self.commit();
        Ok(())
    }

    /// Table Design options.
    pub fn set_table_option(&mut self, s: usize, i: usize, what: &str, on: bool) -> Result<(), String> {
        let mut tables = self.sheets[s].tables.clone();
        let t = tables.get_mut(i).ok_or("no such table")?;
        match what {
            "first_col" => t.first_col = on,
            "last_col" => t.last_col = on,
            "row_stripes" => t.row_stripes = on,
            "col_stripes" => t.col_stripes = on,
            "filter_button" => {
                t.filter_button = on && t.header;
                if !on {
                    t.filter = None;
                }
            }
            "header" => return self.set_table_header(s, i, on),
            "totals" => return self.set_table_totals(s, i, on),
            _ => return Err(format!("unknown option {}", what)),
        }
        self.set_tables(s, tables, "Table Style Options");
        Ok(())
    }

    pub fn set_table_style(&mut self, s: usize, i: usize, style: Option<&str>) -> Result<(), String> {
        let mut tables = self.sheets[s].tables.clone();
        tables.get_mut(i).ok_or("no such table")?.style = style.map(|x| x.to_string());
        self.set_tables(s, tables, "Table Style");
        Ok(())
    }

    fn set_table_header(&mut self, s: usize, i: usize, on: bool) -> Result<(), String> {
        let t = self.sheets[s].tables[i].clone();
        if t.header == on {
            return Ok(());
        }
        self.begin("Header Row", s);
        let mut nt = t.clone();
        if on {
            // the names go back into the row above (it must be free)
            if t.rect.r1 == 0 {
                self.abort();
                return Err("There's no room above the table for the header row.".into());
            }
            let r = t.rect.r1 - 1;
            for (k, col) in t.cols.iter().enumerate() {
                let c = t.rect.c1 + k as u32;
                let cur = self.sheets[s].get(r, c).cloned().unwrap_or_default();
                self.record(Delta::Cell(s, r, c, Some(crate::sheet::Cell { v: Value::string(col.name.clone()), s: cur.s, ..Default::default() })));
            }
            nt.rect.r1 = r;
            nt.header = true;
        } else {
            for c in t.rect.c1..=t.rect.c2 {
                let cur = self.sheets[s].get(t.rect.r1, c).cloned();
                if let Some(cur) = cur {
                    let keep = crate::sheet::Cell { s: cur.s, ..Default::default() };
                    self.record(Delta::Cell(s, t.rect.r1, c, Some(keep)));
                }
            }
            nt.rect.r1 += 1;
            nt.header = false;
            nt.filter_button = false;
            nt.filter = None;
        }
        let mut tables = self.sheets[s].tables.clone();
        tables[i] = nt;
        self.record(Delta::Tables(s, tables));
        self.commit();
        Ok(())
    }

    /// Total Row on or off: a row under the data with Total and the last column's total.
    fn set_table_totals(&mut self, s: usize, i: usize, on: bool) -> Result<(), String> {
        let t = self.sheets[s].tables[i].clone();
        if t.totals == on {
            return Ok(());
        }
        self.begin("Total Row", s);
        let mut nt = t.clone();
        if on {
            let r = t.rect.r2 + 1;
            if r >= MAX_ROWS {
                self.abort();
                return Err("There's no room below the table.".into());
            }
            nt.rect.r2 = r;
            nt.totals = true;
            let n = nt.cols.len();
            for (k, col) in nt.cols.iter_mut().enumerate() {
                if k == 0 && n > 1 {
                    col.totals_label = Some("Total".into());
                    col.totals_fn = None;
                } else if k + 1 == n {
                    let d = t.data();
                    let c = t.rect.c1 + k as u32;
                    let numeric = (d.r1..=d.r2).any(|rr| matches!(self.sheets[s].value(rr, c), Value::Num(_)));
                    col.totals_fn = Some(if numeric { "sum" } else { "count" }.into());
                }
            }
            let mut tables = self.sheets[s].tables.clone();
            tables[i] = nt.clone();
            self.record(Delta::Tables(s, tables));
            self.write_totals(s, &nt);
        } else {
            let r = t.rect.r2;
            for c in t.rect.c1..=t.rect.c2 {
                if self.sheets[s].get(r, c).is_some() {
                    let st = self.sheets[s].style_at(r, c);
                    self.record(Delta::Cell(s, r, c, if st == 0 { None } else { Some(crate::sheet::Cell { s: st, ..Default::default() }) }));
                }
            }
            nt.rect.r2 -= 1;
            nt.totals = false;
            let mut tables = self.sheets[s].tables.clone();
            tables[i] = nt;
            self.record(Delta::Tables(s, tables));
        }
        self.commit();
        Ok(())
    }

    /// The totals row's cells from the columns' settings.
    fn write_totals(&mut self, s: usize, t: &Table) {
        let r = match t.totals_row() {
            Some(r) => r,
            None => return,
        };
        for (k, col) in t.cols.iter().enumerate() {
            let c = t.rect.c1 + k as u32;
            let st = self.sheets[s].style_at(r, c);
            let cell = if let Some(l) = &col.totals_label {
                Some(crate::sheet::Cell { v: Value::string(l.clone()), s: st, ..Default::default() })
            } else if let Some(f) = col.totals_fn.as_deref() {
                let text = if f == "custom" { col.totals_formula.clone() } else { totals_formula(f, &col.name) };
                text.map(|tx| crate::sheet::Cell { f: Some(std::rc::Rc::new(crate::book::make_formula(&tx, crate::sheet::FKind::Dynamic))), s: st, ..Default::default() })
            } else if st != 0 {
                Some(crate::sheet::Cell { s: st, ..Default::default() })
            } else {
                None
            };
            self.record(Delta::Cell(s, r, c, cell));
        }
    }

    /// The function of a totals-row cell: sum, average, count, countNums, max, min,
    /// stdDev, var, none.
    pub fn set_table_total(&mut self, s: usize, i: usize, col: usize, func: &str) -> Result<(), String> {
        let mut tables = self.sheets[s].tables.clone();
        let t = tables.get_mut(i).ok_or("no such table")?;
        if !t.totals {
            return Err("The table has no total row.".into());
        }
        let c = t.cols.get_mut(col).ok_or("no such column")?;
        c.totals_label = None;
        c.totals_fn = if func == "none" { None } else { Some(func.to_string()) };
        let t2 = t.clone();
        self.begin("Total Row", s);
        self.record(Delta::Tables(s, tables));
        self.write_totals(s, &t2);
        self.commit();
        Ok(())
    }

    pub fn rename_table(&mut self, s: usize, i: usize, name: &str) -> Result<(), String> {
        let name = name.trim();
        let old = self.sheets[s].tables.get(i).ok_or("no such table")?.name.clone();
        if old == name {
            return Ok(());
        }
        let valid = !name.is_empty() && name.chars().next().map(|c| c.is_alphabetic() || c == '_' || c == '\\').unwrap_or(false)
            && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '\\')
            && crate::cellref::parse_cell(name).is_none() && name.len() <= 255;
        if !valid {
            return Err("The name that you entered is not valid.".into());
        }
        if self.table_names().iter().any(|n| n.eq_ignore_ascii_case(name) && !n.eq_ignore_ascii_case(&old)) || self.find_name(name, s).is_some() {
            return Err("The name already exists. Enter a unique name.".into());
        }
        self.begin("Table Name", s);
        let new = name.to_string();
        self.rewrite_all_structs(|sr, _, _| {
            if sr.table.as_deref().map(|t| t.eq_ignore_ascii_case(&old)).unwrap_or(false) {
                let mut x = sr.clone();
                x.table = Some(new.clone());
                Some(write_struct(&x, None, false))
            } else {
                None
            }
        });
        let mut tables = self.sheets[s].tables.clone();
        tables[i].name = new;
        self.record(Delta::Tables(s, tables));
        self.commit();
        Ok(())
    }

    /// Rewrite structured references in every formula (recorded). f gets the reference
    /// and the formula's sheet and cell.
    fn rewrite_all_structs(&mut self, f: impl Fn(&StructRef, usize, (u32, u32)) -> Option<String>) {
        for ss in 0..self.sheets.len() {
            let mut edits = Vec::new();
            for ((r, c), cell) in self.sheets[ss].cells.iter() {
                if let Some(fm) = &cell.f {
                    let nt = rewrite_structs(&fm.text, |sr| f(sr, ss, (*r, *c)));
                    if nt != fm.text {
                        let mut n = cell.clone();
                        let mut nf = (**fm).clone();
                        nf.text = nt;
                        nf.expr = formula::parse(&nf.text).ok();
                        n.f = Some(std::rc::Rc::new(nf));
                        edits.push((*r, *c, n));
                    }
                }
            }
            for (r, c, n) in edits {
                self.record(Delta::Cell(ss, r, c, Some(n)));
            }
        }
    }

    /// Resize Table to rc (same header row).
    pub fn resize_table(&mut self, s: usize, i: usize, rc: Rect) -> Result<(), String> {
        let t = self.sheets[s].tables.get(i).cloned().ok_or("no such table")?;
        let rc = self.sheets[s].clamp_rect(rc);
        if rc.r1 != t.rect.r1 {
            return Err("The headers must remain in the same row, and the resulting table range must overlap the original table range.".into());
        }
        if !rc.intersects(&t.rect) || rc.rows() < 1 + t.header as u32 {
            return Err("The resulting table range must overlap the original table range.".into());
        }
        if self.sheets[s].tables.iter().enumerate().any(|(k, x)| k != i && x.rect.intersects(&rc)) {
            return Err("A table can't overlap another table.".into());
        }
        self.begin("Resize Table", s);
        let mut nt = t.clone();
        nt.rect = rc;
        let names = if t.header { self.header_names(s, rc.r1, rc.c1, rc.c2) } else { (0..rc.cols()).map(|k| format!("Column{}", k + 1)).collect() };
        let mut cols = Vec::new();
        let mut next_id = t.cols.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        for (k, n) in names.iter().enumerate() {
            let c = rc.c1 + k as u32;
            match t.cols.get((c as i64 - t.rect.c1 as i64) as usize).filter(|_| c >= t.rect.c1 && c <= t.rect.c2) {
                Some(old) => {
                    let mut x = old.clone();
                    x.name = n.clone();
                    cols.push(x);
                }
                None => {
                    cols.push(TableCol { id: next_id, name: n.clone(), ..Default::default() });
                    next_id += 1;
                }
            }
            if t.header {
                let cur = self.sheets[s].get(rc.r1, c).cloned().unwrap_or_default();
                if cur.v != Value::string(n.clone()) || cur.f.is_some() {
                    self.record(Delta::Cell(s, rc.r1, c, Some(crate::sheet::Cell { v: Value::string(n.clone()), s: cur.s, ..Default::default() })));
                }
            }
        }
        nt.cols = cols;
        let mut tables = self.sheets[s].tables.clone();
        tables[i] = nt;
        self.record(Delta::Tables(s, tables));
        self.commit();
        Ok(())
    }

    /// After typing into (r, c): header names follow their cells, a formula fills its
    /// column, and typing just below a table makes it grow.
    pub(crate) fn tables_after_input(&mut self, s: usize, r: u32, c: u32) {
        if self.sheets[s].tables.is_empty() {
            return;
        }
        // growing: the row below (or the column to the right), when there is no total row
        let grow = self.sheets[s].tables.iter().position(|t| {
            !t.totals && ((r == t.rect.r2 + 1 && c >= t.rect.c1 && c <= t.rect.c2) || (c == t.rect.c2 + 1 && r >= t.rect.r1 && r <= t.rect.r2))
        });
        if let Some(i) = grow {
            if self.sheets[s].has_content(r, c) {
                let t = self.sheets[s].tables[i].clone();
                let mut nt = t.clone();
                let others = self.sheets[s].tables.iter().enumerate().any(|(k, x)| k != i && x.rect.intersects(&Rect::new(r, c, r, c)));
                if !others {
                    if r == t.rect.r2 + 1 {
                        nt.rect.r2 = r;
                    } else {
                        nt.rect.c2 = c;
                        let id = t.cols.iter().map(|x| x.id).max().unwrap_or(0) + 1;
                        let names = if t.header { self.header_names(s, t.rect.r1, t.rect.c1, c) } else { vec![] };
                        let name = names.last().cloned().unwrap_or_else(|| format!("Column{}", t.cols.len() + 1));
                        nt.cols.push(TableCol { id, name: name.clone(), ..Default::default() });
                        if t.header && self.display(s, t.rect.r1, c) != name {
                            let st = self.sheets[s].style_at(t.rect.r1, c);
                            self.record(Delta::Cell(s, t.rect.r1, c, Some(crate::sheet::Cell { v: Value::string(name), s: st, ..Default::default() })));
                        }
                    }
                    let mut tables = self.sheets[s].tables.clone();
                    tables[i] = nt.clone();
                    self.record(Delta::Tables(s, tables));
                    // the new row gets the calculated columns
                    if r == t.rect.r2 + 1 {
                        for (k, col) in nt.cols.iter().enumerate() {
                            let cc = nt.rect.c1 + k as u32;
                            if let Some(f) = &col.calc {
                                if cc != c && !self.sheets[s].has_content(r, cc) {
                                    self.fill_calc_cell(s, &nt, r, cc, f);
                                }
                            }
                        }
                    }
                }
            }
        }
        let i = match self.table_at(s, r, c) {
            Some(i) => i,
            None => return,
        };
        let t = self.sheets[s].tables[i].clone();
        let k = (c - t.rect.c1) as usize;
        if t.header && r == t.rect.r1 {
            // a header cell renamed its column
            let names = self.header_names(s, t.rect.r1, t.rect.c1, t.rect.c2);
            let old = t.cols[k].name.clone();
            let new = names[k].clone();
            if self.display(s, r, c) != new || self.sheets[s].get(r, c).map(|x| x.f.is_some()).unwrap_or(false) {
                let st = self.sheets[s].style_at(r, c);
                self.record(Delta::Cell(s, r, c, Some(crate::sheet::Cell { v: Value::string(new.clone()), s: st, ..Default::default() })));
            }
            if !old.eq_ignore_ascii_case(&new) || old != new {
                let tname = t.name.clone();
                let rect = t.rect;
                let sheet = s;
                self.rewrite_all_structs(|sr, ss, (rr, cc)| {
                    let mine = match &sr.table {
                        Some(n) => n.eq_ignore_ascii_case(&tname),
                        None => ss == sheet && rect.contains(rr, cc),
                    };
                    if !mine {
                        return None;
                    }
                    let mut x = sr.clone();
                    let fix = |n: &String| if n.eq_ignore_ascii_case(&old) { new.clone() } else { n.clone() };
                    x.cols = sr.cols.as_ref().map(|(a, b)| (fix(a), fix(b)));
                    if x == *sr { None } else { Some(write_struct(&x, None, false)) }
                });
                let mut tables = self.sheets[s].tables.clone();
                tables[i].cols[k].name = new;
                self.record(Delta::Tables(s, tables));
            }
            return;
        }
        let d = t.data();
        if r < d.r1 || r > d.r2 || t.totals_row() == Some(r) {
            return;
        }
        // a formula typed into a column that is empty (or all the same formula) fills it
        let f = match self.sheets[s].get(r, c).and_then(|x| x.f.clone()) {
            Some(f) => f,
            None => {
                if t.cols[k].calc.is_some() {
                    let mut tables = self.sheets[s].tables.clone();
                    tables[i].cols[k].calc = None;
                    self.record(Delta::Tables(s, tables));
                }
                return;
            }
        };
        let others_free = (d.r1..=d.r2).filter(|rr| *rr != r).all(|rr| {
            match self.sheets[s].get(rr, c) {
                None => true,
                Some(x) => x.is_blank() || x.f.as_ref().map(|g| t.cols[k].calc.as_deref() == Some(rel_text(&g.text, rr as i64 - r as i64).as_str()) || g.text == f.text).unwrap_or(false),
            }
        });
        if !others_free || d.rows() > 100_000 {
            return;
        }
        for rr in d.r1..=d.r2 {
            if rr != r {
                let text = formula::adjust(&f.text, &formula::Adjust::Shift(rr as i64 - r as i64, 0));
                self.fill_calc_cell(s, &t, rr, c, &text);
            }
        }
        let mut tables = self.sheets[s].tables.clone();
        tables[i].cols[k].calc = Some(f.text.clone());
        self.record(Delta::Tables(s, tables));
    }

    fn fill_calc_cell(&mut self, s: usize, _t: &Table, r: u32, c: u32, text: &str) {
        let st = self.sheets[s].style_at(r, c);
        let cell = crate::sheet::Cell { f: Some(std::rc::Rc::new(crate::book::make_formula(text, crate::sheet::FKind::Dynamic))), s: st, ..Default::default() };
        self.record(Delta::Cell(s, r, c, Some(cell)));
    }

    /// Tables follow inserted and deleted rows and columns (recorded); Err when the
    /// change would cut a table's header.
    pub(crate) fn shift_tables(&mut self, s: usize, rows: bool, at: u32, n: i64, span: Option<(u32, u32)>) {
        if self.sheets[s].tables.is_empty() {
            return;
        }
        let mut out = Vec::new();
        for t in &self.sheets[s].tables {
            let mut t = t.clone();
            let (a, b) = if rows { (t.rect.r1, t.rect.r2) } else { (t.rect.c1, t.rect.c2) };
            let (x1, x2) = if rows { (t.rect.c1, t.rect.c2) } else { (t.rect.r1, t.rect.r2) };
            if let Some((p, q)) = span {
                // cells shifted in some lines only: tables in those lines
                if x2 < p || x1 > q {
                    out.push(t);
                    continue;
                }
            }
            let max = if rows { MAX_ROWS } else { MAX_COLS };
            match crate::formula::adjust_span(a, b, at, n, max) {
                Some((na, nb)) => {
                    if !rows {
                        // deleted columns go from the table; inserted ones come in
                        let old_c1 = t.rect.c1;
                        if n < 0 {
                            let k = (-n) as u32;
                            let keep: Vec<TableCol> = t.cols.iter().enumerate().filter(|(i, _)| {
                                let c = old_c1 + *i as u32;
                                !(c >= at && c < at + k)
                            }).map(|(_, x)| x.clone()).collect();
                            t.cols = keep;
                        } else if at > a && at <= b {
                            let id0 = t.cols.iter().map(|x| x.id).max().unwrap_or(0);
                            let pos = (at - a) as usize;
                            for j in 0..n as usize {
                                let name = format!("Column{}", t.cols.len() + 1);
                                t.cols.insert(pos + j, TableCol { id: id0 + 1 + j as u32, name, ..Default::default() });
                            }
                        }
                        t.rect.c1 = na;
                        t.rect.c2 = nb;
                    } else {
                        // the header row deleted: the table goes
                        if n < 0 && t.header && t.rect.r1 >= at && t.rect.r1 < at + (-n) as u32 && t.rect.r2 >= at + (-n) as u32 {
                            continue;
                        }
                        t.rect.r1 = na;
                        t.rect.r2 = nb;
                    }
                    if t.cols.is_empty() {
                        continue;
                    }
                    if let Some(f) = &mut t.filter {
                        f.rect = t.rect;
                    }
                    out.push(t);
                }
                None => {}
            }
        }
        if out != self.sheets[s].tables {
            self.record(Delta::Tables(s, out));
        }
    }
}

/// A formula's text moved down by dr rows (for comparing calculated columns).
fn rel_text(text: &str, dr: i64) -> String {
    formula::adjust(text, &formula::Adjust::Shift(-dr, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structs() {
        let r = parse_struct("Table1[Sales]").unwrap();
        assert_eq!(r.table.as_deref(), Some("Table1"));
        assert_eq!(r.cols, Some(("Sales".into(), "Sales".into())));
        let r = parse_struct("[@Price]").unwrap();
        assert!(r.this_row && r.table.is_none());
        let r = parse_struct("Table1[[#This Row],[Unit Price]]").unwrap();
        assert!(r.this_row && r.cols == Some(("Unit Price".into(), "Unit Price".into())));
        let r = parse_struct("T[[#Headers],[#Data],[A]:[C]]").unwrap();
        assert!(r.headers && r.data && r.cols == Some(("A".into(), "C".into())));
        assert_eq!(write_struct(&parse_struct("[@Price]").unwrap(), Some("T"), true), "T[[#This Row],[Price]]");
        assert_eq!(to_display("SUM(T[[#This Row],[A]],1)", Some("T")), "SUM([@A],1)");
        assert_eq!(write_struct(&parse_struct("T[#All]").unwrap(), None, false), "T[#All]");
        assert_eq!(write_struct(&parse_struct("[@[Unit Price]]").unwrap(), None, false), "[@[Unit Price]]");
    }
}

#[allow(dead_code)]
fn unused() -> String {
    a1(0, 0)
}

impl Book {
    /// A small picture of a style: a 5-column, 5-row table.
    pub unsafe fn paint_table_style(&self, cr: *mut crate::ffi::cairo_t, style: &str, w: f64, h: f64) {
        use crate::ffi::*;
        let t = Table {
            id: 0,
            name: String::new(),
            rect: Rect::new(0, 0, 4, 4),
            header: true,
            totals: false,
            cols: vec![],
            style: Some(style.to_string()),
            first_col: false,
            last_col: false,
            row_stripes: true,
            col_stripes: false,
            filter_button: false,
            filter: None,
            part: None,
            orig: None,
        };
        let cw = (w - 2.0) / 5.0;
        let ch = (h - 2.0) / 5.0;
        let set = |c: u32| cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
        set(0xFFFFFF);
        cairo_rectangle(cr, 0.0, 0.0, w, h);
        cairo_fill(cr);
        for r in 0..5u32 {
            for c in 0..5u32 {
                let tc = self.table_cell(&t, r, c);
                let (x, y) = (1.0 + c as f64 * cw, 1.0 + r as f64 * ch);
                if let Some(f) = tc.fill {
                    set(f);
                    cairo_rectangle(cr, x, y, cw, ch);
                    cairo_fill(cr);
                }
                // a dash for the text
                set(tc.color.unwrap_or(0x7F7F7F));
                cairo_rectangle(cr, x + cw * 0.25, y + ch * 0.4, cw * 0.5, (ch * 0.2).max(1.0));
                cairo_fill(cr);
                cairo_set_line_width(cr, 1.0);
                for (k, b) in tc.borders.iter().enumerate() {
                    if let Some((_, col)) = b {
                        set(*col);
                        let (x0, y0, x1, y1) = match k {
                            0 => (x, y, x, y + ch),
                            1 => (x + cw, y, x + cw, y + ch),
                            2 => (x, y, x + cw, y),
                            _ => (x, y + ch, x + cw, y + ch),
                        };
                        cairo_move_to(cr, x0.round() + 0.5, y0.round() + 0.5);
                        cairo_line_to(cr, x1.round() + 0.5, y1.round() + 0.5);
                        cairo_stroke(cr);
                    }
                }
            }
        }
    }
}
