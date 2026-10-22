//! Data commands: Sort, AutoFilter, Remove Duplicates.

use crate::book::{Book, Delta};
use crate::cellref::{Rect, MAX_COLS};
use crate::formula::{self, Adjust};
use crate::sheet::{Cell, RowInfo};
use crate::value::Value;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum FilterKind {
    /// show rows whose shown text is one of these (and blanks when `blank`)
    Values { values: Vec<String>, blank: bool },
    /// one or two conditions: (operator, value); and = both must hold
    Custom { and: bool, conds: Vec<(String, String)> },
    Top10 { top: bool, percent: bool, val: f64 },
    /// a filter this program doesn't edit (colour, icon, dynamic, date groups): kept as XML
    Raw(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterCol {
    /// column offset inside the range
    pub col: u32,
    pub kind: FilterKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoFilter {
    pub rect: Rect,
    pub cols: Vec<FilterCol>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FOwner {
    Sheet,
    Table(usize),
}

#[derive(Clone, Debug)]
pub struct SortKey {
    /// absolute column (or row, sorting left to right)
    pub line: u32,
    pub ascending: bool,
}

/// Sort order of values: numbers, text, logicals, errors; blanks last either way.
fn sort_cmp(a: &Value, b: &Value, asc: bool) -> Ordering {
    let rank = |v: &Value| match v {
        Value::Num(_) => 0,
        Value::Str(s) if s.is_empty() => 4,
        Value::Str(_) => 1,
        Value::Bool(_) => 2,
        Value::Err(_) => 3,
        Value::Empty | Value::Arr(_) => 4,
    };
    let (ra, rb) = (rank(a), rank(b));
    if ra == 4 || rb == 4 {
        return ra.cmp(&rb);
    }
    let o = if ra != rb {
        ra.cmp(&rb)
    } else {
        match (a, b) {
            (Value::Num(x), Value::Num(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
            (Value::Str(x), Value::Str(y)) => crate::calc::cmp_text(x, y),
            (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
            _ => Ordering::Equal,
        }
    };
    if asc { o } else { o.reverse() }
}

impl Book {
    fn check_block(&self, s: usize, rc: &Rect) -> Result<(), String> {
        let sh = &self.sheets[s];
        for a in sh.arrays.values() {
            if a.intersects(rc) {
                return Err("You can't change part of an array.".into());
            }
        }
        for m in &sh.merges {
            if m.intersects(rc) {
                return Err("To do this, all the merged cells need to be the same size.".into());
            }
        }
        Ok(())
    }

    /// Does the first row of rc look like a header (text over a column that has numbers,
    /// or formatted differently)? The guess the Sort buttons make.
    pub fn guess_header(&self, s: usize, rc: Rect) -> bool {
        if rc.rows() < 2 {
            return false;
        }
        let sh = &self.sheets[s];
        let mut text_top = 0;
        let mut differs = 0;
        for c in rc.c1..=rc.c2.min(rc.c1 + 200) {
            let top = sh.value(rc.r1, c);
            let below = sh.value(rc.r1 + 1, c);
            if matches!(top, Value::Str(_)) && !matches!(below, Value::Str(_) | Value::Empty) {
                text_top += 1;
            }
            let ft = self.styles.xf(sh.style_at(rc.r1, c));
            let fb = self.styles.xf(sh.style_at(rc.r1 + 1, c));
            if self.styles.font(ft.font).bold && !self.styles.font(fb.font).bold {
                differs += 1;
            }
        }
        text_top > 0 || differs > 0
    }

    /// Sort the rows (or columns, `by_cols`) of rc by keys. Contents, formulas and formats
    /// move together; relative references move with their cells.
    pub fn sort_range(&mut self, s: usize, rc: Rect, keys: &[SortKey], header: bool, by_cols: bool) -> Result<(), String> {
        let rc = self.sheets[s].clamp_rect(rc);
        let rc = match self.sheets[s].data_extent() {
            Some((er, ec)) => Rect::new(rc.r1, rc.c1, rc.r2.min(er.max(rc.r1)), rc.c2.min(ec.max(rc.c1))),
            None => return Ok(()),
        };
        self.check_block(s, &rc)?;
        let body = if by_cols {
            Rect::new(rc.r1, rc.c1 + if header { 1 } else { 0 }, rc.r2, rc.c2)
        } else {
            Rect::new(rc.r1 + if header { 1 } else { 0 }, rc.c1, rc.r2, rc.c2)
        };
        if body.r1 > body.r2 || body.c1 > body.c2 {
            return Ok(());
        }
        let n = if by_cols { body.cols() } else { body.rows() } as usize;
        let sh = &self.sheets[s];
        let line_key = |i: usize, k: &SortKey| -> Value {
            if by_cols { sh.value(k.line, body.c1 + i as u32) } else { sh.value(body.r1 + i as u32, k.line) }
        };
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| {
            for k in keys {
                let o = sort_cmp(&line_key(*a, k), &line_key(*b, k), k.ascending);
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        if order.iter().enumerate().all(|(i, j)| i == *j) {
            return Ok(());
        }
        // the cells of each line, with their formats
        let width = if by_cols { body.rows() } else { body.cols() };
        let get = |line: usize, k: u32| -> (u32, u32) {
            if by_cols { (body.r1 + k, body.c1 + line as u32) } else { (body.r1 + line as u32, body.c1 + k) }
        };
        let mut lines: Vec<Vec<Option<Cell>>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut v = Vec::with_capacity(width as usize);
            for k in 0..width {
                let (r, c) = get(i, k);
                v.push(sh.get(r, c).cloned());
            }
            lines.push(v);
        }
        self.begin("Sort", s);
        for (dst, src) in order.iter().enumerate() {
            if dst == *src {
                continue;
            }
            for k in 0..width {
                let (r, c) = get(dst, k);
                let mut cell = lines[*src][k as usize].clone();
                if let Some(cl) = cell.as_mut() {
                    if let Some(f) = &cl.f {
                        let d = dst as i64 - *src as i64;
                        let how = if by_cols { Adjust::Shift(0, d) } else { Adjust::Shift(d, 0) };
                        let mut nf = (**f).clone();
                        nf.text = formula::adjust(&f.text, &how);
                        nf.expr = formula::parse(&nf.text).ok();
                        cl.f = Some(Rc::new(nf));
                    }
                }
                let cur = self.sheets[s].get(r, c).cloned();
                let same = match (&cur, &cell) {
                    (None, None) => true,
                    _ => false,
                };
                if !same {
                    self.record(Delta::Cell(s, r, c, cell));
                }
            }
        }
        self.commit();
        Ok(())
    }

    /// Remove Duplicates: rows of rc whose values in `cols` repeat an earlier row are
    /// dropped and the rest close up inside rc. Returns (removed, remaining).
    pub fn remove_duplicates(&mut self, s: usize, rc: Rect, cols: &[u32], header: bool) -> Result<(usize, usize), String> {
        let rc = self.sheets[s].clamp_rect(rc);
        let rc = match self.sheets[s].data_extent() {
            Some((er, _)) => Rect::new(rc.r1, rc.c1, rc.r2.min(er.max(rc.r1)), rc.c2),
            None => return Ok((0, 0)),
        };
        self.check_block(s, &rc)?;
        let r1 = rc.r1 + if header { 1 } else { 0 };
        if r1 > rc.r2 {
            return Ok((0, 0));
        }
        let sh = &self.sheets[s];
        let mut seen: std::collections::HashSet<Vec<String>> = std::collections::HashSet::new();
        let mut keep: Vec<u32> = Vec::new();
        for r in r1..=rc.r2 {
            let key: Vec<String> = cols.iter().map(|c| match sh.value(r, *c) {
                Value::Str(t) => format!("s{}", t.to_lowercase()),
                v => format!("v{}", v),
            }).collect();
            if seen.insert(key) {
                keep.push(r);
            }
        }
        let total = (rc.r2 - r1 + 1) as usize;
        let removed = total - keep.len();
        if removed == 0 {
            return Ok((0, keep.len()));
        }
        let mut lines: Vec<Vec<Option<Cell>>> = Vec::new();
        for r in &keep {
            lines.push((rc.c1..=rc.c2).map(|c| sh.get(*r, c).cloned()).collect());
        }
        self.begin("Remove Duplicates", s);
        for i in 0..total as u32 {
            let r = r1 + i;
            for (k, c) in (rc.c1..=rc.c2).enumerate() {
                let cell = if (i as usize) < lines.len() {
                    let mut cell = lines[i as usize][k].clone();
                    if let Some(cl) = cell.as_mut() {
                        if let Some(f) = &cl.f {
                            let d = r as i64 - keep[i as usize] as i64;
                            let mut nf = (**f).clone();
                            nf.text = formula::adjust(&f.text, &Adjust::Shift(d, 0));
                            nf.expr = formula::parse(&nf.text).ok();
                            cl.f = Some(Rc::new(nf));
                        }
                    }
                    cell
                } else {
                    None
                };
                if (i as usize) < lines.len() && keep[i as usize] == r {
                    continue;
                }
                self.record(Delta::Cell(s, r, c, cell));
            }
        }
        self.commit();
        Ok((removed, keep.len()))
    }

    // ------------------------------------------------------------------ AutoFilter
    /// Turn AutoFilter on for rc (None: off, showing every row again).
    pub fn set_autofilter(&mut self, s: usize, rc: Option<Rect>) -> Result<(), String> {
        self.begin(if rc.is_some() { "Filter" } else { "Remove Filter" }, s);
        if let Some(old) = self.sheets[s].autofilter.clone() {
            self.show_rows(s, &old.rect);
        }
        let af = rc.map(|r| AutoFilter { rect: r, cols: vec![] });
        self.record(Delta::Filter(s, af.clone()));
        // the hidden name the file format keeps for the range
        let mut names = self.names.clone();
        names.retain(|n| !(n.scope == Some(s) && n.name.eq_ignore_ascii_case("_xlnm._FilterDatabase")));
        if let Some(af) = &af {
            let mut n = crate::book::Name::new("_xlnm._FilterDatabase", Some(s), &format!("{}!{}", crate::cellref::quote_sheet(&self.sheets[s].name), abs(&af.rect)));
            n.hidden = true;
            names.push(n);
        }
        if names.len() != self.names.len() || af.is_some() {
            self.record(Delta::Names(names));
        }
        self.commit();
        Ok(())
    }

    fn show_rows(&mut self, s: usize, rc: &Rect) {
        let rows: Vec<(u32, RowInfo)> = self.sheets[s].rows.range(rc.r1 + 1..=rc.r2).filter(|(_, ri)| ri.hidden).map(|(r, ri)| (*r, ri.clone())).collect();
        for (r, mut ri) in rows {
            ri.hidden = false;
            self.record(Delta::Row(s, r, Some(ri)));
        }
    }

    /// Whose filter a header cell belongs to: the sheet's AutoFilter or a table's.
    pub fn filter_owner(&self, s: usize, at: Option<(u32, u32)>) -> Option<FOwner> {
        let sh = &self.sheets[s];
        if let Some((r, c)) = at {
            if let Some(i) = sh.tables.iter().position(|t| t.filter_button && t.header && t.rect.r1 == r && c >= t.rect.c1 && c <= t.rect.c2) {
                return Some(FOwner::Table(i));
            }
            if let Some(i) = sh.tables.iter().position(|t| t.filter_button && t.rect.contains(r, c)) {
                if sh.autofilter.as_ref().map(|a| !a.rect.contains(r, c)).unwrap_or(true) {
                    return Some(FOwner::Table(i));
                }
            }
        }
        sh.autofilter.as_ref().map(|_| FOwner::Sheet)
    }

    /// The filter of an owner (a table's covers its header and data rows).
    pub fn owner_filter(&self, s: usize, o: FOwner) -> Option<AutoFilter> {
        match o {
            FOwner::Sheet => self.sheets[s].autofilter.clone(),
            FOwner::Table(i) => {
                let t = self.sheets[s].tables.get(i)?;
                let mut af = t.filter.clone().unwrap_or(AutoFilter { rect: t.filter_rect(), cols: vec![] });
                af.rect = t.filter_rect();
                Some(af)
            }
        }
    }

    fn record_owner_filter(&mut self, s: usize, o: FOwner, af: AutoFilter) {
        match o {
            FOwner::Sheet => self.record(Delta::Filter(s, Some(af))),
            FOwner::Table(i) => {
                let mut tables = self.sheets[s].tables.clone();
                tables[i].filter = if af.cols.is_empty() { None } else { Some(af) };
                self.record(Delta::Tables(s, tables));
            }
        }
    }

    /// Set (or clear, None) the filter of one column of the AutoFilter, and apply.
    pub fn filter_column(&mut self, s: usize, col: u32, kind: Option<FilterKind>) -> Result<(), String> {
        self.filter_column_at(s, FOwner::Sheet, col, kind)
    }

    pub fn filter_column_at(&mut self, s: usize, o: FOwner, col: u32, kind: Option<FilterKind>) -> Result<(), String> {
        let mut af = self.owner_filter(s, o).ok_or("There's no filter on this sheet.")?;
        af.cols.retain(|f| f.col != col);
        if let Some(k) = kind {
            af.cols.push(FilterCol { col, kind: k });
            af.cols.sort_by_key(|f| f.col);
        }
        self.begin("Filter", s);
        self.record_owner_filter(s, o, af);
        self.apply_filter(s);
        self.commit();
        Ok(())
    }

    pub fn clear_filters(&mut self, s: usize) -> Result<(), String> {
        self.clear_filters_at(s, FOwner::Sheet)
    }

    pub fn clear_filters_at(&mut self, s: usize, o: FOwner) -> Result<(), String> {
        let mut af = self.owner_filter(s, o).ok_or("There's no filter on this sheet.")?;
        af.cols.clear();
        self.begin("Clear Filter", s);
        self.record_owner_filter(s, o, af);
        self.apply_filter(s);
        self.commit();
        Ok(())
    }

    pub fn reapply_filter(&mut self, s: usize) -> Result<(), String> {
        if self.sheets[s].autofilter.is_none() && !self.sheets[s].tables.iter().any(|t| t.filter.is_some()) {
            return Err("There's no filter on this sheet.".into());
        }
        self.begin("Reapply", s);
        self.apply_filter(s);
        self.commit();
        Ok(())
    }

    /// Hide the rows the filters exclude, show the others (recorded).
    fn apply_filter(&mut self, s: usize) {
        let mut filters: Vec<(AutoFilter, u32)> = Vec::new();
        if let Some(af) = self.sheets[s].autofilter.clone() {
            let last = self.sheets[s].data_extent().map(|(r, _)| r).unwrap_or(af.rect.r2).max(af.rect.r2);
            filters.push((af, last));
        }
        for i in 0..self.sheets[s].tables.len() {
            if self.sheets[s].tables[i].filter_button {
                if let Some(af) = self.owner_filter(s, FOwner::Table(i)) {
                    let last = af.rect.r2;
                    filters.push((af, last));
                }
            }
        }
        let mut rows: std::collections::BTreeMap<u32, bool> = std::collections::BTreeMap::new();
        for (af, last) in &filters {
            for (r, show) in self.filter_rows(s, af, *last) {
                let e = rows.entry(r).or_insert(true);
                *e = *e && show;
            }
        }
        for (r, show) in rows {
            let cur = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
            if cur.hidden == show {
                let mut ri = cur;
                ri.hidden = !show;
                self.record(Delta::Row(s, r, Some(ri)));
            }
        }
    }

    /// Which rows under a filter's header show.
    fn filter_rows(&self, s: usize, af: &AutoFilter, last: u32) -> Vec<(u32, bool)> {
        // values of the Top 10 filters, worked out once
        let mut tops: Vec<(u32, f64)> = Vec::new();
        for fc in &af.cols {
            if let FilterKind::Top10 { top, percent, val } = &fc.kind {
                let mut nums: Vec<f64> = (af.rect.r1 + 1..=last).filter_map(|r| match self.sheets[s].value(r, af.rect.c1 + fc.col) {
                    Value::Num(n) => Some(n),
                    _ => None,
                }).collect();
                nums.sort_by(|a, b| if *top { b.partial_cmp(a).unwrap() } else { a.partial_cmp(b).unwrap() });
                let k = if *percent { ((nums.len() as f64) * val / 100.0).ceil() as usize } else { *val as usize };
                let cut = nums.get(k.clamp(1, nums.len().max(1)) - 1).copied().unwrap_or(0.0);
                tops.push((fc.col, cut));
            }
        }
        let mut out = Vec::new();
        for r in af.rect.r1 + 1..=last {
            let mut show = true;
            for fc in &af.cols {
                let c = af.rect.c1 + fc.col;
                let v = self.sheets[s].value(r, c);
                let xf = self.sheets[s].style_at(r, c);
                let text = self.format_value(&v, xf, 255).text;
                let ok = match &fc.kind {
                    FilterKind::Values { values, blank } => {
                        if matches!(v, Value::Empty) || text.is_empty() {
                            *blank
                        } else {
                            values.iter().any(|x| x.eq_ignore_ascii_case(&text))
                        }
                    }
                    FilterKind::Custom { and, conds } => {
                        let res: Vec<bool> = conds.iter().map(|(op, val)| custom_match(op, val, &v, &text, self.date1904)).collect();
                        if *and { res.iter().all(|x| *x) } else { res.iter().any(|x| *x) }
                    }
                    FilterKind::Top10 { top, .. } => {
                        let cut = tops.iter().find(|(c2, _)| *c2 == fc.col).map(|x| x.1).unwrap_or(0.0);
                        match v {
                            Value::Num(n) => if *top { n >= cut } else { n <= cut },
                            _ => false,
                        }
                    }
                    FilterKind::Raw(_) => true,
                };
                if !ok {
                    show = false;
                    break;
                }
            }
            out.push((r, show));
        }
        out
    }

    /// The distinct shown values of a filter column (for the filter menu), sorted, and
    /// whether there are blanks.
    pub fn filter_values(&self, s: usize, col: u32) -> (Vec<String>, bool) {
        self.filter_values_at(s, FOwner::Sheet, col)
    }

    pub fn filter_values_at(&self, s: usize, o: FOwner, col: u32) -> (Vec<String>, bool) {
        let af = match self.owner_filter(s, o) {
            Some(a) => a,
            None => return (vec![], false),
        };
        let sh = &self.sheets[s];
        let last = if o == FOwner::Sheet { sh.data_extent().map(|(r, _)| r).unwrap_or(af.rect.r2).max(af.rect.r2) } else { af.rect.r2 };
        let c = af.rect.c1 + col;
        let mut vals: Vec<(Value, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut blank = false;
        for r in af.rect.r1 + 1..=last {
            let v = sh.value(r, c);
            let t = self.format_value(&v, sh.style_at(r, c), 255).text;
            if t.is_empty() {
                blank = true;
                continue;
            }
            if seen.insert(t.to_lowercase()) {
                vals.push((v, t));
            }
            if vals.len() > 10000 {
                break;
            }
        }
        vals.sort_by(|a, b| sort_cmp(&a.0, &b.0, true));
        (vals.into_iter().map(|x| x.1).collect(), blank)
    }
}

fn abs(rc: &Rect) -> String {
    format!("${}${}:${}${}", crate::cellref::col_name(rc.c1), rc.r1 + 1, crate::cellref::col_name(rc.c2), rc.r2 + 1)
}

/// A Custom AutoFilter condition against a cell.
pub fn custom_match(op: &str, val: &str, v: &Value, text: &str, d1904: bool) -> bool {
    let num = crate::calc::text_to_number(val, d1904);
    let cell_num = match v {
        Value::Num(n) => Some(*n),
        _ => None,
    };
    let wild = val.contains('*') || val.contains('?');
    let eq = || -> bool {
        if let (Some(a), Some(b)) = (cell_num, num) {
            return crate::calc::num_eq(a, b);
        }
        if wild {
            return crate::fns::wildcard_match(&val.to_lowercase(), &text.to_lowercase());
        }
        text.eq_ignore_ascii_case(val)
    };
    let cmp = || -> Option<Ordering> {
        match (cell_num, num) {
            (Some(a), Some(b)) => a.partial_cmp(&b),
            (None, None) if !text.is_empty() => Some(crate::calc::cmp_text(text, val)),
            _ => None,
        }
    };
    match op {
        "equal" => eq(),
        "notEqual" => !eq(),
        "greaterThan" => cmp() == Some(Ordering::Greater),
        "greaterThanOrEqual" => matches!(cmp(), Some(Ordering::Greater | Ordering::Equal)),
        "lessThan" => cmp() == Some(Ordering::Less),
        "lessThanOrEqual" => matches!(cmp(), Some(Ordering::Less | Ordering::Equal)),
        _ => true,
    }
}

// --------------------------------------------------------------------------- the file

/// Read <autoFilter> (its XML as written).
pub fn read_autofilter(xml: &str) -> Option<AutoFilter> {
    let (_, e) = crate::xml::parse(xml).ok()?;
    let rect = crate::cellref::parse_range(e.attr("ref")?)?;
    let mut cols = Vec::new();
    for fc in e.elems_named("filterColumn") {
        let col: u32 = fc.attr("colId").and_then(|v| v.parse().ok())?;
        let kind = if let Some(f) = fc.child("filters") {
            if f.elems().any(|x| x.local() != "filter") {
                FilterKind::Raw(fc.to_string())
            } else {
                FilterKind::Values {
                    values: f.elems_named("filter").filter_map(|x| x.attr("val").map(|s| s.to_string())).collect(),
                    blank: matches!(f.attr("blank"), Some("1") | Some("true")),
                }
            }
        } else if let Some(cf) = fc.child("customFilters") {
            FilterKind::Custom {
                and: matches!(cf.attr("and"), Some("1") | Some("true")),
                conds: cf.elems_named("customFilter").map(|x| (x.attr("operator").unwrap_or("equal").to_string(), x.attr("val").unwrap_or("").to_string())).collect(),
            }
        } else if let Some(t) = fc.child("top10") {
            FilterKind::Top10 {
                top: !matches!(t.attr("top"), Some("0") | Some("false")),
                percent: matches!(t.attr("percent"), Some("1") | Some("true")),
                val: t.attr("val").and_then(|v| v.parse().ok()).unwrap_or(10.0),
            }
        } else {
            FilterKind::Raw(fc.to_string())
        };
        cols.push(FilterCol { col, kind });
    }
    Some(AutoFilter { rect, cols })
}

pub fn write_autofilter(af: &AutoFilter, pfx: &str, out: &mut String) {
    let esc = crate::xml::escape_attr;
    out.push_str(&format!("<{}autoFilter ref=\"{}\"", pfx, af.rect.a1()));
    if af.cols.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for fc in &af.cols {
        match &fc.kind {
            FilterKind::Raw(x) => out.push_str(x),
            FilterKind::Values { values, blank } => {
                out.push_str(&format!("<{}filterColumn colId=\"{}\"><{}filters{}>", pfx, fc.col, pfx, if *blank { " blank=\"1\"" } else { "" }));
                for v in values {
                    out.push_str(&format!("<{}filter val=\"{}\"/>", pfx, esc(v)));
                }
                out.push_str(&format!("</{}filters></{}filterColumn>", pfx, pfx));
            }
            FilterKind::Custom { and, conds } => {
                out.push_str(&format!("<{}filterColumn colId=\"{}\"><{}customFilters{}>", pfx, fc.col, pfx, if *and { " and=\"1\"" } else { "" }));
                for (op, v) in conds {
                    if op == "equal" {
                        out.push_str(&format!("<{}customFilter val=\"{}\"/>", pfx, esc(v)));
                    } else {
                        out.push_str(&format!("<{}customFilter operator=\"{}\" val=\"{}\"/>", pfx, op, esc(v)));
                    }
                }
                out.push_str(&format!("</{}customFilters></{}filterColumn>", pfx, pfx));
            }
            FilterKind::Top10 { top, percent, val } => {
                out.push_str(&format!("<{}filterColumn colId=\"{}\"><{}top10{}{} val=\"{}\"/></{}filterColumn>", pfx, fc.col, pfx,
                    if *top { "" } else { " top=\"0\"" }, if *percent { " percent=\"1\"" } else { "" }, crate::xlsx::num_text(*val), pfx));
            }
        }
    }
    out.push_str(&format!("</{}autoFilter>", pfx));
}

#[allow(dead_code)]
fn unused() {
    let _ = MAX_COLS;
}
