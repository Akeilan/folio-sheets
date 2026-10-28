//! Find and Replace, and Go To Special.

use crate::book::Book;
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::value::Value;

#[derive(Clone, Debug, Default)]
pub struct FindOpts {
    pub pattern: String,
    pub case: bool,
    pub whole: bool,
    /// look in formulas (what was typed) rather than values (what shows)
    pub formulas: bool,
    pub by_cols: bool,
}

/// Pattern pieces: literal chars, ? (one), * (any run); ~ escapes.
#[derive(Clone, Debug, PartialEq)]
enum P {
    Ch(char),
    One,
    Any,
}

fn compile(pat: &str, case: bool) -> Vec<P> {
    let mut out = Vec::new();
    let mut it = pat.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '~' => match it.next() {
                Some(n) => out.push(P::Ch(if case { n } else { lower(n) })),
                None => out.push(P::Ch('~')),
            },
            '?' => out.push(P::One),
            '*' => {
                if out.last() != Some(&P::Any) {
                    out.push(P::Any)
                }
            }
            c => out.push(P::Ch(if case { c } else { lower(c) })),
        }
    }
    out
}

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Match of p at t[i..]; returns the end index.
fn match_at(p: &[P], t: &[char], i: usize, anchored_end: bool) -> Option<usize> {
    if p.is_empty() {
        return if !anchored_end || i == t.len() { Some(i) } else { None };
    }
    match &p[0] {
        P::Ch(c) => {
            if i < t.len() && t[i] == *c {
                match_at(&p[1..], t, i + 1, anchored_end)
            } else {
                None
            }
        }
        P::One => {
            if i < t.len() {
                match_at(&p[1..], t, i + 1, anchored_end)
            } else {
                None
            }
        }
        P::Any => {
            // a trailing * takes the rest of the text, as Replace does
            if p.len() == 1 {
                return Some(t.len());
            }
            // greedy: the longest run that still lets the rest match
            let mut k = t.len();
            loop {
                if let Some(e) = match_at(&p[1..], t, k, anchored_end) {
                    return Some(e);
                }
                if k == i {
                    return None;
                }
                k -= 1;
            }
        }
    }
}

/// The matches of the pattern in text: (start, end) char indices, left to right.
fn find_spans(p: &[P], text: &str, case: bool, whole: bool) -> Vec<(usize, usize)> {
    let t: Vec<char> = if case { text.chars().collect() } else { text.chars().map(lower).collect() };
    if whole {
        return match match_at(p, &t, 0, true) {
            Some(e) if e == t.len() => vec![(0, e)],
            _ => vec![],
        };
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i <= t.len() {
        if let Some(e) = match_at(p, &t, i, false) {
            if e > i || p.is_empty() {
                out.push((i, e));
                i = if e > i { e } else { i + 1 };
                continue;
            }
        }
        i += 1;
    }
    out
}

impl Book {
    /// The text Find looks at in a cell.
    fn find_text(&self, s: usize, r: u32, c: u32, formulas: bool) -> String {
        if formulas {
            let t = self.input(s, r, c);
            t.strip_prefix('\'').map(|x| x.to_string()).unwrap_or(t)
        } else {
            self.display(s, r, c)
        }
    }

    /// Every matching cell of the sheets, in search order.
    pub fn find_all(&self, sheets: &[usize], o: &FindOpts) -> Vec<(usize, u32, u32)> {
        let p = compile(&o.pattern, o.case);
        let mut out = Vec::new();
        for &s in sheets {
            let sh = match self.sheets.get(s) {
                Some(x) => x,
                None => continue,
            };
            let mut pos: Vec<(u32, u32)> = sh.cells.iter().filter(|(_, c)| !c.is_blank()).map(|(k, _)| *k).collect();
            if !o.formulas {
                pos.extend(sh.ghosts.keys().copied());
            }
            if o.by_cols {
                pos.sort_by_key(|(r, c)| (*c, *r));
            } else {
                pos.sort();
            }
            pos.dedup();
            for (r, c) in pos {
                let t = self.find_text(s, r, c, o.formulas);
                if t.is_empty() {
                    continue;
                }
                if !find_spans(&p, &t, o.case, o.whole).is_empty() {
                    out.push((s, r, c));
                }
            }
        }
        out
    }

    /// The text of a cell with the matches replaced, or None when nothing matches.
    pub fn replaced_text(&self, s: usize, r: u32, c: u32, o: &FindOpts, with: &str) -> Option<String> {
        let p = compile(&o.pattern, o.case);
        let t = self.input(s, r, c);
        let spans = find_spans(&p, &t, o.case, o.whole);
        if spans.is_empty() {
            return None;
        }
        let chars: Vec<char> = t.chars().collect();
        let mut out = String::new();
        let mut last = 0;
        for (a, b) in spans {
            out.extend(&chars[last..a]);
            out.push_str(with);
            last = b;
        }
        out.extend(&chars[last..]);
        Some(out)
    }

    /// Replace in every matching cell (one undo step). Returns how many cells changed.
    pub fn replace_all(&mut self, sheets: &[usize], o: &FindOpts, with: &str) -> Result<usize, String> {
        let mut o = o.clone();
        o.formulas = true;
        let hits = self.find_all(sheets, &o);
        if hits.is_empty() {
            return Ok(0);
        }
        self.begin_group("Replace", hits[0].0);
        let mut n = 0;
        for (s, r, c) in hits {
            if let Some(t) = self.replaced_text(s, r, c, &o, with) {
                if self.array_range(s, r, c).is_some() {
                    continue;
                }
                match self.set_input(s, r, c, &t) {
                    Ok(()) => n += 1,
                    Err(_) => {}
                }
            }
        }
        self.end_group();
        Ok(n)
    }

    /// Go To Special: the cells of `area` that are `kind` ("blanks", "constants", "formulas",
    /// "lastcell", "visible", "cf", "validation"), with `mask` for constants/formulas
    /// (1 numbers, 2 text, 4 logicals, 16 errors). Returned as ranges (runs along rows).
    pub fn special_cells(&self, s: usize, area: Rect, kind: &str, mask: u32) -> Vec<Rect> {
        let sh = &self.sheets[s];
        if kind == "notes" {
            // notes and threaded comments (Select Comments, Go To Special > Notes)
            let mut cells: Vec<(u32, u32)> = sh.notes.keys().chain(sh.threads.keys()).copied()
                .filter(|(r, c)| area.contains(*r, *c)).collect();
            cells.sort();
            cells.dedup();
            return runs(cells);
        }
        let (er, ec) = sh.used_extent().unwrap_or((0, 0));
        let area = Rect::new(area.r1, area.c1, area.r2.min(er.max(area.r1)), area.c2.min(ec.max(area.c1)));
        let mut cells: Vec<(u32, u32)> = Vec::new();
        let want = |v: &Value| -> bool {
            match v {
                Value::Num(_) => mask & 1 != 0,
                Value::Str(_) => mask & 2 != 0,
                Value::Bool(_) => mask & 4 != 0,
                Value::Err(_) => mask & 16 != 0,
                _ => false,
            }
        };
        match kind {
            "lastcell" => return vec![Rect::cell(er, ec)],
            "blanks" => {
                if area.count() > 5_000_000 {
                    return vec![];
                }
                for r in area.r1..=area.r2 {
                    for c in area.c1..=area.c2 {
                        if !sh.has_content(r, c) {
                            cells.push((r, c));
                        }
                    }
                }
            }
            "constants" | "formulas" => {
                let f = kind == "formulas";
                for ((r, c), cell) in sh.cells_in(area) {
                    if cell.is_blank() || cell.f.is_some() != f {
                        continue;
                    }
                    if want(&cell.v) {
                        cells.push((*r, *c));
                    }
                }
            }
            "visible" => {
                for r in area.r1..=area.r2.min(area.r1 + 100_000) {
                    if sh.row_hidden(r) {
                        continue;
                    }
                    let mut c = area.c1;
                    while c <= area.c2 {
                        if sh.col_hidden(c) {
                            c += 1;
                            continue;
                        }
                        cells.push((r, c));
                        c += 1;
                        if c > area.c1 + 16384 {
                            break;
                        }
                    }
                }
            }
            "cf" | "validation" => {
                let tag = if kind == "cf" { "conditionalFormatting" } else { "dataValidations" };
                let mut out = Vec::new();
                for p in &sh.pieces {
                    if let crate::sheet::Piece::Raw(t, x) = p {
                        if t == tag {
                            let mut rest = x.as_str();
                            while let Some(i) = rest.find("sqref=\"") {
                                let v = &rest[i + 7..];
                                let e = v.find('"').unwrap_or(v.len());
                                for tok in v[..e].split_whitespace() {
                                    if let Some(rc) = crate::cellref::parse_range(tok) {
                                        if let Some(x) = rc.intersect(&Rect::new(0, 0, MAX_ROWS - 1, MAX_COLS - 1)) {
                                            out.push(x);
                                        }
                                    }
                                }
                                rest = &v[e..];
                            }
                        }
                    }
                }
                return out;
            }
            _ => {}
        }
        runs(cells)
    }
}

/// Cells (row-major) gathered into horizontal runs.
fn runs(mut cells: Vec<(u32, u32)>) -> Vec<Rect> {
    cells.sort();
    let mut out: Vec<Rect> = Vec::new();
    for (r, c) in cells {
        if let Some(l) = out.last_mut() {
            if l.r1 == r && l.c2 + 1 == c {
                l.c2 = c;
                continue;
            }
        }
        out.push(Rect::cell(r, c));
    }
    // stack equal runs of consecutive rows
    let mut merged: Vec<Rect> = Vec::new();
    for rc in out {
        if let Some(m) = merged.iter_mut().rev().find(|m| m.c1 == rc.c1 && m.c2 == rc.c2 && m.r2 + 1 == rc.r1) {
            m.r2 = rc.r2;
            continue;
        }
        merged.push(rc);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wildcards() {
        let p = compile("a*c", false);
        assert_eq!(find_spans(&p, "xxABCabc", false, false), vec![(2, 8)]);
        let p = compile("b?", false);
        assert_eq!(find_spans(&p, "abcbd", false, false), vec![(1, 3), (3, 5)]);
        let p = compile("~*", false);
        assert_eq!(find_spans(&p, "a*b", false, false), vec![(1, 2)]);
        let p = compile("abc", false);
        assert!(find_spans(&p, "abcd", false, true).is_empty());
        assert_eq!(find_spans(&p, "ABC", false, true), vec![(0, 3)]);
        assert!(find_spans(&compile("abc", true), "ABC", true, false).is_empty());
    }
}
