//! Inserting and deleting rows, columns and cells: cells move, every formula's references
//! follow, and merges, array formulas, names and the sheet's other ranges (conditional
//! formats, validations, hyperlinks) are adjusted with them.

use crate::book::{rebase, Body, Book, Delta};
use crate::cellref::{parse_range, Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, adjust_span, Adjust};
use crate::sheet::{Cell, ColSpan, FKind, Piece, RowInfo};
use std::collections::BTreeMap;
use std::rc::Rc;

/// One structural change on one sheet.
#[derive(Clone, Copy, Debug)]
pub struct Shift {
    /// true: rows move (insert/delete along rows); false: columns move
    pub rows: bool,
    pub at: u32,
    /// lines inserted (> 0) or deleted (< 0)
    pub n: i64,
    /// limited to these columns (rows moving) or rows (columns moving); None = whole lines
    pub span: Option<(u32, u32)>,
}

impl Shift {
    fn max(&self) -> u32 {
        if self.rows { MAX_ROWS } else { MAX_COLS }
    }
    /// Is (r, c) in the part of the sheet that moves?
    fn affects(&self, r: u32, c: u32) -> bool {
        let (along, across) = if self.rows { (r, c) } else { (c, r) };
        along >= self.at && self.span.map(|(a, b)| across >= a && across <= b).unwrap_or(true)
    }
    /// Where a cell goes (None: deleted).
    fn map(&self, r: u32, c: u32) -> Option<(u32, u32)> {
        if !self.affects(r, c) {
            return Some((r, c));
        }
        let v = if self.rows { r } else { c };
        let nv = if self.n > 0 {
            let x = v as u64 + self.n as u64;
            if x >= self.max() as u64 {
                return None;
            }
            x as u32
        } else {
            let k = (-self.n) as u32;
            if v < self.at + k {
                return None;
            }
            v - k
        };
        Some(if self.rows { (nv, c) } else { (r, nv) })
    }
    /// A rectangle after the change (None: deleted entirely).
    fn rect(&self, rc: &Rect) -> Option<Rect> {
        let (a1, a2, b1, b2) = if self.rows { (rc.r1, rc.r2, rc.c1, rc.c2) } else { (rc.c1, rc.c2, rc.r1, rc.r2) };
        if let Some((s1, s2)) = self.span {
            if b1 < s1 || b2 > s2 {
                return Some(*rc);
            }
        }
        let (x, y) = adjust_span(a1, a2, self.at, self.n, self.max())?;
        Some(if self.rows { Rect::new(x, rc.c1, y, rc.c2) } else { Rect::new(rc.r1, x, rc.r2, y) })
    }
    fn how<'a>(&self, sheet: &'a str) -> Adjust<'a> {
        match (self.rows, self.span) {
            (true, None) => Adjust::Rows { sheet, here: sheet, at: self.at, n: self.n },
            (false, None) => Adjust::Cols { sheet, here: sheet, at: self.at, n: self.n },
            (true, Some((a, b))) => Adjust::RowsIn { sheet, here: sheet, at: self.at, n: self.n, c1: a, c2: b },
            (false, Some((a, b))) => Adjust::ColsIn { sheet, here: sheet, at: self.at, n: self.n, r1: a, r2: b },
        }
    }
    /// Would part of `rc` move while the rest stays? (rectangles that can't be split)
    fn splits(&self, rc: &Rect) -> bool {
        let (a1, a2, b1, b2) = if self.rows { (rc.r1, rc.r2, rc.c1, rc.c2) } else { (rc.c1, rc.c2, rc.r1, rc.r2) };
        let across = match self.span {
            Some((s1, s2)) => {
                if b2 < s1 || b1 > s2 {
                    return false;
                }
                b1 < s1 || b2 > s2
            }
            None => false,
        };
        if across && a2 >= self.at {
            return true;
        }
        if self.n < 0 {
            // a deletion through the middle of the rectangle's lines is fine for merges
            // and arrays only if it takes the whole of them
            let k = (-self.n) as u32;
            let (d1, d2) = (self.at, self.at + k - 1);
            let overlaps = a1 <= d2 && a2 >= d1;
            return overlaps && !(a1 >= d1 && a2 <= d2);
        }
        false
    }
}

const SHIFT_OFF: &str = "To prevent possible loss of data, Folio Sheets can't shift nonblank cells off of the worksheet. Select another location, or delete data from the end of your worksheet.";

impl Book {
    pub fn insert_rows(&mut self, s: usize, at: u32, n: u32) -> Result<(), String> {
        self.shift(s, Shift { rows: true, at, n: n as i64, span: None }, "Insert Rows")
    }
    pub fn delete_rows(&mut self, s: usize, at: u32, n: u32) -> Result<(), String> {
        self.shift(s, Shift { rows: true, at, n: -(n as i64), span: None }, "Delete Rows")
    }
    pub fn insert_cols(&mut self, s: usize, at: u32, n: u32) -> Result<(), String> {
        self.shift(s, Shift { rows: false, at, n: n as i64, span: None }, "Insert Columns")
    }
    pub fn delete_cols(&mut self, s: usize, at: u32, n: u32) -> Result<(), String> {
        self.shift(s, Shift { rows: false, at, n: -(n as i64), span: None }, "Delete Columns")
    }
    /// Insert Cells: shift the cells at rc down (or right) by its size.
    pub fn insert_cells(&mut self, s: usize, rc: Rect, down: bool) -> Result<(), String> {
        let sh = if down {
            Shift { rows: true, at: rc.r1, n: rc.rows() as i64, span: Some((rc.c1, rc.c2)) }
        } else {
            Shift { rows: false, at: rc.c1, n: rc.cols() as i64, span: Some((rc.r1, rc.r2)) }
        };
        self.shift(s, sh, "Insert Cells")
    }
    /// Delete Cells: remove rc and shift the cells below up (or to the right left).
    pub fn delete_cells(&mut self, s: usize, rc: Rect, up: bool) -> Result<(), String> {
        let sh = if up {
            Shift { rows: true, at: rc.r1, n: -(rc.rows() as i64), span: Some((rc.c1, rc.c2)) }
        } else {
            Shift { rows: false, at: rc.c1, n: -(rc.cols() as i64), span: Some((rc.r1, rc.r2)) }
        };
        self.shift(s, sh, "Delete Cells")
    }

    pub fn shift(&mut self, s: usize, mut sh: Shift, label: &str) -> Result<(), String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("This command works on worksheets only.".into());
        }
        if sh.n == 0 {
            return Ok(());
        }
        // a span covering every line across is the same as whole lines
        if let Some((a, b)) = sh.span {
            let max_across = if sh.rows { MAX_COLS } else { MAX_ROWS };
            if a == 0 && b >= max_across - 1 {
                sh.span = None;
            }
        }
        if sh.n < 0 {
            let k = (-sh.n) as u32;
            if sh.at as u64 + k as u64 > sh.max() as u64 {
                sh.n = -((sh.max() - sh.at) as i64);
            }
        }
        let sheet = &self.sheets[s];
        // ---- what can't be done
        for rc in sheet.arrays.values() {
            if sh.splits(rc) {
                return Err("You can't change part of an array.".into());
            }
        }
        for m in &sheet.merges {
            if sh.span.is_some() && sh.splits(m) {
                return Err("This operation will cause some merged cells to unmerge. Unmerge the cells first.".into());
            }
        }
        if sh.n > 0 {
            let limit = sh.max() - sh.n.min(sh.max() as i64) as u32;
            for ((r, c), cell) in sheet.cells.iter() {
                let v = if sh.rows { *r } else { *c };
                if v >= limit && !cell.is_blank() && sh.affects(*r, *c) {
                    return Err(SHIFT_OFF.into());
                }
            }
        }
        let name = sheet.name.clone();
        let how = sh.how(&name);
        self.begin(label, s);
        // ---- formulas on the other sheets
        self.rewrite_formulas(&how, Some(s));
        // ---- this sheet, moved and rewritten
        let sheet = &self.sheets[s];
        let mut cells: BTreeMap<(u32, u32), Cell> = BTreeMap::new();
        for ((r, c), cell) in sheet.cells.iter() {
            let (nr, nc) = match sh.map(*r, *c) {
                Some(p) => p,
                None => continue,
            };
            let mut cell = cell.clone();
            if let Some(f) = &cell.f {
                let mut nf = (**f).clone();
                let t = formula::adjust(&f.text, &rebase(&how, &name));
                if t != nf.text {
                    nf.text = t;
                    nf.expr = formula::parse(&nf.text).ok();
                }
                if let FKind::Array(rc) = nf.kind {
                    nf.kind = FKind::Array(sh.rect(&rc).unwrap_or(Rect::cell(nr, nc)));
                }
                cell.f = Some(Rc::new(nf));
            }
            cells.insert((nr, nc), cell);
        }
        // inserted lines take the formatting of the line before them
        if sh.n > 0 && sh.at > 0 {
            let n = sh.n as u32;
            let src_line = sh.at - 1;
            let mut styled: Vec<(u32, u32)> = Vec::new(); // (across, style)
            for ((r, c), cell) in sheet.cells.iter() {
                let (along, across) = if sh.rows { (*r, *c) } else { (*c, *r) };
                if along == src_line && cell.s != 0 && sh.span.map(|(a, b)| across >= a && across <= b).unwrap_or(true) {
                    styled.push((across, cell.s));
                }
            }
            if (styled.len() as u64) * (n as u64) <= 2_000_000 {
                for (across, st) in styled {
                    for k in 0..n {
                        let along = sh.at + k;
                        let pos = if sh.rows { (along, across) } else { (across, along) };
                        cells.entry(pos).or_insert(Cell { s: st, ..Default::default() });
                    }
                }
            }
        }
        // rows and columns settings (whole lines only)
        let mut rows = sheet.rows.clone();
        let mut cols = sheet.cols.clone();
        if sh.span.is_none() {
            if sh.rows {
                let mut nr: BTreeMap<u32, RowInfo> = BTreeMap::new();
                for (r, ri) in sheet.rows.iter() {
                    if let Some((x, _)) = sh.map(*r, 0) {
                        nr.insert(x, ri.clone());
                    }
                }
                if sh.n > 0 && sh.at > 0 {
                    if let Some(ri) = sheet.rows.get(&(sh.at - 1)) {
                        let mut copy = ri.clone();
                        copy.hidden = false;
                        copy.outline = 0;
                        copy.collapsed = false;
                        if !copy.is_default() && sh.n <= 100_000 {
                            for k in 0..sh.n as u32 {
                                nr.insert(sh.at + k, copy.clone());
                            }
                        }
                    }
                }
                rows = nr;
            } else {
                let mut nc: Vec<ColSpan> = Vec::new();
                for span in &sheet.cols {
                    if let Some((a, b)) = adjust_span(span.min, span.max, sh.at, sh.n, MAX_COLS) {
                        let mut x = span.clone();
                        x.min = a;
                        x.max = b;
                        nc.push(x);
                    }
                }
                // inserted columns copy the column to their left
                if sh.n > 0 && sh.at > 0 {
                    if let Some(left) = sheet.col_span(sh.at - 1) {
                        let mut copy = left.clone();
                        copy.min = sh.at;
                        copy.max = (sh.at as i64 + sh.n - 1).min(MAX_COLS as i64 - 1) as u32;
                        copy.hidden = false;
                        // a span that covered the insertion point was stretched over it
                        nc.retain(|x| !(x.min <= copy.min && x.max >= copy.max && x.same_settings(&copy)));
                        let mut out: Vec<ColSpan> = Vec::new();
                        for x in nc {
                            if x.max < copy.min || x.min > copy.max {
                                out.push(x);
                            } else {
                                if x.min < copy.min {
                                    let mut a = x.clone();
                                    a.max = copy.min - 1;
                                    out.push(a);
                                }
                                if x.max > copy.max {
                                    let mut b = x.clone();
                                    b.min = copy.max + 1;
                                    out.push(b);
                                }
                            }
                        }
                        out.push(copy);
                        nc = out;
                    }
                }
                nc.sort_by_key(|x| x.min);
                cols = nc;
            }
        }
        let merges: Vec<Rect> = sheet.merges.iter().filter_map(|m| sh.rect(m)).filter(|m| m.count() > 1).collect();
        let pieces: Vec<Piece> = sheet.pieces.iter().map(|p| match p {
            Piece::Raw(tag, x) if (tag == "rowBreaks" && sh.rows || tag == "colBreaks" && !sh.rows) && sh.span.is_none() => {
                Piece::Raw(tag.clone(), adjust_breaks(x, sh.at, sh.n))
            }
            Piece::Raw(tag, x) if matches!(tag.as_str(), "conditionalFormatting" | "dataValidations" | "hyperlinks" | "ignoredErrors" | "autoFilter" | "protectedRanges" | "rowBreaks" | "colBreaks") => {
                Piece::Raw(tag.clone(), adjust_piece(x, &sh, &how, &name))
            }
            other => other.clone(),
        }).filter_map(|p| match p {
            Piece::Raw(tag, x) if x.contains(GONE) => drop_gone(&x).map(|x| Piece::Raw(tag, x)),
            other => Some(other),
        }).collect();
        let autofilter = sheet.autofilter.as_ref().and_then(|af| {
            let rect = sh.rect(&af.rect)?;
            let mut af = af.clone();
            af.rect = rect;
            Some(af)
        });
        let body = Body { pieces, cells, rows, cols, merges, autofilter };
        self.record(Delta::Body(s, Box::new(body)));
        if sh.span.is_none() {
            self.shift_objects(s, sh.rows, sh.at, sh.n);
        }
        self.shift_tables(s, sh.rows, sh.at, sh.n, sh.span);
        let shc = sh;
        self.shift_notes(s, &|r, c| shc.map(r, c));
        // ---- defined names
        let mut names = self.names.clone();
        let mut changed = false;
        for n in names.iter_mut() {
            let here = n.scope.and_then(|i| self.sheets.get(i)).map(|x| x.name.clone()).unwrap_or_default();
            let t = formula::adjust(&n.text, &rebase(&how, &here));
            if t != n.text {
                n.text = t;
                changed = true;
            }
        }
        if changed {
            self.record(Delta::Names(names));
        }
        self.commit();
        Ok(())
    }
}

/// Marks a range attribute whose cells were all deleted.
const GONE: &str = "#GONE!";

/// The piece without the elements whose ranges were deleted; None when nothing is left of it.
fn drop_gone(xml: &str) -> Option<String> {
    fn gone(e: &crate::xml::Element) -> bool {
        e.attrs.iter().any(|(_, v)| v.contains(GONE))
    }
    fn prune(e: &mut crate::xml::Element) {
        e.children.retain(|n| !matches!(n, crate::xml::Node::Elem(k) if gone(k)));
        for n in e.children.iter_mut() {
            if let crate::xml::Node::Elem(k) = n {
                prune(k);
            }
        }
    }
    let (_, mut e) = crate::xml::parse(xml).ok()?;
    if gone(&e) {
        return None;
    }
    let before = e.elems().count();
    prune(&mut e);
    let n = e.elems().count();
    if n == 0 && before > 0 {
        return None;
    }
    if e.attr("count").is_some() {
        e.set_attr("count", &n.to_string());
    }
    Some(e.to_string())
}

/// Manual page breaks after lines are inserted or deleted.
fn adjust_breaks(xml: &str, at: u32, n: i64) -> String {
    let (_, mut e) = match crate::xml::parse(xml) {
        Ok(x) => x,
        Err(_) => return xml.to_string(),
    };
    let mut kept = Vec::new();
    for c in e.children.drain(..) {
        if let crate::xml::Node::Elem(mut b) = c {
            if let Some(id) = b.attr("id").and_then(|v| v.parse::<i64>().ok()) {
                let nid = if id >= at as i64 { id + n } else { id };
                if n < 0 && id >= at as i64 && id < at as i64 - n {
                    continue;
                }
                if nid <= 0 {
                    continue;
                }
                b.set_attr("id", &nid.to_string());
            }
            kept.push(crate::xml::Node::Elem(b));
        } else {
            kept.push(c);
        }
    }
    let count = kept.iter().filter(|n| matches!(n, crate::xml::Node::Elem(_))).count();
    e.children = kept;
    e.set_attr("count", &count.to_string());
    if e.attr("manualBreakCount").is_some() {
        e.set_attr("manualBreakCount", &count.to_string());
    }
    e.to_string()
}

/// Ranges (sqref/ref attributes) and formulas inside a kept worksheet element.
fn adjust_piece(xml: &str, sh: &Shift, how: &Adjust, name: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    loop {
        // the next range attribute or formula element, whichever comes first
        let a = find_attr(rest);
        let f = find_formula(rest);
        let attr_first = match (a, f) {
            (Some(x), Some(y)) => x.0 < y.0,
            (Some(_), None) => true,
            _ => false,
        };
        if a.is_none() && f.is_none() {
            out.push_str(rest);
            break;
        }
        if attr_first {
            let (s, e) = a.unwrap();
            out.push_str(&rest[..s]);
            let val = &rest[s..e];
            let mut kept: Vec<String> = Vec::new();
            for tok in val.split_whitespace() {
                match parse_range(tok) {
                    Some(rc) => {
                        if let Some(n) = sh.rect(&rc) {
                            kept.push(n.a1());
                        }
                    }
                    None => kept.push(tok.to_string()),
                }
            }
            if kept.is_empty() && val.split_whitespace().all(|t| parse_range(t).is_some()) && !val.trim().is_empty() {
                // every range it named was deleted: the element goes (see drop_gone)
                out.push_str(GONE);
            } else if kept.is_empty() {
                out.push_str(val);
            } else {
                out.push_str(&kept.join(" "));
            }
            rest = &rest[e..];
        } else {
            let (s, e, _) = f.unwrap();
            out.push_str(&rest[..s]);
            let text = crate::xml::unescape(&rest[s..e]);
            let t = formula::adjust(&text, &rebase(how, name));
            out.push_str(&crate::xml::escape_text(&t));
            rest = &rest[e..];
        }
    }
    out
}

/// The value span of the next sqref="..." or ref="..." attribute.
fn find_attr(s: &str) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for key in [" sqref=\"", " ref=\""] {
        if let Some(i) = s.find(key) {
            let vs = i + key.len();
            if let Some(len) = s[vs..].find('"') {
                if best.map(|(b, _)| vs < b).unwrap_or(true) {
                    best = Some((vs, vs + len));
                }
            }
        }
    }
    best
}

/// The text span of the next <formula>, <formula1>, <formula2> (any prefix).
fn find_formula(s: &str) -> Option<(usize, usize, ())> {
    let mut best: Option<(usize, usize, ())> = None;
    for tag in ["formula>", "formula1>", "formula2>", "f>"] {
        let mut from = 0;
        while let Some(i) = s[from..].find(tag) {
            let i = from + i;
            from = i + tag.len();
            // an opening tag: '<' or '<prefix:' right before the name
            let before = &s[..i];
            let lt = match before.rfind('<') {
                Some(x) => x,
                None => continue,
            };
            let name = &before[lt + 1..];
            if name.starts_with('/') || name.contains(|c: char| c.is_whitespace() || c == '>') || !(name.is_empty() || name.ends_with(':')) {
                continue;
            }
            let ts = i + tag.len();
            if let Some(len) = s[ts..].find('<') {
                if best.map(|(b, _, _)| ts < b).unwrap_or(true) {
                    best = Some((ts, ts + len, ()));
                }
            }
            break;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn piece_adjust() {
        let sh = Shift { rows: true, at: 2, n: 3, span: None };
        let how = Adjust::Rows { sheet: "S", here: "S", at: 2, n: 3 };
        let x = "<conditionalFormatting sqref=\"A1:A10 C5\"><cfRule type=\"expression\" priority=\"1\"><formula>A5&gt;0</formula></cfRule></conditionalFormatting>";
        let y = adjust_piece(x, &sh, &how, "S");
        assert_eq!(y, "<conditionalFormatting sqref=\"A1:A13 C8\"><cfRule type=\"expression\" priority=\"1\"><formula>A8&gt;0</formula></cfRule></conditionalFormatting>");
    }
}
