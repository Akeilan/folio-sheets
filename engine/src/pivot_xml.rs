//! PivotTables in the file: the pivot cache (definition and records) and the pivot table
//! definition, as the desktop spreadsheet writes them — and read back into the model when they
//! use only what the model has (else kept as they are, shown but not refreshed here).

use crate::book::Book;
use crate::cellref::{parse_range, Rect};
use crate::pivot::{Agg, DataField, Layout, PField, Pivot, ShowAs, Source, Subtotals};
use crate::value::Value;
use crate::xml::{self, Element};
use std::collections::HashMap;

pub const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const T_CACHE_DEF: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
pub const T_CACHE_REC: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";
pub const T_TABLE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
pub const CT_CACHE_DEF: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
pub const CT_CACHE_REC: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml";
pub const CT_TABLE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";

fn esc(s: &str) -> String {
    xml::escape_attr(s)
}

fn text_of(b: &Book, v: &Value) -> String {
    match v {
        Value::Empty => "(blank)".into(),
        Value::Str(s) => s.to_string(),
        Value::Bool(t) => if *t { "TRUE".into() } else { "FALSE".into() },
        Value::Num(x) => crate::numfmt::num_to_text(*x),
        Value::Err(e) => e.text().to_string(),
        _ => {
            let _ = b;
            String::new()
        }
    }
}

/// A field's shared items (in first-seen order) and, for each record, the item's index.
struct Shared {
    items: Vec<Value>,
    texts: Vec<String>,
    index: HashMap<String, usize>,
}

fn shared(b: &Book, src: &Source, f: usize) -> Shared {
    let mut s = Shared { items: vec![], texts: vec![], index: HashMap::new() };
    for rec in &src.records {
        let v = rec.get(f).cloned().unwrap_or(Value::Empty);
        let t = text_of(b, &v);
        if !s.index.contains_key(&t) {
            s.index.insert(t.clone(), s.items.len());
            s.items.push(v);
            s.texts.push(t);
        }
    }
    s
}

/// The items of an axis field in the order the pivot shows them (indexes into the shared items).
fn shown_order(sh: &Shared, desc: bool) -> Vec<usize> {
    let rank = |v: &Value| match v { Value::Num(_) => 0, Value::Str(_) | Value::Bool(_) | Value::Err(_) => 1, _ => 2 };
    let mut idx: Vec<usize> = (0..sh.items.len()).collect();
    idx.sort_by(|a, b| {
        let (x, y) = (&sh.items[*a], &sh.items[*b]);
        let o = rank(x).cmp(&rank(y)).then_with(|| match (x, y) {
            (Value::Num(p), Value::Num(q)) => p.partial_cmp(q).unwrap_or(std::cmp::Ordering::Equal),
            _ => sh.texts[*a].to_lowercase().cmp(&sh.texts[*b].to_lowercase()),
        });
        if desc && rank(x) == rank(y) { o.reverse() } else { o }
    });
    idx
}

fn num_attr(x: f64) -> String {
    crate::xlsx::num_text(x)
}

/// The cache definition and its records.
pub fn write_cache(b: &Book, p: &Pivot, src: &Source, records_rid: &str) -> (String, String) {
    let axis: Vec<bool> = (0..src.headers.len()).map(|i| p.rows.contains(&i) || p.cols.contains(&i) || p.pages.iter().any(|x| x.0 == i)).collect();
    let mut def = format!("{}<pivotCacheDefinition xmlns=\"{}\" xmlns:r=\"{}\" r:id=\"{}\" refreshOnLoad=\"1\" createdVersion=\"6\" refreshedVersion=\"6\" minRefreshableVersion=\"3\" recordCount=\"{}\">",
                          xml::DECL, NS_MAIN, NS_R, records_rid, src.records.len());
    match &p.source_table {
        Some(t) => def.push_str(&format!("<cacheSource type=\"worksheet\"><worksheetSource name=\"{}\"/></cacheSource>", esc(t))),
        None => def.push_str(&format!("<cacheSource type=\"worksheet\"><worksheetSource ref=\"{}\" sheet=\"{}\"/></cacheSource>", p.source.a1(), esc(&p.source_sheet))),
    }
    def.push_str(&format!("<cacheFields count=\"{}\">", src.headers.len()));
    let mut shares = Vec::new();
    for (i, h) in src.headers.iter().enumerate() {
        let sh = shared(b, src, i);
        let nums: Vec<f64> = sh.items.iter().filter_map(|v| if let Value::Num(x) = v { Some(*x) } else { None }).collect();
        let has_str = sh.items.iter().any(|v| matches!(v, Value::Str(_) | Value::Bool(_) | Value::Err(_)));
        let has_blank = sh.items.iter().any(|v| v.is_empty());
        let mut attrs = String::new();
        if !nums.is_empty() {
            if !has_str {
                attrs.push_str(" containsSemiMixedTypes=\"0\" containsString=\"0\"");
            } else {
                attrs.push_str(" containsMixedTypes=\"1\"");
            }
            attrs.push_str(" containsNumber=\"1\"");
            if nums.iter().all(|x| x.fract() == 0.0) {
                attrs.push_str(" containsInteger=\"1\"");
            }
            let mn = nums.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            attrs.push_str(&format!(" minValue=\"{}\" maxValue=\"{}\"", num_attr(mn), num_attr(mx)));
        }
        if has_blank {
            attrs.push_str(" containsBlank=\"1\"");
        }
        def.push_str(&format!("<cacheField name=\"{}\" numFmtId=\"0\">", esc(h)));
        // items are listed for the fields on an axis (and for text, which records refer to)
        let list = axis[i] || has_str;
        if list {
            def.push_str(&format!("<sharedItems{} count=\"{}\">", attrs, sh.items.len()));
            for v in &sh.items {
                match v {
                    Value::Num(x) => def.push_str(&format!("<n v=\"{}\"/>", num_attr(*x))),
                    Value::Bool(t) => def.push_str(&format!("<b v=\"{}\"/>", if *t { 1 } else { 0 })),
                    Value::Err(e) => def.push_str(&format!("<e v=\"{}\"/>", esc(e.text()))),
                    Value::Empty => def.push_str("<m/>"),
                    _ => def.push_str(&format!("<s v=\"{}\"/>", esc(&text_of(b, v)))),
                }
            }
            def.push_str("</sharedItems>");
        } else {
            def.push_str(&format!("<sharedItems{}/>", attrs));
        }
        def.push_str("</cacheField>");
        shares.push((sh, list));
    }
    def.push_str("</cacheFields></pivotCacheDefinition>");
    let mut rec = format!("{}<pivotCacheRecords xmlns=\"{}\" xmlns:r=\"{}\" count=\"{}\">", xml::DECL, NS_MAIN, NS_R, src.records.len());
    for r in &src.records {
        rec.push_str("<r>");
        for (i, (sh, list)) in shares.iter().enumerate() {
            let v = r.get(i).cloned().unwrap_or(Value::Empty);
            if *list {
                rec.push_str(&format!("<x v=\"{}\"/>", sh.index.get(&text_of(b, &v)).copied().unwrap_or(0)));
            } else {
                match v {
                    Value::Num(x) => rec.push_str(&format!("<n v=\"{}\"/>", num_attr(x))),
                    Value::Bool(t) => rec.push_str(&format!("<b v=\"{}\"/>", if t { 1 } else { 0 })),
                    Value::Empty => rec.push_str("<m/>"),
                    Value::Err(e) => rec.push_str(&format!("<e v=\"{}\"/>", esc(e.text()))),
                    v => rec.push_str(&format!("<s v=\"{}\"/>", esc(&text_of(b, &v)))),
                }
            }
        }
        rec.push_str("</r>");
    }
    rec.push_str("</pivotCacheRecords>");
    (def, rec)
}

/// The pivot table definition.
pub fn write_table(b: &Book, p: &Pivot, src: &Source, cache_id: u32) -> String {
    let out = crate::pivot::compute(b, p, src);
    let nf = src.headers.len();
    let shares: Vec<Shared> = (0..nf).map(|i| shared(b, src, i)).collect();
    let orders: Vec<Vec<usize>> = (0..nf).map(|i| shown_order(&shares[i], p.fields.get(i).map(|f| f.sort < 0).unwrap_or(false))).collect();
    // an item's place in its field's <items>
    let pos = |f: usize, text: &str| -> usize {
        let si = shares[f].index.get(text).copied().unwrap_or(0);
        orders[f].iter().position(|x| *x == si).unwrap_or(0)
    };
    let tr = p.table_rect().unwrap_or(Rect::cell(p.at.0, p.at.1));
    let multi = p.values.len() > 1;
    let mut s = format!("{}<pivotTableDefinition xmlns=\"{}\" name=\"{}\" cacheId=\"{}\" applyNumberFormats=\"0\" applyBorderFormats=\"0\" applyFontFormats=\"0\" applyPatternFormats=\"0\" applyAlignmentFormats=\"0\" applyWidthHeightFormats=\"1\" dataCaption=\"Values\" updatedVersion=\"6\" minRefreshableVersion=\"3\" useAutoFormatting=\"1\" itemPrintTitles=\"1\" createdVersion=\"6\" indent=\"0\"",
                        xml::DECL, NS_MAIN, esc(&p.name), cache_id);
    match p.layout {
        Layout::Compact => s.push_str(" outline=\"1\" outlineData=\"1\""),
        Layout::Outline => s.push_str(" compact=\"0\" compactData=\"0\" outline=\"1\" outlineData=\"1\""),
        Layout::Tabular => s.push_str(" compact=\"0\" compactData=\"0\""),
    }
    if !p.grand_rows {
        s.push_str(" colGrandTotals=\"0\"");
    }
    if !p.grand_cols {
        s.push_str(" rowGrandTotals=\"0\"");
    }
    if multi && p.values_in_rows {
        s.push_str(" dataOnRows=\"1\"");
    }
    if !p.field_headers {
        s.push_str(" showHeaders=\"0\"");
    }
    s.push_str(" multipleFieldFilters=\"0\">");
    let first_header = 1;
    let empty = p.rows.is_empty() && p.cols.is_empty() && p.values.is_empty();
    let first_data_row = if empty { 1 } else { out.first_data_row - out.page_rows };
    s.push_str(&format!("<location ref=\"{}\" firstHeaderRow=\"{}\" firstDataRow=\"{}\" firstDataCol=\"{}\"", tr.a1(), first_header, first_data_row, if empty { 0 } else { out.first_data_col }));
    if !p.pages.is_empty() {
        s.push_str(&format!(" rowPageCount=\"{}\" colPageCount=\"1\"", p.pages.len()));
    }
    s.push_str("/>");
    s.push_str(&format!("<pivotFields count=\"{}\">", nf));
    for i in 0..nf {
        let f = p.fields.get(i).cloned().unwrap_or_default();
        let axis = if p.rows.contains(&i) { Some("axisRow") } else if p.cols.contains(&i) { Some("axisCol") } else if p.pages.iter().any(|x| x.0 == i) { Some("axisPage") } else { None };
        let data = p.values.iter().any(|d| d.field == i);
        s.push_str("<pivotField");
        if let Some(a) = axis {
            s.push_str(&format!(" axis=\"{}\"", a));
        }
        if data {
            s.push_str(" dataField=\"1\"");
        }
        if p.layout != Layout::Compact {
            s.push_str(" compact=\"0\"");
        }
        if p.layout == Layout::Tabular {
            s.push_str(" outline=\"0\"");
        }
        if f.sort < 0 && axis.is_some() {
            s.push_str(" sortType=\"descending\"");
        }
        let sub_off = !f.subtotal || p.subtotals == Subtotals::None;
        if sub_off && axis.is_some() {
            s.push_str(" defaultSubtotal=\"0\"");
        }
        if p.subtotals == Subtotals::Bottom && p.layout != Layout::Tabular && axis == Some("axisRow") {
            s.push_str(" subtotalTop=\"0\"");
        }
        s.push_str(" showAll=\"0\"");
        if axis.is_some() {
            let n = orders[i].len() + if sub_off { 0 } else { 1 };
            s.push_str(&format!("><items count=\"{}\">", n));
            for si in &orders[i] {
                if f.hidden.contains(&shares[i].texts[*si]) {
                    s.push_str(&format!("<item h=\"1\" x=\"{}\"/>", si));
                } else {
                    s.push_str(&format!("<item x=\"{}\"/>", si));
                }
            }
            if !sub_off {
                s.push_str("<item t=\"default\"/>");
            }
            s.push_str("</items></pivotField>");
        } else {
            s.push_str("/>");
        }
    }
    s.push_str("</pivotFields>");
    // rows
    if !p.rows.is_empty() || (multi && p.values_in_rows) {
        let mut fields: Vec<i64> = p.rows.iter().map(|x| *x as i64).collect();
        if multi && p.values_in_rows {
            fields.push(-2);
        }
        s.push_str(&format!("<rowFields count=\"{}\">", fields.len()));
        for f in fields {
            s.push_str(&format!("<field x=\"{}\"/>", f));
        }
        s.push_str("</rowFields>");
    }
    let mut items = String::new();
    let mut count = 0;
    let mut prev: Vec<String> = Vec::new();
    let vir = multi && p.values_in_rows;
    for (path, kind, vj) in &out.row_lines {
        count += 1;
        if let Some(j) = vj {
            let iattr = if *j > 0 { format!(" i=\"{}\"", j) } else { String::new() };
            match kind.as_str() {
                _ if p.rows.is_empty() => items.push_str(&format!("<i{}>{}</i>", iattr, xv(*j))),
                "Grand" => items.push_str(&format!("<i t=\"grand\"{}><x/></i>", iattr)),
                "Total" => {
                    let d = path.len() - 1;
                    let r = if d > 0 { format!(" r=\"{}\"", d) } else { String::new() };
                    items.push_str(&format!("<i t=\"default\"{}{}>{}</i>", r, iattr, xv(pos(p.rows[d], &path[d]))));
                }
                _ => items.push_str(&format!("<i r=\"{}\"{}>{}</i>", path.len(), iattr, xv(*j))),
            }
            continue;
        }
        match kind.as_str() {
            "Grand" => {
                items.push_str("<i t=\"grand\"><x/></i>");
                prev.clear();
            }
            "Total" => {
                let d = path.len() - 1;
                let v = pos(p.rows[d], &path[d]);
                if d > 0 {
                    items.push_str(&format!("<i t=\"default\" r=\"{}\">{}</i>", d, xv(v)));
                } else {
                    items.push_str(&format!("<i t=\"default\">{}</i>", xv(v)));
                }
            }
            "Blank" => {
                items.push_str(&format!("<i t=\"blank\">{}</i>", xv(pos(p.rows[0], &path[0]))));
            }
            _ => {
                let r = prev.iter().zip(path.iter()).take_while(|(a, b)| a == b).count().min(path.len().saturating_sub(1));
                let r = if prev.len() >= path.len() && prev[..path.len().saturating_sub(1)] == path[..path.len().saturating_sub(1)] { path.len() - 1 } else { r };
                let mut xs = String::new();
                for d in r..path.len() {
                    xs.push_str(&xv(pos(p.rows[d], &path[d])));
                }
                if r > 0 {
                    items.push_str(&format!("<i r=\"{}\">{}</i>", r, xs));
                } else {
                    items.push_str(&format!("<i>{}</i>", xs));
                }
                prev = path.clone();
            }
        }
    }
    if p.rows.is_empty() && !vir {
        s.push_str("<rowItems count=\"1\"><i/></rowItems>");
    } else {
        s.push_str(&format!("<rowItems count=\"{}\">{}</rowItems>", count, items));
    }
    // columns
    let mut cfields: Vec<i64> = p.cols.iter().map(|x| *x as i64).collect();
    if multi && !p.values_in_rows {
        cfields.push(-2);
    }
    if !cfields.is_empty() {
        s.push_str(&format!("<colFields count=\"{}\">", cfields.len()));
        for f in &cfields {
            s.push_str(&format!("<field x=\"{}\"/>", f));
        }
        s.push_str("</colFields>");
    }
    let mut citems = String::new();
    let mut ccount = 0;
    let mut cprev: Vec<String> = Vec::new();
    for (path, kind, j) in &out.col_lines {
        ccount += 1;
        let iattr = if multi && *j > 0 { format!(" i=\"{}\"", j) } else { String::new() };
        let data_x = if multi { xv(*j) } else { String::new() };
        match kind.as_str() {
            "Grand" => {
                citems.push_str(&format!("<i t=\"grand\"{}>{}</i>", iattr, if multi { xv(*j) } else { "<x/>".into() }));
            }
            "Total" => {
                let d = path.len() - 1;
                let v = pos(p.cols[d], &path[d]);
                let r = if d > 0 { format!(" r=\"{}\"", d) } else { String::new() };
                citems.push_str(&format!("<i t=\"default\"{}{}>{}{}</i>", r, iattr, xv(v), data_x));
            }
            _ => {
                if path.is_empty() {
                    citems.push_str(&format!("<i{}>{}</i>", iattr, if multi { xv(*j) } else { String::new() }));
                    continue;
                }
                let same = if multi && *j > 0 { path.len() } else { cprev.iter().zip(path.iter()).take_while(|(a, b)| a == b).count().min(path.len().saturating_sub(1)) };
                let mut xs = String::new();
                for d in same..path.len() {
                    xs.push_str(&xv(pos(p.cols[d], &path[d])));
                }
                xs.push_str(&data_x);
                let r = if same > 0 { format!(" r=\"{}\"", same) } else { String::new() };
                citems.push_str(&format!("<i{}{}>{}</i>", r, iattr, xs));
                cprev = path.clone();
            }
        }
    }
    if cfields.is_empty() {
        s.push_str("<colItems count=\"1\"><i/></colItems>");
    } else {
        s.push_str(&format!("<colItems count=\"{}\">{}</colItems>", ccount, citems));
    }
    if !p.pages.is_empty() {
        s.push_str(&format!("<pageFields count=\"{}\">", p.pages.len()));
        for (f, sel) in &p.pages {
            match sel {
                Some(t) => s.push_str(&format!("<pageField fld=\"{}\" item=\"{}\" hier=\"-1\"/>", f, pos(*f, t))),
                None => s.push_str(&format!("<pageField fld=\"{}\" hier=\"-1\"/>", f)),
            }
        }
        s.push_str("</pageFields>");
    }
    if !p.values.is_empty() {
        s.push_str(&format!("<dataFields count=\"{}\">", p.values.len()));
        for d in &p.values {
            s.push_str(&format!("<dataField name=\"{}\" fld=\"{}\"", esc(&d.name), d.field));
            if d.agg != Agg::Sum {
                s.push_str(&format!(" subtotal=\"{}\"", d.agg.code()));
            }
            if d.show != ShowAs::Normal {
                s.push_str(&format!(" showDataAs=\"{}\"", d.show.code()));
            }
            s.push_str(&format!(" baseField=\"{}\" baseItem=\"0\"", if d.show == ShowAs::RunningTotal { p.rows.first().copied().unwrap_or(0) } else { 0 }));
            if let Some(f) = &d.numfmt {
                let id = b.styles.numfmt_id_ro(f);
                if let Some(id) = id {
                    s.push_str(&format!(" numFmtId=\"{}\"", id));
                }
            }
            s.push_str("/>");
        }
        s.push_str("</dataFields>");
    }
    s.push_str(&format!("<pivotTableStyleInfo name=\"{}\" showRowHeaders=\"{}\" showColHeaders=\"{}\" showRowStripes=\"{}\" showColStripes=\"{}\" showLastColumn=\"1\"/>",
                        esc(&p.style), p.row_headers as u8, p.col_headers as u8, p.banded_rows as u8, p.banded_cols as u8));
    s.push_str("</pivotTableDefinition>");
    s
}

fn xv(v: usize) -> String {
    if v == 0 { "<x/>".into() } else { format!("<x v=\"{}\"/>", v) }
}

// ---------------------------------------------------------------------- reading

/// A pivot table from its definition and its cache definition (None when it isn't one).
pub fn read_pivot(b: &Book, sheet: usize, table_xml: &str, cache_xml: &str, cache_records: Option<&str>) -> Option<Pivot> {
    let (_, t) = xml::parse(table_xml).ok()?;
    let (_, c) = xml::parse(cache_xml).ok()?;
    let name = t.attr("name").unwrap_or("PivotTable1").to_string();
    let loc = t.child("location")?;
    let tr = parse_range(loc.attr("ref")?)?;
    let src_el = c.child("cacheSource");
    let ws = src_el.and_then(|s| s.child("worksheetSource"));
    let mut refreshable = src_el.and_then(|s| s.attr("type")).unwrap_or("worksheet") == "worksheet" && ws.is_some();
    let (source_sheet, source, source_table) = match ws {
        Some(w) => match (w.attr("name"), w.attr("ref")) {
            (Some(n), _) if w.attr("ref").is_none() => (String::new(), Rect::cell(0, 0), Some(n.to_string())),
            (_, Some(r)) => (w.attr("sheet").unwrap_or("").to_string(), parse_range(r).unwrap_or(Rect::cell(0, 0)), None),
            _ => (String::new(), Rect::cell(0, 0), None),
        },
        None => (String::new(), Rect::cell(0, 0), None),
    };
    if w_has_external(ws) {
        refreshable = false;
    }
    // fields and their shared items' texts
    let cfs: Vec<&Element> = c.child("cacheFields").map(|x| x.elems().filter(|e| e.local() == "cacheField").collect()).unwrap_or_default();
    let mut fields = Vec::new();
    let mut item_texts: Vec<Vec<String>> = Vec::new();
    for cf in &cfs {
        if cf.attr("formula").is_some() || cf.child("fieldGroup").is_some() || cf.attr("databaseField") == Some("0") {
            refreshable = false;
        }
        fields.push(PField { name: cf.attr("name").unwrap_or("").to_string(), subtotal: true, ..Default::default() });
        let texts: Vec<String> = cf.child("sharedItems").map(|si| si.elems().map(|e| match e.local() {
            "m" => "(blank)".to_string(),
            "b" => if e.attr("v") == Some("1") { "TRUE".into() } else { "FALSE".into() },
            "n" => e.attr("v").and_then(|v| v.parse::<f64>().ok()).map(crate::numfmt::num_to_text).unwrap_or_default(),
            _ => e.attr("v").unwrap_or("").to_string(),
        }).collect()).unwrap_or_default();
        item_texts.push(texts);
    }
    let _ = cache_records;
    if t.child("dataFields").map(|d| d.elems().any(|x| x.attr("fld").map(|f| f.starts_with('-')).unwrap_or(false))).unwrap_or(false) {
        refreshable = false;
    }
    if t.child("calculatedItems").is_some() || t.child("filters").is_some() || t.attr("asteriskTotals").is_some() || c.child("calculatedItems").is_some() {
        refreshable = false;
    }
    let mut p = Pivot::new(&name, &source_sheet, source, sheet, (tr.r1, tr.c1), fields.iter().map(|f| f.name.clone()).collect());
    p.source_table = source_table;
    p.cache_id = t.attr("cacheId").and_then(|v| v.parse().ok()).unwrap_or(0);
    p.fields = fields;
    let on = |e: &Element, n: &str, d: bool| e.attr(n).map(|v| v == "1" || v == "true").unwrap_or(d);
    p.layout = if !on(&t, "compact", true) && on(&t, "outline", false) { Layout::Outline } else if !on(&t, "compact", true) { Layout::Tabular } else { Layout::Compact };
    p.grand_rows = on(&t, "colGrandTotals", true);
    p.grand_cols = on(&t, "rowGrandTotals", true);
    p.values_in_rows = on(&t, "dataOnRows", false);
    p.field_headers = on(&t, "showHeaders", true);
    let pfs: Vec<&Element> = t.child("pivotFields").map(|x| x.elems().filter(|e| e.local() == "pivotField").collect()).unwrap_or_default();
    let mut any_bottom = false;
    let mut any_sub = false;
    for (i, pf) in pfs.iter().enumerate() {
        if i >= p.fields.len() {
            break;
        }
        if pf.attr("sortType") == Some("descending") {
            p.fields[i].sort = -1;
        }
        if pf.attr("defaultSubtotal") == Some("0") {
            p.fields[i].subtotal = false;
        } else if pf.attr("axis").is_some() {
            any_sub = true;
        }
        if pf.attr("subtotalTop") == Some("0") {
            any_bottom = true;
        }
        if let Some(items) = pf.child("items") {
            for it in items.elems() {
                if it.attr("h") == Some("1") {
                    if let Some(x) = it.attr("x").and_then(|v| v.parse::<usize>().ok()) {
                        if let Some(tx) = item_texts.get(i).and_then(|v| v.get(x)) {
                            p.fields[i].hidden.insert(tx.clone());
                        }
                    }
                }
                if matches!(it.attr("t"), Some("sum") | Some("count") | Some("avg") | Some("max") | Some("min")) {
                    refreshable = false;
                }
            }
        }
    }
    let has_axis = pfs.iter().any(|pf| pf.attr("axis").is_some());
    p.subtotals = if !any_sub && has_axis { Subtotals::None } else if any_bottom || p.layout == Layout::Tabular { Subtotals::Bottom } else { Subtotals::Top };
    if p.subtotals == Subtotals::None {
        for f in p.fields.iter_mut() {
            f.subtotal = true;
        }
    }
    let field_list = |name: &str| -> Vec<i64> {
        t.child(name).map(|x| x.elems().filter_map(|f| f.attr("x").and_then(|v| v.parse::<i64>().ok())).collect()).unwrap_or_default()
    };
    p.rows = field_list("rowFields").into_iter().filter(|x| *x >= 0).map(|x| x as usize).collect();
    p.cols = field_list("colFields").into_iter().filter(|x| *x >= 0).map(|x| x as usize).collect();
    if let Some(pg) = t.child("pageFields") {
        for f in pg.elems() {
            let fld = match f.attr("fld").and_then(|v| v.parse::<usize>().ok()) {
                Some(x) => x,
                None => continue,
            };
            let sel = f.attr("item").and_then(|v| v.parse::<usize>().ok()).and_then(|k| {
                // the item's place in the field's <items>
                let pf = pfs.get(fld)?;
                let x = pf.child("items")?.elems().nth(k)?.attr("x")?.parse::<usize>().ok()?;
                item_texts.get(fld)?.get(x).cloned()
            });
            p.pages.push((fld, sel));
        }
    }
    if let Some(df) = t.child("dataFields") {
        for d in df.elems() {
            let fld = match d.attr("fld").and_then(|v| v.parse::<usize>().ok()) {
                Some(x) => x,
                None => continue,
            };
            let show = d.attr("showDataAs").map(ShowAs::from_code).unwrap_or(ShowAs::Normal);
            if d.attr("showDataAs").map(|v| ShowAs::from_code(v) == ShowAs::Normal && v != "normal").unwrap_or(false) {
                refreshable = false;
            }
            let numfmt = d.attr("numFmtId").and_then(|v| v.parse::<u32>().ok()).filter(|id| *id != 0).map(|id| b.styles.numfmt_code(id));
            p.values.push(DataField {
                field: fld,
                agg: Agg::from_code(d.attr("subtotal").unwrap_or("sum")),
                name: d.attr("name").unwrap_or("").to_string(),
                show,
                numfmt,
            });
        }
    }
    if let Some(si) = t.child("pivotTableStyleInfo") {
        p.style = si.attr("name").unwrap_or("PivotStyleLight16").to_string();
        p.row_headers = on(si, "showRowHeaders", true);
        p.col_headers = on(si, "showColHeaders", true);
        p.banded_rows = on(si, "showRowStripes", false);
        p.banded_cols = on(si, "showColStripes", false);
    }
    p.refreshable = refreshable;
    let pages = p.pages.len() as u32;
    let top = if pages > 0 { tr.r1.saturating_sub(pages + 1) } else { tr.r1 };
    p.out = Some(Rect::new(top, tr.c1, tr.r2, tr.c2));
    Some(p)
}

fn w_has_external(ws: Option<&Element>) -> bool {
    ws.map(|w| w.attr("id").is_some()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use crate::book::Book;
    use crate::cellref::Rect;
    use crate::pivot::*;

    fn book() -> Book {
        let mut b = Book::new_empty();
        let rows = [["Region", "Product", "Sales", "Units"], ["East", "Apples", "10", "1"], ["West", "Apples", "20", "2"],
                    ["East", "Pears", "30", "3"], ["West", "Pears", "40", "4"], ["East", "Apples", "5", "5"]];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                b.set_input(0, r as u32, c as u32, v).unwrap();
            }
        }
        b.insert_sheet(1, None).unwrap();
        b
    }

    fn df(f: usize, agg: Agg, name: &str) -> DataField {
        DataField { field: f, agg, name: name.into(), show: ShowAs::Normal, numfmt: None }
    }

    fn same(a: &Pivot, b: &Pivot) {
        assert_eq!((&a.name, &a.source_sheet, a.source, a.sheet, a.at), (&b.name, &b.source_sheet, b.source, b.sheet, b.at));
        assert_eq!((&a.rows, &a.cols, &a.pages), (&b.rows, &b.cols, &b.pages));
        assert_eq!(a.values, b.values);
        assert_eq!((a.layout, a.subtotals, a.grand_rows, a.grand_cols, a.values_in_rows), (b.layout, b.subtotals, b.grand_rows, b.grand_cols, b.values_in_rows));
        assert_eq!((&a.style, a.banded_rows, a.out), (&b.style, b.banded_rows, b.out));
        for (x, y) in a.fields.iter().zip(b.fields.iter()) {
            assert_eq!((&x.name, &x.hidden, x.sort), (&y.name, &y.hidden, y.sort));
        }
    }

    fn reopen(b: &Book) -> Book {
        let bytes = b.to_bytes().unwrap();
        Book::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn round_trip() {
        let configs: Vec<Box<dyn Fn(&mut Pivot)>> = vec![
            Box::new(|p| { p.rows = vec![0]; p.values = vec![df(2, Agg::Sum, "Sum of Sales")]; }),
            Box::new(|_p| {}),
            Box::new(|p| { p.rows = vec![0, 1]; p.cols = vec![]; p.values = vec![df(2, Agg::Sum, "Sum of Sales"), df(3, Agg::Average, "Average of Units")]; }),
            Box::new(|p| { p.rows = vec![0]; p.cols = vec![1]; p.pages = vec![(3, Some("2".into()))]; p.values = vec![df(2, Agg::Max, "Max of Sales")]; }),
            Box::new(|p| { p.rows = vec![1, 0]; p.layout = Layout::Tabular; p.subtotals = Subtotals::Bottom; p.values = vec![df(2, Agg::Count, "Count of Sales")];
                           p.fields[1].hidden.insert("Pears".into()); p.fields[1].sort = -1; p.style = "PivotStyleMedium9".into(); p.banded_rows = true; }),
            Box::new(|p| { p.rows = vec![0, 1]; p.layout = Layout::Outline; p.grand_cols = false; p.grand_rows = false; p.subtotals = Subtotals::None;
                           p.values = vec![df(2, Agg::Sum, "Sum of Sales")]; }),
            Box::new(|p| { p.cols = vec![0]; p.values = vec![df(2, Agg::Sum, "Sum of Sales"), df(3, Agg::Sum, "Sum of Units")]; p.values_in_rows = true; }),
            Box::new(|p| { p.rows = vec![0]; p.cols = vec![1]; p.grand_rows = false; p.values = vec![df(2, Agg::Sum, "Sum of Sales")]; }),
            Box::new(|p| { p.rows = vec![0, 1]; p.values = vec![df(2, Agg::Sum, "Sum of Sales"), df(3, Agg::Sum, "Sum of Units")]; p.values_in_rows = true; }),
        ];
        for (k, cfg) in configs.iter().enumerate() {
            let mut b = book();
            let i = b.create_pivot(0, Rect::new(0, 0, 5, 3), 1, (2, 0)).unwrap();
            b.edit_pivot(i, "x", |p| cfg(p)).unwrap();
            let c = reopen(&b);
            assert_eq!(c.pivots.len(), 1, "config {}", k);
            let q = &c.pivots[0];
            same(&b.pivots[0], q);
            assert!(q.refreshable, "config {} not adopted", k);
            assert!(!q.dirty);
            assert_eq!(*q.roles, *b.pivots[0].roles, "config {} roles", k);
            // cells as they were
            let o = q.out.unwrap();
            for r in o.r1..=o.r2 {
                for cc in o.c1..=o.c2 {
                    assert_eq!(b.value(1, r, cc), c.value(1, r, cc), "config {} at {},{}", k, r, cc);
                }
            }
            // saved again unchanged: the pivot's parts as they were
            let d = reopen(&c);
            same(q, &d.pivots[0]);
            for (n, data) in &c.file.parts {
                if n.contains("pivot") {
                    let e = d.file.parts.iter().find(|(m, _)| m == n).unwrap_or_else(|| panic!("{} gone", n));
                    assert_eq!(&e.1, data, "{} changed", n);
                }
            }
            std::fs::write(format!("/tmp/pivot_rt_{}.xlsx", k), b.to_bytes().unwrap()).ok();
        }
    }

    #[test]
    fn edit_and_delete_after_reading() {
        let mut b = book();
        let i = b.create_pivot(0, Rect::new(0, 0, 5, 3), 1, (2, 0)).unwrap();
        b.edit_pivot(i, "x", |p| { p.rows = vec![0]; p.values = vec![df(2, Agg::Sum, "Sum of Sales")]; }).unwrap();
        let j = b.create_pivot(0, Rect::new(0, 0, 5, 3), 1, (2, 5)).unwrap();
        b.edit_pivot(j, "x", |p| { p.rows = vec![1]; p.values = vec![df(3, Agg::Sum, "Sum of Units")]; }).unwrap();
        let mut c = reopen(&b);
        assert_eq!(c.pivots.len(), 2);
        assert_ne!(c.pivots[0].cache_part, c.pivots[1].cache_part);
        // edited after reading: written anew
        c.edit_pivot(0, "x", |p| p.rows = vec![1]).unwrap();
        let d = reopen(&c);
        assert_eq!(d.pivots[0].rows, vec![1]);
        assert!(d.pivots[0].refreshable);
        // deleted: its parts go
        let mut e = d;
        let gone = e.pivots[1].table_part.clone().unwrap();
        let gone_cache = e.pivots[1].cache_part.clone().unwrap();
        e.delete_pivot(1).unwrap();
        let f = reopen(&e);
        assert_eq!(f.pivots.len(), 1);
        assert!(!f.file.parts.iter().any(|(n, _)| *n == gone || *n == gone_cache));
        let wb = String::from_utf8_lossy(&f.file.parts.iter().find(|(n, _)| n == "xl/workbook.xml").unwrap().1).to_string();
        assert_eq!(wb.matches("<pivotCache ").count(), 1, "{}", wb);
        // source edited: refresh, then save
        let mut g = f;
        g.set_input(0, 1, 2, "100").unwrap();
        g.refresh_pivot(0).unwrap();
        let h = reopen(&g);
        assert!(h.pivots[0].refreshable);
        // a sheet inserted before the pivot's sheet: the pivot follows
        let mut k = h;
        k.insert_sheet(0, None).unwrap();
        assert_eq!(k.pivots[0].sheet, 2);
        k.undo();
        assert_eq!(k.pivots[0].sheet, 1);
    }
}
