//! Reading and writing .xlsx packages. Parts the engine doesn't model are kept byte for
//! byte; worksheets keep every element they don't rewrite (hyperlinks, conditional
//! formats, drawings...) exactly as read.

use crate::book::{Book, Name};
use crate::cellref::{col_index, col_name, parse_range, Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, Adjust};
use crate::sheet::{Cell, ColSpan, FKind, Formula, Piece, RowInfo, Sheet, View};
use crate::styles::Styles;
use crate::value::{Value, XlErr};
use crate::xml::{self, for_each_attr, raw_attr, unescape, unescape_ooxml, Element, Node, Tok, Tokenizer};
use crate::zip::{ZipReader, ZipWriter};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const T_WORKSHEET: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const T_SST: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
const T_STYLES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const T_META: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata";
const T_DRAWING: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
const T_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const T_TABLE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
const CT_TABLE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml";
const CT_DRAWING: &str = "application/vnd.openxmlformats-officedocument.drawing+xml";
const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const CT_WORKSHEET: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CT_SST: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
const CT_META: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml";
const CT_STYLES: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";

const WS_ORDER: [&str; 39] = ["sheetPr", "dimension", "sheetViews", "sheetFormatPr", "cols", "sheetData", "sheetCalcPr",
    "sheetProtection", "protectedRanges", "scenarios", "autoFilter", "sortState", "dataConsolidate", "customSheetViews",
    "mergeCells", "phoneticPr", "conditionalFormatting", "dataValidations", "hyperlinks", "printOptions", "pageMargins",
    "pageSetup", "headerFooter", "rowBreaks", "colBreaks", "customProperties", "cellWatches", "ignoredErrors", "smartTags",
    "drawing", "legacyDrawing", "legacyDrawingHF", "drawingHF", "picture", "oleObjects", "controls", "webPublishItems",
    "tableParts", "extLst"];

const METADATA_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<metadata xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:xda=\"http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray\"><metadataTypes count=\"1\"><metadataType name=\"XLDAPR\" minSupportedVersion=\"120000\" copy=\"1\" pasteAll=\"1\" pasteValues=\"1\" merge=\"1\" splitFirst=\"1\" rowColShift=\"1\" clearFormats=\"1\" clearComments=\"1\" assign=\"1\" coerce=\"1\" cellMeta=\"1\"/></metadataTypes><futureMetadata name=\"XLDAPR\" count=\"1\"><bk><extLst><ext uri=\"{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}\"><xda:dynamicArrayProperties fDynamic=\"1\" fCollapsed=\"0\"/></ext></extLst></bk></futureMetadata><cellMetadata count=\"1\"><bk><rc t=\"1\" v=\"0\"/></bk></cellMetadata></metadata>";

/// What the package holds besides the modelled parts.
#[derive(Default)]
pub struct FileParts {
    pub path: Option<String>,
    /// every part as read (name, bytes), in order
    pub parts: Vec<(String, Vec<u8>)>,
    pub content_types: Option<(String, Element)>,
    pub wb_part: String,
    pub wb: Option<(String, Element)>,
    pub wb_rels: Option<(String, Element)>,
    pub styles_part: String,
    pub sst_part: Option<String>,
    pub meta_part: Option<String>,
    /// the cm index that marks dynamic array formulas
    pub dyn_cm: Option<u32>,
    /// worksheet part -> relationship id
    pub sheet_rids: HashMap<String, String>,
    /// the persons part (threaded comments' authors)
    pub persons_part: Option<String>,
}

fn rels_name(part: &str) -> String {
    match part.rfind('/') {
        Some(i) => format!("{}/_rels/{}.rels", &part[..i], &part[i + 1..]),
        None => format!("_rels/{}.rels", part),
    }
}

fn resolve(base_part: &str, target: &str) -> String {
    if let Some(t) = target.strip_prefix('/') {
        return t.to_string();
    }
    let dir = match base_part.rfind('/') {
        Some(i) => &base_part[..i],
        None => "",
    };
    let mut parts: Vec<&str> = if dir.is_empty() { vec![] } else { dir.split('/').collect() };
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn relative(base_part: &str, target: &str) -> String {
    let dir: Vec<&str> = match base_part.rfind('/') {
        Some(i) => base_part[..i].split('/').collect(),
        None => vec![],
    };
    let tg: Vec<&str> = target.split('/').collect();
    let mut k = 0;
    while k < dir.len() && k + 1 < tg.len() && dir[k] == tg[k] {
        k += 1;
    }
    if k == 0 && !dir.is_empty() {
        return format!("/{}", target);
    }
    let mut parts: Vec<&str> = vec![".."; dir.len() - k];
    parts.extend_from_slice(&tg[k..]);
    parts.join("/")
}

/// Text escaped for OOXML: control characters as _xHHHH_.
fn esc_ooxml(t: &str, out: &mut String) {
    for ch in t.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\t' | '\n' => out.push(ch),
            '\r' => out.push_str("_x000D_"),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => out.push_str(&format!("_x{:04X}_", c as u32)),
            _ => out.push(ch),
        }
    }
}

fn esc_attr(t: &str) -> String {
    xml::escape_attr(t)
}

pub fn num_text(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    let a = x.abs();
    if !(1e-7..1e21).contains(&a) {
        let s = format!("{:E}", x);
        return s;
    }
    format!("{}", x)
}

fn parse_ref_cell(s: &str) -> Option<(u32, u32)> {
    crate::cellref::parse_cell(s)
}

// --------------------------------------------------------------------------- reading

impl Book {
    pub fn open(path: &str) -> Result<Book, String> {
        let data = std::fs::read(path).map_err(|e| format!("Could not open {}: {}", path, e))?;
        let mut b = Book::from_bytes(&data)?;
        b.file.path = Some(path.to_string());
        Ok(b)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Book, String> {
        let z = ZipReader::open(data).map_err(|_| "This file isn't a workbook this program can open (not an .xlsx package).".to_string())?;
        let mut b = Book::new_empty();
        b.sheets.clear();
        let mut parts = Vec::new();
        for n in z.names() {
            if n.ends_with('/') {
                continue;
            }
            let d = z.read(&n)?;
            parts.push((n, d));
        }
        let get = |name: &str| -> Option<&Vec<u8>> { parts.iter().find(|(n, _)| n == name).map(|(_, d)| d) };
        if let Some(ct) = get("[Content_Types].xml") {
            b.file.content_types = Some(xml::parse_bytes(ct)?);
        }
        // the workbook part
        let mut wb_part = "xl/workbook.xml".to_string();
        if let Some(r) = get("_rels/.rels") {
            let (_, rels) = xml::parse_bytes(r)?;
            for rel in rels.elems() {
                if rel.attr("Type").map(|t| t.ends_with("/officeDocument")).unwrap_or(false) {
                    if let Some(t) = rel.attr("Target") {
                        wb_part = resolve("", t);
                    }
                }
            }
        }
        let wb_bytes = get(&wb_part).ok_or("This file has no workbook part.")?;
        let (wb_prolog, wb) = xml::parse_bytes(wb_bytes)?;
        let wb_rels_name = rels_name(&wb_part);
        let mut targets: HashMap<String, (String, String)> = HashMap::new(); // rId -> (type, part)
        if let Some(r) = get(&wb_rels_name) {
            let (pro, rels) = xml::parse_bytes(r)?;
            for rel in rels.elems() {
                if let (Some(id), Some(t)) = (rel.attr("Id"), rel.attr("Target")) {
                    if rel.attr("TargetMode") == Some("External") {
                        continue;
                    }
                    targets.insert(id.to_string(), (rel.attr("Type").unwrap_or("").to_string(), resolve(&wb_part, t)));
                }
            }
            b.file.wb_rels = Some((pro, rels));
        }
        let find_type = |suffix: &str| -> Option<String> {
            targets.values().find(|(t, _)| t.ends_with(suffix)).map(|(_, p)| p.clone())
        };
        // styles and theme
        b.file.styles_part = find_type("/styles").unwrap_or_else(|| "xl/styles.xml".into());
        let st_text = get(&b.file.styles_part).map(|d| xml::decode(d)).transpose()?;
        b.styles = Styles::parse(st_text.as_deref())?;
        if let Some(t) = find_type("/theme").and_then(|p| get(&p)) {
            b.styles.set_theme(&xml::decode(t)?);
        }
        // shared strings
        let mut sst: Vec<(Rc<str>, Option<Rc<str>>)> = Vec::new();
        if let Some(p) = find_type("/sharedStrings") {
            if let Some(d) = get(&p) {
                sst = read_sst(&xml::decode(d)?)?;
            }
            b.file.sst_part = Some(p);
        }
        // dynamic array metadata
        if let Some(p) = find_type("/sheetMetadata") {
            if let Some(d) = get(&p) {
                b.file.dyn_cm = dynamic_cm(&xml::decode(d)?);
            }
            b.file.meta_part = Some(p);
        }
        // workbook settings
        if let Some(pr) = wb.child("workbookPr") {
            b.date1904 = matches!(pr.attr("date1904"), Some("1") | Some("true"));
        }
        if let Some(c) = wb.child("calcPr") {
            b.calc.manual = c.attr("calcMode") == Some("manual");
            b.calc.iterate = matches!(c.attr("iterate"), Some("1") | Some("true"));
            if let Some(n) = c.attr("iterateCount").and_then(|v| v.parse().ok()) {
                b.calc.count = n;
            }
            if let Some(d) = c.attr("iterateDelta").and_then(|v| v.parse().ok()) {
                b.calc.delta = d;
            }
        }
        // the people who wrote threaded comments
        let persons = find_type("/person").and_then(|p| get(&p).map(|d| xml::decode(d).unwrap_or_default()))
            .map(|t| crate::notes::read_persons(&t)).unwrap_or_default();
        b.file.persons_part = find_type("/person");
        let active_tab: usize = wb.child("bookViews").and_then(|v| v.elems().next()).and_then(|v| v.attr("activeTab")).and_then(|v| v.parse().ok()).unwrap_or(0);
        // sheets
        let sheets_el = wb.child("sheets").cloned().unwrap_or_else(|| Element::new("sheets"));
        for (i, s) in sheets_el.elems().enumerate() {
            let name = s.attr("name").unwrap_or("Sheet").to_string();
            let id: u32 = s.attr("sheetId").and_then(|v| v.parse().ok()).unwrap_or(i as u32 + 1);
            let rid = s.attrs.iter().find(|(k, _)| xml::local(k) == "id" && k.contains(':')).map(|(_, v)| v.clone()).unwrap_or_default();
            let (typ, part) = targets.get(&rid).cloned().unwrap_or_default();
            let mut sh = Sheet::new(id, &name);
            sh.state = s.attr("state").unwrap_or("visible").to_string();
            sh.part = part.clone();
            b.file.sheet_rids.insert(part.clone(), rid.clone());
            let data = get(&part);
            if typ == T_WORKSHEET || typ.ends_with("/worksheet") {
                match data {
                    Some(d) => {
                        let text = xml::decode(d)?;
                        if let Err(e) = read_sheet(&mut sh, &text, &sst, &b.styles, b.file.dyn_cm) {
                            sh.error = Some(e);
                            sh.raw_part = Some(d.clone());
                        } else {
                            sh.fit_to_page = sh.raw_sheetpr.as_ref().and_then(|e| e.child("pageSetUpPr")).map(|p| matches!(p.attr("fitToPage"), Some("1") | Some("true"))).unwrap_or(false);
                            load_drawing(&mut sh, &parts, &b.styles.theme);
                            load_tables(&mut sh, &parts);
                            load_comments(&mut sh, &parts, &persons);
                            load_links(&mut sh, &parts);
                        }
                    }
                    None => sh.error = Some("the sheet's part is missing".into()),
                }
            } else {
                sh.kind = if typ.ends_with("/chartsheet") {
                    "chart".into()
                } else if typ.ends_with("/dialogsheet") {
                    "dialog".into()
                } else if typ.ends_with("/xlMacrosheet") || typ.ends_with("/xlIntlMacrosheet") {
                    "macro".into()
                } else {
                    "other".into()
                };
                sh.raw_part = data.cloned();
            }
            b.sheets.push(sh);
        }
        if b.sheets.is_empty() {
            return Err("This workbook has no sheets.".into());
        }
        // defined names
        if let Some(dn) = wb.child("definedNames") {
            for d in dn.elems() {
                let name = d.attr("name").unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let scope = d.attr("localSheetId").and_then(|v| v.parse::<usize>().ok());
                let text = formula::from_file_text(&d.text());
                let mut n = Name::new(&name, scope, &text);
                n.hidden = matches!(d.attr("hidden"), Some("1") | Some("true"));
                n.comment = d.attr("comment").map(|s| s.to_string());
                n.extra = d.attrs.iter().filter(|(k, _)| !matches!(k.as_str(), "name" | "localSheetId" | "hidden" | "comment")).cloned().collect();
                b.names.push(n);
            }
        }
        b.active = active_tab.min(b.sheets.len() - 1);
        b.file.wb_part = wb_part;
        b.file.wb = Some((wb_prolog, wb));
        b.file.parts = parts;
        // calculate everything once (files may hold stale or no values)
        b.extents.borrow_mut().clear();
        b.deps.rebuild(&b.sheets, &b.names);
        let manual = b.calc.manual;
        b.calc.manual = false;
        b.recalc(true);
        b.calc.manual = manual;
        b.undo_stack.clear();
        b.redo_stack.clear();
        b.state = 0;
        b.saved = 0;
        Ok(b)
    }
}

/// A part's relationships: (id, type, target part or URL, external).
fn find_part<'a>(parts: &'a [(String, Vec<u8>)], name: &str) -> Option<&'a Vec<u8>> {
    parts.iter().find(|(n, _)| n == name).map(|(_, d)| d)
}

fn read_rels(part: &str, parts: &[(String, Vec<u8>)]) -> Vec<(String, String, String, bool)> {
    let mut out = Vec::new();
    if let Some(d) = find_part(parts, &rels_name(part)) {
        if let Ok((_, rels)) = xml::parse_bytes(d) {
            for r in rels.elems() {
                if let (Some(id), Some(t)) = (r.attr("Id"), r.attr("Target")) {
                    let ext = r.attr("TargetMode") == Some("External");
                    out.push((id.to_string(), r.attr("Type").unwrap_or("").to_string(), if ext { t.to_string() } else { resolve(part, t) }, ext));
                }
            }
        }
    }
    out
}

/// Hyperlinks to outside addresses carry the address while the book is open.
fn load_links(sh: &mut Sheet, parts: &[(String, Vec<u8>)]) {
    if !sh.pieces.iter().any(|p| matches!(p, Piece::Raw(t, _) if t == "hyperlinks")) {
        return;
    }
    let rels = read_rels(&sh.part, parts);
    for p in sh.pieces.iter_mut() {
        if let Piece::Raw(t, x) = p {
            if t == "hyperlinks" {
                *x = crate::links::resolve_on_load(x, &rels);
            }
        }
    }
}

/// Notes and threaded comments of a sheet.
fn load_comments(sh: &mut Sheet, parts: &[(String, Vec<u8>)], persons: &HashMap<String, String>) {
    let rels = read_rels(&sh.part, parts);
    let comments = rels.iter().find(|r| r.1.ends_with("/comments") && !r.3).map(|r| r.2.clone());
    let threads = rels.iter().find(|r| r.1.ends_with("/threadedComment") && !r.3).map(|r| r.2.clone());
    let vml = rels.iter().find(|r| r.1.ends_with("/vmlDrawing") && !r.3).map(|r| r.2.clone());
    if comments.is_none() && threads.is_none() {
        return;
    }
    let text = |p: &Option<String>| p.as_ref().and_then(|p| find_part(parts, p)).and_then(|d| xml::decode(d).ok());
    let vml_text = text(&vml);
    let mut cf = crate::notes::CommentsFile { comments: comments.clone(), vml: vml.clone(), vml_text: vml_text.clone(), threads: threads.clone(), ..Default::default() };
    if let Some(t) = text(&comments) {
        match crate::notes::read_comments(&t, vml_text.as_deref()) {
            Some((n, _)) => sh.notes = n,
            None => return,
        }
    }
    if let Some(t) = text(&threads) {
        sh.threads = crate::notes::read_threads(&t, persons);
        // a cell with a thread has only the thread
        for k in sh.threads.keys() {
            sh.notes.remove(k);
        }
    }
    cf.notes = sh.notes.clone();
    cf.thread_map = sh.threads.clone();
    sh.comments_file = Some(cf);
}

/// The sheet's tables, from its table parts (kept as read when one can't be read).
fn load_tables(sh: &mut Sheet, parts: &[(String, Vec<u8>)]) {
    let idx = match sh.pieces.iter().position(|p| matches!(p, Piece::Raw(t, _) if t == "tableParts")) {
        Some(i) => i,
        None => return,
    };
    let raw = match &sh.pieces[idx] {
        Piece::Raw(_, x) => x.clone(),
        _ => return,
    };
    let e = match xml::parse(&raw) {
        Ok((_, e)) => e,
        Err(_) => return,
    };
    let rels = read_rels(&sh.part, parts);
    let mut tables = Vec::new();
    for tp in e.elems() {
        let rid = match tp.attrs.iter().find(|(k, _)| xml::local(k) == "id" && k.contains(':')) {
            Some((_, v)) => v.clone(),
            None => return,
        };
        let target = match rels.iter().find(|r| r.0 == rid) {
            Some(r) => r.2.clone(),
            None => return,
        };
        let text = match find_part(parts, &target).and_then(|d| xml::decode(d).ok()) {
            Some(t) => t,
            None => return,
        };
        match crate::tables::read_table(&target, &text) {
            Some(t) => tables.push(t),
            None => return,
        }
    }
    sh.tables = tables;
    sh.pieces[idx] = Piece::Known("tableParts".into());
}

/// Charts and other objects of a sheet, from its drawing part (kept as read when it
/// can't be read).
fn load_drawing(sh: &mut Sheet, parts: &[(String, Vec<u8>)], theme: &[u32; 12]) {
    let get = |name: &str| find_part(parts, name);
    let idx = match sh.pieces.iter().position(|p| matches!(p, Piece::Raw(t, _) if t == "drawing")) {
        Some(i) => i,
        None => return,
    };
    let raw = match &sh.pieces[idx] {
        Piece::Raw(_, x) => x.clone(),
        _ => return,
    };
    let rid = match xml::parse(&raw) {
        Ok((_, e)) => match e.attrs.iter().find(|(k, _)| xml::local(k) == "id" && k.contains(':')) {
            Some((_, v)) => v.clone(),
            None => return,
        },
        Err(_) => return,
    };
    let rels = read_rels(&sh.part, parts);
    let target = match rels.iter().find(|r| r.0 == rid && !r.3) {
        Some(r) => r.2.clone(),
        None => return,
    };
    let text = match get(&target).and_then(|d| xml::decode(d).ok()) {
        Some(t) => t,
        None => return,
    };
    let drels = read_rels(&target, parts);
    let read = |p: &str| -> Option<String> { get(p).and_then(|d| xml::decode(d).ok()) };
    if let Ok((df, objs)) = crate::drawing::read_drawing(&target, &text, drels, &read, theme) {
        sh.objects = objs;
        sh.drawing = Some(df);
        sh.pieces[idx] = Piece::Known("drawing".into());
    }
}

fn read_sst(text: &str) -> Result<Vec<(Rc<str>, Option<Rc<str>>)>, String> {
    let mut out = Vec::new();
    let mut t = Tokenizer::new(text);
    loop {
        let pos = t.pos;
        match t.next_tok()? {
            None => break,
            Some(Tok::Start(name, _, self_close)) if xml::local(name) == "si" => {
                if self_close {
                    out.push((Rc::from(""), None));
                    continue;
                }
                let _ = pos;
                let inner_start = t.pos;
                let (plain, rich, end) = read_string_item(&mut t, name)?;
                let inner = &text[inner_start..end];
                out.push((Rc::from(plain.as_str()), if rich { Some(Rc::from(inner)) } else { None }));
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The text of an <si>/<is> (runs joined, phonetic runs skipped); whether it has
/// formatting runs; the position where its end tag starts.
fn read_string_item(t: &mut Tokenizer, outer: &str) -> Result<(String, bool, usize), String> {
    let mut plain = String::new();
    let mut rich = false;
    let mut in_t = false;
    let mut skip = 0;
    loop {
        let pos = t.pos;
        match t.next_tok()?.ok_or("unexpected end in a string")? {
            Tok::Start(n, _, sc) => {
                let l = xml::local(n);
                if l == "rPh" && !sc {
                    skip += 1;
                } else if l == "r" {
                    rich = true;
                } else if l == "t" && !sc && skip == 0 {
                    in_t = true;
                }
            }
            Tok::End(n) => {
                if n == outer {
                    return Ok((plain, rich, pos));
                }
                let l = xml::local(n);
                if l == "rPh" {
                    skip -= 1;
                } else if l == "t" {
                    in_t = false;
                }
            }
            Tok::Text(x) => {
                if in_t {
                    plain.push_str(&unescape_ooxml(&unescape(x)));
                }
            }
            Tok::Other(x) => {
                if in_t {
                    if let Some(c) = x.strip_prefix("<![CDATA[").and_then(|y| y.strip_suffix("]]>")) {
                        plain.push_str(c);
                    }
                }
            }
        }
    }
}

fn dynamic_cm(text: &str) -> Option<u32> {
    let (_, root) = xml::parse(text).ok()?;
    let types: Vec<String> = root.child("metadataTypes")?.elems().map(|e| e.attr("name").unwrap_or("").to_string()).collect();
    let k = types.iter().position(|n| n == "XLDAPR")? as u32 + 1;
    let cm = root.child("cellMetadata")?;
    for (i, bk) in cm.elems().enumerate() {
        if bk.elems().any(|rc| rc.attr("t").and_then(|v| v.parse::<u32>().ok()) == Some(k)) {
            return Some(i as u32 + 1);
        }
    }
    None
}

fn flag(v: Option<String>) -> bool {
    matches!(v.as_deref(), Some("1") | Some("true"))
}

fn read_sheet(sh: &mut Sheet, text: &str, sst: &[(Rc<str>, Option<Rc<str>>)], styles: &Styles, dyn_cm: Option<u32>) -> Result<(), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut t = Tokenizer::new(text);
    // prolog and the root start tag
    loop {
        let pos = t.pos;
        match t.next_tok()? {
            None => return Err("empty worksheet".into()),
            Some(Tok::Start(name, _, _)) => {
                sh.prolog = text[..pos].to_string();
                sh.root_open = text[pos..t.pos].to_string();
                sh.root_close = format!("</{}>", name);
                break;
            }
            Some(_) => {}
        }
    }
    let prefix = {
        let n = &sh.root_open[1..];
        let n = &n[..n.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(n.len())];
        match n.find(':') {
            Some(i) => n[..=i].to_string(),
            None => String::new(),
        }
    };
    let _ = prefix;
    loop {
        let pos = t.pos;
        let tok = match t.next_tok()? {
            None => break,
            Some(x) => x,
        };
        match tok {
            Tok::End(_) => break,
            Tok::Text(_) | Tok::Other(_) => {}
            Tok::Start(name, attrs, sc) => {
                let l = xml::local(name).to_string();
                if l == "sheetData" {
                    if !sc {
                        read_sheet_data(sh, &mut t, text, sst, dyn_cm)?;
                    }
                    sh.pieces.push(Piece::Known(l));
                    continue;
                }
                // the whole element as text
                let end = if sc { t.pos } else { skip_element(&mut t, name)? };
                let raw = &text[pos..end];
                let _ = attrs;
                match l.as_str() {
                    "sheetPr" => {
                        let (_, e) = xml::parse(raw)?;
                        if let Some(tc) = e.child("tabColor") {
                            let c = crate::styles::Color::from_el(tc);
                            sh.tab_color = match c {
                                crate::styles::Color::Auto => None,
                                c => Some(styles.rgb(&c, 0)),
                            };
                        }
                        sh.raw_sheetpr = Some(e);
                        sh.pieces.push(Piece::Known(l));
                    }
                    "dimension" => sh.pieces.push(Piece::Known(l)),
                    "sheetViews" => {
                        let (_, e) = xml::parse(raw)?;
                        read_view(&mut sh.view, &e);
                        sh.raw_views = Some(raw.to_string());
                        sh.pieces.push(Piece::Known(l));
                    }
                    "sheetFormatPr" => {
                        let (_, e) = xml::parse(raw)?;
                        if let Some(h) = e.attr("defaultRowHeight").and_then(|v| v.parse().ok()) {
                            sh.default_row_height = h;
                        }
                        sh.custom_row_height = matches!(e.attr("customHeight"), Some("1") | Some("true"));
                        if let Some(w) = e.attr("baseColWidth").and_then(|v| v.parse().ok()) {
                            sh.base_col_width = w;
                        }
                        sh.default_col_width = e.attr("defaultColWidth").and_then(|v| v.parse().ok());
                        sh.raw_format_pr = Some(e);
                        sh.pieces.push(Piece::Known(l));
                    }
                    "cols" => {
                        let (_, e) = xml::parse(raw)?;
                        for c in e.elems() {
                            let a = |k: &str| c.attr(k);
                            let min: u32 = a("min").and_then(|v| v.parse().ok()).unwrap_or(1);
                            let max: u32 = a("max").and_then(|v| v.parse().ok()).unwrap_or(min);
                            if min == 0 || min > MAX_COLS {
                                continue;
                            }
                            let span = ColSpan {
                                min: min - 1,
                                max: (max.min(MAX_COLS)) - 1,
                                width: a("width").and_then(|v| v.parse().ok()),
                                custom: matches!(a("customWidth"), Some("1") | Some("true")),
                                hidden: matches!(a("hidden"), Some("1") | Some("true")),
                                style: a("style").and_then(|v| v.parse().ok()).unwrap_or(0),
                                best_fit: matches!(a("bestFit"), Some("1") | Some("true")),
                                outline: a("outlineLevel").and_then(|v| v.parse().ok()).unwrap_or(0),
                                collapsed: matches!(a("collapsed"), Some("1") | Some("true")),
                                extra: c.attrs.iter().filter(|(k, _)| !matches!(k.as_str(), "min" | "max" | "width" | "customWidth" | "hidden" | "style" | "bestFit" | "outlineLevel" | "collapsed")).cloned().collect(),
                            };
                            sh.cols.push(span);
                        }
                        sh.cols.sort_by_key(|s| s.min);
                        // overlapping spans (invalid but seen): keep the first
                        let mut out: Vec<ColSpan> = Vec::new();
                        for s in sh.cols.drain(..) {
                            if let Some(l) = out.last() {
                                if s.min <= l.max {
                                    continue;
                                }
                            }
                            out.push(s);
                        }
                        sh.cols = out;
                        sh.pieces.push(Piece::Known(l));
                    }
                    "autoFilter" => match crate::book_data::read_autofilter(raw) {
                        Some(af) => {
                            sh.autofilter = Some(af);
                            sh.pieces.push(Piece::Known(l));
                        }
                        None => sh.pieces.push(Piece::Raw(l, raw.to_string())),
                    },
                    "mergeCells" => {
                        let (_, e) = xml::parse(raw)?;
                        for m in e.elems() {
                            if let Some(rc) = m.attr("ref").and_then(parse_range) {
                                if rc.count() > 1 {
                                    sh.merges.push(rc);
                                }
                            }
                        }
                        sh.pieces.push(Piece::Known(l));
                    }
                    _ => sh.pieces.push(Piece::Raw(l, raw.to_string())),
                }
            }
        }
    }
    // spilled values in the file belong to their formulas now
    let dyn_anchors: Vec<((u32, u32), Rect)> = sh.cells.iter().filter_map(|((r, c), cell)| {
        let f = cell.f.as_ref()?;
        if f.kind == FKind::Dynamic {
            let rc = f.extra.iter().find(|(k, _)| k == "ref").and_then(|(_, v)| parse_range(v))?;
            return Some(((*r, *c), rc));
        }
        None
    }).collect();
    for ((ar, ac), rc) in dyn_anchors {
        for (r, c) in sh.positions_in(rc) {
            if (r, c) == (ar, ac) {
                continue;
            }
            let keep_style = sh.cells.get(&(r, c)).map(|x| x.s).unwrap_or(0);
            if sh.cells.get(&(r, c)).map(|x| x.f.is_none()).unwrap_or(false) {
                if keep_style != 0 {
                    sh.cells.insert((r, c), Cell { s: keep_style, ..Default::default() });
                } else {
                    sh.put(r, c, None);
                }
            }
        }
        if let Some(cell) = sh.cells.get_mut(&(ar, ac)) {
            if let Some(f) = &cell.f {
                let mut nf = (**f).clone();
                nf.extra.retain(|(k, _)| k != "ref");
                cell.f = Some(Rc::new(nf));
            }
        }
    }
    sh.arrays = sh.cells.iter().filter_map(|((r, c), cell)| match cell.f.as_ref().map(|f| &f.kind) {
        Some(FKind::Array(rc)) => Some(((*r, *c), *rc)),
        _ => None,
    }).collect();
    Ok(())
}

/// Skip to the end of the element whose start tag was just read; returns the position
/// after its end tag.
fn skip_element(t: &mut Tokenizer, name: &str) -> Result<usize, String> {
    let mut depth = 1;
    loop {
        match t.next_tok()?.ok_or("unexpected end of the worksheet")? {
            Tok::Start(n, _, sc) => {
                if !sc && n == name {
                    depth += 1;
                }
            }
            Tok::End(n) => {
                if n == name {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(t.pos);
                    }
                }
            }
            _ => {}
        }
    }
}

fn read_view(v: &mut View, e: &Element) {
    let sv = match e.elems().next() {
        Some(x) => x,
        None => return,
    };
    v.selected = matches!(sv.attr("tabSelected"), Some("1") | Some("true"));
    v.show_grid = !matches!(sv.attr("showGridLines"), Some("0") | Some("false"));
    v.show_headers = !matches!(sv.attr("showRowColHeaders"), Some("0") | Some("false"));
    v.zoom = sv.attr("zoomScale").and_then(|x| x.parse().ok()).unwrap_or(100);
    v.mode = sv.attr("view").unwrap_or("normal").to_string();
    if let Some((r, c)) = sv.attr("topLeftCell").and_then(parse_ref_cell) {
        v.top_row = r;
        v.left_col = c;
    }
    if let Some(p) = sv.child("pane") {
        if matches!(p.attr("state"), Some("frozen") | Some("frozenSplit")) {
            v.freeze_cols = p.attr("xSplit").and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u32;
            v.freeze_rows = p.attr("ySplit").and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u32;
            if let Some((r, c)) = p.attr("topLeftCell").and_then(parse_ref_cell) {
                if v.freeze_rows > 0 {
                    v.top_row = r;
                }
                if v.freeze_cols > 0 {
                    v.left_col = c;
                }
            }
        }
    }
    // the selection of the pane that has the cursor
    let active_pane = sv.child("pane").and_then(|p| p.attr("activePane")).unwrap_or("topLeft");
    let sel = sv.elems_named("selection").find(|s| s.attr("pane").unwrap_or("topLeft") == active_pane).or_else(|| sv.elems_named("selection").last());
    if let Some(s) = sel {
        if let Some(a) = s.attr("activeCell").and_then(parse_ref_cell) {
            v.active = a;
        }
        let ranges: Vec<Rect> = s.attr("sqref").unwrap_or("").split_whitespace().filter_map(parse_range).collect();
        if !ranges.is_empty() {
            v.sel = ranges;
        } else {
            v.sel = vec![Rect::cell(v.active.0, v.active.1)];
        }
    }
}

fn iso_to_serial(s: &str, d1904: bool) -> Option<f64> {
    // 2026-01-02T12:30:00 or 2026-01-02
    let (date, time) = match s.find('T') {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    };
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    let mut v = crate::dates::ymd_to_serial(y, m, d, d1904)?;
    if !time.is_empty() {
        let t: Vec<f64> = time.trim_end_matches('Z').split(':').filter_map(|x| x.parse().ok()).collect();
        v += (t.first().unwrap_or(&0.0) * 3600.0 + t.get(1).unwrap_or(&0.0) * 60.0 + t.get(2).unwrap_or(&0.0)) / 86400.0;
    }
    Some(v)
}

fn read_sheet_data(sh: &mut Sheet, t: &mut Tokenizer, text: &str, sst: &[(Rc<str>, Option<Rc<str>>)], dyn_cm: Option<u32>) -> Result<(), String> {
    let mut row: u32 = 0;
    let mut next_row: u32 = 0;
    let mut col: u32 = 0;
    let mut shared: HashMap<String, (u32, u32, String)> = HashMap::new();
    loop {
        let tok = t.next_tok()?.ok_or("unexpected end of sheetData")?;
        match tok {
            Tok::End(n) if xml::local(n) == "sheetData" => return Ok(()),
            Tok::Start(n, attrs, sc) if xml::local(n) == "row" => {
                row = match raw_attr(attrs, "r").and_then(|v| v.parse::<u32>().ok()) {
                    Some(r) if r >= 1 => r - 1,
                    _ => next_row,
                };
                next_row = row + 1;
                col = 0;
                let mut ri = RowInfo::default();
                for_each_attr(attrs, |k, v| {
                    let v = unescape(v);
                    match k {
                        "r" | "spans" => {}
                        "ht" => ri.ht = v.parse().ok(),
                        "customHeight" => ri.custom_height = v == "1" || v == "true",
                        "hidden" => ri.hidden = v == "1" || v == "true",
                        "s" => ri.style = v.parse().ok(),
                        "customFormat" => {}
                        "outlineLevel" => ri.outline = v.parse().unwrap_or(0),
                        "collapsed" => ri.collapsed = v == "1" || v == "true",
                        _ => ri.extra.push((k.to_string(), v)),
                    }
                });
                if !flag(raw_attr(attrs, "customFormat")) {
                    ri.style = None;
                }
                if !ri.custom_height && ri.ht.is_some() && !ri.hidden {
                    // a height the application worked out; keep it (it may differ from ours)
                }
                if !ri.is_default() && row < MAX_ROWS {
                    sh.rows.insert(row, ri);
                }
                let _ = sc;
            }
            Tok::Start(n, attrs, sc) if xml::local(n) == "c" => {
                let (r, c) = match raw_attr(attrs, "r").and_then(|v| parse_ref_cell(&v)) {
                    Some(rc) => rc,
                    None => (row, col),
                };
                col = c + 1;
                let mut cell = Cell::default();
                let mut typ = String::from("n");
                let mut extra: Vec<(String, String)> = Vec::new();
                let mut cm: Option<u32> = None;
                for_each_attr(attrs, |k, v| match k {
                    "r" => {}
                    "s" => cell.s = v.parse().unwrap_or(0),
                    "t" => typ = v.to_string(),
                    "cm" => {
                        cm = v.parse().ok();
                        extra.push((k.to_string(), unescape(v)));
                    }
                    _ => extra.push((k.to_string(), unescape(v))),
                });
                let mut vtext: Option<String> = None;
                let mut ftext: Option<String> = None;
                let mut fattrs: Vec<(String, String)> = Vec::new();
                let mut fraw: Option<String> = None;
                let mut inline: Option<(String, Option<Rc<str>>)> = None;
                if !sc {
                    loop {
                        let pos = t.pos;
                        let tk = t.next_tok()?.ok_or("unexpected end in a cell")?;
                        match tk {
                            Tok::End(e) if xml::local(e) == "c" => break,
                            Tok::Start(e, fa, fsc) if xml::local(e) == "f" => {
                                fattrs = xml::parse_attrs(fa);
                                if fsc {
                                    fraw = Some(text[pos..t.pos].to_string());
                                    ftext = Some(String::new());
                                } else {
                                    let mut s = String::new();
                                    loop {
                                        match t.next_tok()?.ok_or("unexpected end in a formula")? {
                                            Tok::Text(x) => s.push_str(&unescape_ooxml(&unescape(x))),
                                            Tok::End(_) => break,
                                            _ => {}
                                        }
                                    }
                                    fraw = Some(text[pos..t.pos].to_string());
                                    ftext = Some(s);
                                }
                            }
                            Tok::Start(e, _, vsc) if xml::local(e) == "v" => {
                                let mut s = String::new();
                                if !vsc {
                                    loop {
                                        match t.next_tok()?.ok_or("unexpected end in a value")? {
                                            Tok::Text(x) => s.push_str(&unescape(x)),
                                            Tok::End(_) => break,
                                            _ => {}
                                        }
                                    }
                                }
                                vtext = Some(s);
                            }
                            Tok::Start(e, _, isc) if xml::local(e) == "is" => {
                                if !isc {
                                    let inner_start = t.pos;
                                    let (plain, rich, end) = read_string_item(t, e)?;
                                    inline = Some((plain, if rich { Some(Rc::from(&text[inner_start..end])) } else { None }));
                                }
                            }
                            Tok::Start(e, _, false) => {
                                skip_element(t, e)?;
                            }
                            _ => {}
                        }
                    }
                }
                if r >= MAX_ROWS || c >= MAX_COLS {
                    continue;
                }
                // the value
                cell.v = match typ.as_str() {
                    "s" => match vtext.as_deref().and_then(|v| v.trim().parse::<usize>().ok()).and_then(|i| sst.get(i)) {
                        Some((plain, rich)) => {
                            cell.rich = rich.clone();
                            Value::Str(plain.clone())
                        }
                        None => Value::Empty,
                    },
                    "str" => Value::string(unescape_ooxml(vtext.as_deref().unwrap_or(""))),
                    "inlineStr" => match inline {
                        Some((p, rich)) => {
                            cell.rich = rich;
                            Value::string(p)
                        }
                        None => Value::string(vtext.clone().unwrap_or_default()),
                    },
                    "b" => Value::Bool(matches!(vtext.as_deref().map(|s| s.trim()), Some("1") | Some("true"))),
                    "e" => Value::Err(vtext.as_deref().and_then(|v| XlErr::parse(v.trim())).unwrap_or(XlErr::NA)),
                    "d" => match vtext.as_deref().and_then(|v| iso_to_serial(v.trim(), false)) {
                        Some(x) => Value::num(x),
                        None => Value::Empty,
                    },
                    _ => match vtext.as_deref().map(|v| v.trim()).filter(|v| !v.is_empty()).and_then(|v| v.parse::<f64>().ok()) {
                        Some(x) => Value::num(x),
                        None => Value::Empty,
                    },
                };
                // the formula
                if let Some(ft) = ftext {
                    let fa = |k: &str| fattrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
                    let ftype = fa("t").unwrap_or_else(|| "normal".into());
                    let mut kind = FKind::Normal;
                    let mut text_f = formula::from_file_text(&ft);
                    let mut keep: Vec<(String, String)> = fattrs.iter().filter(|(k, _)| !matches!(k.as_str(), "t" | "ref" | "si")).cloned().collect();
                    match ftype.as_str() {
                        "shared" => {
                            let si = fa("si").unwrap_or_default();
                            if !ft.is_empty() {
                                shared.insert(si, (r, c, text_f.clone()));
                            } else if let Some((mr, mc, mt)) = shared.get(&si) {
                                text_f = formula::adjust(mt, &Adjust::Shift(r as i64 - *mr as i64, c as i64 - *mc as i64));
                            }
                        }
                        "array" => {
                            let rc = fa("ref").and_then(|v| parse_range(&v)).unwrap_or(Rect::cell(r, c));
                            if cm.is_some() && cm == dyn_cm {
                                kind = FKind::Dynamic;
                                keep.push(("ref".into(), rc.a1()));
                                extra.retain(|(k, _)| k != "cm");
                            } else {
                                kind = FKind::Array(rc);
                            }
                        }
                        "dataTable" => {
                            kind = FKind::DataTable(fraw.clone().unwrap_or_default());
                        }
                        _ => {}
                    }
                    if !text_f.is_empty() || matches!(kind, FKind::DataTable(_)) {
                        let mut f = crate::book::make_formula(&text_f, kind);
                        f.extra = keep;
                        cell.f = Some(Rc::new(f));
                    }
                }
                if !extra.is_empty() {
                    cell.extra = Some(Rc::new(extra));
                }
                sh.put(r, c, Some(cell));
            }
            _ => {}
        }
    }
}

// --------------------------------------------------------------------------- writing

struct Sst {
    items: Vec<String>,
    index: HashMap<(String, bool), usize>,
    count: usize,
}

impl Sst {
    fn add(&mut self, plain: &str, rich: Option<&str>) -> usize {
        self.count += 1;
        let key = match rich {
            Some(r) => (r.to_string(), true),
            None => (plain.to_string(), false),
        };
        if let Some(i) = self.index.get(&key) {
            return *i;
        }
        let mut s = String::from("<si>");
        match rich {
            Some(r) => s.push_str(r),
            None => {
                let sp = plain.starts_with(' ') || plain.ends_with(' ') || plain.contains('\n') || plain.contains('\t');
                s.push_str(if sp { "<t xml:space=\"preserve\">" } else { "<t>" });
                esc_ooxml(plain, &mut s);
                s.push_str("</t>");
            }
        }
        s.push_str("</si>");
        self.items.push(s);
        let i = self.items.len() - 1;
        self.index.insert(key, i);
        i
    }
}

impl Book {
    pub fn save(&mut self, path: &str) -> Result<(), String> {
        let data = self.to_bytes()?;
        let tmp = format!("{}.~tmp", path);
        std::fs::write(&tmp, &data).map_err(|e| format!("Could not save {}: {}", path, e))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("Could not save {}: {}", path, e))?;
        self.file.path = Some(path.to_string());
        self.mark_saved();
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut out: Vec<(String, Vec<u8>)> = Vec::new();
        let mut sst = Sst { items: vec![], index: HashMap::new(), count: 0 };
        let wb_part = if self.file.wb_part.is_empty() { "xl/workbook.xml".to_string() } else { self.file.wb_part.clone() };
        let dir = match wb_part.rfind('/') {
            Some(i) => wb_part[..i].to_string(),
            None => String::new(),
        };
        // relationships of the workbook
        let (rels_pro, mut rels) = self.file.wb_rels.clone().unwrap_or_else(|| (String::new(), Element::with_attrs("Relationships", &[("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")])));
        let (ct_pro, mut ct) = self.file.content_types.clone().unwrap_or_else(|| default_content_types());
        let mut used_rids: Vec<String> = rels.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
        let mut new_rid = |used: &mut Vec<String>| -> String {
            let mut k = used.len() + 1;
            loop {
                let id = format!("rId{}", k);
                if !used.contains(&id) {
                    used.push(id.clone());
                    return id;
                }
                k += 1;
            }
        };
        // sheet parts: keep names, give new sheets new parts
        let mut taken: Vec<String> = self.sheets.iter().map(|s| s.part.clone()).filter(|p| !p.is_empty()).collect();
        let mut sheet_parts: Vec<(String, String)> = Vec::new(); // (part, rId)
        for sh in &self.sheets {
            let mut part = sh.part.clone();
            if part.is_empty() {
                let mut k = 1;
                loop {
                    let p = format!("{}/worksheets/sheet{}.xml", if dir.is_empty() { "xl" } else { &dir }, k);
                    if !taken.contains(&p) && !self.file.parts.iter().any(|(n, _)| *n == p) {
                        part = p;
                        break;
                    }
                    k += 1;
                }
                taken.push(part.clone());
            }
            let rid = match self.file.sheet_rids.get(&part) {
                Some(r) if !r.is_empty() && rels.elems().any(|e| e.attr("Id") == Some(r)) => r.clone(),
                _ => {
                    let id = new_rid(&mut used_rids);
                    rels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_WORKSHEET), ("Target", &relative(&wb_part, &part))]));
                    set_override(&mut ct, &part, CT_WORKSHEET);
                    id
                }
            };
            sheet_parts.push((part, rid));
        }
        // parts of deleted sheets go
        let live: Vec<&str> = sheet_parts.iter().map(|(p, _)| p.as_str()).collect();
        let mut dropped: Vec<String> = Vec::new();
        for (p, rid) in &self.file.sheet_rids {
            if !live.contains(&p.as_str()) {
                dropped.push(p.clone());
                dropped.push(rels_name(p));
                rels.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("Id") == Some(rid)));
                remove_override(&mut ct, p);
            }
        }
        // calcChain: dropped (the application rebuilds it)
        let calc_parts: Vec<String> = rels.elems().filter(|e| e.attr("Type").map(|t| t.ends_with("/calcChain")).unwrap_or(false))
            .filter_map(|e| e.attr("Target").map(|t| resolve(&wb_part, t))).collect();
        rels.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("Type").map(|t| t.ends_with("/calcChain")).unwrap_or(false)));
        for p in &calc_parts {
            remove_override(&mut ct, p);
            dropped.push(p.clone());
        }
        // dynamic arrays need the metadata part
        let any_dynamic = self.sheets.iter().any(|s| !s.spills.is_empty());
        let mut dyn_cm = self.file.dyn_cm;
        let mut meta_new = false;
        if any_dynamic && dyn_cm.is_none() && self.file.meta_part.is_none() {
            dyn_cm = Some(1);
            meta_new = true;
        }
        // drawings and charts
        let mut taken_parts: std::collections::HashSet<String> = self.file.parts.iter().map(|(n, _)| n.clone()).collect();
        let mut used_parts: std::collections::HashSet<String> = std::collections::HashSet::new();
        let base = if dir.is_empty() { "xl".to_string() } else { dir.clone() };
        let mut drawing_rids: Vec<Option<String>> = vec![None; self.sheets.len()];
        let mut table_rids: Vec<Vec<String>> = vec![vec![]; self.sheets.len()];
        let mut legacy_rids: Vec<Option<Option<String>>> = vec![None; self.sheets.len()];
        let mut link_rids: Vec<Vec<String>> = vec![vec![]; self.sheets.len()];
        for (i, sh) in self.sheets.iter().enumerate() {
            if sh.raw_part.is_some() {
                continue;
            }
            let has_d = !(sh.objects.is_empty() && sh.drawing.is_none());
            let has_t = !sh.tables.is_empty() || sh.pieces.iter().any(|p| matches!(p, Piece::Known(t) if t == "tableParts"));
            let has_c = match &sh.comments_file {
                Some(f) => f.notes != sh.notes || f.thread_map != sh.threads,
                None => !sh.notes.is_empty() || !sh.threads.is_empty(),
            };
            let had_links = self.file.parts.iter().any(|(n, d)| *n == rels_name(&sheet_parts[i].0) && String::from_utf8_lossy(d).contains("/hyperlink\""));
            let has_l = crate::links::has_targets(sh) || had_links;
            if !has_d && !has_t && !has_c && !has_l {
                continue;
            }
            let (part, _) = &sheet_parts[i];
            let srels_name = rels_name(part);
            let (srels_pro, mut srels) = self.file.parts.iter().find(|(n, _)| *n == srels_name).and_then(|(_, d)| xml::parse_bytes(d).ok())
                .unwrap_or_else(|| (String::new(), Element::with_attrs("Relationships", &[("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")])));
            let before = srels.to_string();
            if has_d {
                drawing_rids[i] = self.write_drawing_parts(sh, part, &base, &mut out, &mut dropped, &mut ct, &mut taken_parts, &mut used_parts, &mut srels);
            }
            if has_t {
                table_rids[i] = self.write_table_parts(sh, part, &base, &mut out, &mut dropped, &mut ct, &mut taken_parts, &mut used_parts, &mut srels);
            }
            if has_c {
                legacy_rids[i] = Some(self.write_comment_parts(i, part, &base, &mut out, &mut dropped, &mut ct, &mut taken_parts, &mut srels));
            }
            if has_l {
                // the outside links get their relationships anew
                srels.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("Type") == Some(crate::links::REL_HYPERLINK)));
                let mut used: Vec<String> = srels.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
                for p in &sh.pieces {
                    if let Piece::Raw(t, x) = p {
                        if t == "hyperlinks" {
                            if let Ok((_, e)) = xml::parse(x) {
                                for h in e.elems() {
                                    if let Some(target) = h.attr_exact(crate::links::TARGET) {
                                        let mut k = used.len() + 1;
                                        while used.contains(&format!("rId{}", k)) {
                                            k += 1;
                                        }
                                        let id = format!("rId{}", k);
                                        used.push(id.clone());
                                        srels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", crate::links::REL_HYPERLINK), ("Target", target), ("TargetMode", "External")]));
                                        link_rids[i].push(id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if srels.to_string() != before {
                out.push((srels_name, xml::document(&srels_pro, &srels).into_bytes()));
            }
        }
        // the authors of threaded comments
        if let Some(px) = crate::notes::write_persons(self) {
            let pp = self.file.persons_part.clone().unwrap_or_else(|| format!("{}/persons/person.xml", base));
            out.push((pp.clone(), px.into_bytes()));
            set_override(&mut ct, &pp, "application/vnd.ms-excel.person+xml");
            if !rels.elems().any(|e| e.attr("Type").map(|t| t.ends_with("/person")).unwrap_or(false)) {
                let id = new_rid(&mut used_rids);
                rels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", "http://schemas.microsoft.com/office/2017/10/relationships/person"), ("Target", &relative(&wb_part, &pp))]));
            }
        }
        // worksheets
        for (i, sh) in self.sheets.iter().enumerate() {
            let (part, _) = &sheet_parts[i];
            if let Some(raw) = &sh.raw_part {
                out.push((part.clone(), raw.clone()));
                continue;
            }
            let x = self.write_sheet(i, &mut sst, dyn_cm, drawing_rids[i].as_deref(), &table_rids[i], legacy_rids[i].as_ref(), &link_rids[i]);
            out.push((part.clone(), x.into_bytes()));
        }
        // shared strings
        let sst_part = self.file.sst_part.clone().unwrap_or_else(|| format!("{}/sharedStrings.xml", if dir.is_empty() { "xl" } else { &dir }));
        if !sst.items.is_empty() || self.file.sst_part.is_some() {
            let mut s = String::from(xml::DECL);
            s.push_str(&format!("<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{}\" uniqueCount=\"{}\">", sst.count, sst.items.len()));
            for it in &sst.items {
                s.push_str(it);
            }
            s.push_str("</sst>");
            out.push((sst_part.clone(), s.into_bytes()));
            if self.file.sst_part.is_none() {
                let id = new_rid(&mut used_rids);
                rels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_SST), ("Target", &relative(&wb_part, &sst_part))]));
                set_override(&mut ct, &sst_part, CT_SST);
            }
        }
        if meta_new {
            let mp = format!("{}/metadata.xml", if dir.is_empty() { "xl" } else { &dir });
            out.push((mp.clone(), METADATA_XML.as_bytes().to_vec()));
            let id = new_rid(&mut used_rids);
            rels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_META), ("Target", &relative(&wb_part, &mp))]));
            set_override(&mut ct, &mp, CT_META);
        }
        // styles
        let styles_part = if self.file.styles_part.is_empty() { format!("{}/styles.xml", if dir.is_empty() { "xl" } else { &dir }) } else { self.file.styles_part.clone() };
        out.push((styles_part.clone(), self.styles.to_xml().into_bytes()));
        if !rels.elems().any(|e| e.attr("Type") == Some(T_STYLES)) {
            let id = new_rid(&mut used_rids);
            rels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_STYLES), ("Target", &relative(&wb_part, &styles_part))]));
            set_override(&mut ct, &styles_part, CT_STYLES);
        }
        // the workbook
        out.push((wb_part.clone(), self.write_workbook(&sheet_parts).into_bytes()));
        out.push((rels_name(&wb_part), xml::document(&rels_pro, &rels).into_bytes()));
        out.push(("[Content_Types].xml".into(), xml::document(&ct_pro, &ct).into_bytes()));
        // everything else as read
        let written: Vec<String> = out.iter().map(|(n, _)| n.clone()).collect();
        let mut z = ZipWriter::new();
        // [Content_Types].xml first, as applications expect
        let mut ordered: Vec<(String, Vec<u8>)> = Vec::new();
        for (n, d) in &out {
            if n == "[Content_Types].xml" {
                ordered.push((n.clone(), d.clone()));
            }
        }
        for (n, d) in &self.file.parts {
            if n == "[Content_Types].xml" || dropped.contains(n) {
                continue;
            }
            if let Some((_, nd)) = out.iter().find(|(m, _)| m == n) {
                ordered.push((n.clone(), nd.clone()));
            } else {
                ordered.push((n.clone(), d.clone()));
            }
        }
        for (n, d) in &out {
            if !ordered.iter().any(|(m, _)| m == n) {
                ordered.push((n.clone(), d.clone()));
            }
        }
        if !ordered.iter().any(|(n, _)| n == "_rels/.rels") {
            ordered.push(("_rels/.rels".into(), format!("{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"{}\"/></Relationships>", xml::DECL, wb_part).into_bytes()));
        }
        let _ = written;
        for (n, d) in &ordered {
            z.add(n, d, d.len() > 64);
        }
        Ok(z.finish())
    }

    /// Write a sheet's notes and threaded comments (comments part, VML drawing, threaded
    /// comments part); the id of the legacyDrawing relationship, if any.
    #[allow(clippy::too_many_arguments)]
    fn write_comment_parts(&self, i: usize, sheet_part: &str, base: &str, out: &mut Vec<(String, Vec<u8>)>, dropped: &mut Vec<String>,
                           ct: &mut Element, taken: &mut std::collections::HashSet<String>, srels: &mut Element) -> Option<String> {
        let sh = &self.sheets[i];
        let old = sh.comments_file.clone().unwrap_or_default();
        let is = |e: &Element, suffix: &str| e.attr("Type").map(|t| t.ends_with(suffix)).unwrap_or(false);
        srels.children.retain(|n| !matches!(n, Node::Elem(e) if is(e, "/comments") || is(e, "/vmlDrawing") || is(e, "/threadedComment")));
        let fresh = |stem: &str, ext: &str, taken: &mut std::collections::HashSet<String>| -> String {
            let mut k = 1;
            loop {
                let p = format!("{}/{}{}.{}", base, stem, k, ext);
                if !taken.contains(&p) {
                    taken.insert(p.clone());
                    return p;
                }
                k += 1;
            }
        };
        let mut ids: Vec<String> = srels.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
        let mut rid = |ids: &mut Vec<String>| -> String {
            let mut k = 1;
            loop {
                let id = format!("rId{}", k);
                if !ids.contains(&id) {
                    ids.push(id.clone());
                    return id;
                }
                k += 1;
            }
        };
        let empty = sh.notes.is_empty() && sh.threads.is_empty();
        let others = old.vml_text.as_deref().map(|v| {
            let n_shapes = v.matches("<v:shape ").count();
            let n_notes = v.matches("ObjectType=\"Note\"").count();
            n_shapes > n_notes
        }).unwrap_or(false);
        // the comments part
        if !empty {
            let cp = old.comments.clone().unwrap_or_else(|| fresh("comments", "xml", taken));
            out.push((cp.clone(), crate::notes::write_comments(&sh.notes, &sh.threads).into_bytes()));
            set_override(ct, &cp, "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml");
            let id = rid(&mut ids);
            srels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"), ("Target", &relative(sheet_part, &cp))]));
        } else if let Some(cp) = &old.comments {
            dropped.push(cp.clone());
            remove_override(ct, cp);
        }
        // threaded comments
        if !sh.threads.is_empty() {
            let tp = old.threads.clone().unwrap_or_else(|| fresh("threadedComments/threadedComment", "xml", taken));
            out.push((tp.clone(), crate::notes::write_threads(&sh.threads).into_bytes()));
            set_override(ct, &tp, "application/vnd.ms-excel.threadedcomments+xml");
            let id = rid(&mut ids);
            srels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", "http://schemas.microsoft.com/office/2017/10/relationships/threadedComment"), ("Target", &relative(sheet_part, &tp))]));
        } else if let Some(tp) = &old.threads {
            dropped.push(tp.clone());
            remove_override(ct, tp);
        }
        // the drawing that shows them
        if empty && !others {
            if let Some(vp) = &old.vml {
                dropped.push(vp.clone());
                dropped.push(rels_name(vp));
            }
            return None;
        }
        let vp = old.vml.clone().unwrap_or_else(|| fresh("drawings/vmlDrawing", "vml", taken));
        out.push((vp.clone(), crate::notes::write_vml(self, i, i + 1, old.vml_text.as_deref()).into_bytes()));
        set_default(ct, "vml", "application/vnd.openxmlformats-officedocument.vmlDrawing");
        let id = rid(&mut ids);
        srels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing"), ("Target", &relative(sheet_part, &vp))]));
        Some(id)
    }

    /// Write a sheet's table parts and their relationships; the ids the worksheet names.
    #[allow(clippy::too_many_arguments)]
    fn write_table_parts(&self, sh: &Sheet, sheet_part: &str, base: &str, out: &mut Vec<(String, Vec<u8>)>, dropped: &mut Vec<String>,
                         ct: &mut Element, taken: &mut std::collections::HashSet<String>, used: &mut std::collections::HashSet<String>,
                         srels: &mut Element) -> Vec<String> {
        // the old table relationships go; the tables still here are written again
        let old: Vec<(String, String)> = srels.elems().filter(|e| e.attr("Type").map(|t| t.ends_with("/table")).unwrap_or(false))
            .filter_map(|e| Some((e.attr("Id")?.to_string(), resolve(sheet_part, e.attr("Target")?)))).collect();
        srels.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("Type").map(|t| t.ends_with("/table")).unwrap_or(false)));
        let live: Vec<&str> = sh.tables.iter().filter_map(|t| t.part.as_deref()).collect();
        for (_, p) in &old {
            if !live.contains(&p.as_str()) && !used.contains(p) {
                dropped.push(p.clone());
                dropped.push(rels_name(p));
                remove_override(ct, p);
            }
        }
        let mut ids: Vec<String> = srels.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
        let mut out_ids = Vec::new();
        for t in &sh.tables {
            let part = match &t.part {
                Some(p) if !used.contains(p) => p.clone(),
                _ => {
                    let mut k = 1;
                    loop {
                        let p = format!("{}/tables/table{}.xml", base, k);
                        if !taken.contains(&p) {
                            taken.insert(p.clone());
                            break p;
                        }
                        k += 1;
                    }
                }
            };
            used.insert(part.clone());
            out.push((part.clone(), crate::tables::write_table(t).into_bytes()));
            set_override(ct, &part, CT_TABLE);
            let rid = old.iter().find(|(_, p)| *p == part).map(|(i, _)| i.clone()).filter(|i| !ids.contains(i)).unwrap_or_else(|| {
                let mut k = 1;
                loop {
                    let id = format!("rId{}", k);
                    if !ids.contains(&id) {
                        break id;
                    }
                    k += 1;
                }
            });
            ids.push(rid.clone());
            srels.push(Element::with_attrs("Relationship", &[("Id", &rid), ("Type", T_TABLE), ("Target", &relative(sheet_part, &part))]));
            out_ids.push(rid);
        }
        out_ids
    }

    /// Write a sheet's drawing part, its charts and the sheet's relationship to it.
    /// Returns the relationship id the worksheet names (None: no drawing any more).
    #[allow(clippy::too_many_arguments)]
    fn write_drawing_parts(&self, sh: &Sheet, sheet_part: &str, base: &str, out: &mut Vec<(String, Vec<u8>)>, dropped: &mut Vec<String>,
                           ct: &mut Element, taken: &mut std::collections::HashSet<String>, used: &mut std::collections::HashSet<String>,
                           srels: &mut Element) -> Option<String> {
        let fresh = |stem: &str, taken: &mut std::collections::HashSet<String>| -> String {
            let mut k = 1;
            loop {
                let p = format!("{}/{}{}.xml", base, stem, k);
                if !taken.contains(&p) {
                    taken.insert(p.clone());
                    return p;
                }
                k += 1;
            }
        };
        let old = sh.drawing.as_ref();
        let old_rid = old.and_then(|d| srels.elems().find(|e| e.attr("Type").map(|t| t.ends_with("/drawing")).unwrap_or(false)
            && e.attr("Target").map(|t| resolve(sheet_part, t) == d.part).unwrap_or(false)).and_then(|e| e.attr("Id")).map(|s| s.to_string()));
        // charts that are gone take their parts with them
        let live: Vec<&str> = sh.objects.iter().filter_map(|o| o.part.as_deref()).collect();
        let file_parts = &self.file.parts;
        let drop_chart = |target: &str, dropped: &mut Vec<String>, ct: &mut Element, used: &std::collections::HashSet<String>| {
            if used.contains(target) || live.contains(&target) {
                return;
            }
            dropped.push(target.to_string());
            let cr = rels_name(target);
            if let Some((_, d)) = file_parts.iter().find(|(n, _)| *n == cr) {
                if let Ok((_, rels)) = xml::parse_bytes(d) {
                    for r in rels.elems() {
                        let ty = r.attr("Type").unwrap_or("");
                        if ty.ends_with("/chartStyle") || ty.ends_with("/chartColorStyle") {
                            if let Some(t) = r.attr("Target") {
                                let p = resolve(target, t);
                                remove_override(ct, &p);
                                dropped.push(p);
                            }
                        }
                    }
                }
            }
            dropped.push(cr);
            remove_override(ct, target);
        };
        if sh.objects.is_empty() {
            if let Some(d) = old {
                for r in &d.rels {
                    if r.1.ends_with("/chart") {
                        drop_chart(&r.2, dropped, ct, used);
                    }
                }
                if !used.contains(&d.part) {
                    dropped.push(d.part.clone());
                    dropped.push(rels_name(&d.part));
                    remove_override(ct, &d.part);
                }
            }
            if let Some(rid) = &old_rid {
                srels.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("Id") == Some(rid)));
            }
            return None;
        }
        // the drawing part (a copied sheet gets its own)
        let dpart = match old {
            Some(d) if !used.contains(&d.part) => d.part.clone(),
            _ => fresh("drawings/drawing", taken),
        };
        used.insert(dpart.clone());
        let mut drels: Vec<(String, String, String, bool)> = old.map(|d| d.rels.iter().filter(|r| !r.1.ends_with("/chart")).cloned().collect()).unwrap_or_default();
        if let Some(d) = old {
            for r in &d.rels {
                if r.1.ends_with("/chart") {
                    drop_chart(&r.2, dropped, ct, used);
                }
            }
        }
        let mut objs = sh.objects.clone();
        let mut ids: Vec<String> = drels.iter().map(|r| r.0.clone()).collect();
        for o in objs.iter_mut() {
            let ch = match &o.chart {
                Some(c) => c.clone(),
                None => continue,
            };
            let cpart = match &o.part {
                Some(p) if !used.contains(p) => p.clone(),
                _ => fresh("charts/chart", taken),
            };
            used.insert(cpart.clone());
            // a chart copied with its sheet keeps the original's look only if it can
            let xml = ch.to_xml(self);
            out.push((cpart.clone(), xml.into_bytes()));
            if o.part.as_deref() != Some(cpart.as_str()) {
                if let Some(op) = &o.part {
                    // copy the original's style parts' relationships along
                    let orels = rels_name(op);
                    if let Some((_, d)) = self.file.parts.iter().find(|(n, _)| *n == orels) {
                        if ch.orig.is_some() {
                            let txt = xml::decode(d).unwrap_or_default();
                            // targets are relative to the charts folder in both
                            out.push((rels_name(&cpart), txt.into_bytes()));
                        }
                    }
                }
            }
            set_override(ct, &cpart, CT_CHART);
            let mut k = 1;
            let rid = loop {
                let id = format!("rId{}", k);
                if !ids.contains(&id) {
                    ids.push(id.clone());
                    break id;
                }
                k += 1;
            };
            drels.push((rid.clone(), T_CHART.into(), cpart.clone(), false));
            o.set_chart_rid(&rid);
        }
        let dxml = crate::drawing::write_drawing(old, &objs);
        out.push((dpart.clone(), dxml.into_bytes()));
        set_override(ct, &dpart, CT_DRAWING);
        // its relationships
        let mut rels = Element::with_attrs("Relationships", &[("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")]);
        for (id, ty, target, ext) in &drels {
            let tg = if *ext { target.clone() } else { relative(&dpart, target) };
            let mut e = Element::with_attrs("Relationship", &[("Id", id), ("Type", ty), ("Target", &tg)]);
            if *ext {
                e.set_attr("TargetMode", "External");
            }
            rels.push(e);
        }
        out.push((rels_name(&dpart), xml::document("", &rels).into_bytes()));
        // the sheet names the drawing
        let target = relative(sheet_part, &dpart);
        let rid = match &old_rid {
            Some(r) if old.map(|d| d.part == dpart).unwrap_or(false) => r.clone(),
            _ => {
                let ids: Vec<String> = srels.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
                let mut k = 1;
                let id = loop {
                    let id = format!("rId{}", k);
                    if !ids.contains(&id) {
                        break id;
                    }
                    k += 1;
                };
                if let Some(orid) = &old_rid {
                    srels.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("Id") == Some(orid)));
                }
                srels.push(Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_DRAWING), ("Target", &target)]));
                id
            }
        };
        Some(rid)
    }

    fn write_workbook(&self, sheet_parts: &[(String, String)]) -> String {
        let (pro, mut wb) = self.file.wb.clone().unwrap_or_else(|| {
            let (p, e) = xml::parse("<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><bookViews><workbookView/></bookViews><sheets/><calcPr calcId=\"191029\"/></workbook>").unwrap();
            (p, e)
        });
        const WB_ORDER: [&str; 22] = ["fileVersion", "fileSharing", "workbookPr", "workbookProtection", "bookViews", "sheets",
            "functionGroups", "externalReferences", "definedNames", "calcPr", "oleSize", "customWorkbookViews", "pivotCaches",
            "smartTagPr", "smartTagTypes", "webPublishing", "fileRecoveryPr", "webPublishObjects", "extLst", "", "", ""];
        let rprefix = wb.attrs.iter().find(|(k, v)| k.starts_with("xmlns:") && v == REL_NS).map(|(k, _)| k[6..].to_string()).unwrap_or_else(|| {
            "r".to_string()
        });
        if !wb.attrs.iter().any(|(k, v)| k.starts_with("xmlns:") && v == REL_NS) {
            wb.attrs.push(("xmlns:r".into(), REL_NS.into()));
        }
        let pfx = match wb.name.find(':') {
            Some(i) => wb.name[..=i].to_string(),
            None => String::new(),
        };
        // workbookPr
        if self.date1904 || wb.child("workbookPr").is_some() {
            if wb.child("workbookPr").is_none() {
                wb.insert_ordered(Element::new(&format!("{}workbookPr", pfx)), &WB_ORDER);
            }
            let p = wb.child_mut("workbookPr").unwrap();
            if self.date1904 {
                p.set_attr("date1904", "1");
            } else {
                p.remove_attr("date1904");
            }
        }
        // the active sheet
        if wb.child("bookViews").is_none() {
            let mut bv = Element::new(&format!("{}bookViews", pfx));
            bv.push(Element::new(&format!("{}workbookView", pfx)));
            wb.insert_ordered(bv, &WB_ORDER);
        }
        if let Some(bv) = wb.child_mut("bookViews") {
            if bv.elems().next().is_none() {
                bv.push(Element::new(&format!("{}workbookView", pfx)));
            }
            if let Some(v) = bv.elems_mut().next() {
                if self.active > 0 {
                    v.set_attr("activeTab", &self.active.to_string());
                } else {
                    v.remove_attr("activeTab");
                }
                if let Some(f) = v.attr("firstSheet").and_then(|x| x.parse::<usize>().ok()) {
                    if f >= self.sheets.len() {
                        v.remove_attr("firstSheet");
                    }
                }
            }
        }
        // sheets
        let mut sheets = Element::new(&format!("{}sheets", pfx));
        for (i, sh) in self.sheets.iter().enumerate() {
            let mut e = Element::new(&format!("{}sheet", pfx));
            e.set_attr("name", &sh.name);
            e.set_attr("sheetId", &sh.id.to_string());
            if sh.state != "visible" {
                e.set_attr("state", &sh.state);
            }
            e.set_attr(&format!("{}:id", rprefix), &sheet_parts[i].1);
            sheets.push(e);
        }
        match wb.children.iter_mut().find(|n| matches!(n, Node::Elem(e) if e.local() == "sheets")) {
            Some(Node::Elem(e)) => *e = sheets,
            _ => wb.insert_ordered(sheets, &WB_ORDER),
        }
        // defined names
        wb.remove_children("definedNames");
        if !self.names.is_empty() {
            let mut dn = Element::new(&format!("{}definedNames", pfx));
            for n in &self.names {
                let mut e = Element::new(&format!("{}definedName", pfx));
                e.set_attr("name", &n.name);
                for (k, v) in &n.extra {
                    e.set_attr(k, v);
                }
                if let Some(c) = &n.comment {
                    e.set_attr("comment", c);
                }
                if let Some(s) = n.scope {
                    e.set_attr("localSheetId", &s.to_string());
                }
                if n.hidden {
                    e.set_attr("hidden", "1");
                }
                e.children.push(Node::Text(formula::to_file_text(&n.text)));
                dn.push(e);
            }
            wb.insert_ordered(dn, &WB_ORDER);
        }
        // calculation settings
        if wb.child("calcPr").is_none() {
            wb.insert_ordered(Element::with_attrs(&format!("{}calcPr", pfx), &[("calcId", "191029")]), &WB_ORDER);
        }
        if let Some(c) = wb.child_mut("calcPr") {
            if self.calc.manual {
                c.set_attr("calcMode", "manual");
            } else {
                c.remove_attr("calcMode");
            }
            if self.calc.iterate {
                c.set_attr("iterate", "1");
                c.set_attr("iterateCount", &self.calc.count.to_string());
                c.set_attr("iterateDelta", &num_text(self.calc.delta));
            } else {
                c.remove_attr("iterate");
                c.remove_attr("iterateCount");
                c.remove_attr("iterateDelta");
            }
        }
        xml::document(&pro, &wb)
    }

    fn write_sheet(&self, i: usize, sst: &mut Sst, dyn_cm: Option<u32>, drawing_rid: Option<&str>, table_rids: &[String], legacy: Option<&Option<String>>, link_rids: &[String]) -> String {
        let sh = &self.sheets[i];
        let mut out = String::with_capacity(4096 + sh.cells.len() * 40);
        if sh.root_open.is_empty() {
            out.push_str(xml::DECL);
            out.push_str("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">");
        } else {
            out.push_str(if sh.prolog.trim().is_empty() { xml::DECL } else { &sh.prolog });
            out.push_str(&sh.root_open);
        }
        let pfx = {
            let n = sh.root_open.trim_start_matches('<');
            let n = &n[..n.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(n.len())];
            match n.find(':') {
                Some(k) => n[..=k].to_string(),
                None => String::new(),
            }
        };
        // the pieces, in schema order
        let mut pieces: Vec<Piece> = sh.pieces.clone();
        let have = |p: &[Piece], tag: &str| p.iter().any(|x| matches!(x, Piece::Known(t) if t == tag));
        for tag in ["sheetPr", "dimension", "sheetViews", "sheetFormatPr", "cols", "sheetData", "autoFilter", "mergeCells"] {
            if !have(&pieces, tag) {
                pieces.push(Piece::Known(tag.to_string()));
            }
        }
        if drawing_rid.is_some() && !pieces.iter().any(|x| matches!(x, Piece::Known(t) | Piece::Raw(t, _) if t == "drawing")) {
            pieces.push(Piece::Known("drawing".into()));
        }
        if !table_rids.is_empty() && !pieces.iter().any(|x| matches!(x, Piece::Known(t) | Piece::Raw(t, _) if t == "tableParts")) {
            pieces.push(Piece::Known("tableParts".into()));
        }
        if legacy.is_some() {
            // the notes' drawing is written anew (or goes)
            pieces.retain(|x| !matches!(x, Piece::Known(t) | Piece::Raw(t, _) if t == "legacyDrawing"));
            pieces.push(Piece::Known("legacyDrawing".into()));
        }
        let rank = |p: &Piece| -> usize {
            let t = match p {
                Piece::Known(t) => t.as_str(),
                Piece::Raw(t, _) => t.as_str(),
            };
            WS_ORDER.iter().position(|x| *x == t).unwrap_or(usize::MAX)
        };
        // stable sort; unknown elements stay after what preceded them
        let mut keyed: Vec<(usize, usize, Piece)> = Vec::new();
        let mut last = 0;
        for (k, p) in pieces.into_iter().enumerate() {
            let r = rank(&p);
            let r = if r == usize::MAX { last } else { r };
            last = r;
            keyed.push((r, k, p));
        }
        keyed.sort_by_key(|(r, k, _)| (*r, *k));
        let mut link_ids = link_rids.iter();
        for (_, _, p) in keyed {
            match p {
                Piece::Raw(t, x) if t == "hyperlinks" && x.contains(crate::links::TARGET) => {
                    out.push_str(&crate::links::write_out(&x, |_| link_ids.next().cloned().unwrap_or_default()));
                }
                Piece::Raw(_, x) => out.push_str(&x),
                Piece::Known(t) => match t.as_str() {
                    "sheetPr" => self.write_sheetpr(sh, &pfx, &mut out),
                    "dimension" => {
                        let d = match sh.used_extent() {
                            Some((r, c)) if r > 0 || c > 0 => format!("A1:{}", crate::cellref::a1(r, c)),
                            _ => "A1".into(),
                        };
                        out.push_str(&format!("<{}dimension ref=\"{}\"/>", pfx, d));
                    }
                    "sheetViews" => self.write_views(sh, i, &pfx, &mut out),
                    "sheetFormatPr" => self.write_format_pr(sh, &pfx, &mut out),
                    "cols" => {
                        if !sh.cols.is_empty() {
                            out.push_str(&format!("<{}cols>", pfx));
                            let mdw = self.mdw(i);
                            for c in &sh.cols {
                                out.push_str(&format!("<{}col min=\"{}\" max=\"{}\"", pfx, c.min + 1, c.max + 1));
                                let w = c.width.unwrap_or_else(|| sh.default_width_units(mdw));
                                out.push_str(&format!(" width=\"{}\"", num_text(w)));
                                if c.style != 0 {
                                    out.push_str(&format!(" style=\"{}\"", c.style));
                                }
                                if c.hidden {
                                    out.push_str(" hidden=\"1\"");
                                }
                                if c.best_fit {
                                    out.push_str(" bestFit=\"1\"");
                                }
                                if c.custom || c.width.is_some() {
                                    out.push_str(" customWidth=\"1\"");
                                }
                                if c.outline > 0 {
                                    out.push_str(&format!(" outlineLevel=\"{}\"", c.outline));
                                }
                                if c.collapsed {
                                    out.push_str(" collapsed=\"1\"");
                                }
                                for (k, v) in &c.extra {
                                    out.push_str(&format!(" {}=\"{}\"", k, esc_attr(v)));
                                }
                                out.push_str("/>");
                            }
                            out.push_str(&format!("</{}cols>", pfx));
                        }
                    }
                    "sheetData" => self.write_sheet_data(sh, sst, dyn_cm, &pfx, &mut out),
                    "autoFilter" => {
                        if let Some(af) = &sh.autofilter {
                            crate::book_data::write_autofilter(af, &pfx, &mut out);
                        }
                    }
                    "legacyDrawing" => {
                        if let Some(Some(rid)) = legacy {
                            out.push_str(&format!("<{}legacyDrawing r:id=\"{}\"/>", pfx, esc_attr(rid)));
                        }
                    }
                    "tableParts" => {
                        if !table_rids.is_empty() {
                            out.push_str(&format!("<{}tableParts count=\"{}\">", pfx, table_rids.len()));
                            for rid in table_rids {
                                out.push_str(&format!("<{}tablePart r:id=\"{}\"/>", pfx, esc_attr(rid)));
                            }
                            out.push_str(&format!("</{}tableParts>", pfx));
                        }
                    }
                    "drawing" => {
                        if let Some(rid) = drawing_rid {
                            out.push_str(&format!("<{}drawing r:id=\"{}\"/>", pfx, esc_attr(rid)));
                        }
                    }
                    "mergeCells" => {
                        if !sh.merges.is_empty() {
                            out.push_str(&format!("<{}mergeCells count=\"{}\">", pfx, sh.merges.len()));
                            for m in &sh.merges {
                                out.push_str(&format!("<{}mergeCell ref=\"{}\"/>", pfx, m.a1()));
                            }
                            out.push_str(&format!("</{}mergeCells>", pfx));
                        }
                    }
                    _ => {}
                },
            }
        }
        out.push_str(if sh.root_close.is_empty() { "</worksheet>" } else { &sh.root_close });
        out
    }

    fn write_sheetpr(&self, sh: &Sheet, pfx: &str, out: &mut String) {
        let mut e = match &sh.raw_sheetpr {
            Some(e) => e.clone(),
            None => {
                if sh.tab_color.is_none() && !sh.fit_to_page {
                    return;
                }
                Element::new(&format!("{}sheetPr", pfx))
            }
        };
        // fit to pages
        let had_fit = e.child("pageSetUpPr").map(|p| matches!(p.attr("fitToPage"), Some("1") | Some("true"))).unwrap_or(false);
        if had_fit != sh.fit_to_page {
            match e.child_mut("pageSetUpPr") {
                Some(p) => {
                    if sh.fit_to_page { p.set_attr("fitToPage", "1") } else { p.remove_attr("fitToPage") }
                }
                None => {
                    let p = Element::with_attrs(&format!("{}pageSetUpPr", pfx), &[("fitToPage", "1")]);
                    e.insert_ordered(p, &["tabColor", "outlinePr", "pageSetUpPr"]);
                }
            }
            e.children.retain(|n| !matches!(n, Node::Elem(x) if x.local() == "pageSetUpPr" && x.attrs.is_empty() && x.children.is_empty()));
        }
        let had = e.child("tabColor").map(|t| crate::styles::Color::from_el(t));
        let same = match (had, sh.tab_color) {
            (None, None) => true,
            (Some(c), Some(v)) => self.styles.rgb(&c, 0) == v,
            _ => false,
        };
        if !same {
            e.remove_children("tabColor");
            if let Some(v) = sh.tab_color {
                let t = Element::with_attrs(&format!("{}tabColor", pfx), &[("rgb", &format!("FF{:06X}", v))]);
                e.children.insert(0, Node::Elem(t));
            }
        }
        if e.children.is_empty() && e.attrs.is_empty() {
            return;
        }
        e.write(out);
    }

    fn write_views(&self, sh: &Sheet, i: usize, pfx: &str, out: &mut String) {
        let selected = i == self.active;
        if let Some(raw) = &sh.raw_views {
            if !sh.view.changed && sh.view.selected == selected {
                out.push_str(raw);
                return;
            }
        }
        let v = &sh.view;
        let mut s = format!("<{}sheetViews><{}sheetView", pfx, pfx);
        if selected {
            s.push_str(" tabSelected=\"1\"");
        }
        if !v.show_grid {
            s.push_str(" showGridLines=\"0\"");
        }
        if !v.show_headers {
            s.push_str(" showRowColHeaders=\"0\"");
        }
        if v.mode != "normal" && !v.mode.is_empty() {
            s.push_str(&format!(" view=\"{}\"", v.mode));
        }
        if v.zoom != 100 {
            s.push_str(&format!(" zoomScale=\"{}\" zoomScaleNormal=\"{}\"", v.zoom, v.zoom));
        }
        let frozen = v.freeze_rows > 0 || v.freeze_cols > 0;
        let top_left = if frozen { crate::cellref::a1(0, 0) } else { crate::cellref::a1(v.top_row, v.left_col) };
        if top_left != "A1" {
            s.push_str(&format!(" topLeftCell=\"{}\"", top_left));
        }
        s.push_str(" workbookViewId=\"0\">");
        let sqref: Vec<String> = v.sel.iter().map(|r| r.a1()).collect();
        let active = crate::cellref::a1(v.active.0, v.active.1);
        if frozen {
            let pane = if v.freeze_rows > 0 && v.freeze_cols > 0 { "bottomRight" } else if v.freeze_rows > 0 { "bottomLeft" } else { "topRight" };
            s.push_str(&format!("<{}pane", pfx));
            if v.freeze_cols > 0 {
                s.push_str(&format!(" xSplit=\"{}\"", v.freeze_cols));
            }
            if v.freeze_rows > 0 {
                s.push_str(&format!(" ySplit=\"{}\"", v.freeze_rows));
            }
            s.push_str(&format!(" topLeftCell=\"{}\" activePane=\"{}\" state=\"frozen\"/>", crate::cellref::a1(v.top_row.max(v.freeze_rows), v.left_col.max(v.freeze_cols)), pane));
            s.push_str(&format!("<{}selection pane=\"{}\" activeCell=\"{}\" sqref=\"{}\"/>", pfx, pane, active, sqref.join(" ")));
        } else {
            s.push_str(&format!("<{}selection activeCell=\"{}\" sqref=\"{}\"/>", pfx, active, sqref.join(" ")));
        }
        s.push_str(&format!("</{}sheetView></{}sheetViews>", pfx, pfx));
        out.push_str(&s);
    }

    fn write_format_pr(&self, sh: &Sheet, pfx: &str, out: &mut String) {
        let mut e = sh.raw_format_pr.clone().unwrap_or_else(|| Element::new(&format!("{}sheetFormatPr", pfx)));
        if sh.base_col_width != 8 {
            e.set_attr("baseColWidth", &sh.base_col_width.to_string());
        }
        match sh.default_col_width {
            Some(w) => e.set_attr("defaultColWidth", &num_text(w)),
            None => e.remove_attr("defaultColWidth"),
        }
        e.set_attr("defaultRowHeight", &num_text(sh.default_row_height));
        if sh.custom_row_height {
            e.set_attr("customHeight", "1");
        } else {
            e.remove_attr("customHeight");
        }
        // keep defaultRowHeight first, as written by the original
        e.attrs.sort_by_key(|(k, _)| match k.as_str() {
            "baseColWidth" => 0,
            "defaultColWidth" => 1,
            "defaultRowHeight" => 2,
            "customHeight" => 3,
            _ => 4,
        });
        e.write(out);
    }

    fn write_sheet_data(&self, sh: &Sheet, sst: &mut Sst, dyn_cm: Option<u32>, pfx: &str, out: &mut String) {
        // cells plus spilled values, row by row
        let mut cells: BTreeMap<(u32, u32), (Option<&Cell>, Option<&Value>)> = BTreeMap::new();
        for (k, c) in sh.cells.iter() {
            cells.insert(*k, (Some(c), None));
        }
        for (k, v) in sh.ghosts.iter() {
            let e = cells.entry(*k).or_insert((None, None));
            if e.0.map(|c| c.is_blank()).unwrap_or(true) {
                e.1 = Some(v);
            }
        }
        if cells.is_empty() && sh.rows.is_empty() {
            out.push_str(&format!("<{}sheetData/>", pfx));
            return;
        }
        out.push_str(&format!("<{}sheetData>", pfx));
        let mut rows: Vec<u32> = sh.rows.keys().copied().collect();
        let mut cur: Option<u32> = None;
        for (r, _) in cells.keys() {
            if cur != Some(*r) {
                rows.push(*r);
                cur = Some(*r);
            }
        }
        rows.sort_unstable();
        rows.dedup();
        for r in rows {
            out.push_str(&format!("<{}row r=\"{}\"", pfx, r + 1));
            if let Some(ri) = sh.rows.get(&r) {
                if let Some(st) = ri.style {
                    out.push_str(&format!(" s=\"{}\" customFormat=\"1\"", st));
                }
                if let Some(h) = ri.ht {
                    out.push_str(&format!(" ht=\"{}\"", num_text(h)));
                }
                if ri.custom_height {
                    out.push_str(" customHeight=\"1\"");
                }
                if ri.hidden {
                    out.push_str(" hidden=\"1\"");
                }
                if ri.outline > 0 {
                    out.push_str(&format!(" outlineLevel=\"{}\"", ri.outline));
                }
                if ri.collapsed {
                    out.push_str(" collapsed=\"1\"");
                }
                for (k, v) in &ri.extra {
                    out.push_str(&format!(" {}=\"{}\"", k, esc_attr(v)));
                }
            }
            let row_cells: Vec<(&(u32, u32), &(Option<&Cell>, Option<&Value>))> = cells.range((r, 0)..=(r, MAX_COLS)).collect();
            if row_cells.is_empty() {
                out.push_str("/>");
                continue;
            }
            out.push('>');
            for ((_, c), (cell, ghost)) in row_cells {
                self.write_cell(sh, r, *c, *cell, *ghost, sst, dyn_cm, pfx, out);
            }
            out.push_str(&format!("</{}row>", pfx));
        }
        out.push_str(&format!("</{}sheetData>", pfx));
    }

    #[allow(clippy::too_many_arguments)]
    fn write_cell(&self, sh: &Sheet, r: u32, c: u32, cell: Option<&Cell>, ghost: Option<&Value>, sst: &mut Sst, dyn_cm: Option<u32>, pfx: &str, out: &mut String) {
        let s = cell.map(|x| x.s).unwrap_or_else(|| sh.style_at(r, c));
        out.push_str(&format!("<{}c r=\"{}{}\"", pfx, col_name(c), r + 1));
        if s != 0 {
            out.push_str(&format!(" s=\"{}\"", s));
        }
        let v: Value = match (cell, ghost) {
            (Some(x), _) if !x.is_blank() => x.v.clone(),
            (_, Some(g)) => g.clone(),
            _ => Value::Empty,
        };
        let f = cell.and_then(|x| x.f.as_ref());
        let mut body = String::new();
        let mut t_attr: Option<&str> = None;
        let mut cm: Option<u32> = None;
        if let Some(f) = f {
            let text = if f.text.contains('[') {
                // references inside a table are written with the table's name
                let here = crate::tables::table_at(&self.sheets, self.sheets.iter().position(|x| std::ptr::eq(x, sh)).unwrap_or(0), r, c)
                    .map(|i| sh.tables[i].name.clone());
                formula::to_file_text(&crate::tables::to_file(&f.text, here.as_deref()))
            } else {
                formula::to_file_text(&f.text)
            };
            match &f.kind {
                FKind::DataTable(raw) => body.push_str(raw),
                FKind::Array(rc) => {
                    body.push_str(&format!("<{}f t=\"array\" ref=\"{}\"", pfx, rc.a1()));
                    for (k, val) in &f.extra {
                        body.push_str(&format!(" {}=\"{}\"", k, esc_attr(val)));
                    }
                    body.push('>');
                    esc_ooxml(&text, &mut body);
                    body.push_str(&format!("</{}f>", pfx));
                }
                kind => {
                    let spill = sh.spills.get(&(r, c));
                    let dynamic = *kind == FKind::Dynamic && spill.is_some() && dyn_cm.is_some();
                    if dynamic {
                        body.push_str(&format!("<{}f t=\"array\" ref=\"{}\"", pfx, spill.unwrap().a1()));
                        cm = dyn_cm;
                    } else {
                        body.push_str(&format!("<{}f", pfx));
                    }
                    for (k, val) in &f.extra {
                        if k == "ref" {
                            continue;
                        }
                        body.push_str(&format!(" {}=\"{}\"", k, esc_attr(val)));
                    }
                    body.push('>');
                    esc_ooxml(&text, &mut body);
                    body.push_str(&format!("</{}f>", pfx));
                }
            }
            match &v {
                Value::Num(n) => body.push_str(&format!("<{}v>{}</{}v>", pfx, num_text(*n), pfx)),
                Value::Str(t) => {
                    t_attr = Some("str");
                    body.push_str(&format!("<{}v>", pfx));
                    esc_ooxml(t, &mut body);
                    body.push_str(&format!("</{}v>", pfx));
                }
                Value::Bool(b) => {
                    t_attr = Some("b");
                    body.push_str(&format!("<{}v>{}</{}v>", pfx, if *b { 1 } else { 0 }, pfx));
                }
                Value::Err(e) => {
                    t_attr = Some("e");
                    body.push_str(&format!("<{}v>{}</{}v>", pfx, e.text(), pfx));
                }
                _ => {}
            }
        } else {
            match &v {
                Value::Num(n) => body.push_str(&format!("<{}v>{}</{}v>", pfx, num_text(*n), pfx)),
                Value::Str(t) => {
                    t_attr = Some("s");
                    let idx = sst.add(t, cell.and_then(|x| x.rich.as_deref()));
                    body.push_str(&format!("<{}v>{}</{}v>", pfx, idx, pfx));
                }
                Value::Bool(b) => {
                    t_attr = Some("b");
                    body.push_str(&format!("<{}v>{}</{}v>", pfx, if *b { 1 } else { 0 }, pfx));
                }
                Value::Err(e) => {
                    t_attr = Some("e");
                    body.push_str(&format!("<{}v>{}</{}v>", pfx, e.text(), pfx));
                }
                _ => {}
            }
        }
        if let Some(t) = t_attr {
            out.push_str(&format!(" t=\"{}\"", t));
        }
        if let Some(x) = cell.and_then(|x| x.extra.as_ref()) {
            for (k, val) in x.iter() {
                if k == "cm" {
                    continue;
                }
                out.push_str(&format!(" {}=\"{}\"", k, esc_attr(val)));
            }
        }
        if let Some(k) = cm {
            out.push_str(&format!(" cm=\"{}\"", k));
        } else if let Some(x) = cell.and_then(|x| x.extra.as_ref()) {
            if let Some((_, val)) = x.iter().find(|(k, _)| k == "cm") {
                if f.map(|f| f.kind != FKind::Dynamic).unwrap_or(true) {
                    out.push_str(&format!(" cm=\"{}\"", esc_attr(val)));
                }
            }
        }
        if body.is_empty() {
            out.push_str("/>");
        } else {
            out.push('>');
            out.push_str(&body);
            out.push_str(&format!("</{}c>", pfx));
        }
    }
}

fn default_content_types() -> (String, Element) {
    xml::parse("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/></Types>").unwrap()
}

fn set_default(ct: &mut Element, ext: &str, typ: &str) {
    if ct.elems().any(|e| e.local() == "Default" && e.attr("Extension").map(|x| x.eq_ignore_ascii_case(ext)).unwrap_or(false)) {
        return;
    }
    ct.children.insert(0, Node::Elem(Element::with_attrs("Default", &[("Extension", ext), ("ContentType", typ)])));
}

fn set_override(ct: &mut Element, part: &str, typ: &str) {
    let name = format!("/{}", part);
    for n in ct.children.iter_mut() {
        if let Node::Elem(e) = n {
            if e.local() == "Override" && e.attr("PartName").map(|p| p.eq_ignore_ascii_case(&name)).unwrap_or(false) {
                e.set_attr("ContentType", typ);
                return;
            }
        }
    }
    ct.push(Element::with_attrs("Override", &[("PartName", &name), ("ContentType", typ)]));
}

fn remove_override(ct: &mut Element, part: &str) {
    let name = format!("/{}", part);
    ct.children.retain(|n| !matches!(n, Node::Elem(e) if e.local() == "Override" && e.attr("PartName").map(|p| p.eq_ignore_ascii_case(&name)).unwrap_or(false)));
}

#[allow(dead_code)]
fn unused() {
    let _ = col_index("A");
    let _: Option<Formula> = None;
}
