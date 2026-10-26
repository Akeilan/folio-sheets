//! The objects floating over a worksheet (its drawing part): charts, and pictures and
//! shapes kept as read. Positions are cell anchors, as in the file.

use crate::book::Book;
use crate::chart::Chart;
use crate::xml::{self, Element, Node};
use std::rc::Rc;

/// EMUs per pixel at 96 dpi.
pub const EMU: f64 = 9525.0;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Marker {
    pub col: u32,
    pub col_off: i64,
    pub row: u32,
    pub row_off: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Anchor {
    /// moves and sizes with cells (edit_as "oneCell"/"absolute" change that)
    Two { from: Marker, to: Marker, edit_as: Option<String> },
    One { from: Marker, cx: i64, cy: i64 },
    Abs { x: i64, y: i64, cx: i64, cy: i64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Obj {
    pub anchor: Anchor,
    /// the anchor element's other children as read (the graphic frame, picture, shape,
    /// clientData); for a new chart, made up when written
    pub body: Vec<Node>,
    pub chart: Option<Rc<Chart>>,
    pub name: String,
    /// the chart part this came from
    pub part: Option<String>,
    /// a picture's image, and the media part it was read from (None when new)
    pub picture: Option<Rc<crate::picture::Picture>>,
    pub pic_part: Option<String>,
}

/// A sheet's drawing part as read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawingFile {
    pub part: String,
    pub prolog: String,
    pub root: Element,
    /// the drawing's relationships (id, type, target part, external)
    pub rels: Vec<(String, String, String, bool)>,
}

fn num<T: std::str::FromStr>(e: &Element, name: &str) -> Option<T> {
    e.child(name).and_then(|x| x.text().trim().parse().ok())
}

fn marker(e: &Element) -> Marker {
    Marker {
        col: num(e, "col").unwrap_or(0),
        col_off: num(e, "colOff").unwrap_or(0),
        row: num(e, "row").unwrap_or(0),
        row_off: num(e, "rowOff").unwrap_or(0),
    }
}

impl Obj {
    pub fn kind(&self) -> &'static str {
        if self.chart.is_some() {
            return "chart";
        }
        if self.picture.is_some() {
            return "picture";
        }
        for n in &self.body {
            if let Node::Elem(e) = n {
                match e.local() {
                    "pic" => return "picture",
                    "sp" | "cxnSp" => return "shape",
                    "grpSp" => return "group",
                    "graphicFrame" => return "frame",
                    _ => {}
                }
            }
        }
        "object"
    }

    /// The chart reference id inside the graphic frame.
    pub fn chart_rid(&self) -> Option<String> {
        fn find(n: &[Node]) -> Option<String> {
            for x in n {
                if let Node::Elem(e) = x {
                    if e.local() == "chart" && e.attrs.iter().any(|(k, _)| xml::local(k) == "id") {
                        return e.attrs.iter().find(|(k, _)| xml::local(k) == "id" && k.contains(':')).map(|(_, v)| v.clone());
                    }
                    if let Some(r) = find(&e.children) {
                        return Some(r);
                    }
                }
            }
            None
        }
        find(&self.body)
    }

    /// The picture's image reference id (a:blip r:embed).
    pub fn blip_rid(&self) -> Option<String> {
        fn find(n: &[Node]) -> Option<String> {
            for x in n {
                if let Node::Elem(e) = x {
                    if e.local() == "blip" {
                        if let Some((_, v)) = e.attrs.iter().find(|(k, _)| xml::local(k) == "embed") {
                            return Some(v.clone());
                        }
                    }
                    if let Some(r) = find(&e.children) {
                        return Some(r);
                    }
                }
            }
            None
        }
        find(&self.body)
    }

    pub fn set_blip_rid(&mut self, rid: &str) {
        fn walk(n: &mut [Node], rid: &str) -> bool {
            for x in n.iter_mut() {
                if let Node::Elem(e) = x {
                    if e.local() == "blip" {
                        for a in e.attrs.iter_mut() {
                            if xml::local(&a.0) == "embed" {
                                a.1 = rid.to_string();
                                return true;
                            }
                        }
                    }
                    if walk(&mut e.children, rid) {
                        return true;
                    }
                }
            }
            false
        }
        walk(&mut self.body, rid);
    }

    pub fn set_chart_rid(&mut self, rid: &str) {
        fn walk(n: &mut [Node], rid: &str) -> bool {
            for x in n.iter_mut() {
                if let Node::Elem(e) = x {
                    if e.local() == "chart" {
                        for a in e.attrs.iter_mut() {
                            if xml::local(&a.0) == "id" && a.0.contains(':') {
                                a.1 = rid.to_string();
                                return true;
                            }
                        }
                    }
                    if walk(&mut e.children, rid) {
                        return true;
                    }
                }
            }
            false
        }
        walk(&mut self.body, rid);
    }
}

/// Read a drawing part. Charts whose parts can be read become charts; anything else is
/// kept as it is.
pub fn read_drawing(part: &str, text: &str, rels: Vec<(String, String, String, bool)>, get: &dyn Fn(&str) -> Option<String>,
                    get_bytes: &dyn Fn(&str) -> Option<Vec<u8>>, theme: &[u32; 12]) -> Result<(DrawingFile, Vec<Obj>), String> {
    let (prolog, root) = xml::parse(text)?;
    let mut objs = Vec::new();
    for a in root.elems() {
        let anchor = match a.local() {
            "twoCellAnchor" => Anchor::Two {
                from: a.child("from").map(marker).unwrap_or_default(),
                to: a.child("to").map(marker).unwrap_or_default(),
                edit_as: a.attr("editAs").map(|s| s.to_string()),
            },
            "oneCellAnchor" => {
                let ext = a.child("ext");
                Anchor::One {
                    from: a.child("from").map(marker).unwrap_or_default(),
                    cx: ext.and_then(|e| e.attr("cx")).and_then(|v| v.parse().ok()).unwrap_or(0),
                    cy: ext.and_then(|e| e.attr("cy")).and_then(|v| v.parse().ok()).unwrap_or(0),
                }
            }
            "absoluteAnchor" => {
                let pos = a.child("pos");
                let ext = a.child("ext");
                Anchor::Abs {
                    x: pos.and_then(|e| e.attr("x")).and_then(|v| v.parse().ok()).unwrap_or(0),
                    y: pos.and_then(|e| e.attr("y")).and_then(|v| v.parse().ok()).unwrap_or(0),
                    cx: ext.and_then(|e| e.attr("cx")).and_then(|v| v.parse().ok()).unwrap_or(0),
                    cy: ext.and_then(|e| e.attr("cy")).and_then(|v| v.parse().ok()).unwrap_or(0),
                }
            }
            _ => continue,
        };
        let body: Vec<Node> = a.children.iter().filter(|n| !matches!(n, Node::Elem(e) if matches!(e.local(), "from" | "to" | "ext" | "pos"))).cloned().collect();
        let mut o = Obj { anchor, body, chart: None, name: String::new(), part: None, picture: None, pic_part: None };
        o.name = find_name(&o.body).unwrap_or_default();
        // a picture: its image from the media part
        let is_pic = o.body.iter().any(|n| matches!(n, Node::Elem(e) if e.local() == "pic"));
        if is_pic {
            if let Some(rid) = o.blip_rid() {
                if let Some((_, _, target, ext)) = rels.iter().find(|r| r.0 == rid && r.1.ends_with("/image")) {
                    if !*ext {
                        if let Some(b) = get_bytes(target) {
                            if crate::picture::sniff(&b).is_some() {
                                o.picture = Some(Rc::new(crate::picture::Picture::lazy(b)));
                                o.pic_part = Some(target.clone());
                            }
                        }
                    }
                }
            }
        }
        if let Some(rid) = o.chart_rid() {
            if let Some((_, _, target, _)) = rels.iter().find(|r| r.0 == rid && r.1.ends_with("/chart")) {
                if let Some(t) = get(target) {
                    if let Ok(ch) = Chart::parse(&t, theme) {
                        o.chart = Some(Rc::new(ch));
                        o.part = Some(target.clone());
                    }
                }
            }
        }
        objs.push(o);
    }
    Ok((DrawingFile { part: part.to_string(), prolog, root, rels }, objs))
}

fn find_name(n: &[Node]) -> Option<String> {
    for x in n {
        if let Node::Elem(e) = x {
            if e.local() == "cNvPr" {
                return e.attr("name").map(|s| s.to_string());
            }
            if let Some(r) = find_name(&e.children) {
                return Some(r);
            }
        }
    }
    None
}

fn write_marker(tag: &str, m: &Marker, out: &mut String) {
    out.push_str(&format!("<xdr:{t}><xdr:col>{}</xdr:col><xdr:colOff>{}</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>{}</xdr:rowOff></xdr:{t}>",
        m.col, m.col_off, m.row, m.row_off, t = tag));
}

/// The graphic frame of a new chart.
pub fn chart_frame(id: u32, name: &str) -> Vec<Node> {
    let s = format!("<x><xdr:graphicFrame macro=\"\"><xdr:nvGraphicFramePr><xdr:cNvPr id=\"{}\" name=\"{}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"rId1\"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></x>",
        id, xml::escape_attr(name));
    xml::parse(&s).map(|(_, e)| e.children).unwrap_or_default()
}

/// The anchor body of a new picture (its image reference is set when written).
pub fn picture_body(id: u32, name: &str, cx: i64, cy: i64) -> Vec<Node> {
    let s = format!("<x><xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"{}\" name=\"{}\"/><xdr:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></xdr:cNvPicPr></xdr:nvPicPr><xdr:blipFill><a:blip xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:embed=\"rId1\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></x>",
        id, xml::escape_attr(name), cx, cy);
    xml::parse(&s).map(|(_, e)| e.children).unwrap_or_default()
}

/// The drawing part's XML for these objects.
pub fn write_drawing(file: Option<&DrawingFile>, objs: &[Obj]) -> String {
    let (prolog, mut root) = match file {
        Some(f) => (f.prolog.clone(), f.root.clone()),
        None => (String::new(), Element::with_attrs("xdr:wsDr", &[
            ("xmlns:xdr", "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"),
            ("xmlns:a", "http://schemas.openxmlformats.org/drawingml/2006/main"),
        ])),
    };
    // the anchors use the prefix the root does
    let pfx = root.name.split(':').next().filter(|_| root.name.contains(':')).unwrap_or("").to_string();
    root.children.clear();
    let mut out = String::new();
    for o in objs {
        let mut s = String::new();
        match &o.anchor {
            Anchor::Two { from, to, edit_as } => {
                s.push_str("<xdr:twoCellAnchor");
                if let Some(e) = edit_as {
                    s.push_str(&format!(" editAs=\"{}\"", xml::escape_attr(e)));
                }
                s.push('>');
                write_marker("from", from, &mut s);
                write_marker("to", to, &mut s);
            }
            Anchor::One { from, cx, cy } => {
                s.push_str("<xdr:oneCellAnchor>");
                write_marker("from", from, &mut s);
                s.push_str(&format!("<xdr:ext cx=\"{}\" cy=\"{}\"/>", cx, cy));
            }
            Anchor::Abs { x, y, cx, cy } => {
                s.push_str(&format!("<xdr:absoluteAnchor><xdr:pos x=\"{}\" y=\"{}\"/><xdr:ext cx=\"{}\" cy=\"{}\"/>", x, y, cx, cy));
            }
        }
        for n in &o.body {
            match n {
                Node::Elem(e) => e.write(&mut s),
                Node::Text(t) => xml::escape_text_into(t, &mut s),
                Node::Raw(r) => s.push_str(r),
            }
        }
        s.push_str(match o.anchor {
            Anchor::Two { .. } => "</xdr:twoCellAnchor>",
            Anchor::One { .. } => "</xdr:oneCellAnchor>",
            Anchor::Abs { .. } => "</xdr:absoluteAnchor>",
        });
        if pfx != "xdr" {
            let p = if pfx.is_empty() { String::new() } else { format!("{}:", pfx) };
            s = s.replace("<xdr:", &format!("<{}", p)).replace("</xdr:", &format!("</{}", p));
        }
        out.push_str(&s);
    }
    root.children.push(Node::Raw(out));
    xml::document(&prolog, &root)
}

// ---------------------------------------------------------------------- geometry

impl Book {
    /// Sheet pixels (100 %) of a marker.
    pub fn marker_px(&self, s: usize, m: &Marker) -> (f64, f64) {
        (self.col_x(s, m.col) + m.col_off as f64 / EMU, self.row_y(s, m.row) + m.row_off as f64 / EMU)
    }

    /// The marker at sheet pixels (x, y).
    pub fn px_marker(&self, s: usize, x: f64, y: f64) -> Marker {
        let x = x.max(0.0);
        let y = y.max(0.0);
        let col = self.col_at(s, x);
        let row = self.row_at(s, y);
        let co = ((x - self.col_x(s, col)).max(0.0) * EMU).round() as i64;
        let ro = ((y - self.row_y(s, row)).max(0.0) * EMU).round() as i64;
        Marker { col, col_off: co, row, row_off: ro }
    }

    /// An object's rectangle in sheet pixels at 100 %: (x, y, w, h).
    pub fn obj_rect(&self, s: usize, o: &Obj) -> (f64, f64, f64, f64) {
        match &o.anchor {
            Anchor::Two { from, to, .. } => {
                let (x1, y1) = self.marker_px(s, from);
                let (x2, y2) = self.marker_px(s, to);
                (x1, y1, (x2 - x1).max(0.0), (y2 - y1).max(0.0))
            }
            Anchor::One { from, cx, cy } => {
                let (x1, y1) = self.marker_px(s, from);
                (x1, y1, *cx as f64 / EMU, *cy as f64 / EMU)
            }
            Anchor::Abs { x, y, cx, cy } => (*x as f64 / EMU, *y as f64 / EMU, *cx as f64 / EMU, *cy as f64 / EMU),
        }
    }

    /// The anchor of the same kind placing an object at (x, y, w, h).
    pub fn anchor_at(&self, s: usize, like: &Anchor, x: f64, y: f64, w: f64, h: f64) -> Anchor {
        let w = w.max(1.0);
        let h = h.max(1.0);
        match like {
            Anchor::Two { edit_as, .. } => Anchor::Two { from: self.px_marker(s, x, y), to: self.px_marker(s, x + w, y + h), edit_as: edit_as.clone() },
            Anchor::One { .. } => Anchor::One { from: self.px_marker(s, x, y), cx: (w * EMU).round() as i64, cy: (h * EMU).round() as i64 },
            Anchor::Abs { .. } => Anchor::Abs { x: (x.max(0.0) * EMU) as i64, y: (y.max(0.0) * EMU) as i64, cx: (w * EMU) as i64, cy: (h * EMU) as i64 },
        }
    }
}

/// Markers after rows (rows true) or columns are inserted (n > 0) or deleted at `at`.
pub fn shift_marker(m: &Marker, rows: bool, at: u32, n: i64, is_to: bool) -> Marker {
    let mut m = *m;
    let (v, off) = if rows { (m.row, m.row_off) } else { (m.col, m.col_off) };
    let (nv, noff) = if n > 0 {
        // the end of an object resting on the line's top edge stays above it
        if v > at || v == at && (!is_to || off > 0) { ((v as i64 + n).min(u32::MAX as i64) as u32, off) } else { (v, off) }
    } else {
        let k = (-n) as u32;
        if v >= at + k {
            (v - k, off)
        } else if v >= at {
            (at, 0)
        } else {
            (v, off)
        }
    };
    if rows {
        m.row = nv;
        m.row_off = noff;
    } else {
        m.col = nv;
        m.col_off = noff;
    }
    m
}

pub fn shift_anchor(a: &Anchor, rows: bool, at: u32, n: i64) -> Anchor {
    match a {
        Anchor::Two { from, to, edit_as } => {
            let nf = shift_marker(from, rows, at, n, false);
            let nt = match edit_as.as_deref() {
                Some("absolute") => return a.clone(),
                // moves but keeps its size (in lines)
                Some("oneCell") => {
                    let d = if rows { nf.row as i64 - from.row as i64 } else { nf.col as i64 - from.col as i64 };
                    let mut t = *to;
                    if rows {
                        t.row = (t.row as i64 + d).max(0) as u32;
                    } else {
                        t.col = (t.col as i64 + d).max(0) as u32;
                    }
                    t
                }
                _ => shift_marker(to, rows, at, n, true),
            };
            Anchor::Two { from: nf, to: nt, edit_as: edit_as.clone() }
        }
        Anchor::One { from, cx, cy } => Anchor::One { from: shift_marker(from, rows, at, n, false), cx: *cx, cy: *cy },
        Anchor::Abs { .. } => a.clone(),
    }
}
