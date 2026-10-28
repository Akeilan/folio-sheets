//! Data > Advanced (Advanced Filter): a criteria range (rows are "or", columns "and",
//! computed criteria as formulas), filter in place or copy to another place, unique records.
use crate::book::{Book, Delta, Name};
use crate::cellref::{quote_sheet, Rect};
use crate::fns::db::{db_criteria, header_text};
use crate::formula::{self, Adjust};
use crate::sheet::{Cell, RowInfo};
use crate::value::Value;
use std::collections::HashSet;

fn abs_ref(sheet: &str, r: &Rect) -> String {
    format!("{}!${}${}:${}${}", quote_sheet(sheet), crate::cellref::col_name(r.c1), r.r1 + 1, crate::cellref::col_name(r.c2), r.r2 + 1)
}

impl Book {
    /// The data rows of `list` (its first row is the headers) that the criteria let through.
    pub fn advanced_matches(&mut self, s: usize, list: Rect, crit: Option<(usize, Rect)>, unique: bool) -> Result<Vec<u32>, String> {
        let list = self.adv_clamp(s, list);
        if list.r2 <= list.r1 {
            return Err("The list range must have a header row and at least one row of data.".into());
        }
        let d1904 = self.date1904;
        let heads: Vec<String> = (list.c1..=list.c2).map(|c| header_text(&self.sheets[s].value(list.r1, c))).collect();
        // rules: per criteria row, (column, condition) and computed formulas
        enum Rule {
            Col(usize, crate::fns::Criteria),
            Calc(String),
        }
        let mut rows: Vec<Vec<Rule>> = Vec::new();
        if let Some((cs, cr)) = crit {
            if cr.r2 <= cr.r1 {
                return Err("The criteria range must have a header row and at least one row of conditions.".into());
            }
            for r in cr.r1 + 1..=cr.r2 {
                let mut row = Vec::new();
                for c in cr.c1..=cr.c2 {
                    let h = header_text(&self.sheets[cs].value(cr.r1, c));
                    let cell = self.sheets[cs].cells.get(&(r, c)).cloned();
                    match heads.iter().position(|x| !h.is_empty() && *x == h) {
                        Some(dc) => {
                            let v = self.sheets[cs].value(r, c);
                            if let Some(k) = db_criteria(&v, d1904) {
                                row.push(Rule::Col(dc, k));
                            }
                        }
                        None => {
                            // a computed criterion: a formula about the first data row
                            if let Some(f) = cell.as_ref().and_then(|x| x.f.clone()) {
                                row.push(Rule::Calc(f.text.clone()));
                            }
                        }
                    }
                }
                rows.push(row);
            }
        }
        let mut out = Vec::new();
        let mut seen: HashSet<Vec<String>> = HashSet::new();
        for r in list.r1 + 1..=list.r2 {
            let mut ok = rows.is_empty();
            for row in &rows {
                let mut all = true;
                for rule in row {
                    let m = match rule {
                        Rule::Col(dc, k) => k.matches(&self.sheets[s].value(r, list.c1 + *dc as u32)),
                        Rule::Calc(t) => {
                            let t2 = formula::adjust(t, &Adjust::Shift(r as i64 - (list.r1 as i64 + 1), 0));
                            match formula::parse(&t2) {
                                Ok(e) => {
                                    let mut ev = crate::calc::Eval::new(self, s, r, list.c1, false);
                                    let v = ev.eval_top(&e);
                                    matches!(v, Value::Bool(true)) || matches!(v, Value::Num(x) if x != 0.0)
                                }
                                Err(_) => false,
                            }
                        }
                    };
                    if !m {
                        all = false;
                        break;
                    }
                }
                if all {
                    ok = true;
                    break;
                }
            }
            if !ok {
                continue;
            }
            if unique {
                let key: Vec<String> = (list.c1..=list.c2).map(|c| self.sheets[s].value(r, c).to_string().to_lowercase()).collect();
                if !seen.insert(key) {
                    continue;
                }
            }
            out.push(r);
        }
        Ok(out)
    }

    /// Filter the list in place: rows that don't match are hidden. Returns the rows shown.
    /// Whole rows or columns count as far as the sheet is used.
    fn adv_clamp(&self, s: usize, r: Rect) -> Rect {
        let (er, ec) = self.sheets[s].used_extent().unwrap_or((0, 0));
        Rect::new(r.r1, r.c1, r.r2.min(er.max(r.r1 + 1)), r.c2.min(ec.max(r.c1)))
    }

    pub fn advanced_filter(&mut self, s: usize, list: Rect, crit: Option<(usize, Rect)>, unique: bool) -> Result<usize, String> {
        let list = self.adv_clamp(s, list);
        let crit = crit.map(|(cs, cr)| (cs, self.adv_clamp(cs, cr)));
        let crit = crit.map(|(cs, cr)| (cs, self.adv_clamp(cs, cr)));
        let keep = self.advanced_matches(s, list, crit, unique)?;
        let keep: HashSet<u32> = keep.into_iter().collect();
        self.begin("Advanced Filter", s);
        for r in list.r1 + 1..=list.r2 {
            let old = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
            let mut ri: RowInfo = old.clone();
            ri.hidden = !keep.contains(&r);
            if !ri.hidden && ri.ht == Some(0.0) {
                ri.ht = None;
            }
            if ri != old {
                self.record(Delta::Row(s, r, Some(ri)));
            }
        }
        self.set_filter_names(s, list, crit, None);
        self.commit();
        Ok(keep.len())
    }

    /// Copy the matching rows (headers first) to `dest` (its top-left cell, or a header row
    /// naming the columns to copy). Returns the rows copied.
    pub fn advanced_copy(&mut self, s: usize, list: Rect, crit: Option<(usize, Rect)>, unique: bool, ds: usize, dest: Rect) -> Result<usize, String> {
        let list = self.adv_clamp(s, list);
        let crit = crit.map(|(cs, cr)| (cs, self.adv_clamp(cs, cr)));
        if ds >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        if ds != s {
            return Err("You can only copy filtered data to the active sheet.".into());
        }
        let crit = crit.map(|(cs, cr)| (cs, self.adv_clamp(cs, cr)));
        let keep = self.advanced_matches(s, list, crit, unique)?;
        // the columns: those the destination's header row names, or all of them
        let heads: Vec<String> = (list.c1..=list.c2).map(|c| header_text(&self.sheets[s].value(list.r1, c))).collect();
        let mut cols: Vec<u32> = Vec::new();
        if dest.c2 > dest.c1 || !matches!(self.sheets[ds].value(dest.r1, dest.c1), Value::Empty) {
            for c in dest.c1..=dest.c2 {
                let h = header_text(&self.sheets[ds].value(dest.r1, c));
                match heads.iter().position(|x| !h.is_empty() && *x == h) {
                    Some(k) => cols.push(list.c1 + k as u32),
                    None if dest.c1 == dest.c2 => {}
                    None => return Err("The extract range has a missing or not valid field name.".into()),
                }
            }
        }
        if cols.is_empty() {
            cols = (list.c1..=list.c2).collect();
        }
        let src_rows: Vec<u32> = std::iter::once(list.r1).chain(keep.iter().copied()).collect();
        // the overlap the original refuses
        let out_rect = Rect::new(dest.r1, dest.c1, dest.r1 + src_rows.len() as u32 - 1, dest.c1 + cols.len() as u32 - 1);
        if ds == s && out_rect.intersect(&list).is_some() {
            return Err("The extract range can't overlap the list range.".into());
        }
        let blank = Cell::default();
        self.begin("Advanced Filter", ds);
        for (k, &r) in src_rows.iter().enumerate() {
            for (j, &c) in cols.iter().enumerate() {
                let src = self.sheets[s].cells.get(&(r, c)).cloned().unwrap_or_else(|| blank.clone());
                let mut cell = src.clone();
                if cell.f.is_some() {
                    // the copy holds the value
                    cell.f = None;
                }
                let (tr, tc) = (dest.r1 + k as u32, dest.c1 + j as u32);
                self.record(Delta::Cell(ds, tr, tc, if cell.is_blank() && cell.s == 0 { None } else { Some(cell) }));
            }
        }
        self.set_filter_names(s, list, crit, Some((ds, out_rect)));
        self.commit();
        Ok(keep.len())
    }

    fn set_filter_names(&mut self, s: usize, list: Rect, crit: Option<(usize, Rect)>, extract: Option<(usize, Rect)>) {
        let sn = self.sheets[s].name.clone();
        let mut names = self.names.clone();
        let mut put = |names: &mut Vec<Name>, nm: &str, text: String, hidden: bool| {
            names.retain(|n| !(n.scope == Some(s) && n.name.eq_ignore_ascii_case(nm)));
            let mut n = Name::new(nm, Some(s), &text);
            n.hidden = hidden;
            n.expr = formula::parse(&n.text).ok().map(std::rc::Rc::new);
            names.push(n);
        };
        put(&mut names, "_xlnm._FilterDatabase", abs_ref(&sn, &list), true);
        if let Some((cs, cr)) = crit {
            let cn = self.sheets[cs].name.clone();
            put(&mut names, "_xlnm.Criteria", abs_ref(&cn, &cr), false);
        }
        if let Some((es, er)) = extract {
            let en = self.sheets[es].name.clone();
            put(&mut names, "_xlnm.Extract", abs_ref(&en, &er), false);
        }
        self.record(Delta::Names(names));
    }

    /// Show All after an advanced filter: the list's rows are shown again.
    pub fn advanced_show_all(&mut self, s: usize) -> Result<bool, String> {
        let list = self.names.iter().find(|n| n.scope == Some(s) && n.name.eq_ignore_ascii_case("_xlnm._FilterDatabase"))
            .and_then(|n| crate::cellref::parse_range(&n.text.rsplit('!').next().unwrap_or("").replace('$', "")));
        let list = match list {
            Some(l) => l,
            None => return Ok(false),
        };
        let hidden: Vec<u32> = (list.r1 + 1..=list.r2).filter(|r| self.sheets[s].rows.get(r).map(|x| x.hidden).unwrap_or(false)).collect();
        if hidden.is_empty() {
            return Ok(false);
        }
        self.begin("Clear", s);
        for r in hidden {
            let mut ri = self.sheets[s].rows[&r].clone();
            ri.hidden = false;
            if ri.ht == Some(0.0) {
                ri.ht = None;
            }
            self.record(Delta::Row(s, r, Some(ri)));
        }
        self.commit();
        Ok(true)
    }
}
