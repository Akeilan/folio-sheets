//! The newer chart types (the chartEx part): Waterfall, Funnel, Histogram, Pareto, Box and
//! Whisker, Treemap and Sunburst. Read from and written to the file as Excel does, drawn here.
use crate::book::Book;
use crate::cellref::{col_name, quote_sheet, Rect};
use crate::chart::{auto_color, CVal, Src};
use crate::ffi::*;
use crate::render::Pen;
use crate::xml::{self, Element};
use std::f64::consts::PI;

pub const NS_CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
pub const T_CHARTEX: &str = "http://schemas.microsoft.com/office/2014/relationships/chartEx";
pub const CT_CHARTEX: &str = "application/vnd.ms-office.chartex+xml";
pub const URI_CHARTEX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

const TEXT: u32 = 0x595959;
const LINE: u32 = 0xD9D9D9;

pub const KINDS: [&str; 7] = ["waterfall", "funnel", "histogram", "pareto", "boxwhisker", "treemap", "sunburst"];

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ExSeries {
    pub name: Option<Src>,
    pub val: Src,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ChartEx {
    /// waterfall, funnel, histogram, pareto, boxwhisker, treemap, sunburst
    pub kind: String,
    /// None: the series name, as the original shows
    pub title: Option<String>,
    /// category levels, the leaf level first (several for treemap and sunburst)
    pub cats: Vec<Src>,
    pub series: Vec<ExSeries>,
    /// waterfall points that are totals
    pub subtotals: Vec<usize>,
    /// the part as read, written back while nothing the model shows changed
    pub orig: Option<String>,
    pub orig_model: Option<Box<ChartEx>>,
}

fn layout_id(kind: &str) -> &'static str {
    match kind {
        "waterfall" => "waterfall",
        "funnel" => "funnel",
        "histogram" => "clusteredColumn",
        "pareto" => "clusteredColumn",
        "boxwhisker" => "boxWhisker",
        "treemap" => "treemap",
        "sunburst" => "sunburst",
        _ => "clusteredColumn",
    }
}

/// The Requires / namespace pair of the drawing's mc:Choice for a kind.
pub fn requires(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "funnel" => ("cx2", "http://schemas.microsoft.com/office/drawing/2015/10/21/chartex"),
        _ => ("cx1", "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex"),
    }
}

fn txt(e: &Element) -> String {
    e.text()
}

fn read_dim(d: &Element) -> Vec<Src> {
    // one Src per level; the reference covers all levels (columns), so each level gets its column
    let f = d.child("f").map(txt);
    let lvls: Vec<&Element> = d.elems().filter(|e| e.local() == "lvl").collect();
    let n = lvls.len().max(1);
    let mut out = Vec::new();
    for (k, l) in lvls.iter().enumerate() {
        let cnt: usize = l.attr("ptCount").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut cache = vec![CVal::Empty; cnt];
        for p in l.elems().filter(|e| e.local() == "pt") {
            let i: usize = p.attr("idx").and_then(|v| v.parse().ok()).unwrap_or(0);
            if i >= cache.len() {
                cache.resize(i + 1, CVal::Empty);
            }
            let t = txt(p);
            cache[i] = if d.local() == "numDim" { t.trim().parse::<f64>().map(CVal::Num).unwrap_or(CVal::Empty) } else { CVal::Str(t) };
        }
        // a multi-level reference: level k is column (last - k) of the block
        let fk = match (&f, n) {
            (Some(f), 1) => Some(f.clone()),
            (Some(f), _) => level_ref(f, n - 1 - k),
            _ => None,
        };
        out.push(Src { f: fk, cache, fmt: l.attr("formatCode").map(|s| s.to_string()) });
    }
    if out.is_empty() {
        out.push(Src { f, cache: vec![], fmt: None });
    }
    out
}

/// One column (0-based) of a block reference "Sheet1!$A$2:$C$9".
fn level_ref(f: &str, col: usize) -> Option<String> {
    let (sheet, rest) = f.rsplit_once('!')?;
    let rc = crate::cellref::parse_range(&rest.replace('$', ""))?;
    let c = rc.c1 + col as u32;
    Some(format!("{}!${}${}:${}${}", sheet, col_name(c), rc.r1 + 1, col_name(c), rc.r2 + 1))
}

impl ChartEx {
    pub fn parse(text: &str) -> Result<ChartEx, String> {
        let (_, root) = xml::parse(text)?;
        let mut cx = ChartEx::default();
        let data = root.child("chartData").ok_or("no chart data")?;
        let mut dims: Vec<(String, Vec<Src>, Vec<Src>)> = Vec::new(); // id, str, num
        for d in data.elems().filter(|e| e.local() == "data") {
            let id = d.attr("id").unwrap_or("0").to_string();
            let mut s = Vec::new();
            let mut n = Vec::new();
            for dim in d.elems() {
                match dim.local() {
                    "strDim" => s = read_dim(dim),
                    "numDim" => n = read_dim(dim),
                    _ => {}
                }
            }
            dims.push((id, s, n));
        }
        let chart = root.child("chart").ok_or("no chart")?;
        if let Some(t) = chart.child("title") {
            cx.title = t.child("tx").and_then(|x| x.child("txData")).and_then(|x| x.child("v")).map(txt)
                .or_else(|| t.child("tx").and_then(|x| x.child("rich")).map(|r| r.text()));
        }
        let region = chart.child("plotArea").and_then(|p| p.child("plotAreaRegion")).ok_or("no plot area")?;
        for se in region.elems().filter(|e| e.local() == "series") {
            let lid = se.attr("layoutId").unwrap_or("");
            if lid == "paretoLine" {
                cx.kind = "pareto".into();
                continue;
            }
            if cx.kind.is_empty() {
                cx.kind = match lid {
                    "waterfall" => "waterfall",
                    "funnel" => "funnel",
                    "boxWhisker" => "boxwhisker",
                    "treemap" => "treemap",
                    "sunburst" => "sunburst",
                    _ => "histogram",
                }.to_string();
            }
            let did = se.child("dataId").and_then(|d| d.attr("val")).unwrap_or("0");
            let name = se.child("tx").and_then(|t| t.child("txData")).map(|td| Src {
                f: td.child("f").map(txt),
                cache: td.child("v").map(|v| vec![CVal::Str(txt(v))]).unwrap_or_default(),
                fmt: None,
            });
            if let Some((_, s, n)) = dims.iter().find(|d| d.0 == did) {
                if cx.cats.is_empty() {
                    cx.cats = s.clone();
                }
                if let Some(v) = n.first() {
                    cx.series.push(ExSeries { name, val: v.clone() });
                }
            }
            if let Some(st) = se.child("layoutPr").and_then(|l| l.child("subtotals")) {
                cx.subtotals = st.elems().filter_map(|e| e.attr("val").and_then(|v| v.parse().ok())).collect();
            }
        }
        if cx.kind.is_empty() {
            return Err("an unknown kind of chart".into());
        }
        cx.orig = Some(text.to_string());
        let mut m = cx.clone();
        m.orig = None;
        cx.orig_model = Some(Box::new(m));
        Ok(cx)
    }

    fn model_changed(&self) -> bool {
        match &self.orig_model {
            Some(m) => {
                let mut me = self.clone();
                me.orig = None;
                me.orig_model = None;
                **m != me
            }
            None => true,
        }
    }

    pub fn to_xml(&self, book: &Book) -> String {
        if let (Some(o), false) = (&self.orig, self.model_changed()) {
            return o.clone();
        }
        let esc = xml::escape_text;
        let mut s = String::from(xml::DECL);
        s.push_str(&format!("<cx:chartSpace xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:cx=\"{}\">", NS_CX));
        s.push_str("<cx:chartData>");
        let hier = matches!(self.kind.as_str(), "treemap" | "sunburst");
        for (k, se) in self.series.iter().enumerate() {
            s.push_str(&format!("<cx:data id=\"{}\">", k));
            if !self.cats.is_empty() {
                // one reference over all levels, the leaf level written first
                let f = if hier && self.cats.len() > 1 {
                    let mut lo: Option<(String, Rect)> = None;
                    let mut hi: Option<Rect> = None;
                    for c in &self.cats {
                        if let Some((sh, rc)) = c.f.as_deref().and_then(split_ref) {
                            lo = Some(match lo {
                                None => (sh, rc),
                                Some((s0, r0)) => (s0, if rc.c1 < r0.c1 { rc } else { r0 }),
                            });
                            hi = Some(match hi {
                                None => rc,
                                Some(r0) => if rc.c2 > r0.c2 { rc } else { r0 },
                            });
                        }
                    }
                    match (lo, hi) {
                        (Some((sh, a)), Some(b)) => Some(format!("{}!${}${}:${}${}", sh, col_name(a.c1), a.r1 + 1, col_name(b.c2), b.r2 + 1)),
                        _ => None,
                    }
                } else {
                    self.cats[0].f.clone()
                };
                s.push_str("<cx:strDim type=\"cat\">");
                if let Some(f) = f {
                    s.push_str(&format!("<cx:f>{}</cx:f>", esc(&f)));
                }
                for c in &self.cats {
                    let pts = book.chart_values(c);
                    s.push_str(&format!("<cx:lvl ptCount=\"{}\">", pts.len()));
                    for (i, p) in pts.iter().enumerate() {
                        s.push_str(&format!("<cx:pt idx=\"{}\">{}</cx:pt>", i, esc(&p.text)));
                    }
                    s.push_str("</cx:lvl>");
                }
                s.push_str("</cx:strDim>");
            }
            let pts = book.chart_values(&se.val);
            s.push_str(&format!("<cx:numDim type=\"{}\">", if hier { "size" } else { "val" }));
            if let Some(f) = &se.val.f {
                s.push_str(&format!("<cx:f>{}</cx:f>", esc(f)));
            }
            s.push_str(&format!("<cx:lvl ptCount=\"{}\" formatCode=\"{}\">", pts.len(), xml::escape_attr(&book.chart_format(&se.val))));
            for (i, p) in pts.iter().enumerate() {
                if let Some(v) = p.v {
                    s.push_str(&format!("<cx:pt idx=\"{}\">{}</cx:pt>", i, crate::xlsx::num_text(v)));
                }
            }
            s.push_str("</cx:lvl></cx:numDim></cx:data>");
        }
        s.push_str("</cx:chartData><cx:chart>");
        let title = self.title.clone().unwrap_or_else(|| self.series.first().and_then(|x| x.name.as_ref()).map(|n| book.chart_values(n).first().map(|p| p.text.clone()).unwrap_or_default()).unwrap_or_default());
        if !title.is_empty() {
            s.push_str(&format!("<cx:title pos=\"t\" align=\"ctr\" overlay=\"0\"><cx:tx><cx:txData><cx:v>{}</cx:v></cx:txData></cx:tx></cx:title>", esc(&title)));
        }
        s.push_str("<cx:plotArea><cx:plotAreaRegion>");
        for (k, se) in self.series.iter().enumerate() {
            s.push_str(&format!("<cx:series layoutId=\"{}\" uniqueId=\"{{0000000{}-F011-4E0A-8C7A-6F5D2B1E{:04X}}}\">", layout_id(&self.kind), k % 10, k));
            if let Some(n) = &se.name {
                s.push_str("<cx:tx><cx:txData>");
                if let Some(f) = &n.f {
                    s.push_str(&format!("<cx:f>{}</cx:f>", esc(f)));
                }
                let v = book.chart_values(n).first().map(|p| p.text.clone()).unwrap_or_default();
                s.push_str(&format!("<cx:v>{}</cx:v></cx:txData></cx:tx>", esc(&v)));
            }
            s.push_str(&format!("<cx:dataId val=\"{}\"/>", k));
            match self.kind.as_str() {
                "waterfall" => {
                    s.push_str("<cx:layoutPr>");
                    if !self.subtotals.is_empty() {
                        s.push_str("<cx:subtotals>");
                        for i in &self.subtotals {
                            s.push_str(&format!("<cx:idx val=\"{}\"/>", i));
                        }
                        s.push_str("</cx:subtotals>");
                    }
                    s.push_str("</cx:layoutPr>");
                }
                "histogram" | "pareto" => s.push_str("<cx:layoutPr><cx:binning intervalClosed=\"r\"/></cx:layoutPr>"),
                "boxwhisker" => s.push_str("<cx:layoutPr><cx:visibility meanLine=\"0\" meanMarker=\"1\" nonoutliers=\"0\" outliers=\"1\"/><cx:statistics quartileMethod=\"exclusive\"/></cx:layoutPr>"),
                "treemap" => s.push_str("<cx:layoutPr><cx:parentLabelLayout val=\"overlapping\"/></cx:layoutPr>"),
                _ => {}
            }
            s.push_str("</cx:series>");
            if self.kind == "pareto" {
                s.push_str(&format!("<cx:series layoutId=\"paretoLine\" ownerIdx=\"{}\"><cx:axisId val=\"2\"/></cx:series>", k));
            }
        }
        s.push_str("</cx:plotAreaRegion>");
        match self.kind.as_str() {
            "treemap" | "sunburst" => {}
            "funnel" => s.push_str("<cx:axis id=\"0\"><cx:catScaling gapWidth=\"0.06\"/><cx:tickLabels/></cx:axis>"),
            _ => {
                s.push_str("<cx:axis id=\"0\"><cx:catScaling gapWidth=\"0.5\"/><cx:tickLabels/></cx:axis>");
                s.push_str("<cx:axis id=\"1\"><cx:valScaling/><cx:majorGridlines/><cx:tickLabels/></cx:axis>");
                if self.kind == "pareto" {
                    s.push_str("<cx:axis id=\"2\"><cx:valScaling max=\"1\" min=\"0\"/><cx:units unit=\"percentage\"/><cx:tickLabels/></cx:axis>");
                }
            }
        }
        s.push_str("</cx:plotArea>");
        if matches!(self.kind.as_str(), "waterfall" | "treemap" | "boxwhisker") {
            s.push_str("<cx:legend pos=\"t\" align=\"ctr\" overlay=\"0\"/>");
        }
        s.push_str("</cx:chart></cx:chartSpace>");
        s
    }

    /// The references move with inserted or deleted cells and renamed sheets.
    pub fn adjust(&mut self, how: &crate::formula::Adjust) -> bool {
        let mut ch = false;
        let mut fix = |src: &mut Src| {
            if let Some(f) = &src.f {
                let t = crate::formula::adjust(f, how);
                if t != *f {
                    src.f = Some(t);
                    ch = true;
                }
            }
        };
        for c in self.cats.iter_mut() {
            fix(c);
        }
        for se in self.series.iter_mut() {
            fix(&mut se.val);
            if let Some(n) = se.name.as_mut() {
                fix(n);
            }
        }
        ch
    }
}

fn split_ref(f: &str) -> Option<(String, Rect)> {
    let (sh, rest) = f.rsplit_once('!')?;
    Some((sh.to_string(), crate::cellref::parse_range(&rest.replace('$', ""))?))
}

fn abs(sheet: &str, rc: Rect) -> String {
    format!("{}!${}${}:${}${}", quote_sheet(sheet), col_name(rc.c1), rc.r1 + 1, col_name(rc.c2), rc.r2 + 1)
}

/// The drawing body for a new chartEx frame (its reference id is set when written).
pub fn frame(id: u32, name: &str) -> Vec<xml::Node> {
    let s = format!("<x><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{}\" name=\"{}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"{}\"><cx:chart xmlns:cx=\"{}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"rId1\"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></x>",
        id, xml::escape_attr(name), URI_CHARTEX, NS_CX);
    xml::parse(&s).map(|(_, e)| e.children).unwrap_or_default()
}

/// What older programs show instead (the mc:Fallback).
pub fn fallback_body(id: u32) -> String {
    format!("<xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{}\" name=\"\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"4572000\" cy=\"2743200\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:prstClr val=\"white\"/></a:solidFill><a:ln w=\"1\"><a:solidFill><a:prstClr val=\"green\"/></a:solidFill></a:ln></xdr:spPr><xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\"/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"1100\"/><a:t>This chart isn't available in your version of the spreadsheet program.</a:t></a:r></a:p></xdr:txBody></xdr:sp><xdr:clientData/>", id + 1000)
}

// ---------------------------------------------------------------------- statistics

fn quantile_excl(v: &[f64], q: f64) -> f64 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return v[0];
    }
    let pos = (q * (n as f64 + 1.0)).clamp(1.0, n as f64);
    let lo = pos.floor() as usize;
    let frac = pos - lo as f64;
    let a = v[lo - 1];
    let b = v[(lo).min(n - 1)];
    a + (b - a) * frac
}

/// Histogram bins (Scott's rule, as the original's automatic bins): (labels, counts).
pub fn bins(vals: &[f64]) -> (Vec<String>, Vec<f64>) {
    if vals.is_empty() {
        return (vec![], vec![]);
    }
    let n = vals.len() as f64;
    let mean = vals.iter().sum::<f64>() / n;
    let sd = (vals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
    let min = vals.iter().cloned().fold(f64::MAX, f64::min);
    let max = vals.iter().cloned().fold(f64::MIN, f64::max);
    let mut w = 3.5 * sd / n.cbrt();
    if !(w > 0.0) {
        w = 1.0;
    }
    let k = (((max - min) / w).ceil() as usize).clamp(1, 60);
    let w = if max > min { (max - min) / k as f64 } else { 1.0 };
    let mut counts = vec![0.0; k];
    for &x in vals {
        let mut i = ((x - min) / w).ceil() as usize;
        i = i.saturating_sub(1).min(k - 1);
        counts[i] += 1.0;
    }
    let f = |x: f64| { let r = (x * 100.0).round() / 100.0; crate::numfmt::num_to_text(r) };
    let labels = (0..k).map(|i| {
        let a = min + i as f64 * w;
        format!("{}{}, {}]", if i == 0 { "[" } else { "(" }, f(a), f(a + w))
    }).collect();
    (labels, counts)
}

struct Node {
    label: String,
    value: f64,
    kids: Vec<Node>,
}

fn tree(levels: &[Vec<String>], vals: &[f64]) -> Node {
    // levels: [root-most .. leaf] per row
    let mut root = Node { label: String::new(), value: 0.0, kids: vec![] };
    for (i, path) in levels.iter().enumerate() {
        let v = vals.get(i).copied().unwrap_or(0.0).max(0.0);
        let mut cur = &mut root;
        cur.value += v;
        for name in path.iter().filter(|n| !n.is_empty()) {
            let k = match cur.kids.iter().position(|c| &c.label == name) {
                Some(k) => k,
                None => {
                    cur.kids.push(Node { label: name.clone(), value: 0.0, kids: vec![] });
                    cur.kids.len() - 1
                }
            };
            cur = &mut cur.kids[k];
            cur.value += v;
        }
    }
    root
}

/// Squarified treemap of values into (x, y, w, h).
fn squarify(vals: &[f64], x: f64, y: f64, w: f64, h: f64) -> Vec<(f64, f64, f64, f64)> {
    let total: f64 = vals.iter().sum();
    let mut out = vec![(0.0, 0.0, 0.0, 0.0); vals.len()];
    if total <= 0.0 || vals.is_empty() {
        return out;
    }
    let mut idx: Vec<usize> = (0..vals.len()).collect();
    idx.sort_by(|a, b| vals[*b].partial_cmp(&vals[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let (mut x, mut y, mut w, mut h) = (x, y, w, h);
    let mut rest: f64 = total;
    let mut i = 0;
    while i < idx.len() {
        let side = w.min(h);
        let area_k = w * h / rest;
        let mut row = vec![idx[i]];
        let worst = |row: &[usize]| -> f64 {
            let s: f64 = row.iter().map(|&k| vals[k] * area_k).sum();
            let mx = row.iter().map(|&k| vals[k] * area_k).fold(0.0, f64::max);
            let mn = row.iter().map(|&k| vals[k] * area_k).fold(f64::MAX, f64::min);
            ((side * side * mx) / (s * s)).max((s * s) / (side * side * mn.max(1e-9)))
        };
        let mut j = i + 1;
        while j < idx.len() {
            let mut cand = row.clone();
            cand.push(idx[j]);
            if worst(&cand) > worst(&row) {
                break;
            }
            row = cand;
            j += 1;
        }
        let s: f64 = row.iter().map(|&k| vals[k]).sum();
        if w >= h {
            let rw = if rest > 0.0 { w * s / rest } else { 0.0 };
            let mut yy = y;
            for &k in &row {
                let hh = if s > 0.0 { h * vals[k] / s } else { 0.0 };
                out[k] = (x, yy, rw, hh);
                yy += hh;
            }
            x += rw;
            w -= rw;
        } else {
            let rh = if rest > 0.0 { h * s / rest } else { 0.0 };
            let mut xx = x;
            for &k in &row {
                let ww = if s > 0.0 { w * vals[k] / s } else { 0.0 };
                out[k] = (xx, y, ww, rh);
                xx += ww;
            }
            y += rh;
            h -= rh;
        }
        rest -= s;
        i = j;
    }
    out
}

unsafe fn rgb(cr: *mut cairo_t, c: u32) {
    cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
}

fn light(c: u32, k: f64) -> u32 {
    let f = |v: u32| ((v as f64) + (255.0 - v as f64) * k).round().min(255.0) as u32;
    (f((c >> 16) & 255) << 16) | (f((c >> 8) & 255) << 8) | f(c & 255)
}

impl Book {
    /// A new chart of a kind from the selected cells: text columns are the categories
    /// (several for treemap and sunburst), number columns the values.
    pub fn chartex_from_range(&self, s: usize, rc: Rect, kind: &str) -> Result<ChartEx, String> {
        if !KINDS.contains(&kind) {
            return Err("an unknown kind of chart".into());
        }
        let sh = &self.sheets[s];
        let rc = sh.clamp_rect(rc);
        let header = (rc.c1..=rc.c2).any(|c| matches!(sh.value(rc.r1, c), crate::value::Value::Str(_)))
            && (rc.c1..=rc.c2).any(|c| matches!(sh.value((rc.r1 + 1).min(rc.r2), c), crate::value::Value::Num(_)));
        let r1 = if header && rc.r2 > rc.r1 { rc.r1 + 1 } else { rc.r1 };
        let name = sh.name.clone();
        let numeric = |c: u32| (r1..=rc.r2).any(|r| matches!(sh.value(r, c), crate::value::Value::Num(_)))
            && !(r1..=rc.r2).any(|r| matches!(sh.value(r, c), crate::value::Value::Str(_)));
        let text_cols: Vec<u32> = (rc.c1..=rc.c2).filter(|&c| !numeric(c)).collect();
        let num_cols: Vec<u32> = (rc.c1..=rc.c2).filter(|&c| numeric(c)).collect();
        if num_cols.is_empty() {
            return Err("Select cells with numbers to chart.".into());
        }
        let col = |c: u32| Src { f: Some(abs(&name, Rect::new(r1, c, rc.r2, c))), cache: vec![], fmt: None };
        let mut cx = ChartEx { kind: kind.to_string(), ..Default::default() };
        let hier = matches!(kind, "treemap" | "sunburst");
        // categories: the leaf (rightmost) text column first
        if hier {
            for &c in text_cols.iter().rev() {
                cx.cats.push(col(c));
            }
        } else if let Some(&c) = text_cols.first() {
            cx.cats.push(col(c));
        }
        let series_cols: Vec<u32> = if matches!(kind, "boxwhisker") { num_cols.clone() } else { vec![num_cols[0]] };
        for c in series_cols {
            let nm = if header { Some(Src { f: Some(format!("{}!${}${}", quote_sheet(&name), col_name(c), rc.r1 + 1)), cache: vec![], fmt: None }) } else { None };
            cx.series.push(ExSeries { name: nm, val: col(c) });
        }
        Ok(cx)
    }

    pub fn chartex_title(&self, cx: &ChartEx) -> String {
        if let Some(t) = &cx.title {
            return t.clone();
        }
        match cx.kind.as_str() {
            "treemap" | "sunburst" | "waterfall" | "funnel" | "histogram" | "pareto" | "boxwhisker" => {
                cx.series.first().and_then(|s| s.name.as_ref()).map(|n| self.chart_values(n).first().map(|p| p.text.clone()).unwrap_or_default())
                    .filter(|t| !t.is_empty()).unwrap_or_else(|| "Chart Title".into())
            }
            _ => String::new(),
        }
    }

    pub(crate) unsafe fn paint_chartex(&self, cx: &ChartEx, cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64, z: f64, pen: &mut Pen) {
        if w < 4.0 || h < 4.0 {
            return;
        }
        let theme = self.styles.theme;
        let col = |i: usize| auto_color(&theme, 0, i);
        cairo_save(cr);
        cairo_rectangle(cr, x, y, w, h);
        cairo_clip(cr);
        rgb(cr, 0xFFFFFF);
        cairo_rectangle(cr, x, y, w, h);
        cairo_fill(cr);
        rgb(cr, LINE);
        cairo_set_line_width(cr, 1.0);
        cairo_rectangle(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0);
        cairo_stroke(cr);
        let font = |size: f64, bold: bool| crate::layout::FontSpec { family: self.styles.theme_fonts.1.clone(), size, bold, italic: false, underline: 0, strike: false, vert: 0, color: TEXT };
        let pad = (8.0 * z).max(2.0);
        let (l, mut t, r, b) = (x + pad, y + pad, x + w - pad, y + h - pad);
        let title = self.chartex_title(cx);
        pen.set_align(PANGO_ALIGN_CENTER);
        if !title.is_empty() {
            let (_, th) = pen.set(&title, &font(14.0, false), z, Some(r - l));
            rgb(cr, TEXT);
            pen.show(cr, l, t);
            t += th + 6.0 * z;
        }
        pen.set_align(PANGO_ALIGN_LEFT);
        let cats: Vec<String> = cx.cats.first().map(|c| self.chart_values(c).into_iter().map(|p| p.text).collect()).unwrap_or_default();
        let vals: Vec<Option<f64>> = cx.series.first().map(|s| self.chart_values(&s.val).into_iter().map(|p| p.v).collect()).unwrap_or_default();
        let small = font(9.0, false);
        let mut label = |pen: &mut Pen, text: &str, cx0: f64, cy0: f64, center: bool, color: u32, maxw: Option<f64>| {
            let (tw, th) = pen.set(text, &small, z, maxw);
            rgb(cr, color);
            pen.show(cr, if center { cx0 - tw / 2.0 } else { cx0 }, cy0 - th / 2.0);
            (tw, th)
        };
        match cx.kind.as_str() {
            "treemap" => {
                // levels root-most first
                let lv: Vec<Vec<String>> = cx.cats.iter().rev().map(|c| self.chart_values(c).into_iter().map(|p| p.text).collect()).collect();
                let n = vals.len();
                let paths: Vec<Vec<String>> = (0..n).map(|i| lv.iter().map(|l| l.get(i).cloned().unwrap_or_default()).collect()).collect();
                let root = tree(&paths, &vals.iter().map(|v| v.unwrap_or(0.0)).collect::<Vec<_>>());
                let tops: Vec<f64> = root.kids.iter().map(|k| k.value).collect();
                let rects = squarify(&tops, l, t, r - l, b - t);
                for (k, (node, rc)) in root.kids.iter().zip(rects.iter()).enumerate() {
                    let c = col(k);
                    let leaves: Vec<&Node> = if node.kids.is_empty() { vec![node] } else { node.kids.iter().collect() };
                    let lr = squarify(&leaves.iter().map(|x| x.value).collect::<Vec<_>>(), rc.0, rc.1, rc.2, rc.3);
                    for (lf, q) in leaves.iter().zip(lr.iter()) {
                        rgb(cr, c);
                        cairo_rectangle(cr, q.0, q.1, q.2, q.3);
                        cairo_fill(cr);
                        rgb(cr, 0xFFFFFF);
                        cairo_set_line_width(cr, 1.5);
                        cairo_rectangle(cr, q.0, q.1, q.2, q.3);
                        cairo_stroke(cr);
                        if q.2 > 24.0 && q.3 > 14.0 {
                            pen.set(&lf.label, &small, z, Some(q.2 - 6.0));
                            rgb(cr, 0xFFFFFF);
                            pen.show(cr, q.0 + 3.0, q.1 + q.3 - 14.0 * z);
                        }
                    }
                    if rc.2 > 30.0 && !node.kids.is_empty() {
                        pen.set(&node.label, &font(10.0, true), z, Some(rc.2 - 6.0));
                        rgb(cr, 0xFFFFFF);
                        pen.show(cr, rc.0 + 3.0, rc.1 + 2.0);
                    }
                }
            }
            "sunburst" => {
                let lv: Vec<Vec<String>> = cx.cats.iter().rev().map(|c| self.chart_values(c).into_iter().map(|p| p.text).collect()).collect();
                let n = vals.len();
                let paths: Vec<Vec<String>> = (0..n).map(|i| lv.iter().map(|l| l.get(i).cloned().unwrap_or_default()).collect()).collect();
                let root = tree(&paths, &vals.iter().map(|v| v.unwrap_or(0.0)).collect::<Vec<_>>());
                let depth = lv.len().max(1);
                let (ox, oy) = ((l + r) / 2.0, (t + b) / 2.0);
                let rad = ((r - l).min(b - t) / 2.0).max(4.0);
                let ring = rad / (depth as f64 + 0.6);
                unsafe fn arcs(this: &Book, cr: *mut cairo_t, pen: &mut Pen, node: &Node, a0: f64, a1: f64, d: usize, base: Option<u32>,
                               k0: usize, ox: f64, oy: f64, ring: f64, theme: &[u32; 12], z: f64, small: &crate::layout::FontSpec) {
                    let total = node.value.max(1e-12);
                    let mut a = a0;
                    for (k, c) in node.kids.iter().enumerate() {
                        let span = (a1 - a0) * c.value / total;
                        let colr = match base {
                            Some(b) => light(b, 0.18 * d as f64),
                            None => auto_color(theme, 0, k0 + k),
                        };
                        let r_in = ring * (d as f64 + 0.6);
                        let r_out = r_in + ring;
                        cairo_new_path(cr);
                        cairo_arc(cr, ox, oy, r_out, a, a + span);
                        cairo_arc_negative(cr, ox, oy, r_in, a + span, a);
                        cairo_close_path(cr);
                        rgb(cr, colr);
                        cairo_fill_preserve(cr);
                        rgb(cr, 0xFFFFFF);
                        cairo_set_line_width(cr, 1.2);
                        cairo_stroke(cr);
                        let mid = a + span / 2.0;
                        let rm = (r_in + r_out) / 2.0;
                        if span * rm > 22.0 {
                            let (tw, th) = pen.set(&c.label, small, z, Some(ring));
                            rgb(cr, 0xFFFFFF);
                            pen.show(cr, ox + rm * mid.cos() - tw / 2.0, oy + rm * mid.sin() - th / 2.0);
                        }
                        arcs(this, cr, pen, c, a, a + span, d + 1, Some(base.unwrap_or(colr)), 0, ox, oy, ring, theme, z, small);
                        a += span;
                    }
                }
                arcs(self, cr, pen, &root, -PI / 2.0, 3.0 * PI / 2.0, 0, None, 0, ox, oy, ring, &theme, z, &small);
            }
            "funnel" => {
                let vs: Vec<f64> = vals.iter().map(|v| v.unwrap_or(0.0).max(0.0)).collect();
                let mx = vs.iter().cloned().fold(0.0, f64::max).max(1e-12);
                let n = vs.len().max(1);
                let lw = cats.iter().map(|c| pen.width(c, &small, z)).fold(0.0, f64::max).min((r - l) * 0.3) + 6.0;
                let bh = (b - t) / n as f64;
                let (pl, pr) = (l + lw, r);
                for (k, v) in vs.iter().enumerate() {
                    let bw = (pr - pl) * v / mx;
                    let bx = pl + (pr - pl - bw) / 2.0;
                    let by = t + k as f64 * bh + bh * 0.03;
                    rgb(cr, col(0));
                    cairo_rectangle(cr, bx, by, bw, bh * 0.94);
                    cairo_fill(cr);
                    label(pen, &crate::numfmt::num_to_text(*v), pl + (pr - pl) / 2.0, by + bh * 0.47, true, 0xFFFFFF, None);
                    if let Some(c) = cats.get(k) {
                        label(pen, c, l, by + bh * 0.47, false, TEXT, Some(lw - 6.0));
                    }
                }
            }
            "boxwhisker" => {
                let ns = cx.series.len().max(1);
                let all: Vec<Vec<f64>> = cx.series.iter().map(|s| {
                    let mut v: Vec<f64> = self.chart_values(&s.val).into_iter().filter_map(|p| p.v).collect();
                    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    v
                }).collect();
                let lo = all.iter().flatten().cloned().fold(f64::MAX, f64::min);
                let hi = all.iter().flatten().cloned().fold(f64::MIN, f64::max);
                let (amin, amax, step) = crate::chart_render::nice_axis(if lo == f64::MAX { 0.0 } else { lo }, if hi == f64::MIN { 1.0 } else { hi }, 6.0, true);
                let (pl, pt, pr, pb) = self.cx_value_axis(cr, pen, l, t + 14.0 * z, r, b, amin, amax, step, z, &small);
                // legend
                let mut lx = l;
                for (k, s) in cx.series.iter().enumerate() {
                    let nm = s.name.as_ref().map(|n| self.chart_values(n).first().map(|p| p.text.clone()).unwrap_or_default()).unwrap_or_else(|| format!("Series{}", k + 1));
                    rgb(cr, col(k));
                    cairo_rectangle(cr, lx, t, 8.0 * z, 8.0 * z);
                    cairo_fill(cr);
                    let (tw, _) = label(pen, &nm, lx + 11.0 * z, t + 4.0 * z, false, TEXT, None);
                    lx += tw + 24.0 * z;
                }
                let yv = |v: f64| pb - (v - amin) / (amax - amin) * (pb - pt);
                let slot = (pr - pl) / ns as f64;
                for (k, v) in all.iter().enumerate() {
                    if v.is_empty() {
                        continue;
                    }
                    let q1 = quantile_excl(v, 0.25);
                    let q2 = quantile_excl(v, 0.5);
                    let q3 = quantile_excl(v, 0.75);
                    let iqr = q3 - q1;
                    let wl = v.iter().cloned().filter(|x| *x >= q1 - 1.5 * iqr).fold(f64::MAX, f64::min);
                    let wh = v.iter().cloned().filter(|x| *x <= q3 + 1.5 * iqr).fold(f64::MIN, f64::max);
                    let mean = v.iter().sum::<f64>() / v.len() as f64;
                    let cxm = pl + slot * (k as f64 + 0.5);
                    let bw = slot * 0.5;
                    let c = col(k);
                    rgb(cr, c);
                    cairo_set_line_width(cr, 1.2);
                    cairo_rectangle(cr, cxm - bw / 2.0, yv(q3), bw, yv(q1) - yv(q3));
                    cairo_stroke(cr);
                    cairo_set_source_rgba(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0, 0.35);
                    cairo_rectangle(cr, cxm - bw / 2.0, yv(q3), bw, yv(q1) - yv(q3));
                    cairo_fill(cr);
                    rgb(cr, c);
                    for (a, bb) in [(yv(q2), yv(q2))] {
                        cairo_move_to(cr, cxm - bw / 2.0, a);
                        cairo_line_to(cr, cxm + bw / 2.0, bb);
                    }
                    cairo_move_to(cr, cxm, yv(q3));
                    cairo_line_to(cr, cxm, yv(wh));
                    cairo_move_to(cr, cxm - bw / 4.0, yv(wh));
                    cairo_line_to(cr, cxm + bw / 4.0, yv(wh));
                    cairo_move_to(cr, cxm, yv(q1));
                    cairo_line_to(cr, cxm, yv(wl));
                    cairo_move_to(cr, cxm - bw / 4.0, yv(wl));
                    cairo_line_to(cr, cxm + bw / 4.0, yv(wl));
                    // the mean: an x
                    let (mx, my, d) = (cxm, yv(mean), 3.5 * z);
                    cairo_move_to(cr, mx - d, my - d);
                    cairo_line_to(cr, mx + d, my + d);
                    cairo_move_to(cr, mx - d, my + d);
                    cairo_line_to(cr, mx + d, my - d);
                    cairo_stroke(cr);
                    for &o in v.iter().filter(|x| **x < wl || **x > wh) {
                        cairo_arc(cr, cxm, yv(o), 2.5 * z, 0.0, 2.0 * PI);
                        cairo_stroke(cr);
                    }
                }
            }
            _ => {
                // column-like: waterfall, histogram, pareto
                let (labels, heights, starts, colors): (Vec<String>, Vec<f64>, Vec<f64>, Vec<u32>) = match cx.kind.as_str() {
                    "waterfall" => {
                        let mut run = 0.0;
                        let mut st = Vec::new();
                        let mut hs = Vec::new();
                        let mut cs = Vec::new();
                        for (k, v) in vals.iter().enumerate() {
                            let v = v.unwrap_or(0.0);
                            if cx.subtotals.contains(&k) {
                                st.push(0.0);
                                hs.push(run);
                                cs.push(col(2));
                            } else {
                                st.push(run);
                                hs.push(v);
                                cs.push(if v >= 0.0 { col(0) } else { col(1) });
                                run += v;
                            }
                        }
                        (cats.clone(), hs, st, cs)
                    }
                    _ => {
                        let nums: Vec<f64> = vals.iter().filter_map(|v| *v).collect();
                        if cx.kind == "pareto" && !cats.is_empty() {
                            // categories by their values, largest first
                            let mut pairs: Vec<(String, f64)> = cats.iter().cloned().zip(vals.iter().map(|v| v.unwrap_or(0.0))).collect();
                            pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                            let n = pairs.len();
                            (pairs.iter().map(|p| p.0.clone()).collect(), pairs.iter().map(|p| p.1).collect(), vec![0.0; n], vec![col(0); n])
                        } else {
                            let (lb, mut cnt) = bins(&nums);
                            let mut lb = lb;
                            if cx.kind == "pareto" {
                                let mut pairs: Vec<(String, f64)> = lb.into_iter().zip(cnt.into_iter()).collect();
                                pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                lb = pairs.iter().map(|p| p.0.clone()).collect();
                                cnt = pairs.iter().map(|p| p.1).collect();
                            }
                            let n = cnt.len();
                            (lb, cnt, vec![0.0; n], vec![col(0); n])
                        }
                    }
                };
                let lo = starts.iter().zip(heights.iter()).map(|(s, h)| s.min(s + h)).fold(0.0, f64::min);
                let hi = starts.iter().zip(heights.iter()).map(|(s, h)| s.max(s + h)).fold(0.0, f64::max);
                let (amin, amax, step) = crate::chart_render::nice_axis(lo, if hi == lo { lo + 1.0 } else { hi }, 6.0, true);
                let top = if cx.kind == "waterfall" { t + 14.0 * z } else { t };
                if cx.kind == "waterfall" {
                    let mut lx = l + (r - l) / 2.0 - 110.0 * z;
                    for (k, nm) in ["Increase", "Decrease", "Total"].iter().enumerate() {
                        rgb(cr, col(k));
                        cairo_rectangle(cr, lx, t, 8.0 * z, 8.0 * z);
                        cairo_fill(cr);
                        let (tw, _) = label(pen, nm, lx + 11.0 * z, t + 4.0 * z, false, TEXT, None);
                        lx += tw + 24.0 * z;
                    }
                }
                let right = if cx.kind == "pareto" { r - 34.0 * z } else { r };
                let n = heights.len().max(1);
                let lab_h = 16.0 * z;
                let (pl, pt, pr, pb) = self.cx_value_axis(cr, pen, l, top, right, b - lab_h, amin, amax, step, z, &small);
                let yv = |v: f64| pb - (v - amin) / (amax - amin) * (pb - pt);
                let slot = (pr - pl) / n as f64;
                let gap = if matches!(cx.kind.as_str(), "histogram" | "pareto") { 0.04 } else { 0.25 };
                for k in 0..heights.len() {
                    let (a, bb) = (yv(starts[k]), yv(starts[k] + heights[k]));
                    rgb(cr, colors[k]);
                    let bx = pl + slot * k as f64 + slot * gap / 2.0;
                    cairo_rectangle(cr, bx, a.min(bb), slot * (1.0 - gap), (a - bb).abs().max(0.5));
                    cairo_fill(cr);
                    if let Some(lb) = labels.get(k) {
                        if slot > 14.0 {
                            label(pen, lb, pl + slot * (k as f64 + 0.5), pb + lab_h / 2.0, true, TEXT, Some(slot));
                        }
                    }
                }
                if cx.kind == "pareto" {
                    let total: f64 = heights.iter().map(|h| h.max(0.0)).sum::<f64>().max(1e-12);
                    let mut acc = 0.0;
                    rgb(cr, col(1));
                    cairo_set_line_width(cr, 2.0 * z);
                    for (k, h) in heights.iter().enumerate() {
                        acc += h.max(0.0);
                        let px = pl + slot * (k as f64 + 0.5);
                        let py = pb - (acc / total).clamp(0.0, 1.0) * (pb - pt);
                        if k == 0 {
                            cairo_move_to(cr, px, py);
                        } else {
                            cairo_line_to(cr, px, py);
                        }
                    }
                    cairo_stroke(cr);
                    for k in 0..=4 {
                        let v = k as f64 * 0.25;
                        label(pen, &format!("{}%", (v * 100.0) as i32), pr + 4.0 * z, pb - v * (pb - pt), false, TEXT, None);
                    }
                }
            }
        }
        cairo_restore(cr);
    }

    /// The value axis and gridlines; returns the plot rectangle (l, t, r, b).
    #[allow(clippy::too_many_arguments)]
    unsafe fn cx_value_axis(&self, cr: *mut cairo_t, pen: &mut Pen, l: f64, t: f64, r: f64, b: f64, amin: f64, amax: f64, step: f64, z: f64,
                            small: &crate::layout::FontSpec) -> (f64, f64, f64, f64) {
        let mut labels = Vec::new();
        let mut v = amin;
        let mut n = 0;
        while v <= amax + step * 1e-6 && n < 60 {
            labels.push((v, crate::numfmt::num_to_text((v * 1e9).round() / 1e9)));
            v += step;
            n += 1;
        }
        let lw = labels.iter().map(|x| pen.width(&x.1, small, z)).fold(0.0, f64::max) + 6.0 * z;
        let (pl, pt, pr, pb) = (l + lw, t + 4.0 * z, r, b);
        for (val, text) in &labels {
            let y = pb - (val - amin) / (amax - amin) * (pb - pt);
            rgb(cr, LINE);
            cairo_set_line_width(cr, 1.0);
            cairo_move_to(cr, pl, y.floor() + 0.5);
            cairo_line_to(cr, pr, y.floor() + 0.5);
            cairo_stroke(cr);
            let (tw, th) = pen.set(text, small, z, None);
            rgb(cr, TEXT);
            pen.show(cr, pl - 4.0 * z - tw, y - th / 2.0);
        }
        (pl, pt, pr, pb)
    }
}
