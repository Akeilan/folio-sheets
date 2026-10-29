//! Sparklines (SH-21): little line, column and win/loss charts in cells, kept as Excel keeps them
//! (x14:sparklineGroups in the worksheet's extLst), drawn in their cells.

use crate::book::{Book, Delta};
use crate::cellref::{parse_range, Rect};
use crate::sheet::Piece;
use crate::value::Value;
use crate::styles::theme_ref;
use crate::xml::{self, Element, Node};
use std::rc::Rc;

pub const EXT_URI: &str = "{05C60535-1F16-4fd2-B633-F4F36F0B64E0}";
const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const NS_XM: &str = "http://schemas.microsoft.com/office/excel/2006/main";

#[derive(Clone, Debug, PartialEq)]
pub struct SparkGroup {
    /// "line", "column" or "stacked" (win/loss)
    pub kind: String,
    pub color: u32,
    pub negative: u32,
    pub markers_color: u32,
    pub high_color: u32,
    pub low_color: u32,
    pub first_color: u32,
    pub last_color: u32,
    pub markers: bool,
    pub high: bool,
    pub low: bool,
    pub first: bool,
    pub last: bool,
    pub show_negative: bool,
    /// points
    pub weight: f64,
    /// vertical axis: "individual" (each its own), "group" (same for all) or "custom" (manual_*)
    pub min_type: String,
    pub max_type: String,
    pub manual_min: Option<f64>,
    pub manual_max: Option<f64>,
    /// Show Axis (a line at zero), its colour, Plot Data Right-to-left
    pub show_axis: bool,
    pub axis_color: u32,
    pub right_to_left: bool,
    /// Date Axis Type: the dates the points sit at (a range), or None for evenly spaced points
    pub date_range: Option<String>,
    /// Hidden & Empty Cells: "gap", "zero" or "span"; show data in hidden rows and columns
    pub empty_as: String,
    pub show_hidden: bool,
    /// (data reference, location cell)
    pub items: Vec<(String, (u32, u32))>,
    /// the element as read (what isn't modelled is written back)
    pub el: Option<Element>,
}

impl SparkGroup {
    pub fn new(kind: &str) -> SparkGroup {
        SparkGroup {
            // the desktop spreadsheet's defaults, on the theme: accent 1 darker 50%, accent 2 for
            // negative points, accent 1 (lighter 40% for the first and last points)
            kind: kind.to_string(),
            color: theme_ref(4, -0.5),
            negative: theme_ref(5, 0.0),
            markers_color: theme_ref(4, -0.5),
            high_color: theme_ref(4, 0.0),
            low_color: theme_ref(4, 0.0),
            first_color: theme_ref(4, 0.4),
            last_color: theme_ref(4, 0.4),
            markers: false,
            high: false,
            low: false,
            first: false,
            last: false,
            show_negative: kind == "stacked",
            weight: 0.75,
            min_type: "individual".into(),
            max_type: "individual".into(),
            manual_min: None,
            manual_max: None,
            show_axis: false,
            axis_color: 0x000000,
            right_to_left: false,
            date_range: None,
            empty_as: "gap".into(),
            show_hidden: false,
            items: vec![],
            el: None,
        }
    }
}

impl SparkGroup {
    /// The group with its theme colours worked out, for drawing.
    pub fn resolved(&self, f: impl Fn(u32) -> u32) -> SparkGroup {
        let mut g = self.clone();
        for c in [&mut g.color, &mut g.negative, &mut g.markers_color, &mut g.high_color, &mut g.low_color, &mut g.first_color, &mut g.last_color, &mut g.axis_color] {
            *c = f(*c);
        }
        g
    }
}

fn argb(e: Option<&Element>, d: u32) -> u32 {
    let e = match e {
        Some(e) => e,
        None => return d,
    };
    if let Some(v) = e.attr("rgb").and_then(|v| u32::from_str_radix(&v[v.len().saturating_sub(6)..], 16).ok()) {
        return v & 0xFFFFFF;
    }
    if let Some(t) = e.attr("theme").and_then(|v| v.parse::<u32>().ok()) {
        let tint = e.attr("tint").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        return theme_ref(t, tint);
    }
    d
}

fn flag(e: &Element, n: &str) -> bool {
    matches!(e.attr(n), Some("1") | Some("true"))
}

fn parse_group(g: &Element) -> SparkGroup {
    let kind = g.attr("type").unwrap_or("line").to_string();
    let mut s = SparkGroup::new(&kind);
    s.color = argb(g.child("colorSeries"), s.color);
    s.negative = argb(g.child("colorNegative"), s.negative);
    s.markers_color = argb(g.child("colorMarkers"), s.markers_color);
    s.high_color = argb(g.child("colorHigh"), s.high_color);
    s.low_color = argb(g.child("colorLow"), s.low_color);
    s.first_color = argb(g.child("colorFirst"), s.first_color);
    s.last_color = argb(g.child("colorLast"), s.last_color);
    s.markers = flag(g, "markers");
    s.high = flag(g, "high");
    s.low = flag(g, "low");
    s.first = flag(g, "first");
    s.last = flag(g, "last");
    s.show_negative = flag(g, "negative");
    s.weight = g.attr("lineWeight").and_then(|v| v.parse().ok()).unwrap_or(0.75);
    s.min_type = g.attr("minAxisType").unwrap_or("individual").to_string();
    s.max_type = g.attr("maxAxisType").unwrap_or("individual").to_string();
    s.manual_min = g.attr("manualMin").and_then(|v| v.parse().ok());
    s.manual_max = g.attr("manualMax").and_then(|v| v.parse().ok());
    s.show_axis = flag(g, "displayXAxis");
    s.axis_color = argb(g.child("colorAxis"), 0x000000);
    s.right_to_left = flag(g, "rightToLeft");
    s.empty_as = g.attr("displayEmptyCellsAs").unwrap_or("zero").to_string();
    s.show_hidden = flag(g, "displayHidden");
    if flag(g, "dateAxis") {
        s.date_range = g.elems().find(|e| e.local() == "f").map(|e| e.text());
    }
    if let Some(list) = g.child("sparklines") {
        for sp in list.elems().filter(|e| e.local() == "sparkline") {
            let f = sp.child("f").map(|e| e.text()).unwrap_or_default();
            let loc = sp.child("sqref").map(|e| e.text()).unwrap_or_default();
            if let Some(r) = parse_range(loc.trim()) {
                s.items.push((f, (r.r1, r.c1)));
            }
        }
    }
    s.el = Some(g.clone());
    s
}

/// The groups in a sheet's kept extLst.
pub fn read_groups(pieces: &[Piece]) -> Vec<SparkGroup> {
    let mut out = Vec::new();
    for p in pieces {
        if let Piece::Raw(t, x) = p {
            if t == "extLst" && x.contains("sparklineGroup") {
                if let Ok((_, e)) = xml::parse(x) {
                    for ext in e.elems().filter(|x| x.attr("uri").map(|u| u.eq_ignore_ascii_case(EXT_URI)).unwrap_or(false)) {
                        for gs in ext.elems().filter(|x| x.local() == "sparklineGroups") {
                            for g in gs.elems().filter(|x| x.local() == "sparklineGroup") {
                                out.push(parse_group(g));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

fn color_el(name: &str, c: u32) -> Element {
    crate::styles::color_el(&format!("x14:{}", name), c)
}

fn write_group(g: &SparkGroup) -> Element {
    let mut e = g.el.clone().unwrap_or_else(|| Element::with_attrs("x14:sparklineGroup", &[("displayEmptyCellsAs", "gap")]));
    let set = |e: &mut Element, n: &str, on: bool| {
        if on {
            e.set_attr(n, "1");
        } else {
            e.remove_attr(n);
        }
    };
    if g.kind == "line" {
        e.remove_attr("type");
    } else {
        e.set_attr("type", &g.kind);
    }
    if (g.weight - 0.75).abs() > 1e-9 {
        e.set_attr("lineWeight", &format!("{}", g.weight));
    } else {
        e.remove_attr("lineWeight");
    }
    set(&mut e, "markers", g.markers);
    set(&mut e, "high", g.high);
    set(&mut e, "low", g.low);
    set(&mut e, "first", g.first);
    set(&mut e, "last", g.last);
    set(&mut e, "negative", g.show_negative);
    set(&mut e, "displayXAxis", g.show_axis);
    set(&mut e, "rightToLeft", g.right_to_left);
    set(&mut e, "displayHidden", g.show_hidden);
    set(&mut e, "dateAxis", g.date_range.is_some());
    e.set_attr("displayEmptyCellsAs", &g.empty_as);
    for (n, t) in [("minAxisType", &g.min_type), ("maxAxisType", &g.max_type)] {
        if t == "individual" {
            e.remove_attr(n);
        } else {
            e.set_attr(n, t);
        }
    }
    for (n, v) in [("manualMin", g.manual_min), ("manualMax", g.manual_max)] {
        match v {
            Some(x) => e.set_attr(n, &crate::xlsx::num_text(x)),
            None => e.remove_attr(n),
        }
    }
    // colours, then the sparklines, in the schema's order
    e.children.retain(|n| !matches!(n, Node::Elem(c) if c.local().starts_with("color") || c.local() == "sparklines" || c.local() == "f"));
    let mut kids = vec![
        color_el("colorSeries", g.color),
        color_el("colorNegative", g.negative),
        color_el("colorAxis", g.axis_color),
        color_el("colorMarkers", g.markers_color),
        color_el("colorFirst", g.first_color),
        color_el("colorLast", g.last_color),
        color_el("colorHigh", g.high_color),
        color_el("colorLow", g.low_color),
    ];
    if let Some(d) = &g.date_range {
        let mut fe = Element::new("xm:f");
        fe.children.push(Node::Text(d.clone()));
        kids.push(fe);
    }
    let mut list = Element::new("x14:sparklines");
    for (f, (r, c)) in &g.items {
        let mut sp = Element::new("x14:sparkline");
        let mut fe = Element::new("xm:f");
        fe.children.push(Node::Text(f.clone()));
        let mut sq = Element::new("xm:sqref");
        sq.children.push(Node::Text(crate::cellref::Rect { r1: *r, c1: *c, r2: *r, c2: *c }.a1()));
        sp.push(fe);
        sp.push(sq);
        list.push(sp);
    }
    kids.push(list);
    // anything else the group had (its extLst) goes after
    let rest: Vec<Node> = e.children.drain(..).collect();
    e.children = kids.into_iter().map(Node::Elem).collect();
    e.children.extend(rest);
    e
}

/// The sheet's pieces with these groups as its sparklines (an empty list removes them).
pub fn with_groups(pieces: &[Piece], groups: &[SparkGroup]) -> Vec<Piece> {
    let mut gs = Element::with_attrs("x14:sparklineGroups", &[("xmlns:xm", NS_XM)]);
    for g in groups {
        gs.push(write_group(g));
    }
    let mut ext = Element::with_attrs("ext", &[("uri", EXT_URI), ("xmlns:x14", NS_X14)]);
    ext.push(gs);
    let mut out: Vec<Piece> = Vec::new();
    let mut placed = false;
    for p in pieces {
        match p {
            Piece::Raw(t, x) if t == "extLst" => {
                let mut e = match xml::parse(x) {
                    Ok((_, e)) => e,
                    Err(_) => {
                        out.push(p.clone());
                        continue;
                    }
                };
                // its own prefix for the ext element (extLst may be unprefixed or "x:")
                let pfx = e.name.split(':').next().filter(|_| e.name.contains(':')).map(|p| format!("{}:", p)).unwrap_or_default();
                let mut ext2 = ext.clone();
                ext2.name = format!("{}ext", pfx);
                e.children.retain(|n| !matches!(n, Node::Elem(c) if c.attr("uri").map(|u| u.eq_ignore_ascii_case(EXT_URI)).unwrap_or(false)));
                if !groups.is_empty() {
                    e.children.push(Node::Elem(ext2));
                }
                placed = true;
                if e.elems().next().is_some() {
                    out.push(Piece::Raw(t.clone(), e.to_string()));
                }
            }
            other => out.push(other.clone()),
        }
    }
    if !placed && !groups.is_empty() {
        let mut e = Element::new("extLst");
        e.push(ext);
        out.push(Piece::Raw("extLst".into(), e.to_string()));
    }
    out
}

impl Book {
    pub fn sparkline_groups_cached(&self, s: usize) -> Rc<Vec<SparkGroup>> {
        let sh = match self.sheets.get(s) { Some(x) => x, None => return Rc::new(vec![]) };
        let fp = crate::cf::fingerprint(&sh.pieces);
        if let Some((k, v)) = self.spark_cache.borrow().get(&s) {
            if *k == fp { return v.clone(); }
        }
        let rc = Rc::new(read_groups(&sh.pieces));
        self.spark_cache.borrow_mut().insert(s, (fp, rc.clone()));
        rc
    }

    pub fn sparkline_groups(&self, s: usize) -> Vec<SparkGroup> {
        self.sheets.get(s).map(|sh| read_groups(&sh.pieces)).unwrap_or_default()
    }

    fn set_groups(&mut self, s: usize, groups: Vec<SparkGroup>, label: &str) {
        let pieces = with_groups(&self.sheets[s].pieces, &groups);
        self.begin(label, s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
    }

    /// The group and item of a sparkline in a cell.
    pub fn sparkline_at(&self, s: usize, r: u32, c: u32) -> Option<(usize, usize)> {
        for (gi, g) in self.sparkline_groups(s).iter().enumerate() {
            if let Some(ii) = g.items.iter().position(|(_, l)| *l == (r, c)) {
                return Some((gi, ii));
            }
        }
        None
    }

    /// Insert › Sparklines: one sparkline per row (or column) of the data, in the cells of the
    /// location range (one row or column of cells).
    pub fn add_sparklines(&mut self, s: usize, kind: &str, data: &str, location: Rect) -> Result<usize, String> {
        if s >= self.sheets.len() {
            return Err("No such sheet.".into());
        }
        let (sheet_part, rng) = match data.rfind('!') {
            Some(i) => (Some(data[..i].trim_matches('\'').to_string()), &data[i + 1..]),
            None => (None, data),
        };
        let d = parse_range(&rng.replace('$', "")).ok_or("The reference for the data range isn't valid.")?;
        let qual = match &sheet_part {
            Some(n) => n.clone(),
            None => self.sheets[s].name.clone(),
        };
        let q = if qual.chars().all(|c| c.is_alphanumeric() || c == '_') { qual } else { format!("'{}'", qual.replace('\'', "''")) };
        let cells: Vec<(u32, u32)> = if location.r1 == location.r2 {
            (location.c1..=location.c2).map(|c| (location.r1, c)).collect()
        } else if location.c1 == location.c2 {
            (location.r1..=location.r2).map(|r| (r, location.c1)).collect()
        } else {
            return Err("The location reference must be a single row or column of cells.".into());
        };
        let (rows, cols) = (d.r2 - d.r1 + 1, d.c2 - d.c1 + 1);
        let n = cells.len() as u32;
        let mut items = Vec::new();
        if n == 1 && (rows == 1 || cols == 1) {
            items.push((format!("{}!{}", q, d.a1()), cells[0]));
        } else if rows == n {
            for (k, cell) in cells.iter().enumerate() {
                let r = d.r1 + k as u32;
                items.push((format!("{}!{}", q, Rect { r1: r, c1: d.c1, r2: r, c2: d.c2 }.a1()), *cell));
            }
        } else if cols == n {
            for (k, cell) in cells.iter().enumerate() {
                let c = d.c1 + k as u32;
                items.push((format!("{}!{}", q, Rect { r1: d.r1, c1: c, r2: d.r2, c2: c }.a1()), *cell));
            }
        } else {
            return Err("Location reference is not valid because the cells are not all in the same column or same row. Select cells that are all in a single row or column.".into());
        }
        let mut groups = self.sparkline_groups(s);
        // a cell takes one sparkline: ones already there go
        for g in groups.iter_mut() {
            g.items.retain(|(_, l)| !cells.contains(l));
        }
        groups.retain(|g| !g.items.is_empty());
        let mut g = SparkGroup::new(match kind {
            "column" => "column",
            "winloss" | "stacked" => "stacked",
            _ => "line",
        });
        g.items = items;
        groups.push(g);
        let i = groups.len() - 1;
        self.set_groups(s, groups, "Insert Sparklines");
        Ok(i)
    }

    /// Change a group: kind, colours, which points show, line weight.
    pub fn edit_sparkline_group(&mut self, s: usize, gi: usize, f: impl FnOnce(&mut SparkGroup)) -> Result<(), String> {
        let mut groups = self.sparkline_groups(s);
        let g = groups.get_mut(gi).ok_or("No such sparkline group.")?;
        f(g);
        self.set_groups(s, groups, "Sparkline");
        Ok(())
    }

    /// Clear the sparklines in a range (whole groups when `group`).
    pub fn clear_sparklines(&mut self, s: usize, rg: Rect, group: bool) -> Result<(), String> {
        let mut groups = self.sparkline_groups(s);
        let inside = |l: &(u32, u32)| rg.r1 <= l.0 && l.0 <= rg.r2 && rg.c1 <= l.1 && l.1 <= rg.c2;
        if group {
            groups.retain(|g| !g.items.iter().any(|(_, l)| inside(l)));
        } else {
            for g in groups.iter_mut() {
                g.items.retain(|(_, l)| !inside(l));
            }
            groups.retain(|g| !g.items.is_empty());
        }
        self.set_groups(s, groups, "Clear Sparklines");
        Ok(())
    }

    /// Group (one group, with the first's settings) or Ungroup (one group per sparkline) the
    /// sparklines in a range.
    pub fn regroup_sparklines(&mut self, s: usize, rg: Rect, group: bool) -> Result<(), String> {
        let inside = |l: &(u32, u32)| rg.r1 <= l.0 && l.0 <= rg.r2 && rg.c1 <= l.1 && l.1 <= rg.c2;
        let groups = self.sparkline_groups(s);
        let mut out: Vec<SparkGroup> = Vec::new();
        let mut merged: Option<usize> = None;
        for g in groups {
            let (inn, rest): (Vec<_>, Vec<_>) = g.items.iter().cloned().partition(|(_, l)| inside(l));
            if inn.is_empty() {
                out.push(g);
                continue;
            }
            if !rest.is_empty() {
                let mut k = g.clone();
                k.items = rest;
                out.push(k);
            }
            if group {
                match merged {
                    Some(i) => out[i].items.extend(inn),
                    None => {
                        let mut k = g.clone();
                        k.items = inn;
                        out.push(k);
                        merged = Some(out.len() - 1);
                    }
                }
            } else {
                for it in inn {
                    let mut k = g.clone();
                    k.items = vec![it];
                    out.push(k);
                }
            }
        }
        self.set_groups(s, out, if group { "Group" } else { "Ungroup" });
        Ok(())
    }

    /// A sparkline's numbers as its group shows them: without hidden rows and columns unless
    /// the group shows them.
    pub fn group_values(&self, s: usize, g: &SparkGroup, f: &str) -> Vec<Option<f64>> {
        let vals = self.sparkline_values(s, f);
        if g.show_hidden {
            return vals;
        }
        let (sheet, rng) = match f.rfind('!') {
            Some(i) => (self.sheet_index(f[..i].trim_matches('\'')).unwrap_or(s), &f[i + 1..]),
            None => (s, f),
        };
        let r = match parse_range(&rng.replace('$', "")) {
            Some(r) => r,
            None => return vals,
        };
        let horizontal = r.r1 == r.r2;
        let sh = &self.sheets[sheet];
        vals.into_iter().enumerate().filter(|(k, _)| {
            let k = *k as u32;
            if horizontal { !sh.col_hidden(r.c1 + k) } else { !sh.rows.get(&(r.r1 + k)).map(|x| x.hidden).unwrap_or(false) }
        }).map(|(_, v)| v).collect()
    }

    /// The group's vertical axis when it is the same for all (min, max over its sparklines).
    pub fn group_bounds(&self, s: usize, g: &SparkGroup) -> Option<(f64, f64)> {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for (f, _) in &g.items {
            for v in self.group_values(s, g, f).into_iter().flatten() {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if lo <= hi { Some((lo, hi)) } else { None }
    }

    /// Where the points sit along the cell (0..1) on a date axis.
    pub fn date_positions(&self, s: usize, g: &SparkGroup) -> Option<Vec<f64>> {
        let d = g.date_range.as_ref()?;
        let ds: Vec<f64> = self.sparkline_values(s, d).into_iter().map(|v| v.unwrap_or(f64::NAN)).collect();
        let ok: Vec<f64> = ds.iter().copied().filter(|v| v.is_finite()).collect();
        if ok.len() < 2 {
            return None;
        }
        let lo = ok.iter().cloned().fold(f64::MAX, f64::min);
        let hi = ok.iter().cloned().fold(f64::MIN, f64::max);
        if hi <= lo {
            return None;
        }
        Some(ds.iter().map(|v| if v.is_finite() { (v - lo) / (hi - lo) } else { 0.0 }).collect())
    }

    /// A sparkline's numbers (non-numbers are gaps).
    pub fn sparkline_values(&self, s: usize, f: &str) -> Vec<Option<f64>> {
        let (sheet, rng) = match f.rfind('!') {
            Some(i) => (self.sheet_index(f[..i].trim_matches('\'')).unwrap_or(s), &f[i + 1..]),
            None => (s, f),
        };
        let r = match parse_range(&rng.replace('$', "")) {
            Some(r) => r,
            None => return vec![],
        };
        let mut out = Vec::new();
        let horizontal = r.r1 == r.r2;
        let n = if horizontal { r.c2 - r.c1 + 1 } else { r.r2 - r.r1 + 1 };
        for k in 0..n.min(5000) {
            let (row, col) = if horizontal { (r.r1, r.c1 + k) } else { (r.r1 + k, r.c1) };
            out.push(match self.value(sheet, row, col) {
                Value::Num(x) => Some(x),
                Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
                _ => None,
            });
        }
        out
    }
}

/// Draw one sparkline in a cell box.
/// The vertical axis of one sparkline: its own numbers, or the group's (Same for All), or the
/// custom values. `group` = (min, max) over every sparkline of the group.
pub fn bounds(g: &SparkGroup, vals: &[Option<f64>], group: Option<(f64, f64)>) -> Option<(f64, f64)> {
    let nums: Vec<f64> = vals.iter().flatten().copied().collect();
    if nums.is_empty() {
        return None;
    }
    let own_mn = nums.iter().cloned().fold(f64::MAX, f64::min);
    let own_mx = nums.iter().cloned().fold(f64::MIN, f64::max);
    let mn = match g.min_type.as_str() {
        "group" => group.map(|x| x.0).unwrap_or(own_mn),
        "custom" => g.manual_min.unwrap_or(own_mn),
        _ => own_mn,
    };
    let mx = match g.max_type.as_str() {
        "group" => group.map(|x| x.1).unwrap_or(own_mx),
        "custom" => g.manual_max.unwrap_or(own_mx),
        _ => own_mx,
    };
    Some((mn, mx.max(mn)))
}

/// Draw one sparkline in a cell box. `range` = the vertical axis (bounds()); `xs` = where the
/// points sit along the cell (0..1) for a date axis.
#[allow(clippy::too_many_arguments)]
pub unsafe fn paint(cr: *mut crate::ffi::cairo_t, g: &SparkGroup, vals: &[Option<f64>], x: f64, y: f64, w: f64, h: f64, z: f64) {
    paint_with(cr, g, vals, None, None, x, y, w, h, z)
}

#[allow(clippy::too_many_arguments)]
pub unsafe fn paint_with(cr: *mut crate::ffi::cairo_t, g: &SparkGroup, vals: &[Option<f64>], range: Option<(f64, f64)>, xs: Option<&[f64]>,
                         x: f64, y: f64, w: f64, h: f64, z: f64) {
    use crate::ffi::*;
    let pad = (2.0 * z).max(1.5);
    let (x, y, w, h) = (x + pad, y + pad, (w - 2.0 * pad).max(1.0), (h - 2.0 * pad).max(1.0));
    // empty cells as zero; right to left
    let mut owned: Vec<Option<f64>> = if g.empty_as == "zero" { vals.iter().map(|v| Some(v.unwrap_or(0.0))).collect() } else { vals.to_vec() };
    let mut xs_owned: Option<Vec<f64>> = xs.map(|v| v.to_vec());
    if g.right_to_left {
        owned.reverse();
        if let Some(v) = xs_owned.as_mut() {
            v.reverse();
            for t in v.iter_mut() {
                *t = 1.0 - *t;
            }
        }
    }
    let vals = &owned[..];
    let xs = xs_owned.as_deref();
    let nums: Vec<f64> = vals.iter().flatten().copied().collect();
    if nums.is_empty() {
        return;
    }
    let (mn, mx) = match range.or_else(|| bounds(g, vals, None)) {
        Some(b) => b,
        None => return,
    };
    let n = vals.len().max(1);
    let col = |c: u32| ((c >> 16 & 255) as f64 / 255.0, (c >> 8 & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
    let set = |c: u32| {
        let (r, g2, b) = col(c);
        cairo_set_source_rgb(cr, r, g2, b);
    };
    let first_i = vals.iter().position(|v| v.is_some());
    let last_i = vals.iter().rposition(|v| v.is_some());
    let point_color = |i: usize, v: f64| -> Option<u32> {
        if g.high && v == mx {
            return Some(g.high_color);
        }
        if g.low && v == mn {
            return Some(g.low_color);
        }
        if g.first && Some(i) == first_i {
            return Some(g.first_color);
        }
        if g.last && Some(i) == last_i {
            return Some(g.last_color);
        }
        if g.show_negative && v < 0.0 {
            return Some(g.negative);
        }
        None
    };
    match g.kind.as_str() {
        "column" | "stacked" => {
            let bw = w / n as f64;
            let gap = (bw * 0.15).max(0.5);
            if g.kind == "stacked" {
                let mid = y + h / 2.0;
                for (i, v) in vals.iter().enumerate() {
                    let v = match v {
                        Some(v) if *v != 0.0 => *v,
                        _ => continue,
                    };
                    set(point_color(i, v).unwrap_or(if v < 0.0 { g.negative } else { g.color }));
                    let bx = x + i as f64 * bw + gap / 2.0;
                    if v > 0.0 {
                        cairo_rectangle(cr, bx, y, bw - gap, h / 2.0 - 0.5);
                    } else {
                        cairo_rectangle(cr, bx, mid + 0.5, bw - gap, h / 2.0 - 0.5);
                    }
                    cairo_fill(cr);
                }
            } else {
                let lo = if g.min_type == "individual" { mn.min(0.0) } else { mn };
                let hi = if g.max_type == "individual" { mx.max(0.0) } else { mx };
                if g.show_axis && lo < 0.0 && hi > 0.0 {
                    set(g.axis_color);
                    cairo_set_line_width(cr, 1.0);
                    let ay = (y + h * (hi / (hi - lo))).floor() + 0.5;
                    cairo_move_to(cr, x, ay);
                    cairo_line_to(cr, x + w, ay);
                    cairo_stroke(cr);
                }
                let span = if hi > lo { hi - lo } else { 1.0 };
                let ypos = |v: f64| y + h * ((hi - v.clamp(lo, hi)) / span);
                let base = ypos(0.0);
                for (i, v) in vals.iter().enumerate() {
                    let v = match v {
                        Some(v) => *v,
                        None => continue,
                    };
                    set(point_color(i, v).unwrap_or(if v < 0.0 && g.show_negative { g.negative } else { g.color }));
                    let bx = match xs {
                        Some(t) if i < t.len() => x + (w - bw) * t[i] + gap / 2.0,
                        _ => x + i as f64 * bw + gap / 2.0,
                    };
                    let yv = ypos(v);
                    let (top, bottom) = if yv < base { (yv, base) } else { (base, yv) };
                    let hh = (bottom - top).max(1.0);
                    cairo_rectangle(cr, bx, top, bw - gap, hh);
                    cairo_fill(cr);
                }
            }
        }
        _ => {
            let span = if mx > mn { mx - mn } else { 1.0 };
            let px = |i: usize| match xs {
                Some(v) if i < v.len() => x + w * v[i],
                _ => if n > 1 { x + w * i as f64 / (n - 1) as f64 } else { x + w / 2.0 },
            };
            let py = |v: f64| if mx > mn { y + h * (mx - v) / span } else { (y + h / 2.0).floor() + 0.5 };
            set(g.color);
            cairo_set_line_width(cr, (g.weight * 96.0 / 72.0 * z).max(0.75));
            cairo_set_line_join(cr, 1);
            cairo_set_line_cap(cr, 1);
            let mut down = false;
            for (i, v) in vals.iter().enumerate() {
                match v {
                    Some(v) => {
                        if down {
                            cairo_line_to(cr, px(i), py(*v));
                        } else {
                            cairo_move_to(cr, px(i), py(*v));
                            down = true;
                        }
                    }
                    None => if g.empty_as != "span" { down = false },
                }
            }
            cairo_stroke(cr);
            if g.show_axis && mn < 0.0 && mx > 0.0 {
                set(g.axis_color);
                cairo_set_line_width(cr, 1.0);
                let ay = py(0.0).floor() + 0.5;
                cairo_move_to(cr, x, ay);
                cairo_line_to(cr, x + w, ay);
                cairo_stroke(cr);
            }
            let r = (1.8 * z).max(1.5);
            for (i, v) in vals.iter().enumerate() {
                if let Some(v) = v {
                    let c = point_color(i, *v).or(if g.markers { Some(g.markers_color) } else { None });
                    if let Some(c) = c {
                        set(c);
                        cairo_arc(cr, px(i), py(*v), r, 0.0, std::f64::consts::TAU);
                        cairo_fill(cr);
                    }
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_bounds_and_round_trip() {
        let mut g = SparkGroup::new("line");
        let v = vec![Some(2.0), None, Some(8.0)];
        assert_eq!(bounds(&g, &v, Some((-5.0, 20.0))), Some((2.0, 8.0)));
        g.min_type = "group".into();
        g.max_type = "custom".into();
        g.manual_max = Some(50.0);
        assert_eq!(bounds(&g, &v, Some((-5.0, 20.0))), Some((-5.0, 50.0)));
        g.show_axis = true;
        g.right_to_left = true;
        g.date_range = Some("Sheet1!A1:C1".into());
        g.items.push(("Sheet1!A2:C2".into(), (3, 4)));
        let e = write_group(&g);
        let x = e.to_string();
        assert!(x.contains("minAxisType=\"group\"") && x.contains("manualMax=\"50\"") && x.contains("dateAxis=\"1\""), "{}", x);
        let back = parse_group(&e);
        assert_eq!(back.min_type, "group");
        assert_eq!(back.manual_max, Some(50.0));
        assert!(back.show_axis && back.right_to_left);
        assert_eq!(back.date_range.as_deref(), Some("Sheet1!A1:C1"));
        assert_eq!(back.items.len(), 1);
    }

    #[test]
    fn read_and_write() {
        let x = "<extLst><ext uri=\"{05C60535-1F16-4fd2-B633-F4F36F0B64E0}\" xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\"><x14:sparklineGroups xmlns:xm=\"http://schemas.microsoft.com/office/excel/2006/main\"><x14:sparklineGroup type=\"column\" displayEmptyCellsAs=\"gap\" high=\"1\"><x14:colorSeries rgb=\"FF376092\"/><x14:colorHigh rgb=\"FF00B050\"/><x14:sparklines><x14:sparkline><xm:f>Sheet1!A1:E1</xm:f><xm:sqref>F1</xm:sqref></x14:sparkline><x14:sparkline><xm:f>Sheet1!A2:E2</xm:f><xm:sqref>F2</xm:sqref></x14:sparkline></x14:sparklines></x14:sparklineGroup></x14:sparklineGroups></ext><ext uri=\"{other}\"><keep/></ext></extLst>";
        let pieces = vec![Piece::Raw("extLst".into(), x.into())];
        let gs = read_groups(&pieces);
        assert_eq!(gs.len(), 1);
        assert_eq!(gs[0].kind, "column");
        assert!(gs[0].high && gs[0].high_color == 0x00B050);
        assert_eq!(gs[0].items, vec![("Sheet1!A1:E1".to_string(), (0, 5)), ("Sheet1!A2:E2".to_string(), (1, 5))]);
        let mut g2 = gs.clone();
        g2[0].kind = "line".into();
        g2[0].markers = true;
        let p2 = with_groups(&pieces, &g2);
        let back = read_groups(&p2);
        assert_eq!(back[0].kind, "line");
        assert!(back[0].markers && back[0].high);
        match &p2[0] {
            Piece::Raw(_, x) => assert!(x.contains("{other}") && x.contains("displayEmptyCellsAs")),
            _ => panic!(),
        }
        // removing them all keeps the other ext
        let p3 = with_groups(&p2, &[]);
        assert!(read_groups(&p3).is_empty());
        match &p3[0] {
            Piece::Raw(_, x) => assert!(x.contains("{other}")),
            _ => panic!(),
        }
    }
}
