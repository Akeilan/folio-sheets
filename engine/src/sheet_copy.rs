//! Move or Copy > Create a copy: a worksheet copied with everything on it.
use crate::book::{rebase, Book, Delta};
use crate::formula::{self, Adjust};
use crate::sheet::Piece;
use std::rc::Rc;

/// "Sales" -> "Sales (2)", "Sales (2)" -> "Sales (3)", kept within 31 characters.
pub fn copy_name(b: &Book, name: &str) -> String {
    let base = match name.rfind(" (") {
        Some(i) if name.ends_with(')') && name[i + 2..name.len() - 1].parse::<u32>().is_ok() => &name[..i],
        _ => name,
    };
    let mut k = 2;
    loop {
        let tail = format!(" ({})", k);
        let mut head: String = base.chars().collect();
        while head.chars().count() + tail.chars().count() > 31 {
            head.pop();
        }
        let n = format!("{}{}", head, tail);
        if b.sheet_index(&n).is_none() {
            return n;
        }
        k += 1;
    }
}

/// Elements of the sheet part that point at relationships the copy doesn't have.
fn needs_rels(tag: &str, xml: &str) -> bool {
    tag != "hyperlinks" && (xml.contains("r:id=") || xml.contains(":id=\"rId"))
}

impl Book {
    /// Copy sheet `from` and put the copy before position `at` (sheets.len() = at the end).
    /// Returns the copy's index.
    pub fn copy_sheet(&mut self, from: usize, at: usize) -> Result<usize, String> {
        if from >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        let src = &self.sheets[from];
        if src.raw_part.is_some() || src.kind != "work" {
            return Err("Only worksheets can be copied in this version; chart sheets and dialog sheets can be moved.".into());
        }
        let old = src.name.clone();
        let new = copy_name(self, &old);
        let mut sh = src.clone();
        sh.id = self.sheets.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        sh.name = new.clone();
        sh.part = String::new();
        sh.comments_file = None;
        sh.error = None;
        sh.pieces.retain(|p| !matches!(p, Piece::Raw(t, x) if needs_rels(t, x)));
        // objects: slicers and timelines stay with the original (they belong to its caches)
        sh.objects.retain(|o| o.slicer.is_none());
        // tables get names and ids of their own
        let mut taken = self.table_names();
        let mut next_id = self.next_table_id();
        let mut renames: Vec<(String, String)> = Vec::new();
        for t in sh.tables.iter_mut() {
            let stem = t.name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
            let stem = if stem.is_empty() { "Table".to_string() } else { stem };
            let mut k = 1;
            let nm = loop {
                let n = format!("{}{}", stem, k);
                if !taken.contains(&n.to_lowercase()) && self.find_name(&n, 0).is_none() {
                    break n;
                }
                k += 1;
            };
            taken.push(nm.to_lowercase());
            renames.push((t.name.clone(), nm.clone()));
            t.name = nm;
            t.id = next_id;
            next_id += 1;
            t.part = None;
            if let Some((p, e)) = t.orig.take() {
                let mut e = e;
                e.set_attr("id", &t.id.to_string());
                e.set_attr("name", &t.name);
                e.set_attr("displayName", &t.name);
                t.orig = Some((p, e));
            }
        }
        // threaded comments get new ids
        let mut rng = self.rng;
        for th in sh.threads.values_mut() {
            for e in th.entries.iter_mut() {
                e.id = crate::notes::guid(&mut rng);
            }
        }
        self.rng = rng;
        // formulas that name the sheet itself now name the copy; its tables' references follow
        let how = Adjust::RenameSheet(&old, &new);
        let how2 = rebase(&how, &new);
        let mut edits = Vec::new();
        for ((r, c), cell) in sh.cells.iter() {
            if let Some(f) = &cell.f {
                let mut t = formula::adjust(&f.text, &how2);
                if !renames.is_empty() {
                    t = crate::tables::rewrite_structs(&t, |sr| {
                        let tn = sr.table.as_deref()?;
                        let (_, nn) = renames.iter().find(|(o, _)| o.eq_ignore_ascii_case(tn))?;
                        let mut x = sr.clone();
                        x.table = Some(nn.clone());
                        Some(crate::tables::write_struct(&x, None, false))
                    });
                }
                if t != f.text {
                    let mut n = cell.clone();
                    let mut nf = (**f).clone();
                    nf.text = t;
                    nf.expr = formula::parse(&nf.text).ok();
                    n.f = Some(Rc::new(nf));
                    edits.push(((*r, *c), n));
                }
            }
        }
        for (k, n) in edits {
            sh.cells.insert(k, n);
        }
        for o in sh.objects.iter_mut() {
            if let Some(ch) = &o.chart {
                let mut c = (**ch).clone();
                if c.adjust(&how2) {
                    o.chart = Some(Rc::new(c));
                }
            }
            if let Some(cx) = &o.chartex {
                let mut c = (**cx).clone();
                if c.adjust(&how2) {
                    o.chartex = Some(Rc::new(c));
                }
            }
        }
        let at = at.min(self.sheets.len());
        self.begin("Copy Sheet", at);
        self.record(Delta::AddSheet(at, Box::new(sh)));
        let src_i = if from >= at { from + 1 } else { from };
        // names scoped to the sheet get a copy scoped to the copy
        let scoped: Vec<_> = self.names.iter().filter(|n| n.scope == Some(src_i)).cloned().collect();
        if !scoped.is_empty() {
            let mut names = self.names.clone();
            for mut n in scoped {
                n.scope = Some(at);
                n.text = formula::adjust(&n.text, &how);
                n.expr = formula::parse(&n.text).ok().map(Rc::new);
                names.push(n);
            }
            self.record(Delta::Names(names));
        }
        // PivotTables on the sheet are copied too (same caches)
        let ps: Vec<crate::pivot::Pivot> = self.pivots.iter().filter(|p| p.sheet == src_i).cloned().collect();
        if !ps.is_empty() {
            let mut all = self.pivots.clone();
            for mut p in ps {
                p.sheet = at;
                p.table_part = None;
                p.dirty = true;
                p.slicer_fields.clear();
                all.push(p);
            }
            self.record(Delta::Pivots(all));
        }
        self.commit();
        Ok(at)
    }
}
