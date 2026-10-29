//! Working across open workbooks: a sheet copied or moved into another workbook (S3-01), Paste
//! Link from another workbook (S3-03), and the external-link parts both need.
use crate::book::{Book, Delta};
use crate::cellref::Rect;
use crate::extlinks::T_EXTLINK;
use crate::formula::{self, Adjust};
use crate::sheet::{Cell, Piece};
use crate::value::Value;
use crate::xlsx::{rels_name, relative};
use crate::xml::{self, Element, Node};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

const T_EXTPATH: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLinkPath";
const CT_EXTLINK: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.externalLink+xml";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WB_ORDER: [&str; 19] = ["fileVersion", "fileSharing", "workbookPr", "workbookProtection", "bookViews", "sheets",
    "functionGroups", "externalReferences", "definedNames", "calcPr", "oleSize", "customWorkbookViews", "pivotCaches",
    "smartTagPr", "smartTagTypes", "webPublishing", "fileRecoveryPr", "webPublishObjects", "extLst"];

/// The name a formula shows for a workbook path: "Book2.xlsx" for /home/x/Book2.xlsx.
pub fn book_label(target: &str) -> String {
    let t = target.trim_start_matches("file://");
    t.rsplit(['/', '\\']).next().unwrap_or(t).to_string()
}

fn quote_sheet(name: &str) -> String {
    let plain = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') && !name.chars().next().unwrap().is_ascii_digit();
    if plain { name.to_string() } else { format!("'{}'", name.replace('\'', "''")) }
}

impl Book {
    fn ensure_wb_parts(&mut self) {
        if self.file.wb_part.is_empty() {
            self.file.wb_part = "xl/workbook.xml".into();
        }
        if self.file.wb.is_none() {
            let src = format!("<workbook xmlns=\"{}\" xmlns:r=\"{}\"><bookViews><workbookView/></bookViews><sheets/><calcPr calcId=\"191029\"/></workbook>", NS_MAIN, NS_REL);
            let (p, e) = xml::parse(&src).unwrap();
            self.file.wb = Some((p, e));
        }
        if self.file.wb_rels.is_none() {
            self.file.wb_rels = Some(("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n".into(),
                Element::with_attrs("Relationships", &[("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")])));
        }
        if self.file.content_types.is_none() {
            self.file.content_types = Some(crate::xlsx::default_content_types());
        }
    }

    /// The link number ([n] in formulas) for workbook `target`, made when there is none yet,
    /// holding the kept values of `cells` (sheet name, range) read from `src`.
    pub fn ext_link_for(&mut self, target: &str, src: &Book, cells: &[(String, Rect)]) -> Result<usize, String> {
        let books = self.ext_books();
        let label = book_label(target);
        let found = books.iter().find(|b| b.target == target || book_label(&b.target).eq_ignore_ascii_case(&label)).map(|b| (b.index, b.part.clone()));
        let (n, part) = match found {
            Some(x) => x,
            None => self.new_ext_link(target)?,
        };
        // the sheets and cells the link keeps
        let d = self.file.parts.iter().find(|(k, _)| *k == part).map(|(_, d)| d.clone()).ok_or("the link's part is missing")?;
        let (pro, mut root) = xml::parse_bytes(&d)?;
        let pfx = match root.name.find(':') {
            Some(i) => root.name[..=i].to_string(),
            None => String::new(),
        };
        {
            let book = root.child_mut("externalBook").ok_or("the link names no workbook")?;
            let mut names: Vec<String> = book.child("sheetNames").map(|sn| sn.elems().filter_map(|e| e.attr("val").map(|s| s.to_string())).collect()).unwrap_or_default();
            // (sheet index) -> row -> cols
            let mut want: BTreeMap<usize, BTreeMap<u32, BTreeSet<u32>>> = BTreeMap::new();
            if let Some(ds) = book.child("sheetDataSet") {
                for sd in ds.elems() {
                    let sid: usize = sd.attr("sheetId").and_then(|x| x.parse().ok()).unwrap_or(0);
                    for row in sd.elems() {
                        for c in row.elems() {
                            if let Some((r, cc)) = c.attr("r").and_then(crate::cellref::parse_cell) {
                                want.entry(sid).or_default().entry(r).or_default().insert(cc);
                            }
                        }
                    }
                }
            }
            let mut count = 0usize;
            for (sn, rc) in cells {
                let si = match names.iter().position(|x| x.eq_ignore_ascii_case(sn)) {
                    Some(i) => i,
                    None => {
                        names.push(sn.clone());
                        names.len() - 1
                    }
                };
                let rc = match src.sheet_index(sn) {
                    Some(s) => src.sheets[s].clamp_rect(*rc),
                    None => *rc,
                };
                for r in rc.r1..=rc.r2 {
                    for c in rc.c1..=rc.c2 {
                        count += 1;
                        if count > 200_000 {
                            break;
                        }
                        want.entry(si).or_default().entry(r).or_default().insert(c);
                    }
                }
            }
            book.remove_children("sheetNames");
            book.remove_children("sheetDataSet");
            let mut sn = Element::new(&format!("{}sheetNames", pfx));
            for nm in &names {
                sn.push(Element::with_attrs(&format!("{}sheetName", pfx), &[("val", nm)]));
            }
            book.push(sn);
            let mut ds = Element::new(&format!("{}sheetDataSet", pfx));
            for si in 0..names.len() {
                let mut sd = Element::with_attrs(&format!("{}sheetData", pfx), &[("sheetId", &si.to_string())]);
                if let Some(rows) = want.get(&si) {
                    for (r, cs) in rows {
                        let mut row = Element::with_attrs(&format!("{}row", pfx), &[("r", &(r + 1).to_string())]);
                        for c in cs {
                            row.push(Element::with_attrs(&format!("{}cell", pfx), &[("r", &Rect::new(*r, *c, *r, *c).a1())]));
                        }
                        sd.push(row);
                    }
                }
                ds.push(sd);
            }
            book.push(ds);
        }
        let bytes = xml::document(&pro, &root).into_bytes();
        if let Some(p) = self.file.parts.iter_mut().find(|(k, _)| *k == part) {
            p.1 = bytes;
        }
        *self.file.ext_cache.borrow_mut() = None;
        self.ext_update_from(n, src)?;
        Ok(n)
    }

    fn new_ext_link(&mut self, target: &str) -> Result<(usize, String), String> {
        self.ensure_wb_parts();
        let dir = match self.file.wb_part.rfind('/') {
            Some(i) => self.file.wb_part[..i].to_string(),
            None => "xl".into(),
        };
        let mut k = 1;
        let part = loop {
            let p = format!("{}/externalLinks/externalLink{}.xml", dir, k);
            if !self.file.parts.iter().any(|(n, _)| *n == p) {
                break p;
            }
            k += 1;
        };
        let link = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<externalLink xmlns=\"{}\" xmlns:r=\"{}\"><externalBook r:id=\"rId1\"><sheetNames/><sheetDataSet/></externalBook></externalLink>", NS_MAIN, NS_REL);
        let rels = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{}\" Target=\"{}\" TargetMode=\"External\"/></Relationships>",
            T_EXTPATH, xml::escape_attr(target));
        self.file.parts.push((part.clone(), link.into_bytes()));
        self.file.parts.push((rels_name(&part), rels.into_bytes()));
        let wb_part = self.file.wb_part.clone();
        let rid = {
            let (_, r) = self.file.wb_rels.as_mut().unwrap();
            let used: Vec<String> = r.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
            let mut i = used.len() + 1;
            let id = loop {
                let id = format!("rId{}", i);
                if !used.contains(&id) {
                    break id;
                }
                i += 1;
            };
            r.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_EXTLINK), ("Target", &relative(&wb_part, &part))]));
            id
        };
        if let Some((_, ct)) = self.file.content_types.as_mut() {
            crate::xlsx::set_override(ct, &part, CT_EXTLINK);
        }
        let (_, wb) = self.file.wb.as_mut().unwrap();
        let pfx = match wb.name.find(':') {
            Some(i) => wb.name[..=i].to_string(),
            None => String::new(),
        };
        let rp = wb.attrs.iter().find(|(k, v)| k.starts_with("xmlns:") && v == NS_REL).map(|(k, _)| k[6..].to_string());
        let rp = match rp {
            Some(p) => p,
            None => {
                wb.attrs.push(("xmlns:r".into(), NS_REL.into()));
                "r".into()
            }
        };
        if wb.child("externalReferences").is_none() {
            wb.insert_ordered(Element::new(&format!("{}externalReferences", pfx)), &WB_ORDER);
        }
        let er = wb.child_mut("externalReferences").unwrap();
        er.push(Element::with_attrs(&format!("{}externalReference", pfx), &[(&format!("{}:id", rp), &rid)]));
        let n = er.elems().count();
        *self.file.ext_cache.borrow_mut() = None;
        self.edits += 1;
        Ok((n, part))
    }

    /// Paste Link from another workbook: each cell of dst gets =[n]Sheet!$A$1 for its cell of src.
    pub fn paste_link_from(&mut self, other: &Book, target: &str, src_s: usize, src: Rect, dst_s: usize, r0: u32, c0: u32) -> Result<Rect, String> {
        if src_s >= other.sheets.len() || dst_s >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        let src = other.sheets[src_s].clamp_rect(src);
        let src = match other.sheets[src_s].used_extent() {
            Some((er, ec)) if src.rows() > 10000 || src.cols() > 500 => Rect::new(src.r1, src.c1, src.r2.min(er.max(src.r1)), src.c2.min(ec.max(src.c1))),
            _ => src,
        };
        let sname = other.sheets[src_s].name.clone();
        let n = self.ext_link_for(target, other, &[(sname.clone(), src)])?;
        let out = Rect::new(r0, c0, r0 + src.rows() - 1, c0 + src.cols() - 1);
        self.begin("Paste Link", dst_s);
        for i in 0..src.rows() {
            for j in 0..src.cols() {
                let a = Rect::new(src.r1 + i, src.c1 + j, src.r1 + i, src.c1 + j).a1();
                let abs = format!("${}", a.trim_start_matches(|c: char| c.is_ascii_alphabetic()))
                    .to_string();
                let col: String = a.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                let text = ext_ref(n, &sname, &format!("${}{}", col, abs));
                let cur = self.sheets[dst_s].get(r0 + i, c0 + j).cloned();
                let mut cell = cur.unwrap_or_default();
                let f = crate::book::make_formula(&text, crate::sheet::FKind::Normal);
                cell.f = Some(Rc::new(f));
                cell.v = match other.sheets[src_s].value(src.r1 + i, src.c1 + j) {
                    Value::Empty => Value::Num(0.0),
                    v => v,
                };
                self.record(Delta::Cell(dst_s, r0 + i, c0 + j, Some(cell)));
            }
        }
        self.commit();
        Ok(out)
    }

    /// Move or Copy to another workbook: a copy of other's sheet `from` goes before `at`.
    /// Formats come along; formulas that read the source's other sheets read them through a
    /// link to the source workbook (`target`), as in the original.
    pub fn import_sheet(&mut self, other: &Book, from: usize, at: usize, target: &str) -> Result<usize, String> {
        let src = other.sheets.get(from).ok_or("no such sheet")?;
        if src.raw_part.is_some() || src.kind != "work" {
            return Err("Only worksheets can be copied to another workbook in this version.".into());
        }
        let mut sh = src.clone();
        // a name of its own here
        let base = src.name.clone();
        let mut name = base.clone();
        let mut k = 2;
        while self.sheet_index(&name).is_some() {
            name = format!("{} ({})", base, k);
            k += 1;
        }
        sh.name = name.clone();
        sh.id = self.sheets.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        sh.part = String::new();
        sh.comments_file = None;
        sh.error = None;
        sh.drawing = None;
        sh.background = sh.background.clone();
        sh.bg_part = None;
        sh.pieces.retain(|p| !matches!(p, Piece::Raw(t, x) if crate::sheet_copy::needs_rels(t, x)));
        sh.objects.retain(|o| o.slicer.is_none());
        // formats: the source's styles made here
        let mut memo: HashMap<u32, u32> = HashMap::new();
        let mut map = |me: &mut Book, xf: u32| -> u32 {
            if xf == 0 {
                return 0;
            }
            *memo.entry(xf).or_insert_with(|| me.styles.derive(0, &crate::book_clip::import_change(&other.styles, xf)))
        };
        let keys: Vec<(u32, u32)> = sh.cells.keys().copied().collect();
        for key in keys {
            let s0 = sh.cells[&key].s;
            let s1 = map(self, s0);
            sh.cells.get_mut(&key).unwrap().s = s1;
        }
        let rkeys: Vec<u32> = sh.rows.keys().copied().collect();
        for r in rkeys {
            if let Some(st) = sh.rows[&r].style {
                let n = map(self, st);
                sh.rows.get_mut(&r).unwrap().style = Some(n);
            }
        }
        for i in 0..sh.cols.len() {
            let st = sh.cols[i].style;
            sh.cols[i].style = map(self, st);
        }
        // conditional formats: their formats (dxfs) made here
        let mut dmemo: HashMap<u32, u32> = HashMap::new();
        for p in sh.pieces.iter_mut() {
            if let Piece::Raw(t, x) = p {
                if t == "conditionalFormatting" && x.contains("dxfId") {
                    if let Ok((_, mut e)) = xml::parse(x) {
                        for rule in e.elems_mut() {
                            if let Some(id) = rule.attr("dxfId").and_then(|v| v.parse::<u32>().ok()) {
                                let nid = match dmemo.get(&id) {
                                    Some(v) => *v,
                                    None => {
                                        let v = self.copy_dxf_from(other, id);
                                        dmemo.insert(id, v);
                                        v
                                    }
                                };
                                rule.set_attr("dxfId", &nid.to_string());
                            }
                        }
                        *x = e.to_string();
                    }
                }
            }
        }
        // tables: names of their own
        let mut taken = self.table_names();
        let mut next_id = self.next_table_id();
        let mut renames: Vec<(String, String)> = Vec::new();
        for t in sh.tables.iter_mut() {
            if taken.contains(&t.name.to_lowercase()) || self.find_name(&t.name, 0).is_some() {
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
                renames.push((t.name.clone(), nm.clone()));
                t.name = nm;
            }
            taken.push(t.name.to_lowercase());
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
        // formulas: the sheet's own name follows the copy; the source's other sheets become links
        let others: Vec<String> = other.sheets.iter().enumerate().filter(|(i, _)| *i != from).map(|(_, s)| s.name.clone()).collect();
        let mut refs: Vec<(String, Rect)> = Vec::new();
        let mut texts: Vec<((u32, u32), String)> = Vec::new();
        let mut need_link = false;
        for ((r, c), cell) in sh.cells.iter() {
            if let Some(f) = &cell.f {
                let mut t = f.text.clone();
                if name != base {
                    t = formula::adjust(&t, &Adjust::RenameSheet(&base, &name));
                }
                for o in &others {
                    if formula_names_sheet(&t, o) {
                        need_link = true;
                        for rc in sheet_refs(&t, o) {
                            refs.push((o.clone(), rc));
                        }
                    }
                }
                if !renames.is_empty() {
                    t = crate::tables::rewrite_structs(&t, |sr| {
                        let tn = sr.table.as_deref()?;
                        let (_, nn) = renames.iter().find(|(o, _)| o.eq_ignore_ascii_case(tn))?;
                        let mut x = sr.clone();
                        x.table = Some(nn.clone());
                        Some(crate::tables::write_struct(&x, None, false))
                    });
                }
                texts.push(((*r, *c), t));
            }
        }
        let link_n = if need_link { Some(self.ext_link_for(target, other, &refs)?) } else { None };
        for (key, mut t) in texts {
            if let Some(n) = link_n {
                for o in &others {
                    if formula_names_sheet(&t, o) {
                        t = to_ext(&t, o, n);
                    }
                }
            }
            let cell = sh.cells.get_mut(&key).unwrap();
            if let Some(f) = &cell.f {
                if f.text != t {
                    let mut nf = (**f).clone();
                    nf.text = t;
                    nf.expr = formula::parse(&nf.text).ok();
                    cell.f = Some(Rc::new(nf));
                }
            }
        }
        for o in sh.objects.iter_mut() {
            if name != base {
                let how = Adjust::RenameSheet(&base, &name);
                if let Some(ch) = &o.chart {
                    let mut c = (**ch).clone();
                    if c.adjust(&how) {
                        o.chart = Some(Rc::new(c));
                    }
                }
            }
        }
        let at = at.min(self.sheets.len());
        self.begin("Copy Sheet", at);
        self.record(Delta::AddSheet(at, Box::new(sh)));
        self.commit();
        self.active = at;
        self.rebuild = true;
        self.pending = true;
        Ok(at)
    }

    /// A dxf given as XML, added to the styles (the same one again is reused); its index.
    pub fn add_raw_dxf(&mut self, dxf_xml: &str) -> u32 {
        let mut tmp = Book::new_empty();
        if let Ok((_, e)) = xml::parse(dxf_xml) {
            let mut list = Element::with_attrs("dxfs", &[("count", "1")]);
            list.push(e);
            tmp.styles.root.remove_children("dxfs");
            tmp.styles.root.insert_ordered(list, &crate::styles::STYLE_ORDER);
            return self.copy_dxf_from(&tmp, 0);
        }
        0
    }

    /// A dxf of `other` made in this workbook's styles; its index here.
    pub fn copy_dxf_from(&mut self, other: &Book, id: u32) -> u32 {
        let el = match other.styles.root.child("dxfs").and_then(|d| d.elems().nth(id as usize)) {
            Some(e) => e.clone(),
            None => return 0,
        };
        let root = &mut self.styles.root;
        if root.child("dxfs").is_none() {
            root.insert_ordered(Element::with_attrs("dxfs", &[("count", "0")]), &crate::styles::STYLE_ORDER);
        }
        let list = root.child_mut("dxfs").unwrap();
        let mut el = el;
        let tag = if list.name.contains(':') { format!("{}:dxf", list.name.split(':').next().unwrap()) } else { "dxf".to_string() };
        el.name = tag;
        let s = el.to_string();
        if let Some(i) = list.elems().position(|e| e.to_string() == s) {
            return i as u32;
        }
        list.push(el);
        let n = list.elems().count();
        list.set_attr("count", &n.to_string());
        self.styles.dirty = true;
        (n - 1) as u32
    }
}

/// Whether formula text refers to sheet `name` (Sheet!A1 or 'Sheet name'!A1).
fn formula_names_sheet(text: &str, name: &str) -> bool {
    !sheet_refs_raw(text, name).is_empty()
}

fn sheet_refs_raw(text: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let toks = match formula::tokenize(text) {
        Ok(t) => t,
        Err(_) => return out,
    };
    for t in toks {
        let s = &text[t.start..t.end];
        if let Some(i) = s.rfind('!') {
            let sheet = s[..i].trim_matches('\'').replace("''", "'");
            let sheet = sheet.rsplit(':').next().unwrap_or(&sheet).to_string();
            if sheet.eq_ignore_ascii_case(name) && !s.starts_with('[') && !s.starts_with("'[") {
                out.push(s[i + 1..].to_string());
            }
        }
    }
    out
}

fn sheet_refs(text: &str, name: &str) -> Vec<Rect> {
    sheet_refs_raw(text, name).iter().filter_map(|a| crate::cellref::parse_range(&a.replace('$', ""))).collect()
}

/// A reference to link n's sheet: [1]Sheet1!$A$1, or '[1]Sheet name'!$A$1.
pub fn ext_ref(n: usize, sheet: &str, addr: &str) -> String {
    let q = quote_sheet(sheet);
    if q.starts_with('\'') {
        format!("'[{}]{}'!{}", n, sheet.replace('\'', "''"), addr)
    } else {
        format!("[{}]{}!{}", n, sheet, addr)
    }
}

/// Formula text whose references to sheet `name` read link n's sheet of that name instead.
fn to_ext(text: &str, name: &str, n: usize) -> String {
    let toks = match formula::tokenize(text) {
        Ok(t) => t,
        Err(_) => return text.to_string(),
    };
    let mut out = String::new();
    let mut last = 0;
    for t in toks {
        let s = &text[t.start..t.end];
        if let Some(i) = s.rfind('!') {
            if s.starts_with('[') || s.starts_with("'[") {
                continue;
            }
            let sheet = s[..i].trim_matches('\'').replace("''", "'");
            if sheet.eq_ignore_ascii_case(name) {
                out.push_str(&text[last..t.start]);
                out.push_str(&ext_ref(n, &sheet, &s[i + 1..]));
                last = t.end;
            }
        }
    }
    out.push_str(&text[last..]);
    out
}

#[allow(dead_code)]
fn unused(_: Cell, _: Node) {}
