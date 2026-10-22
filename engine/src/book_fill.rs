//! AutoFill (the fill handle), Fill Series and Flash Fill.

use crate::book::{Book, Delta};
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, Adjust};
use crate::sheet::Cell;
use crate::value::Value;
use std::rc::Rc;

const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// A value from one of the built-in lists: (list, index, long form, case 0 as-is/1 upper/2 lower).
fn list_item(t: &str) -> Option<(u8, usize, bool, u8)> {
    let lower = t.to_lowercase();
    let case = if t.chars().all(|c| !c.is_lowercase()) && t.len() > 1 { 1 } else if t.chars().all(|c| !c.is_uppercase()) { 2 } else { 0 };
    for (i, d) in DAYS.iter().enumerate() {
        if lower == d.to_lowercase() {
            return Some((0, i, true, case));
        }
        if lower == d[..3].to_lowercase() {
            return Some((0, i, false, case));
        }
    }
    for (i, m) in MONTHS.iter().enumerate() {
        if lower == m.to_lowercase() {
            return Some((1, i, true, case));
        }
        if lower == m[..3].to_lowercase() {
            return Some((1, i, false, case));
        }
    }
    None
}

fn list_text(list: u8, idx: i64, long: bool, case: u8) -> String {
    let (items, n): (&[&str], i64) = if list == 0 { (&DAYS, 7) } else { (&MONTHS, 12) };
    let s = items[idx.rem_euclid(n) as usize];
    let s = if long { s.to_string() } else { s[..3].to_string() };
    match case {
        1 => s.to_uppercase(),
        2 => s.to_lowercase(),
        _ => s,
    }
}

/// "Item 7" -> ("Item ", 7, "", digits) : the last number in a text.
fn text_number(t: &str) -> Option<(String, i64, String, usize)> {
    let b: Vec<char> = t.chars().collect();
    let mut end = b.len();
    while end > 0 && !b[end - 1].is_ascii_digit() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let mut start = end;
    while start > 0 && b[start - 1].is_ascii_digit() {
        start -= 1;
    }
    if end - start > 15 {
        return None;
    }
    let num: String = b[start..end].iter().collect();
    let prefix: String = b[..start].iter().collect();
    let suffix: String = b[end..].iter().collect();
    // a number with a sign or decimal point in front is not a counter
    Some((prefix, num.parse().ok()?, suffix, num.len()))
}

/// Least-squares line through (0, v0), (1, v1)...: (a, b) for a + b*i.
fn trend(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mx = (n - 1.0) / 2.0;
    let my = v.iter().sum::<f64>() / n;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for (i, y) in v.iter().enumerate() {
        let dx = i as f64 - mx;
        sxy += dx * (y - my);
        sxx += dx * dx;
    }
    let b = if sxx == 0.0 { 0.0 } else { sxy / sxx };
    (my - b * mx, b)
}

#[derive(Debug)]
enum Series {
    Copy,
    Linear(f64, f64),
    Months(f64, i64),
    Text(String, f64, f64, String, usize),
    List(u8, i64, i64, bool, u8),
}

impl Book {
    fn is_date_cell(&self, s: usize, r: u32, c: u32) -> bool {
        crate::numfmt::get(&self.number_format(s, r, c)).is_date()
    }

    /// Cells with the same key next to each other continue as one series.
    fn run_key(&self, s: usize, r: u32, c: u32) -> Option<String> {
        let sh = &self.sheets[s];
        if sh.get(r, c).map(|x| x.f.is_some()).unwrap_or(false) {
            return None;
        }
        match sh.value(r, c) {
            Value::Num(_) => Some(if self.is_date_cell(s, r, c) { "d".into() } else { "n".into() }),
            Value::Str(_) => {
                let t = sh.value(r, c).to_string();
                if let Some(it) = list_item(&t) {
                    return Some(format!("l{}", it.0));
                }
                text_number(&t).map(|(p, _, sfx, _)| format!("t{}\u{0}{}", p, sfx))
            }
            _ => None,
        }
    }

    /// What a line of source cells continues as.
    fn classify(&self, s: usize, cells: &[(u32, u32)], series: bool) -> Series {
        let sh = &self.sheets[s];
        let vals: Vec<Value> = cells.iter().map(|(r, c)| sh.value(*r, *c)).collect();
        let formulas = cells.iter().any(|(r, c)| sh.get(*r, *c).map(|x| x.f.is_some()).unwrap_or(false));
        if formulas || !series {
            return Series::Copy;
        }
        let k = vals.len();
        if vals.iter().all(|v| matches!(v, Value::Num(_))) {
            let nums: Vec<f64> = vals.iter().map(|v| if let Value::Num(n) = v { *n } else { 0.0 }).collect();
            let dates = cells.iter().all(|(r, c)| self.is_date_cell(s, *r, *c));
            if k == 1 {
                return if dates && nums[0].fract() == 0.0 { Series::Linear(nums[0], 1.0) } else { Series::Copy };
            }
            if dates {
                // same day of the month, months apart: step in months
                let ymd: Vec<_> = nums.iter().map(|n| crate::dates::serial_to_ymd(*n, self.date1904)).collect();
                if ymd.iter().all(|x| x.is_some()) {
                    let ymd: Vec<_> = ymd.into_iter().map(|x| x.unwrap()).collect();
                    let months: Vec<i64> = ymd.iter().map(|(y, m, _)| y * 12 + *m as i64).collect();
                    let same_day = ymd.iter().all(|x| x.2 == ymd[0].2);
                    let step = months[1] - months[0];
                    if same_day && step != 0 && months.windows(2).all(|w| w[1] - w[0] == step) {
                        return Series::Months(nums[0], step);
                    }
                }
            }
            let (a, b) = trend(&nums);
            return Series::Linear(a, b);
        }
        if vals.iter().all(|v| matches!(v, Value::Str(_))) {
            let texts: Vec<String> = vals.iter().map(|v| v.to_string()).collect();
            let items: Vec<_> = texts.iter().map(|t| list_item(t)).collect();
            if items.iter().all(|x| x.is_some()) {
                let items: Vec<_> = items.into_iter().map(|x| x.unwrap()).collect();
                if items.iter().all(|x| x.0 == items[0].0) {
                    let n = if items[0].0 == 0 { 7 } else { 12 };
                    let step = if k == 1 { 1 } else { (items[1].1 as i64 - items[0].1 as i64).rem_euclid(n) };
                    let ok = items.windows(2).all(|w| (w[1].1 as i64 - w[0].1 as i64).rem_euclid(n) == step);
                    if ok && step != 0 {
                        return Series::List(items[0].0, items[0].1 as i64, step, items[0].2, items[0].3);
                    }
                }
            }
            let tn: Vec<_> = texts.iter().map(|t| text_number(t)).collect();
            if tn.iter().all(|x| x.is_some()) {
                let tn: Vec<_> = tn.into_iter().map(|x| x.unwrap()).collect();
                if tn.iter().all(|x| x.0 == tn[0].0 && x.2 == tn[0].2) {
                    if k == 1 {
                        return Series::Text(tn[0].0.clone(), tn[0].1 as f64, 1.0, tn[0].2.clone(), tn[0].3);
                    }
                    let nums: Vec<f64> = tn.iter().map(|x| x.1 as f64).collect();
                    let (a, b) = trend(&nums);
                    return Series::Text(tn[0].0.clone(), a, b, tn[0].2.clone(), tn[0].3);
                }
            }
        }
        Series::Copy
    }

    /// The fill handle: continue src into target (which contains src and extends it in
    /// one direction). `series` false copies (Ctrl+drag, or Copy Cells); `formats_only` /
    /// `values_only` as the AutoFill options.
    pub fn autofill(&mut self, s: usize, src: Rect, target: Rect, series: bool, formats_only: bool, values_only: bool) -> Result<(), String> {
        if !target.contains_rect(&src) || target == src {
            return Ok(());
        }
        let down = target.r2 > src.r2;
        let up = target.r1 < src.r1;
        let right = target.c2 > src.c2;
        let left = target.c1 < src.c1;
        let vertical = down || up;
        if (vertical && (left || right)) || target.count() > 5_000_000 {
            return Err("The selection is too large to fill.".into());
        }
        for a in self.sheets[s].arrays.values() {
            if a.intersects(&target) && !src.contains_rect(a) {
                return Err("You can't change part of an array.".into());
            }
        }
        self.begin("AutoFill", s);
        let lines: Vec<u32> = if vertical { (src.c1..=src.c2).collect() } else { (src.r1..=src.r2).collect() };
        for line in lines {
            let cells: Vec<(u32, u32)> = if vertical {
                (src.r1..=src.r2).map(|r| (r, line)).collect()
            } else {
                (src.c1..=src.c2).map(|c| (line, c)).collect()
            };
            let k = cells.len() as i64;
            // runs of cells that continue together (numbers, one list, one text counter);
            // blanks, formulas and other text copy one by one
            let keys: Vec<Option<String>> = cells.iter().map(|(r, c)| self.run_key(s, *r, *c)).collect();
            let mut runs: Vec<(usize, usize, Series)> = Vec::new();
            let mut i0 = 0;
            while i0 < cells.len() {
                let mut i1 = i0 + 1;
                if keys[i0].is_some() {
                    while i1 < cells.len() && keys[i1] == keys[i0] {
                        i1 += 1;
                    }
                }
                let kind = self.classify(s, &cells[i0..i1], series);
                runs.push((i0, i1, kind));
                i0 = i1;
            }
            let targets: Vec<(i64, (u32, u32))> = if down {
                (src.r2 + 1..=target.r2).enumerate().map(|(t, r)| (k + t as i64, (r, line))).collect()
            } else if up {
                (target.r1..src.r1).rev().enumerate().map(|(t, r)| (-(t as i64) - 1, (r, line))).collect()
            } else if right {
                (src.c2 + 1..=target.c2).enumerate().map(|(t, c)| (k + t as i64, (line, c))).collect()
            } else {
                (target.c1..src.c1).rev().enumerate().map(|(t, c)| (-(t as i64) - 1, (line, c))).collect()
            };
            for (ti, (r, c)) in targets {
                let si = ti.rem_euclid(k) as usize;
                let q = ti.div_euclid(k);
                let (a0, a1, kind) = runs.iter().find(|x| x.0 <= si && si < x.1).unwrap();
                let i = q * (*a1 - *a0) as i64 + (si - a0) as i64;
                let (sr, sc) = cells[si];
                let srccell = self.sheets[s].get(sr, sc).cloned();
                let src_xf = self.sheets[s].style_at(sr, sc);
                let cur = self.sheets[s].get(r, c).cloned();
                let mut cell = if formats_only {
                    let mut x = cur.clone().unwrap_or_default();
                    x.s = src_xf;
                    x
                } else {
                    let mut x = match kind {
                        Series::Copy => {
                            let mut x = srccell.clone().unwrap_or_default();
                            if let Some(f) = &x.f {
                                let mut nf = (**f).clone();
                                nf.text = formula::adjust(&f.text, &Adjust::Shift(r as i64 - sr as i64, c as i64 - sc as i64));
                                nf.expr = formula::parse(&nf.text).ok();
                                if let crate::sheet::FKind::Array(_) = nf.kind {
                                    nf.kind = crate::sheet::FKind::Dynamic;
                                }
                                x.f = Some(Rc::new(nf));
                            }
                            x.extra = None;
                            x
                        }
                        Series::Linear(a, b) => Cell { v: Value::num(clean(a + b * i as f64)), ..Default::default() },
                        Series::Months(start, step) => {
                            let (y, m, d) = crate::dates::serial_to_ymd(*start, self.date1904).unwrap_or((1900, 1, 1));
                            let total = y * 12 + (m as i64 - 1) + step * i;
                            let ny = total.div_euclid(12);
                            let nm = total.rem_euclid(12) + 1;
                            let dim = crate::dates::days_in_month(ny, nm as u32);
                            let v = crate::dates::ymd_to_serial(ny, nm, d.min(dim) as i64, self.date1904).unwrap_or(0.0);
                            Cell { v: Value::num(v + start.fract()), ..Default::default() }
                        }
                        Series::Text(p, a, b, sfx, width) => {
                            let n = (a + b * i as f64).round() as i64;
                            let digits = if n < 0 { format!("{}", n.abs()) } else { format!("{:0w$}", n, w = *width) };
                            Cell { v: Value::string(format!("{}{}{}", p, digits, sfx)), ..Default::default() }
                        }
                        Series::List(list, start, step, long, case) => {
                            Cell { v: Value::string(list_text(*list, start + step * i, *long, *case)), ..Default::default() }
                        }
                    };
                    x.s = if values_only { cur.as_ref().map(|y| y.s).unwrap_or_else(|| self.sheets[s].style_at(r, c)) } else { src_xf };
                    x
                };
                if values_only && cell.f.is_some() {
                    // values only still copies formulas (as the original does) but not formats
                }
                let blank = cell.is_blank() && cell.rich.is_none();
                let keep = !(blank && cell.s == self.inherited_xf(s, r, c));
                if keep {
                    cell.rich = if cell.rich.is_some() && !matches!(kind, Series::Copy) { None } else { cell.rich.take() };
                    self.record(Delta::Cell(s, r, c, Some(cell)));
                } else if cur.is_some() {
                    self.record(Delta::Cell(s, r, c, None));
                }
            }
        }
        self.commit();
        Ok(())
    }

    /// Fill Series: for each row (by_rows) or column of rc, from its first cell.
    /// kind: "linear", "growth", "date" (unit "day", "weekday", "month", "year"), "autofill".
    #[allow(clippy::too_many_arguments)]
    pub fn fill_series(&mut self, s: usize, rc: Rect, by_rows: bool, kind: &str, unit: &str, step: f64, stop: Option<f64>, trend_on: bool) -> Result<usize, String> {
        let rc = self.sheets[s].clamp_rect(rc);
        if kind == "autofill" {
            let src = if by_rows { Rect::new(rc.r1, rc.c1, rc.r2, rc.c1) } else { Rect::new(rc.r1, rc.c1, rc.r1, rc.c2) };
            self.autofill(s, src, rc, true, false, false)?;
            return Ok(0);
        }
        self.begin("Series", s);
        let mut n = 0;
        let lines: Vec<u32> = if by_rows { (rc.r1..=rc.r2).collect() } else { (rc.c1..=rc.c2).collect() };
        for line in lines {
            let pos: Vec<(u32, u32)> = if by_rows { (rc.c1..=rc.c2).map(|c| (line, c)).collect() } else { (rc.r1..=rc.r2).map(|r| (r, line)).collect() };
            // with Trend: a best-fit line through the numbers already there
            let filled: Vec<f64> = pos.iter().map_while(|(r, c)| match self.sheets[s].value(*r, *c) {
                Value::Num(x) => Some(x),
                _ => None,
            }).collect();
            if filled.is_empty() {
                continue;
            }
            let (a, b) = if trend_on && filled.len() > 1 { trend(&filled) } else { (filled[0], step) };
            let start = if trend_on { filled.len() } else { 1 };
            let mut v = filled[0];
            for (i, (r, c)) in pos.iter().enumerate().skip(1) {
                let nv = match kind {
                    "growth" => {
                        if trend_on && filled.len() > 1 {
                            let logs: Vec<f64> = filled.iter().map(|x| x.ln()).collect();
                            let (la, lb) = trend(&logs);
                            (la + lb * i as f64).exp()
                        } else {
                            v * step
                        }
                    }
                    "date" => {
                        let st = step.round() as i64;
                        match unit {
                            "weekday" => {
                                let mut x = v;
                                let mut k = st.abs();
                                while k > 0 {
                                    x += if st > 0 { 1.0 } else { -1.0 };
                                    let wd = crate::dates::weekday(x, self.date1904);
                                    if wd != 1 && wd != 7 {
                                        k -= 1;
                                    }
                                }
                                x
                            }
                            "month" | "year" => {
                                let months = if unit == "year" { st * 12 } else { st };
                                let (y, m, d) = crate::dates::serial_to_ymd(filled[0], self.date1904).unwrap_or((1900, 1, 1));
                                let total = y * 12 + (m as i64 - 1) + months * i as i64;
                                let ny = total.div_euclid(12);
                                let nm = total.rem_euclid(12) + 1;
                                let dim = crate::dates::days_in_month(ny, nm as u32);
                                crate::dates::ymd_to_serial(ny, nm, d.min(dim) as i64, self.date1904).unwrap_or(v)
                            }
                            _ => v + st as f64,
                        }
                    }
                    _ => {
                        if trend_on && filled.len() > 1 { a + b * i as f64 } else { v + step }
                    }
                };
                if i < start {
                    v = nv;
                    continue;
                }
                if let Some(stop) = stop {
                    let past = if step >= 0.0 || kind == "growth" { nv > stop + 1e-12 } else { nv < stop - 1e-12 };
                    if past {
                        break;
                    }
                }
                v = clean(nv);
                let xf = self.sheets[s].get(pos[0].0, pos[0].1).map(|x| x.s).unwrap_or(0);
                let cur_xf = self.sheets[s].style_at(*r, *c);
                self.record(Delta::Cell(s, *r, *c, Some(Cell { v: Value::num(v), s: if cur_xf != 0 { cur_xf } else { xf }, ..Default::default() })));
                n += 1;
            }
        }
        self.commit();
        Ok(n)
    }

    /// Flash Fill: fill the empty cells of column `col` below the examples, by finding a
    /// way to build each example from the cells to its left and right on the same row.
    /// Returns the number of cells filled.
    pub fn flash_fill(&mut self, s: usize, row: u32, col: u32) -> Result<usize, String> {
        let sh = &self.sheets[s];
        // the data block next to the column
        let source_cols: Vec<u32> = {
            let mut v = Vec::new();
            let mut c = col;
            while c > 0 && sh.has_content(row, c - 1) {
                c -= 1;
                v.push(c);
            }
            let mut c = col + 1;
            while c < MAX_COLS && sh.has_content(row, c) {
                v.push(c);
                c += 1;
            }
            v.sort();
            v
        };
        if source_cols.is_empty() {
            return Err("Flash Fill needs data in the columns next to it and an example of what you want.".into());
        }
        // rows of the block
        let mut r1 = row;
        while r1 > 0 && source_cols.iter().any(|c| sh.has_content(r1 - 1, *c)) {
            r1 -= 1;
        }
        let mut r2 = row;
        while r2 + 1 < MAX_ROWS && source_cols.iter().any(|c| sh.has_content(r2 + 1, *c)) && r2 - r1 < 100_000 {
            r2 += 1;
        }
        let texts = |r: u32| -> Vec<String> { source_cols.iter().map(|c| self.display(s, r, *c)).collect() };
        let examples: Vec<(Vec<String>, String)> = (r1..=r2).filter(|r| sh.has_content(*r, col)).map(|r| (texts(r), self.display(s, r, col))).collect();
        if examples.is_empty() {
            return Err("Type an example of what you want first.".into());
        }
        let prog = crate::flashfill::learn(&examples).ok_or("Flash Fill couldn't recognize a pattern to fill the values. Type a few more examples, or change the column next to it.")?;
        let mut outs: Vec<(u32, String, u32)> = Vec::new();
        for r in r1..=r2 {
            if sh.has_content(r, col) {
                continue;
            }
            if let Some(out) = prog.run(&texts(r)) {
                if !out.is_empty() {
                    outs.push((r, out, sh.style_at(r, col)));
                }
            }
        }
        self.begin("Flash Fill", s);
        let n = outs.len();
        for (r, out, xf) in outs {
            self.record(Delta::Cell(s, r, col, Some(Cell { v: Value::string(out), s: xf, ..Default::default() })));
        }
        self.commit();
        Ok(n)
    }
}

/// Remove float noise from series arithmetic (0.1*3 = 0.30000000000000004).
fn clean(x: f64) -> f64 {
    let (d, e) = crate::numfmt::sig15(x.abs());
    if d.is_empty() {
        return 0.0;
    }
    let s: String = d.iter().map(|c| (b'0' + c) as char).collect();
    let v: f64 = format!("0.{}e{}", s, e).parse().unwrap_or(x);
    if x < 0.0 { -v } else { v }
}
