//! Slicers and timelines in the file, as Excel keeps them: a slicer part per sheet
//! (x14:slicerList in the sheet's extLst), a slicer cache per filtered field (x14:slicerCaches
//! in the workbook's extLst, with a Slicer_ name of #N/A), the drawing's graphic frame in an
//! mc:AlternateContent; timelines the same with the 2010/11 parts.

use crate::book::Book;
use crate::slicer::Slicer;
use crate::xlsx::{read_rels, rels_name, relative, remove_override, set_override};
use crate::xml::{self, Element, Node};
use std::collections::HashMap;
use std::rc::Rc;

pub const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
pub const NS_X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";
const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_SLE: &str = "http://schemas.microsoft.com/office/drawing/2010/slicer";
pub const NS_TSLE: &str = "http://schemas.microsoft.com/office/drawing/2012/timeslicer";

const T_SLICER: &str = "http://schemas.microsoft.com/office/2007/relationships/slicer";
const T_SLICER_CACHE: &str = "http://schemas.microsoft.com/office/2007/relationships/slicerCache";
const T_TIMELINE: &str = "http://schemas.microsoft.com/office/2011/relationships/timeline";
const T_TIMELINE_CACHE: &str = "http://schemas.microsoft.com/office/2011/relationships/timelineCache";
const CT_SLICER: &str = "application/vnd.ms-excel.slicer+xml";
const CT_SLICER_CACHE: &str = "application/vnd.ms-excel.slicerCache+xml";
const CT_TIMELINE: &str = "application/vnd.ms-excel.timeline+xml";
const CT_TIMELINE_CACHE: &str = "application/vnd.ms-excel.timelineCache+xml";

const URI_SHEET_SLICERS: &str = "{A8765BA9-456A-4dab-B4F3-ACF838C121DE}";
const URI_SHEET_TIMELINES: &str = "{7E03D99C-DC04-49d9-9315-930204A7B6E9}";
const URI_WB_SLICERS: &str = "{BBE1A952-AA13-448e-AADC-164F8A28A991}";
const URI_WB_TIMELINES: &str = "{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}";

fn esc(s: &str) -> String {
    xml::escape_attr(s)
}

/// What the workbook part gets: its extLst entries and the Slicer_ names.
#[derive(Default)]
pub struct WbAdds {
    pub slicer_caches: Vec<String>,
    pub timeline_caches: Vec<String>,
    pub names: Vec<String>,
    pub touched: bool,
}

fn is_slicer_part(n: &str) -> bool {
    n.contains("/slicers/") || n.contains("/slicerCaches/") || n.contains("/timelines/") || n.contains("/timelineCaches/")
}

fn date_text(b: &Book, x: f64) -> String {
    let (y, m, d) = crate::dates::serial_to_ymd(x, b.date1904).unwrap_or((1900, 1, 1));
    format!("{:04}-{:02}-{:02}T00:00:00", y, m, d)
}

fn date_serial(b: &Book, t: &str) -> Option<f64> {
    let d = t.get(0..10)?;
    let mut it = d.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let dd: i64 = it.next()?.parse().ok()?;
    crate::dates::ymd_to_serial(y, m, dd, b.date1904)
}

fn part_text(out: &[(String, Vec<u8>)], b: &Book, name: &str) -> Option<String> {
    out.iter().rev().find(|(n, _)| n == name).or_else(|| b.file.parts.iter().find(|(n, _)| n == name))
        .and_then(|(_, d)| xml::decode(d).ok())
}

fn put(out: &mut Vec<(String, Vec<u8>)>, name: &str, data: Vec<u8>) {
    out.retain(|(n, _)| n != name);
    out.push((name.to_string(), data));
}

/// Remove the ext with this uri (and an extLst left empty).
pub fn strip_ext(x: &str, uri: &str) -> String {
    let mut s = x.to_string();
    let key = format!("uri=\"{}\"", uri);
    while let Some(pos) = s.find(&key) {
        let Some(start) = s[..pos].rfind('<') else { break };
        let tag: String = s[start + 1..].chars().take_while(|c| !c.is_whitespace() && *c != '>').collect();
        let close = format!("</{}>", tag);
        let Some(e) = s[pos..].find(&close) else { break };
        s.replace_range(start..pos + e + close.len(), "");
    }
    for empty in ["<extLst></extLst>", "<extLst/>"] {
        s = s.replace(empty, "");
    }
    s
}

/// Add exts to the root's own extLst (its last child), making it if there's none.
fn add_ext(x: &str, root: &str, exts: &str) -> String {
    if exts.is_empty() {
        return x.to_string();
    }
    let close = format!("</{}>", root);
    let Some(end) = x.rfind(&close) else { return x.to_string() };
    let before = x[..end].trim_end();
    if before.ends_with("</extLst>") {
        let at = before.len() - "</extLst>".len();
        format!("{}{}{}", &x[..at], exts, &x[at..])
    } else {
        format!("{}<extLst>{}</extLst>{}", &x[..end], exts, &x[end..])
    }
}

/// The workbook with its slicer caches and names.
pub fn patch_workbook(x: String, a: &WbAdds) -> String {
    if !a.touched {
        return x;
    }
    let mut s = strip_ext(&strip_ext(&x, URI_WB_SLICERS), URI_WB_TIMELINES);
    // Slicer_ names: #N/A, as Excel has them
    let missing: Vec<&String> = a.names.iter().filter(|n| !s.contains(&format!("name=\"{}\"", esc(n)))).collect();
    if !missing.is_empty() {
        let defs: String = missing.iter().map(|n| format!("<definedName name=\"{}\">#N/A</definedName>", esc(n))).collect();
        if let Some(i) = s.find("</definedNames>") {
            s.insert_str(i, &defs);
        } else if let Some(i) = s.find("</sheets>") {
            s.insert_str(i + "</sheets>".len(), &format!("<definedNames>{}</definedNames>", defs));
        }
    }
    let mut exts = String::new();
    if !a.slicer_caches.is_empty() {
        exts.push_str(&format!("<ext uri=\"{}\" xmlns:x14=\"{}\"><x14:slicerCaches>", URI_WB_SLICERS, NS_X14));
        for r in &a.slicer_caches {
            exts.push_str(&format!("<x14:slicerCache xmlns:r=\"{}\" r:id=\"{}\"/>", NS_R, r));
        }
        exts.push_str("</x14:slicerCaches></ext>");
    }
    if !a.timeline_caches.is_empty() {
        exts.push_str(&format!("<ext uri=\"{}\" xmlns:x15=\"{}\"><x15:timelineCacheRefs>", URI_WB_TIMELINES, NS_X15));
        for r in &a.timeline_caches {
            exts.push_str(&format!("<x15:timelineCacheRef xmlns:r=\"{}\" r:id=\"{}\"/>", NS_R, r));
        }
        exts.push_str("</x15:timelineCacheRefs></ext>");
    }
    let root = if s.contains("</x:workbook>") { "x:workbook" } else { "workbook" };
    add_ext(&s, root, &exts)
}

fn new_id(used: &mut Vec<String>) -> String {
    let mut k = used.len() + 1;
    loop {
        let id = format!("rId{}", k);
        if !used.contains(&id) {
            used.push(id.clone());
            return id;
        }
        k += 1;
    }
}

/// Write the slicer and timeline parts; returns what the workbook part needs.
#[allow(clippy::too_many_arguments)]
pub fn write(b: &Book, wb_part: &str, sheet_parts: &[(String, String)], ids: &HashMap<String, (String, u32)>,
             out: &mut Vec<(String, Vec<u8>)>, dropped: &mut Vec<String>, ct: &mut Element, rels: &mut Element, used: &mut Vec<String>) -> WbAdds {
    let mut adds = WbAdds::default();
    let had = b.file.parts.iter().any(|(n, _)| is_slicer_part(n));
    let any = b.sheets.iter().any(|s| s.objects.iter().any(|o| o.slicer.is_some()));
    if !had && !any {
        return adds;
    }
    adds.touched = true;
    // the parts as read go; they're written anew
    for (n, _) in &b.file.parts {
        if is_slicer_part(n) && !n.contains("/_rels/") {
            dropped.push(n.clone());
            dropped.push(rels_name(n));
            remove_override(ct, n);
        }
    }
    rels.children.retain(|n| !matches!(n, Node::Elem(e) if matches!(e.attr("Type"), Some(T_SLICER_CACHE) | Some(T_TIMELINE_CACHE))));
    let base = match wb_part.rfind('/') {
        Some(i) => wb_part[..i].to_string(),
        None => "xl".to_string(),
    };
    // the caches: one per cache name
    let mut caches: Vec<Rc<Slicer>> = Vec::new();
    for sh in &b.sheets {
        for o in &sh.objects {
            if let Some(sl) = &o.slicer {
                if !caches.iter().any(|c| c.cache == sl.cache) {
                    caches.push(sl.clone());
                }
            }
        }
    }
    let (mut ns, mut nt) = (0, 0);
    for sl in &caches {
        let mut tables = String::new();
        let mut cache_id: Option<u32> = None;
        let mut target = None;
        for name in &sl.pivots {
            if let Some(p) = b.pivots.iter().find(|p| &p.name == name) {
                let tab = b.sheets.get(p.sheet).map(|s| s.id).unwrap_or(1);
                tables.push_str(&format!("<pivotTable tabId=\"{}\" name=\"{}\"/>", tab, esc(&p.name)));
                if cache_id.is_none() {
                    if let Some((cp, id)) = ids.get(&p.name) {
                        // the 2010 id the cache carries, else its workbook id
                        let x14 = part_text(out, b, cp).and_then(|t| {
                            let i = t.find("pivotCacheId=\"")? + 14;
                            t[i..].split('"').next()?.parse::<u32>().ok()
                        });
                        cache_id = Some(x14.unwrap_or(*id));
                    }
                    target = Some(p);
                }
            }
        }
        let cid = cache_id.unwrap_or(1);
        let (part, xmlt) = if sl.timeline {
            nt += 1;
            let part = format!("{}/timelineCaches/timelineCache{}.xml", base, nt);
            let mut s = format!("{}<timelineCacheDefinition xmlns=\"{}\" xmlns:mc=\"{}\" mc:Ignorable=\"x\" xmlns:x=\"{}\" name=\"{}\" sourceName=\"{}\"><pivotTables>{}</pivotTables>",
                                xml::DECL, NS_X15, NS_MC, NS_MAIN, esc(&sl.cache), esc(&sl.field), tables);
            let dates = target.and_then(|p| {
                let pi = b.pivots.iter().position(|q| std::ptr::eq(q, p))?;
                let f = p.fields.iter().position(|f| f.name == sl.field)?;
                Some(b.timeline_dates(pi, f))
            }).unwrap_or_default();
            s.push_str(&format!("<state minimalRefreshVersion=\"6\" lastRefreshVersion=\"6\" pivotCacheId=\"{}\" filterType=\"{}\">", cid, if sl.span.is_some() { "dateBetween" } else { "unknown" }));
            if let Some((a, z)) = sl.span {
                s.push_str(&format!("<selection startDate=\"{}\" endDate=\"{}\"/>", date_text(b, a), date_text(b, z)));
            }
            if let (Some(a), Some(z)) = (dates.first(), dates.last()) {
                s.push_str(&format!("<bounds startDate=\"{}\" endDate=\"{}\"/>", date_text(b, *a), date_text(b, z + 1.0)));
            }
            s.push_str("</state></timelineCacheDefinition>");
            (part, s)
        } else {
            ns += 1;
            let part = format!("{}/slicerCaches/slicerCache{}.xml", base, ns);
            let mut s = format!("{}<slicerCacheDefinition xmlns=\"{}\" xmlns:mc=\"{}\" mc:Ignorable=\"x\" xmlns:x=\"{}\" name=\"{}\" sourceName=\"{}\"><pivotTables>{}</pivotTables>",
                                xml::DECL, NS_X14, NS_MC, NS_MAIN, esc(&sl.cache), esc(&sl.field), tables);
            let items = target.and_then(|p| {
                let src = b.pivot_source(p)?;
                let f = p.fields.iter().position(|f| f.name == sl.field)?;
                Some(crate::pivot_xml::slicer_cache_items(b, p, &src, f))
            });
            if let Some(items) = items {
                s.push_str(&format!("<data><tabular pivotCacheId=\"{}\"><items count=\"{}\">", cid, items.len()));
                for (x, on) in items {
                    if on {
                        s.push_str(&format!("<i x=\"{}\" s=\"1\"/>", x));
                    } else {
                        s.push_str(&format!("<i x=\"{}\"/>", x));
                    }
                }
                s.push_str("</items></tabular></data>");
            }
            s.push_str("</slicerCacheDefinition>");
            (part, s)
        };
        out.push((part.clone(), xmlt.into_bytes()));
        set_override(ct, &part, if sl.timeline { CT_TIMELINE_CACHE } else { CT_SLICER_CACHE });
        let rid = new_id(used);
        rels.push(Element::with_attrs("Relationship", &[("Id", &rid), ("Type", if sl.timeline { T_TIMELINE_CACHE } else { T_SLICER_CACHE }), ("Target", &relative(wb_part, &part))]));
        if sl.timeline {
            adds.timeline_caches.push(rid);
        } else {
            adds.slicer_caches.push(rid);
        }
        adds.names.push(sl.cache.clone());
    }
    // each sheet's slicer and timeline parts
    let (mut ks, mut kt) = (0, 0);
    for (i, sh) in b.sheets.iter().enumerate() {
        if sh.raw_part.is_some() {
            continue;
        }
        let (part, _) = &sheet_parts[i];
        let slicers: Vec<&Slicer> = sh.objects.iter().filter_map(|o| o.slicer.as_deref()).filter(|s| !s.timeline).collect();
        let timelines: Vec<&Slicer> = sh.objects.iter().filter_map(|o| o.slicer.as_deref()).filter(|s| s.timeline).collect();
        let srels_name = rels_name(part);
        let old_rels = part_text(out, b, &srels_name);
        let had_here = old_rels.as_deref().map(|t| t.contains(T_SLICER) || t.contains(T_TIMELINE)).unwrap_or(false);
        if slicers.is_empty() && timelines.is_empty() && !had_here {
            continue;
        }
        let (pro, mut srels) = old_rels.and_then(|t| xml::parse(&t).ok())
            .unwrap_or_else(|| (String::new(), Element::with_attrs("Relationships", &[("xmlns", "http://schemas.openxmlformats.org/package/2006/relationships")])));
        srels.children.retain(|n| !matches!(n, Node::Elem(e) if matches!(e.attr("Type"), Some(T_SLICER) | Some(T_TIMELINE))));
        let mut sused: Vec<String> = srels.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
        let mut exts = String::new();
        if !slicers.is_empty() {
            ks += 1;
            let sp = format!("{}/slicers/slicer{}.xml", base, ks);
            let mut s = format!("{}<slicers xmlns=\"{}\" xmlns:mc=\"{}\" mc:Ignorable=\"x\" xmlns:x=\"{}\">", xml::DECL, NS_X14, NS_MC, NS_MAIN);
            for sl in &slicers {
                s.push_str(&format!("<slicer name=\"{}\" cache=\"{}\" caption=\"{}\"", esc(&sl.name), esc(&sl.cache), esc(&sl.caption)));
                if sl.columns > 1 {
                    s.push_str(&format!(" columnCount=\"{}\"", sl.columns));
                }
                if !sl.show_header {
                    s.push_str(" showCaption=\"0\"");
                }
                s.push_str(&format!(" style=\"{}\" rowHeight=\"241300\"/>", esc(&sl.style)));
            }
            s.push_str("</slicers>");
            out.push((sp.clone(), s.into_bytes()));
            set_override(ct, &sp, CT_SLICER);
            let rid = new_id(&mut sused);
            srels.push(Element::with_attrs("Relationship", &[("Id", &rid), ("Type", T_SLICER), ("Target", &relative(part, &sp))]));
            exts.push_str(&format!("<ext uri=\"{}\" xmlns:x14=\"{}\"><x14:slicerList><x14:slicer xmlns:r=\"{}\" r:id=\"{}\"/></x14:slicerList></ext>", URI_SHEET_SLICERS, NS_X14, NS_R, rid));
        }
        if !timelines.is_empty() {
            kt += 1;
            let tp = format!("{}/timelines/timeline{}.xml", base, kt);
            let mut s = format!("{}<timelines xmlns=\"{}\" xmlns:mc=\"{}\" mc:Ignorable=\"x\" xmlns:x=\"{}\">", xml::DECL, NS_X15, NS_MC, NS_MAIN);
            for sl in &timelines {
                s.push_str(&format!("<timeline name=\"{}\" cache=\"{}\" caption=\"{}\" level=\"{}\" selectionLevel=\"{}\"",
                                    esc(&sl.name), esc(&sl.cache), esc(&sl.caption), sl.level, sl.level));
                if let Some((a, _)) = sl.span {
                    s.push_str(&format!(" scrollPosition=\"{}\"", date_text(b, a)));
                }
                if !sl.show_header {
                    s.push_str(" showHeader=\"0\"");
                }
                s.push_str(&format!(" style=\"{}\"/>", esc(&sl.style)));
            }
            s.push_str("</timelines>");
            out.push((tp.clone(), s.into_bytes()));
            set_override(ct, &tp, CT_TIMELINE);
            let rid = new_id(&mut sused);
            srels.push(Element::with_attrs("Relationship", &[("Id", &rid), ("Type", T_TIMELINE), ("Target", &relative(part, &tp))]));
            exts.push_str(&format!("<ext uri=\"{}\" xmlns:x15=\"{}\"><x15:timelineRefs><x15:timelineRef xmlns:r=\"{}\" r:id=\"{}\"/></x15:timelineRefs></ext>", URI_SHEET_TIMELINES, NS_X15, NS_R, rid));
        }
        put(out, &srels_name, xml::document(&pro, &srels).into_bytes());
        if let Some(sx) = part_text(out, b, part) {
            let sx = strip_ext(&strip_ext(&sx, URI_SHEET_SLICERS), URI_SHEET_TIMELINES);
            let root = if sx.contains("</x:worksheet>") { "x:worksheet" } else { "worksheet" };
            put(out, part, add_ext(&sx, root, &exts).into_bytes());
        }
    }
    adds
}

/// The slicers and timelines of a workbook just read: their objects in the drawings, the
/// pivot fields they filter.
pub fn read(b: &mut Book) {
    let parts = b.file.parts.clone();
    let wb = b.file.wb_part.clone();
    let get = |n: &str| parts.iter().find(|(m, _)| m == n).and_then(|(_, d)| xml::decode(d).ok()).and_then(|t| xml::parse(&t).ok()).map(|x| x.1);
    // caches: name -> (field, pivots, span)
    let mut caches: HashMap<String, (String, Vec<String>, Option<(f64, f64)>, String)> = HashMap::new();
    for (_, typ, target, ext) in read_rels(&wb, &parts) {
        if ext || !(typ == T_SLICER_CACHE || typ == T_TIMELINE_CACHE) {
            continue;
        }
        let Some(c) = get(&target) else { continue };
        let name = c.attr("name").unwrap_or("").to_string();
        let field = c.attr("sourceName").unwrap_or("").to_string();
        let pivots: Vec<String> = c.child("pivotTables").map(|p| p.elems().filter_map(|e| e.attr("name").map(|s| s.to_string())).collect()).unwrap_or_default();
        let span = c.child("state").and_then(|s| s.child("selection")).and_then(|sel| {
            Some((date_serial(b, sel.attr("startDate")?)?, date_serial(b, sel.attr("endDate")?)?))
        });
        caches.insert(name, (field, pivots, span, target));
    }
    if caches.is_empty() {
        return;
    }
    for si in 0..b.sheets.len() {
        let part = b.sheets[si].part.clone();
        if part.is_empty() {
            continue;
        }
        let mut found: HashMap<String, Slicer> = HashMap::new();
        for (_, typ, target, ext) in read_rels(&part, &parts) {
            if ext || !(typ == T_SLICER || typ == T_TIMELINE) {
                continue;
            }
            let Some(root) = get(&target) else { continue };
            let timeline = typ == T_TIMELINE;
            for e in root.elems() {
                let name = e.attr("name").unwrap_or("").to_string();
                let cache = e.attr("cache").unwrap_or("").to_string();
                let Some((field, pivots, span, cpart)) = caches.get(&cache) else { continue };
                let mut sl = Slicer::new(&name, field, pivots.first().map(|s| s.as_str()).unwrap_or(""), timeline);
                sl.pivots = pivots.clone();
                sl.cache = cache.clone();
                sl.caption = e.attr("caption").map(|s| s.to_string()).unwrap_or_else(|| field.clone());
                if let Some(c) = e.attr("columnCount").and_then(|v| v.parse().ok()) {
                    sl.columns = c;
                }
                if let Some(st) = e.attr("style") {
                    sl.style = st.to_string();
                }
                let hdr = if timeline { "showHeader" } else { "showCaption" };
                sl.show_header = !matches!(e.attr(hdr), Some("0") | Some("false"));
                if timeline {
                    sl.level = e.attr("level").and_then(|v| v.parse().ok()).unwrap_or(2).min(3);
                    sl.span = *span;
                }
                sl.part = Some(target.clone());
                sl.cache_part = Some(cpart.clone());
                found.insert(name, sl);
            }
        }
        if found.is_empty() {
            continue;
        }
        let mut fields: Vec<(String, String)> = Vec::new();
        for o in b.sheets[si].objects.iter_mut() {
            let Some(raw) = &o.raw else { continue };
            let mut x = String::new();
            raw.write(&mut x);
            let name = ["<sle:slicer ", "<tsle:timeslicer ", "slicer name=", "timeslicer name="].iter().find_map(|k| {
                let i = x.find(k)?;
                let j = x[i..].find("name=\"")? + i + 6;
                Some(xml::unescape(x[j..].split('"').next()?))
            });
            if let Some(sl) = name.and_then(|n| found.remove(&n)) {
                fields.push((sl.field.clone(), sl.pivots.first().cloned().unwrap_or_default()));
                o.name = sl.name.clone();
                o.slicer = Some(Rc::new(sl));
                o.raw = None;
            }
        }
        for (field, pivot) in fields {
            for p in b.pivots.iter_mut().filter(|p| p.name == pivot) {
                if let Some(f) = p.fields.iter().position(|x| x.name == field) {
                    if !p.slicer_fields.contains(&f) {
                        p.slicer_fields.push(f);
                    }
                }
            }
        }
    }
}

/// A slicer's drawing object: the graphic frame for applications that know slicers, a
/// note in a box for the rest.
pub fn drawing_body(sl: &Slicer, id: usize, cx: i64, cy: i64) -> (String, String) {
    let (uri, el) = if sl.timeline {
        (NS_TSLE, format!("<tsle:timeslicer xmlns:tsle=\"{}\" name=\"{}\"/>", NS_TSLE, esc(&sl.name)))
    } else {
        (NS_SLE, format!("<sle:slicer xmlns:sle=\"{}\" name=\"{}\"/>", NS_SLE, esc(&sl.name)))
    };
    let frame = format!("<xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{id}\" name=\"{n}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>\
                         <xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{uri}\">{el}</a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/>",
                        id = id, n = esc(&sl.name), uri = uri, el = el);
    let what = if sl.timeline { "timeline" } else { "slicer" };
    let note = format!("<xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"0\" name=\"\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr>\
                        <xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:prstClr val=\"white\"/></a:solidFill><a:ln w=\"1\"><a:solidFill><a:prstClr val=\"green\"/></a:solidFill></a:ln></xdr:spPr>\
                        <xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\"/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"1100\"/><a:t>This shape represents a {what}. It needs a spreadsheet application that supports {what}s.</a:t></a:r></a:p></xdr:txBody></xdr:sp><xdr:clientData/>",
                       cx = cx, cy = cy, what = what);
    (frame, note)
}

/// mc:AlternateContent requirement for a slicer (a14) or timeline (a15).
pub fn requires(sl: &Slicer) -> (&'static str, &'static str) {
    if sl.timeline {
        ("a15", "http://schemas.microsoft.com/office/drawing/2012/main")
    } else {
        ("a14", "http://schemas.microsoft.com/office/drawing/2010/main")
    }
}
