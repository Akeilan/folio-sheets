//! Links to other workbooks: [1]Sheet1!A1 references read from the values the file keeps
//! for them (the externalLink parts), Workbook Links (update from the source, change the
//! source, break the link).
use crate::book::{Book, Delta};
use crate::cellref::{parse_range, Rect};
use crate::value::{Array, Value, XlErr};
use crate::xlsx::{rels_name, remove_override, resolve};
use crate::xml::{self, Element, Node};
use std::collections::HashMap;
use std::rc::Rc;

pub const T_EXTLINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLink";

#[derive(Clone, Debug, Default)]
pub struct ExtBook {
    /// [n] in formulas
    pub index: usize,
    pub rid: String,
    pub part: String,
    /// the source workbook as the file has it (relative, absolute or a file: URL)
    pub target: String,
    pub sheets: Vec<String>,
    /// (sheet, row, col) -> the value kept
    pub cells: HashMap<(usize, u32, u32), Value>,
}

fn part_bytes<'a>(b: &'a Book, name: &str) -> Option<&'a [u8]> {
    b.file.parts.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_slice())
}

fn cell_value(c: &Element) -> Value {
    let v = c.child("v").map(|v| v.text()).unwrap_or_default();
    match c.attr("t").unwrap_or("n") {
        "s" | "str" | "inlineStr" => Value::str(&v),
        "b" => Value::Bool(v.trim() == "1"),
        "e" => Value::Err(XlErr::parse(v.trim()).unwrap_or(XlErr::Ref)),
        _ => match v.trim().parse::<f64>() {
            Ok(x) => Value::Num(x),
            Err(_) if v.is_empty() => Value::Empty,
            Err(_) => Value::str(&v),
        },
    }
}

impl Book {
    /// The workbooks this one links to, in [n] order.
    pub fn ext_books(&self) -> Vec<ExtBook> {
        let mut out = Vec::new();
        let wb = match &self.file.wb {
            Some((_, e)) => e,
            None => return out,
        };
        let rels = match &self.file.wb_rels {
            Some((_, e)) => e,
            None => return out,
        };
        let refs = match wb.child("externalReferences") {
            Some(e) => e,
            None => return out,
        };
        for (k, er) in refs.elems().enumerate() {
            let rid = match er.attrs.iter().find(|(n, _)| n.ends_with(":id")) {
                Some((_, v)) => v.clone(),
                None => continue,
            };
            let target = match rels.elems().find(|e| e.attr("Id") == Some(rid.as_str())).and_then(|e| e.attr("Target")) {
                Some(t) => resolve(&self.file.wb_part, t),
                None => continue,
            };
            let mut eb = ExtBook { index: k + 1, rid, part: target.clone(), ..Default::default() };
            if let Some(d) = part_bytes(self, &target) {
                if let Ok((_, root)) = xml::parse_bytes(d) {
                    if let Some(book) = root.child("externalBook") {
                        let brid = book.attrs.iter().find(|(n, _)| n.ends_with(":id")).map(|(_, v)| v.clone());
                        if let (Some(brid), Some(rd)) = (brid, part_bytes(self, &rels_name(&target))) {
                            if let Ok((_, rr)) = xml::parse_bytes(rd) {
                                if let Some(t) = rr.elems().find(|e| e.attr("Id") == Some(brid.as_str())).and_then(|e| e.attr("Target")) {
                                    eb.target = t.to_string();
                                }
                            }
                        }
                        if let Some(sn) = book.child("sheetNames") {
                            eb.sheets = sn.elems().filter_map(|e| e.attr("val").map(|s| s.to_string())).collect();
                        }
                        if let Some(ds) = book.child("sheetDataSet") {
                            for sd in ds.elems() {
                                let sid: usize = sd.attr("sheetId").and_then(|x| x.parse().ok()).unwrap_or(0);
                                for row in sd.elems() {
                                    for c in row.elems() {
                                        if let Some((r, cc)) = c.attr("r").and_then(crate::cellref::parse_cell) {
                                            eb.cells.insert((sid, r, cc), cell_value(c));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            out.push(eb);
        }
        out
    }

    /// The value of an external reference ("[1]Sheet1!$A$1" or a range), from the kept values.
    pub fn ext_value(&self, text: &str) -> Option<Value> {
        let t = text.trim();
        let (n, rest) = {
            let t = t.strip_prefix('\'').unwrap_or(t);
            let t = t.strip_prefix('[')?;
            let e = t.find(']')?;
            (t[..e].parse::<usize>().ok()?, &t[e + 1..])
        };
        let (sheet, addr) = match rest.rfind('!') {
            Some(i) => (rest[..i].trim_end_matches('\'').trim_start_matches('\'').replace("''", "'"), &rest[i + 1..]),
            None => return None,
        };
        let books = self.ext_cache();
        let eb = books.iter().find(|b| b.index == n)?;
        let si = eb.sheets.iter().position(|s| s.eq_ignore_ascii_case(&sheet))?;
        let rc: Rect = parse_range(&addr.replace('$', ""))?;
        let get = |r: u32, c: u32| eb.cells.get(&(si, r, c)).cloned().unwrap_or(Value::Num(0.0));
        if rc.r1 == rc.r2 && rc.c1 == rc.c2 {
            return Some(match get(rc.r1, rc.c1) {
                Value::Empty => Value::Num(0.0),
                v => v,
            });
        }
        if rc.count() > 1_000_000 {
            return Some(Value::Err(XlErr::Ref));
        }
        let rows = (rc.r2 - rc.r1 + 1) as usize;
        let cols = (rc.c2 - rc.c1 + 1) as usize;
        let mut data = Vec::with_capacity(rows * cols);
        for r in rc.r1..=rc.r2 {
            for c in rc.c1..=rc.c2 {
                data.push(get(r, c));
            }
        }
        Some(Value::Arr(Rc::new(Array { rows, cols, data })))
    }

    pub fn ext_cache(&self) -> Rc<Vec<ExtBook>> {
        let mut c = self.file.ext_cache.borrow_mut();
        if c.is_none() {
            *c = Some(Rc::new(self.ext_books()));
        }
        c.as_ref().unwrap().clone()
    }

    fn ext_changed(&mut self) {
        *self.file.ext_cache.borrow_mut() = None;
        self.edits += 1;
        // formulas that read other workbooks are worked out again
        let mut keys = Vec::new();
        for (s, sh) in self.sheets.iter().enumerate() {
            for ((r, c), cell) in sh.cells.iter() {
                if let Some(f) = &cell.f {
                    if f.text.contains('[') && has_ext(&f.text) {
                        keys.push((s, *r, *c));
                    }
                }
            }
        }
        for k in keys {
            self.forced.insert(k);
        }
        self.rebuild = true;
        self.pending = true;
    }

    /// Point link n at another workbook (Change Source) and read its values from `src`
    /// (the source opened by the caller, when it could be).
    pub fn ext_set_source(&mut self, n: usize, target: &str) -> Result<(), String> {
        let books = self.ext_books();
        let eb = books.iter().find(|b| b.index == n).ok_or("no such link")?;
        let rn = rels_name(&eb.part);
        let rid = {
            let d = part_bytes(self, &eb.part).ok_or("the link's part is missing")?;
            let (_, root) = xml::parse_bytes(d)?;
            root.child("externalBook").and_then(|b| b.attrs.iter().find(|(k, _)| k.ends_with(":id")).map(|(_, v)| v.clone()))
                .ok_or("the link names no workbook")?
        };
        let (pro, mut rr) = match part_bytes(self, &rn) {
            Some(d) => xml::parse_bytes(d)?,
            None => return Err("the link's relationships are missing".into()),
        };
        for n in rr.children.iter_mut() {
            if let Node::Elem(e) = n {
                if e.attr("Id") == Some(rid.as_str()) {
                    e.set_attr("Target", target);
                    e.set_attr("TargetMode", "External");
                }
            }
        }
        let bytes = xml::document(&pro, &rr).into_bytes();
        if let Some(p) = self.file.parts.iter_mut().find(|(k, _)| *k == rn) {
            p.1 = bytes;
        }
        self.ext_changed();
        Ok(())
    }

    /// Update Values: the kept values of link n from the source workbook.
    pub fn ext_update_from(&mut self, n: usize, src: &Book) -> Result<usize, String> {
        let books = self.ext_books();
        let eb = books.iter().find(|b| b.index == n).ok_or("no such link")?;
        let d = part_bytes(self, &eb.part).ok_or("the link's part is missing")?;
        let (pro, mut root) = xml::parse_bytes(d)?;
        let mut updated = 0;
        if let Some(book) = root.child_mut("externalBook") {
            if let Some(ds) = book.child_mut("sheetDataSet") {
                for sd in ds.children.iter_mut() {
                    let sd = match sd {
                        Node::Elem(e) => e,
                        _ => continue,
                    };
                    let sid: usize = sd.attr("sheetId").and_then(|x| x.parse().ok()).unwrap_or(0);
                    let si = eb.sheets.get(sid).and_then(|nm| src.sheet_index(nm));
                    for row in sd.children.iter_mut() {
                        let row = match row {
                            Node::Elem(e) => e,
                            _ => continue,
                        };
                        for c in row.children.iter_mut() {
                            let c = match c {
                                Node::Elem(e) => e,
                                _ => continue,
                            };
                            let (r, cc) = match c.attr("r").and_then(crate::cellref::parse_cell) {
                                Some(x) => x,
                                None => continue,
                            };
                            let v = match si {
                                Some(s) => src.sheets[s].value(r, cc),
                                None => Value::Err(XlErr::Ref),
                            };
                            let pfx = match c.name.find(':') {
                                Some(i) => c.name[..=i].to_string(),
                                None => String::new(),
                            };
                            c.children.clear();
                            c.remove_attr("t");
                            let (t, text) = match &v {
                                Value::Num(x) => (None, crate::xlsx::num_text(*x)),
                                Value::Str(s) => (Some("str"), s.to_string()),
                                Value::Bool(b) => (Some("b"), if *b { "1".into() } else { "0".into() }),
                                Value::Err(e) => (Some("e"), e.text().to_string()),
                                _ => (None, String::new()),
                            };
                            if let Some(t) = t {
                                c.set_attr("t", t);
                            }
                            if !text.is_empty() {
                                let mut ve = Element::new(&format!("{}v", pfx));
                                ve.children.push(Node::Text(text));
                                c.children.push(Node::Elem(ve));
                            }
                            updated += 1;
                        }
                    }
                }
            }
        }
        let bytes = xml::document(&pro, &root).into_bytes();
        let part = eb.part.clone();
        if let Some(p) = self.file.parts.iter_mut().find(|(k, _)| *k == part) {
            p.1 = bytes;
        }
        self.ext_changed();
        Ok(updated)
    }

    /// Break Link: formulas that read workbook n become their values, and the link goes.
    /// One undo step for the cells (the link itself can't come back, as in the original).
    pub fn ext_break(&mut self, n: usize) -> Result<usize, String> {
        let books = self.ext_books();
        let eb = books.iter().find(|b| b.index == n).ok_or("no such link")?.clone();
        let tag = format!("[{}]", n);
        self.begin("Break Link", self.active);
        let mut count = 0;
        for s in 0..self.sheets.len() {
            let mut edits = Vec::new();
            for ((r, c), cell) in self.sheets[s].cells.iter() {
                if let Some(f) = &cell.f {
                    if f.text.contains(&tag) && has_ext(&f.text) {
                        let mut nc = cell.clone();
                        nc.f = None;
                        edits.push((*r, *c, nc));
                    }
                }
            }
            for (r, c, nc) in edits {
                count += 1;
                self.record(Delta::Cell(s, r, c, Some(nc)));
            }
        }
        // names that point there: their values
        let mut names = self.names.clone();
        let mut nchanged = false;
        for nm in names.iter_mut() {
            if nm.text.contains(&tag) {
                if let Some(v) = self.ext_value(&nm.text) {
                    nm.text = match v {
                        Value::Num(x) => crate::xlsx::num_text(x),
                        Value::Str(s) => format!("\"{}\"", s.replace('"', "\"\"")),
                        Value::Bool(b) => if b { "TRUE".into() } else { "FALSE".into() },
                        _ => "#REF!".into(),
                    };
                } else {
                    nm.text = "#REF!".into();
                }
                nm.expr = crate::formula::parse(&nm.text).ok().map(Rc::new);
                nchanged = true;
            }
        }
        if nchanged {
            self.record(Delta::Names(names));
        }
        self.commit();
        // the link's parts go; the others keep their numbers in the file only if they follow it
        let mut gone = vec![eb.part.clone(), rels_name(&eb.part)];
        self.file.parts.retain(|(k, _)| !gone.contains(k));
        if let Some((_, rels)) = &mut self.file.wb_rels {
            rels.children.retain(|x| !matches!(x, Node::Elem(e) if e.attr("Id") == Some(eb.rid.as_str())));
        }
        if let Some((_, ct)) = &mut self.file.content_types {
            remove_override(ct, &eb.part);
        }
        if let Some((_, wb)) = &mut self.file.wb {
            if let Some(er) = wb.child_mut("externalReferences") {
                er.children.retain(|x| !matches!(x, Node::Elem(e) if e.attrs.iter().any(|(k, v)| k.ends_with(":id") && *v == eb.rid)));
                if er.elems().next().is_none() {
                    wb.remove_children("externalReferences");
                }
            }
        }
        // the later links move up a number: their formulas follow
        let later: Vec<usize> = books.iter().map(|b| b.index).filter(|&i| i > n).collect();
        if !later.is_empty() {
            for s in 0..self.sheets.len() {
                let keys: Vec<(u32, u32)> = self.sheets[s].cells.iter().filter(|(_, c)| c.f.as_ref().map(|f| has_ext(&f.text)).unwrap_or(false)).map(|(k, _)| *k).collect();
                for k in keys {
                    let cell = self.sheets[s].cells.get_mut(&k).unwrap();
                    if let Some(f) = &cell.f {
                        let mut t = f.text.clone();
                        for &i in &later {
                            t = t.replace(&format!("[{}]", i), &format!("[{}]", i - 1));
                        }
                        let mut nf = (**f).clone();
                        nf.text = t;
                        nf.expr = crate::formula::parse(&nf.text).ok();
                        cell.f = Some(Rc::new(nf));
                    }
                }
            }
        }
        gone.clear();
        self.ext_changed();
        Ok(count)
    }
}

/// Whether formula text has an external reference (a '[' outside strings and structured refs).
pub fn has_ext(text: &str) -> bool {
    match crate::formula::tokenize(text) {
        Ok(ts) => ts.iter().any(|t| t.kind == crate::formula::TokKind::Ext),
        Err(_) => false,
    }
}
