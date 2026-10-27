//! What-If Analysis (SH-20): trying values in cells without keeping them — Goal Seek, Data
//! Tables (TABLE results worked out again as the sheet changes) and the Scenario Manager.

use crate::book::{Book, Delta};
use crate::cellref::{parse_range, Rect};
use crate::sheet::{Cell, FKind, Piece};
use crate::value::Value;
use crate::xml::{self, Element};
use std::rc::Rc;

pub type Key = (usize, u32, u32);

/// A data table: its results range, the two input cells (row input, column input) and the kind.
#[derive(Clone, Debug, PartialEq)]
pub struct DataTable {
    pub sheet: usize,
    pub anchor: (u32, u32),
    pub rect: Rect,
    pub two_d: bool,
    /// one input, laid out in a row (the row input cell)
    pub row: bool,
    pub r1: Option<(u32, u32)>,
    pub r2: Option<(u32, u32)>,
}

fn cell_ref(v: &str) -> Option<(u32, u32)> {
    parse_range(&v.replace('$', "")).map(|r| (r.r1, r.c1))
}

pub fn parse_data_table(sheet: usize, anchor: (u32, u32), raw: &str) -> Option<DataTable> {
    let (_, e) = xml::parse(raw).ok()?;
    let rect = parse_range(e.attr("ref")?)?;
    let on = |n: &str| matches!(e.attr(n), Some("1") | Some("true"));
    let del1 = on("del1");
    let del2 = on("del2");
    Some(DataTable {
        sheet,
        anchor,
        rect,
        two_d: on("dt2D"),
        row: on("dtr"),
        r1: if del1 { None } else { e.attr("r1").and_then(cell_ref) },
        r2: if del2 { None } else { e.attr("r2").and_then(cell_ref) },
    })
}

impl Book {
    /// Values of `outputs` for each run of `inputs` set as constants; nothing is kept, nothing
    /// goes in the undo history, and the sheet is as it was after.
    pub fn what_if(&mut self, runs: &[Vec<(Key, Value)>], outputs: &[Key]) -> Vec<Vec<Value>> {
        let was = self.in_what_if;
        self.in_what_if = true;
        let mut saved: Vec<(Key, Option<Cell>)> = Vec::new();
        for run in runs {
            for (k, _) in run {
                if !saved.iter().any(|(s, _)| s == k) && k.0 < self.sheets.len() {
                    saved.push((*k, self.sheets[k.0].get(k.1, k.2).cloned()));
                }
            }
        }
        let mut out = Vec::with_capacity(runs.len());
        for run in runs {
            for (k, v) in run {
                if k.0 >= self.sheets.len() {
                    continue;
                }
                let style = self.sheets[k.0].get(k.1, k.2).map(|c| c.s).unwrap_or_else(|| self.sheets[k.0].style_at(k.1, k.2));
                let cell = Cell { v: v.clone(), s: style, ..Default::default() };
                self.sheets[k.0].put(k.1, k.2, Some(cell));
                self.changed.push(*k);
            }
            self.recalc(false);
            out.push(outputs.iter().map(|k| self.value(k.0, k.1, k.2)).collect());
        }
        for (k, c) in saved {
            let had_f = c.as_ref().map(|c| c.f.is_some()).unwrap_or(false);
            self.sheets[k.0].put(k.1, k.2, c);
            self.changed.push(k);
            if had_f {
                self.forced.insert(k);
            }
        }
        self.recalc(false);
        self.in_what_if = was;
        out
    }

    /// Goal Seek: the value of `changing` that makes `target` equal `goal`. Returns
    /// (the value found, the target's value with it, whether it got there).
    pub fn goal_seek(&mut self, s: usize, target: (u32, u32), goal: f64, changing: (u32, u32)) -> Result<(f64, f64, bool), String> {
        let sh = self.sheets.get(s).ok_or("No such sheet.")?;
        if sh.get(target.0, target.1).and_then(|c| c.f.as_ref()).is_none() {
            return Err("Cell must contain a formula.".into());
        }
        if sh.get(changing.0, changing.1).map(|c| c.f.is_some()).unwrap_or(false) {
            return Err("Cell must contain a value.".into());
        }
        let x0 = match self.value(s, changing.0, changing.1) {
            Value::Num(x) => x,
            Value::Empty => 0.0,
            _ => return Err("Cell must contain a value.".into()),
        };
        let tk: Key = (s, target.0, target.1);
        let ck: Key = (s, changing.0, changing.1);
        let mut f = |b: &mut Book, x: f64| -> Option<f64> {
            match b.what_if(&[vec![(ck, Value::num(x))]], &[tk]).pop().and_then(|mut v| v.pop()) {
                Some(Value::Num(y)) => Some(y - goal),
                _ => None,
            }
        };
        let tol = 0.001_f64.max(goal.abs() * 1e-9);
        let mut best = (x0, f(self, x0).unwrap_or(f64::INFINITY));
        if best.1.abs() <= tol {
            return Ok((x0, best.1 + goal, true));
        }
        // secant steps, then a bracketing search if they stall
        let mut a = x0;
        let mut fa = best.1;
        let mut b = if x0 == 0.0 { 0.01 } else { x0 * 1.01 };
        let mut fb = f(self, b).unwrap_or(f64::INFINITY);
        for _ in 0..100 {
            if fb.is_finite() && fb.abs() < best.1.abs() {
                best = (b, fb);
            }
            if best.1.abs() <= tol {
                break;
            }
            if !fa.is_finite() || !fb.is_finite() || fb == fa {
                break;
            }
            let c = b - fb * (b - a) / (fb - fa);
            if !c.is_finite() {
                break;
            }
            a = b;
            fa = fb;
            b = c;
            fb = f(self, b).unwrap_or(f64::INFINITY);
        }
        if best.1.abs() > tol {
            // look outwards for a sign change, then bisect
            let mut step = (x0.abs()).max(1.0);
            let mut lo = x0;
            let flo = f(self, lo).unwrap_or(f64::NAN);
            let mut hi = None;
            for _ in 0..60 {
                for cand in [x0 + step, x0 - step] {
                    if let Some(fc) = f(self, cand) {
                        if fc.abs() < best.1.abs() {
                            best = (cand, fc);
                        }
                        if flo.is_finite() && fc.signum() != flo.signum() {
                            hi = Some((cand, fc));
                            break;
                        }
                    }
                }
                if hi.is_some() {
                    break;
                }
                step *= 2.0;
            }
            if let Some((mut h, _)) = hi {
                let mut fl = flo;
                for _ in 0..200 {
                    let m = (lo + h) / 2.0;
                    let fm = match f(self, m) {
                        Some(v) => v,
                        None => break,
                    };
                    if fm.abs() < best.1.abs() {
                        best = (m, fm);
                    }
                    if fm.abs() <= tol {
                        break;
                    }
                    if fm.signum() == fl.signum() {
                        lo = m;
                        fl = fm;
                    } else {
                        h = m;
                    }
                }
            }
        }
        Ok((best.0, best.1 + goal, best.1.abs() <= tol))
    }

    // ------------------------------------------------------------------ data tables

    pub fn data_tables(&self) -> Vec<DataTable> {
        let mut out = Vec::new();
        for (s, sh) in self.sheets.iter().enumerate() {
            if !sh.has_data_tables.get() {
                continue;
            }
            let mut any = false;
            for ((r, c), cell) in sh.cells.iter() {
                if let Some(f) = &cell.f {
                    if let FKind::DataTable(raw) = &f.kind {
                        any = true;
                        if let Some(t) = parse_data_table(s, (*r, *c), raw) {
                            out.push(t);
                        }
                    }
                }
            }
            sh.has_data_tables.set(any);
        }
        out
    }

    pub fn data_table_at(&self, s: usize, r: u32, c: u32) -> Option<DataTable> {
        self.data_tables().into_iter().find(|t| t.sheet == s && t.rect.contains(r, c))
    }

    /// Work the data tables out again (after a change).
    pub fn recalc_data_tables(&mut self) {
        if self.in_what_if {
            return;
        }
        let tables = self.data_tables();
        if tables.is_empty() {
            return;
        }
        let mut results: Vec<(Key, Value)> = Vec::new();
        for t in &tables {
            let s = t.sheet;
            let rc = t.rect;
            let mut runs = Vec::new();
            let mut outs: Vec<Key> = Vec::new();
            let mut cells: Vec<(Key, usize)> = Vec::new();
            if t.two_d {
                let f = (s, rc.r1.saturating_sub(1), rc.c1.saturating_sub(1));
                outs.push(f);
                for r in rc.r1..=rc.r2 {
                    for c in rc.c1..=rc.c2 {
                        let mut run = Vec::new();
                        if let Some(i) = t.r1 {
                            run.push(((s, i.0, i.1), self.value(s, rc.r1 - 1, c)));
                        }
                        if let Some(i) = t.r2 {
                            run.push(((s, i.0, i.1), self.value(s, r, rc.c1 - 1)));
                        }
                        cells.push(((s, r, c), runs.len()));
                        runs.push(run);
                    }
                }
                let vals = self.what_if(&runs, &outs);
                for ((k, i), _) in cells.iter().zip(0..) {
                    results.push((*k, vals[*i][0].clone()));
                }
            } else if t.row {
                // values across the row above, formulas down the column to the left
                let input = match t.r1 {
                    Some(i) => (s, i.0, i.1),
                    None => continue,
                };
                for r in rc.r1..=rc.r2 {
                    outs.push((s, r, rc.c1.saturating_sub(1)));
                }
                for c in rc.c1..=rc.c2 {
                    runs.push(vec![(input, self.value(s, rc.r1.saturating_sub(1), c))]);
                }
                let vals = self.what_if(&runs, &outs);
                for (ci, c) in (rc.c1..=rc.c2).enumerate() {
                    for (ri, r) in (rc.r1..=rc.r2).enumerate() {
                        results.push(((s, r, c), vals[ci][ri].clone()));
                    }
                }
            } else {
                // values down the column to the left, formulas across the row above
                let input = match t.r1 {
                    Some(i) => (s, i.0, i.1),
                    None => continue,
                };
                for c in rc.c1..=rc.c2 {
                    outs.push((s, rc.r1.saturating_sub(1), c));
                }
                for r in rc.r1..=rc.r2 {
                    runs.push(vec![(input, self.value(s, r, rc.c1.saturating_sub(1)))]);
                }
                let vals = self.what_if(&runs, &outs);
                for (ri, r) in (rc.r1..=rc.r2).enumerate() {
                    for (ci, c) in (rc.c1..=rc.c2).enumerate() {
                        results.push(((s, r, c), vals[ri][ci].clone()));
                    }
                }
            }
        }
        // the results are the cells' values (the anchor keeps its TABLE formula)
        let mut moved = Vec::new();
        for (k, v) in results {
            let sh = &mut self.sheets[k.0];
            let cur = sh.get(k.1, k.2).cloned();
            if cur.as_ref().map(|c| c.v == v).unwrap_or(false) {
                continue;
            }
            let mut cell = cur.unwrap_or_else(|| Cell { s: sh.style_at(k.1, k.2), ..Default::default() });
            cell.v = v;
            sh.put(k.1, k.2, Some(cell));
            moved.push(k);
        }
        if !moved.is_empty() {
            self.in_what_if = true;
            self.changed.extend(moved);
            self.recalc(false);
            self.in_what_if = false;
        }
    }

    /// Data › What-If Analysis › Data Table: a table of results for the formulas in its first row
    /// (or column), with the values in its first column (or row) put in the input cells.
    pub fn create_data_table(&mut self, s: usize, sel: Rect, row_input: Option<(u32, u32)>, col_input: Option<(u32, u32)>) -> Result<(), String> {
        if s >= self.sheets.len() {
            return Err("No such sheet.".into());
        }
        if sel.rows() < 2 || sel.cols() < 2 {
            return Err("Select the formulas and the values to try, with the cells for the results.".into());
        }
        if row_input.is_none() && col_input.is_none() {
            return Err("Input cell reference is not valid.".into());
        }
        for i in [row_input, col_input].iter().flatten() {
            if sel.contains(i.0, i.1) {
                return Err("Input cell reference is not valid.".into());
            }
        }
        let rect = Rect::new(sel.r1 + 1, sel.c1 + 1, sel.r2, sel.c2);
        if self.data_tables().iter().any(|t| t.sheet == s && t.rect.intersects(&rect)) {
            return Err("Cannot change part of a data table.".into());
        }
        let a1 = |p: (u32, u32)| crate::cellref::a1(p.0, p.1);
        let raw = match (row_input, col_input) {
            (Some(ri), Some(ci)) => format!("<f t=\"dataTable\" ref=\"{}\" dt2D=\"1\" dtr=\"1\" r1=\"{}\" r2=\"{}\"/>", rect.a1(), a1(ri), a1(ci)),
            (Some(ri), None) => format!("<f t=\"dataTable\" ref=\"{}\" dt2D=\"0\" dtr=\"1\" r1=\"{}\"/>", rect.a1(), a1(ri)),
            (None, Some(ci)) => format!("<f t=\"dataTable\" ref=\"{}\" dt2D=\"0\" dtr=\"0\" r1=\"{}\"/>", rect.a1(), a1(ci)),
            (None, None) => unreachable!(),
        };
        // one-variable tables put results only beside their formulas
        let rect2 = match (row_input, col_input) {
            (Some(_), None) => Rect::new(sel.r1 + 1, sel.c1 + 1, sel.r2, sel.c2),
            _ => rect,
        };
        let raw = raw.replace(&format!("ref=\"{}\"", rect.a1()), &format!("ref=\"{}\"", rect2.a1()));
        self.begin("Data Table", s);
        for r in rect2.r1..=rect2.r2 {
            for c in rect2.c1..=rect2.c2 {
                let style = self.sheets[s].style_at(r, c);
                let mut cell = Cell { s: style, ..Default::default() };
                if (r, c) == (rect2.r1, rect2.c1) {
                    cell.f = Some(Rc::new(crate::book::make_formula("", FKind::DataTable(raw.clone()))));
                    self.sheets[s].has_data_tables.set(true);
                }
                self.record(Delta::Cell(s, r, c, Some(cell)));
            }
        }
        self.commit();
        self.recalc_data_tables();
        Ok(())
    }

    /// What the formula bar shows in a data table's cells: {=TABLE(row input, column input)}.
    pub fn data_table_text(&self, s: usize, r: u32, c: u32) -> Option<String> {
        let t = self.data_table_at(s, r, c)?;
        let a1 = |p: Option<(u32, u32)>| p.map(|p| crate::cellref::a1(p.0, p.1)).unwrap_or_default();
        Some(if t.two_d {
            format!("{{=TABLE({},{})}}", a1(t.r1), a1(t.r2))
        } else if t.row {
            format!("{{=TABLE({},)}}", a1(t.r1))
        } else {
            format!("{{=TABLE(,{})}}", a1(t.r1))
        })
    }

    // ------------------------------------------------------------------ scenarios

    pub fn scenarios(&self, s: usize) -> Vec<Scenario> {
        self.sheets.get(s).map(|sh| read_scenarios(&sh.pieces)).unwrap_or_default()
    }

    pub fn set_scenarios(&mut self, s: usize, list: &[Scenario], label: &str) -> Result<(), String> {
        let sh = self.sheets.get(s).ok_or("No such sheet.")?;
        let pieces = with_scenarios(&sh.pieces, list);
        self.begin(label, s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }

    /// Show a scenario: its values go in their cells (one undo step).
    pub fn show_scenario(&mut self, s: usize, i: usize) -> Result<(), String> {
        let sc = self.scenarios(s).get(i).cloned().ok_or("No such scenario.")?;
        self.begin_group(&format!("Show Scenario {}", sc.name), s);
        let mut res = Ok(());
        for ((r, c), v) in &sc.cells {
            if let Err(e) = self.set_input(s, *r, *c, v) {
                res = Err(e);
                break;
            }
        }
        self.end_group();
        res
    }

    /// Scenario Summary: each scenario's result cells, worked out without changing the sheet.
    pub fn scenario_results(&mut self, s: usize, results: &[(u32, u32)]) -> Vec<Vec<Value>> {
        let list = self.scenarios(s);
        let runs: Vec<Vec<(Key, Value)>> = list.iter().map(|sc| {
            sc.cells.iter().map(|((r, c), v)| ((s, *r, *c), typed(v))).collect()
        }).collect();
        let outs: Vec<Key> = results.iter().map(|(r, c)| (s, *r, *c)).collect();
        self.what_if(&runs, &outs)
    }
}

fn typed(v: &str) -> Value {
    match v.trim().parse::<f64>() {
        Ok(x) => Value::num(x),
        Err(_) => match v {
            "TRUE" => Value::Bool(true),
            "FALSE" => Value::Bool(false),
            _ => Value::Str(Rc::from(v)),
        },
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Scenario {
    pub name: String,
    pub comment: String,
    pub user: String,
    /// Prevent changes / Hide
    pub locked: bool,
    pub hidden: bool,
    pub cells: Vec<((u32, u32), String)>,
}

pub fn read_scenarios(pieces: &[Piece]) -> Vec<Scenario> {
    for p in pieces {
        if let Piece::Raw(t, x) = p {
            if t == "scenarios" {
                if let Ok((_, e)) = xml::parse(x) {
                    return e.elems().filter(|c| c.local() == "scenario").map(|sc| Scenario {
                        name: sc.attr("name").unwrap_or("").to_string(),
                        comment: sc.attr("comment").unwrap_or("").to_string(),
                        user: sc.attr("user").unwrap_or("").to_string(),
                        locked: sc.attr("locked") != Some("0"),
                        hidden: matches!(sc.attr("hidden"), Some("1") | Some("true")),
                        cells: sc.elems().filter(|c| c.local() == "inputCells").filter_map(|ic| {
                            let at = cell_ref(ic.attr("r")?)?;
                            Some((at, ic.attr("val").unwrap_or("").to_string()))
                        }).collect(),
                    }).collect();
                }
            }
        }
    }
    vec![]
}

pub fn with_scenarios(pieces: &[Piece], list: &[Scenario]) -> Vec<Piece> {
    let mut e = Element::new("scenarios");
    let mut sq: Vec<String> = Vec::new();
    for sc in list {
        let mut x = Element::with_attrs("scenario", &[("name", &sc.name), ("count", &sc.cells.len().to_string())]);
        if !sc.locked {
            x.set_attr("locked", "0");
        }
        if sc.hidden {
            x.set_attr("hidden", "1");
        }
        if !sc.user.is_empty() {
            x.set_attr("user", &sc.user);
        }
        if !sc.comment.is_empty() {
            x.set_attr("comment", &sc.comment);
        }
        for ((r, c), v) in &sc.cells {
            let a = crate::cellref::a1(*r, *c);
            if !sq.contains(&a) {
                sq.push(a.clone());
            }
            x.push(Element::with_attrs("inputCells", &[("r", &a), ("val", v)]));
        }
        e.push(x);
    }
    if !sq.is_empty() {
        e.attrs.insert(0, ("sqref".into(), sq.join(" ")));
    }
    let mut out: Vec<Piece> = pieces.iter().filter(|p| !matches!(p, Piece::Raw(t, _) if t == "scenarios")).cloned().collect();
    if !list.is_empty() {
        // the writer puts the sheet's parts in their schema order
        out.push(Piece::Raw("scenarios".into(), e.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenarios_read_and_write() {
        let x = "<scenarios current=\"0\" sqref=\"B1 B2\"><scenario name=\"Best\" locked=\"1\" count=\"2\" user=\"ron\" comment=\"c\"><inputCells r=\"B1\" val=\"10\"/><inputCells r=\"B2\" val=\"0.5\"/></scenario></scenarios>";
        let p = vec![Piece::Raw("scenarios".into(), x.into())];
        let l = read_scenarios(&p);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].cells, vec![((0, 1), "10".to_string()), ((1, 1), "0.5".to_string())]);
        let back = read_scenarios(&with_scenarios(&p, &l));
        assert_eq!(back, l);
        assert!(with_scenarios(&p, &[]).is_empty());
    }
}
