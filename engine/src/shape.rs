//! Shapes, text boxes and WordArt over a worksheet (SH-21): DrawingML preset geometries drawn
//! with their fill, outline and text, and the edits the Shape Format tab makes to them. The
//! shape's XML stays the source of truth: what isn't modelled is written back as it was.

use crate::xml::{self, Element, Node};

pub const EMU_PER_PT: f64 = 12700.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
    Close,
}

/// A path of a preset: its segments, and whether it is filled (lines and arcs aren't).
pub struct SubPath {
    pub segs: Vec<Seg>,
    pub fill: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: u32,
    pub font: Option<String>,
    /// a text outline (WordArt): colour and width in points
    pub outline: Option<(u32, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Para {
    /// 0 left, 1 centre, 2 right
    pub align: i32,
    pub runs: Vec<Run>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapeView {
    pub geom: String,
    pub adj: Vec<(String, f64)>,
    pub fill: Option<(u32, f64)>,
    /// colour, width in points, dashed
    pub line: Option<(u32, f64, bool)>,
    pub head: bool,
    pub tail: bool,
    pub rot: f64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub paras: Vec<Para>,
    /// 't', 'c' or 'b'
    pub anchor: char,
    pub wrap: bool,
    /// insets in points (l, t, r, b)
    pub insets: (f64, f64, f64, f64),
    pub textbox: bool,
    pub connector: bool,
    /// an equation (Insert › Equation): what it says, its size in points
    pub math: Option<(crate::math::M, f64)>,
}

fn emu_attr(e: &Element, n: &str) -> Option<f64> {
    e.attr(n).and_then(|v| v.parse::<f64>().ok())
}

/// The shape element (sp or cxnSp) of an object's body.
pub fn shape_elem(body: &[Node]) -> Option<&Element> {
    body.iter().find_map(|n| match n {
        Node::Elem(e) if e.local() == "sp" || e.local() == "cxnSp" => Some(e),
        _ => None,
    })
}

pub fn shape_elem_mut(body: &mut [Node]) -> Option<&mut Element> {
    body.iter_mut().find_map(|n| match n {
        Node::Elem(e) if e.local() == "sp" || e.local() == "cxnSp" => Some(e),
        _ => None,
    })
}

fn color_in(e: &Element, theme: &[u32; 12]) -> Option<u32> {
    e.elems().find_map(|c| crate::chart::color_of(c, theme))
}

fn fill_of(e: &Element, theme: &[u32; 12]) -> Option<Option<(u32, f64)>> {
    for c in e.elems() {
        match c.local() {
            "noFill" => return Some(None),
            "solidFill" => {
                let col = color_in(c, theme)?;
                let alpha = c.elems().next().and_then(|x| x.elems().find(|m| m.local() == "alpha"))
                    .and_then(|m| m.attr("val")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 100000.0).unwrap_or(1.0);
                return Some(Some((col, alpha)));
            }
            "gradFill" => {
                // the first stop stands for the gradient
                let gs = c.child("gsLst").and_then(|l| l.elems().next())?;
                return Some(Some((color_in(gs, theme)?, 1.0)));
            }
            _ => {}
        }
    }
    None
}

impl ShapeView {
    pub fn parse(body: &[Node], theme: &[u32; 12]) -> Option<ShapeView> {
        let sp = shape_elem(body)?;
        let connector = sp.local() == "cxnSp";
        let sppr = sp.child("spPr");
        let mut v = ShapeView {
            geom: "rect".into(),
            adj: vec![],
            fill: None,
            line: None,
            head: false,
            tail: false,
            rot: 0.0,
            flip_h: false,
            flip_v: false,
            paras: vec![],
            anchor: 't',
            wrap: true,
            insets: (7.2, 3.6, 7.2, 3.6),
            textbox: sp.child("nvSpPr").and_then(|n| n.child("cNvSpPr")).and_then(|c| c.attr("txBox")).map(|t| t == "1").unwrap_or(false),
            connector,
            math: None,
        };
        // the style's references: what a shape looks like unless spPr says otherwise
        let style = sp.child("style");
        let style_color = |name: &str| style.and_then(|s| s.child(name)).and_then(|r| color_in(r, theme));
        let style_idx = |name: &str| style.and_then(|s| s.child(name)).and_then(|r| r.attr("idx")).and_then(|i| i.parse::<u32>().ok()).unwrap_or(0);
        if style_idx("fillRef") > 0 {
            v.fill = style_color("fillRef").map(|c| (c, 1.0));
        }
        if style_idx("lnRef") > 0 {
            if let Some(c) = style_color("lnRef") {
                v.line = Some((c, if style_idx("lnRef") >= 2 { 1.0 } else { 0.75 }, false));
            }
        }
        if let Some(pr) = sppr {
            if let Some(x) = pr.child("xfrm") {
                v.rot = emu_attr(x, "rot").unwrap_or(0.0) / 60000.0;
                v.flip_h = x.attr("flipH") == Some("1");
                v.flip_v = x.attr("flipV") == Some("1");
            }
            if let Some(g) = pr.child("prstGeom") {
                v.geom = g.attr("prst").unwrap_or("rect").to_string();
                if let Some(av) = g.child("avLst") {
                    for gd in av.elems() {
                        if let (Some(n), Some(f)) = (gd.attr("name"), gd.attr("fmla")) {
                            if let Some(val) = f.strip_prefix("val ").and_then(|x| x.trim().parse::<f64>().ok()) {
                                v.adj.push((n.to_string(), val));
                            }
                        }
                    }
                }
            } else if pr.child("custGeom").is_some() {
                v.geom = "rect".into();
            }
            if let Some(f) = fill_of(pr, theme) {
                v.fill = f;
            }
            if let Some(ln) = pr.child("ln") {
                let w = emu_attr(ln, "w").map(|w| w / EMU_PER_PT);
                match fill_of(ln, theme) {
                    Some(None) => v.line = None,
                    Some(Some((c, _))) => v.line = Some((c, w.unwrap_or(0.75), false)),
                    None => {
                        if let (Some(w), Some(l)) = (w, v.line.as_mut()) {
                            l.1 = w;
                        }
                    }
                }
                if let Some(d) = ln.child("prstDash").and_then(|d| d.attr("val")) {
                    if let Some(l) = v.line.as_mut() {
                        l.2 = d != "solid";
                    }
                }
                let arrow = |n: &str| ln.child(n).and_then(|h| h.attr("type")).map(|t| t != "none").unwrap_or(false);
                v.head = arrow("headEnd");
                v.tail = arrow("tailEnd");
            }
        }
        // text
        let font_color = style_color("fontRef").unwrap_or(theme[1]);
        if let Some(tx) = sp.child("txBody") {
            if let Some(me) = crate::math::find_math(tx) {
                let size = crate::math::math_size(me)
                    .or_else(|| tx.elems().filter(|e| e.local() == "p").filter_map(|p| p.child("endParaRPr")).filter_map(|r| r.attr("sz")).filter_map(|v| v.parse::<f64>().ok()).next().map(|x| x / 100.0))
                    .unwrap_or(11.0);
                v.math = Some((crate::math::read_omml(me), size));
            }
            if let Some(bp) = tx.child("bodyPr") {
                v.anchor = match bp.attr("anchor") {
                    Some("ctr") => 'c',
                    Some("b") => 'b',
                    _ => 't',
                };
                v.wrap = bp.attr("wrap") != Some("none");
                let ins = |n: &str, d: f64| emu_attr(bp, n).map(|x| x / EMU_PER_PT).unwrap_or(d);
                v.insets = (ins("lIns", 7.2), ins("tIns", 3.6), ins("rIns", 7.2), ins("bIns", 3.6));
            }
            for p in tx.elems().filter(|e| e.local() == "p") {
                let align = match p.child("pPr").and_then(|pp| pp.attr("algn")) {
                    Some("ctr") => 1,
                    Some("r") => 2,
                    _ => 0,
                };
                let mut runs = Vec::new();
                for r in p.elems() {
                    let (rpr, text) = match r.local() {
                        "r" => (r.child("rPr"), r.child("t").map(|t| t.text()).unwrap_or_default()),
                        "fld" => (r.child("rPr"), r.child("t").map(|t| t.text()).unwrap_or_default()),
                        "br" => (r.child("rPr"), "\n".to_string()),
                        _ => continue,
                    };
                    let mut run = Run { text, size: 11.0, bold: false, italic: false, underline: false, color: font_color, font: None, outline: None };
                    if let Some(rp) = rpr {
                        run.size = emu_attr(rp, "sz").map(|s| s / 100.0).unwrap_or(11.0);
                        run.bold = rp.attr("b") == Some("1");
                        run.italic = rp.attr("i") == Some("1");
                        run.underline = rp.attr("u").map(|u| u != "none").unwrap_or(false);
                        if let Some(Some((c, _))) = fill_of(rp, theme) {
                            run.color = c;
                        }
                        if let Some(ln) = rp.child("ln") {
                            if let Some(Some((c, _))) = fill_of(ln, theme) {
                                run.outline = Some((c, emu_attr(ln, "w").map(|w| w / EMU_PER_PT).unwrap_or(0.75)));
                            }
                        }
                        run.font = rp.child("latin").and_then(|l| l.attr("typeface")).filter(|t| !t.starts_with('+')).map(|t| t.to_string());
                    }
                    runs.push(run);
                }
                v.paras.push(Para { align, runs });
            }
        }
        Some(v)
    }

    pub fn text(&self) -> String {
        self.paras.iter().map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    fn adj(&self, name: &str, default: f64) -> f64 {
        self.adj.iter().find(|(n, _)| n == name).map(|(_, v)| *v).unwrap_or(default)
    }
}

// ---------------------------------------------------------------------- preset geometry

const K: f64 = 0.552_284_749_8; // bezier circle constant

fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<Seg> {
    vec![
        Seg::Move(cx + rx, cy),
        Seg::Curve(cx + rx, cy + ry * K, cx + rx * K, cy + ry, cx, cy + ry),
        Seg::Curve(cx - rx * K, cy + ry, cx - rx, cy + ry * K, cx - rx, cy),
        Seg::Curve(cx - rx, cy - ry * K, cx - rx * K, cy - ry, cx, cy - ry),
        Seg::Curve(cx + rx * K, cy - ry, cx + rx, cy - ry * K, cx + rx, cy),
        Seg::Close,
    ]
}

fn poly(pts: &[(f64, f64)]) -> Vec<Seg> {
    let mut v = Vec::with_capacity(pts.len() + 1);
    for (i, &(x, y)) in pts.iter().enumerate() {
        v.push(if i == 0 { Seg::Move(x, y) } else { Seg::Line(x, y) });
    }
    v.push(Seg::Close);
    v
}

fn round_rect(x: f64, y: f64, w: f64, h: f64, r: f64) -> Vec<Seg> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = r * (1.0 - K);
    vec![
        Seg::Move(x + r, y),
        Seg::Line(x + w - r, y),
        Seg::Curve(x + w - k, y, x + w, y + k, x + w, y + r),
        Seg::Line(x + w, y + h - r),
        Seg::Curve(x + w, y + h - k, x + w - k, y + h, x + w - r, y + h),
        Seg::Line(x + r, y + h),
        Seg::Curve(x + k, y + h, x, y + h - k, x, y + h - r),
        Seg::Line(x, y + r),
        Seg::Curve(x, y + k, x + k, y, x + r, y),
        Seg::Close,
    ]
}

fn regular(n: usize, w: f64, h: f64, start: f64) -> Vec<Seg> {
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let a = start + i as f64 * std::f64::consts::TAU / n as f64;
            (w / 2.0 + w / 2.0 * a.cos(), h / 2.0 + h / 2.0 * a.sin())
        })
        .collect();
    poly(&pts)
}

fn star(n: usize, w: f64, h: f64, inner: f64) -> Vec<Seg> {
    let mut pts = Vec::new();
    for i in 0..n * 2 {
        let a = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / n as f64;
        let r = if i % 2 == 0 { 1.0 } else { inner };
        pts.push((w / 2.0 + w / 2.0 * r * a.cos(), h / 2.0 + h / 2.0 * r * a.sin()));
    }
    poly(&pts)
}

fn filled(segs: Vec<Seg>) -> SubPath {
    SubPath { segs, fill: true }
}

fn stroke_only(segs: Vec<Seg>) -> SubPath {
    SubPath { segs, fill: false }
}

/// The paths of a preset shape in a w × h box (unknown presets are drawn as rectangles).
pub fn preset(v: &ShapeView, w: f64, h: f64) -> Vec<SubPath> {
    let ss = w.min(h);
    let a = |n: &str, d: f64| v.adj(n, d) / 100000.0;
    let g = v.geom.as_str();
    match g {
        "line" | "straightConnector1" | "bentConnector3" | "curvedConnector3" => vec![stroke_only(vec![Seg::Move(0.0, 0.0), Seg::Line(w, h)])],
        "rect" | "flowChartProcess" | "textBox" => vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]))],
        "roundRect" | "flowChartAlternateProcess" => vec![filled(round_rect(0.0, 0.0, w, h, ss * a("adj", 16667.0)))],
        "flowChartTerminator" => vec![filled(round_rect(0.0, 0.0, w, h, ss / 2.0))],
        "snip1Rect" => {
            let d = ss * a("adj", 16667.0);
            vec![filled(poly(&[(0.0, 0.0), (w - d, 0.0), (w, d), (w, h), (0.0, h)]))]
        }
        "ellipse" | "flowChartConnector" => vec![filled(ellipse(w / 2.0, h / 2.0, w / 2.0, h / 2.0))],
        "triangle" | "flowChartExtract" => vec![filled(poly(&[(w * a("adj", 50000.0), 0.0), (w, h), (0.0, h)]))],
        "flowChartMerge" => vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w / 2.0, h)]))],
        "rtTriangle" => vec![filled(poly(&[(0.0, 0.0), (w, h), (0.0, h)]))],
        "parallelogram" | "flowChartInputOutput" => {
            let d = if g == "parallelogram" { ss * a("adj", 25000.0) } else { w / 5.0 };
            vec![filled(poly(&[(d, 0.0), (w, 0.0), (w - d, h), (0.0, h)]))]
        }
        "trapezoid" | "flowChartManualOperation" => {
            let d = ss * a("adj", 25000.0);
            if g == "trapezoid" {
                vec![filled(poly(&[(d, 0.0), (w - d, 0.0), (w, h), (0.0, h)]))]
            } else {
                vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w - w / 5.0, h), (w / 5.0, h)]))]
            }
        }
        "diamond" | "flowChartDecision" => vec![filled(poly(&[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]))],
        "pentagon" => vec![filled(regular(5, w, h, -std::f64::consts::FRAC_PI_2))],
        "hexagon" | "flowChartPreparation" => {
            let d = if g == "hexagon" { ss * a("adj", 25000.0) } else { w / 5.0 };
            vec![filled(poly(&[(d, 0.0), (w - d, 0.0), (w, h / 2.0), (w - d, h), (d, h), (0.0, h / 2.0)]))]
        }
        "heptagon" => vec![filled(regular(7, w, h, -std::f64::consts::FRAC_PI_2))],
        "octagon" => {
            let d = ss * a("adj", 29289.0);
            vec![filled(poly(&[(d, 0.0), (w - d, 0.0), (w, d), (w, h - d), (w - d, h), (d, h), (0.0, h - d), (0.0, d)]))]
        }
        "decagon" => vec![filled(regular(10, w, h, 0.0))],
        "dodecagon" => vec![filled(regular(12, w, h, 0.0))],
        "plus" | "mathPlus" => {
            let t = if g == "plus" { ss * a("adj", 25000.0) * 2.0 } else { ss * 0.24 };
            let (cx, cy) = (w / 2.0, h / 2.0);
            let (hx, hy) = if g == "plus" { (w / 2.0, h / 2.0) } else { (w * 0.4, h * 0.4) };
            vec![filled(poly(&[(cx - t / 2.0, cy - hy), (cx + t / 2.0, cy - hy), (cx + t / 2.0, cy - t / 2.0), (cx + hx, cy - t / 2.0),
                               (cx + hx, cy + t / 2.0), (cx + t / 2.0, cy + t / 2.0), (cx + t / 2.0, cy + hy), (cx - t / 2.0, cy + hy),
                               (cx - t / 2.0, cy + t / 2.0), (cx - hx, cy + t / 2.0), (cx - hx, cy - t / 2.0), (cx - t / 2.0, cy - t / 2.0)]))]
        }
        "mathMinus" => {
            let t = ss * 0.24;
            vec![filled(poly(&[(w * 0.1, h / 2.0 - t / 2.0), (w * 0.9, h / 2.0 - t / 2.0), (w * 0.9, h / 2.0 + t / 2.0), (w * 0.1, h / 2.0 + t / 2.0)]))]
        }
        "mathEqual" => {
            let t = ss * 0.2;
            vec![
                filled(poly(&[(w * 0.1, h / 2.0 - t * 1.3), (w * 0.9, h / 2.0 - t * 1.3), (w * 0.9, h / 2.0 - t * 0.3), (w * 0.1, h / 2.0 - t * 0.3)])),
                filled(poly(&[(w * 0.1, h / 2.0 + t * 0.3), (w * 0.9, h / 2.0 + t * 0.3), (w * 0.9, h / 2.0 + t * 1.3), (w * 0.1, h / 2.0 + t * 1.3)])),
            ]
        }
        "mathMultiply" => {
            let t = ss * 0.17;
            let (cx, cy) = (w / 2.0, h / 2.0);
            let rr = ss * 0.4;
            let arm = |ang: f64| -> Vec<(f64, f64)> {
                let (c, s) = (ang.cos(), ang.sin());
                let (nx, ny) = (-s * t / 2.0, c * t / 2.0);
                vec![(cx - c * rr + nx, cy - s * rr + ny), (cx + c * rr + nx, cy + s * rr + ny), (cx + c * rr - nx, cy + s * rr - ny), (cx - c * rr - nx, cy - s * rr - ny)]
            };
            vec![filled(poly(&arm(std::f64::consts::FRAC_PI_4))), filled(poly(&arm(-std::f64::consts::FRAC_PI_4)))]
        }
        "mathDivide" => {
            let t = ss * 0.18;
            vec![
                filled(poly(&[(w * 0.1, h / 2.0 - t / 2.0), (w * 0.9, h / 2.0 - t / 2.0), (w * 0.9, h / 2.0 + t / 2.0), (w * 0.1, h / 2.0 + t / 2.0)])),
                filled(ellipse(w / 2.0, h * 0.22, t * 0.7, t * 0.7)),
                filled(ellipse(w / 2.0, h * 0.78, t * 0.7, t * 0.7)),
            ]
        }
        "mathNotEqual" => {
            let t = ss * 0.18;
            let mut v2 = vec![
                filled(poly(&[(w * 0.1, h / 2.0 - t * 1.3), (w * 0.9, h / 2.0 - t * 1.3), (w * 0.9, h / 2.0 - t * 0.3), (w * 0.1, h / 2.0 - t * 0.3)])),
                filled(poly(&[(w * 0.1, h / 2.0 + t * 0.3), (w * 0.9, h / 2.0 + t * 0.3), (w * 0.9, h / 2.0 + t * 1.3), (w * 0.1, h / 2.0 + t * 1.3)])),
            ];
            v2.push(filled(poly(&[(w * 0.62, h * 0.08), (w * 0.72, h * 0.12), (w * 0.38, h * 0.92), (w * 0.28, h * 0.88)])));
            v2
        }
        "star4" => vec![filled(star(4, w, h, 0.38))],
        "star5" => vec![filled(star(5, w, h, 0.38))],
        "star6" => vec![filled(star(6, w, h, 0.58))],
        "star7" => vec![filled(star(7, w, h, 0.6))],
        "star8" => vec![filled(star(8, w, h, 0.7))],
        "star10" => vec![filled(star(10, w, h, 0.75))],
        "star12" => vec![filled(star(12, w, h, 0.75))],
        "star16" | "star24" | "star32" => vec![filled(star(16, w, h, 0.8))],
        "rightArrow" | "leftArrow" | "upArrow" | "downArrow" | "notchedRightArrow" => {
            let (along, across) = if g == "upArrow" || g == "downArrow" { (h, w) } else { (w, h) };
            let shaft = across * a("adj1", 50000.0);
            let head = (ss * a("adj2", 50000.0)).min(along);
            let y1 = (across - shaft) / 2.0;
            let mut pts = vec![(0.0, y1), (along - head, y1), (along - head, 0.0), (along, across / 2.0), (along - head, across), (along - head, y1 + shaft), (0.0, y1 + shaft)];
            if g == "notchedRightArrow" {
                pts.push((head / 2.0, across / 2.0));
            }
            let map: Vec<(f64, f64)> = pts.into_iter().map(|(x, y)| match g {
                "leftArrow" => (w - x, y),
                "downArrow" => (y, x),
                "upArrow" => (y, h - x),
                _ => (x, y),
            }).collect();
            vec![filled(poly(&map))]
        }
        "leftRightArrow" | "upDownArrow" => {
            let vert = g == "upDownArrow";
            let (along, across) = if vert { (h, w) } else { (w, h) };
            let shaft = across * a("adj1", 50000.0);
            let head = (ss * a("adj2", 50000.0)).min(along / 2.0);
            let y1 = (across - shaft) / 2.0;
            let pts = [(0.0, across / 2.0), (head, 0.0), (head, y1), (along - head, y1), (along - head, 0.0), (along, across / 2.0),
                       (along - head, across), (along - head, y1 + shaft), (head, y1 + shaft), (head, across)];
            let map: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| if vert { (y, x) } else { (x, y) }).collect();
            vec![filled(poly(&map))]
        }
        "quadArrow" => {
            let t = ss * 0.11;
            let hd = ss * 0.22;
            let (cx, cy) = (w / 2.0, h / 2.0);
            vec![filled(poly(&[
                (cx, 0.0), (cx + hd, hd), (cx + t, hd), (cx + t, cy - t), (w - hd, cy - t), (w - hd, cy - hd), (w, cy), (w - hd, cy + hd),
                (w - hd, cy + t), (cx + t, cy + t), (cx + t, h - hd), (cx + hd, h - hd), (cx, h), (cx - hd, h - hd), (cx - t, h - hd),
                (cx - t, cy + t), (hd, cy + t), (hd, cy + hd), (0.0, cy), (hd, cy - hd), (hd, cy - t), (cx - t, cy - t), (cx - t, hd), (cx - hd, hd),
            ]))]
        }
        "homePlate" => {
            let d = ss * a("adj", 50000.0);
            vec![filled(poly(&[(0.0, 0.0), (w - d, 0.0), (w, h / 2.0), (w - d, h), (0.0, h)]))]
        }
        "chevron" => {
            let d = ss * a("adj", 50000.0);
            vec![filled(poly(&[(0.0, 0.0), (w - d, 0.0), (w, h / 2.0), (w - d, h), (0.0, h), (d, h / 2.0)]))]
        }
        "frame" => {
            let d = ss * a("adj1", 12500.0);
            let mut o = poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]);
            o.extend(poly(&[(d, d), (d, h - d), (w - d, h - d), (w - d, d)]));
            vec![filled(o)]
        }
        "donut" => {
            let d = ss * a("adj", 25000.0);
            // the hole goes the other way round
            let mut o = ellipse(w / 2.0, h / 2.0, w / 2.0, h / 2.0);
            o.extend(reverse_ellipse(w / 2.0, h / 2.0, (w / 2.0 - d).max(0.5), (h / 2.0 - d).max(0.5)));
            vec![filled(o)]
        }
        "can" | "flowChartMagneticDisk" => {
            let ry = h * a("adj", 25000.0) / 2.0;
            let mut body = vec![Seg::Move(0.0, ry), Seg::Line(0.0, h - ry)];
            body.push(Seg::Curve(0.0, h - ry + ry * K, w / 2.0 - w / 2.0 * K, h, w / 2.0, h));
            body.push(Seg::Curve(w / 2.0 + w / 2.0 * K, h, w, h - ry + ry * K, w, h - ry));
            body.push(Seg::Line(w, ry));
            body.push(Seg::Curve(w, ry + ry * K, w / 2.0 + w / 2.0 * K, 2.0 * ry, w / 2.0, 2.0 * ry));
            body.push(Seg::Curve(w / 2.0 - w / 2.0 * K, 2.0 * ry, 0.0, ry + ry * K, 0.0, ry));
            body.push(Seg::Close);
            vec![filled(body), filled(ellipse(w / 2.0, ry, w / 2.0, ry))]
        }
        "cube" => {
            let d = ss * a("adj", 25000.0);
            vec![
                filled(poly(&[(0.0, d), (w - d, d), (w - d, h), (0.0, h)])),
                filled(poly(&[(0.0, d), (d, 0.0), (w, 0.0), (w - d, d)])),
                filled(poly(&[(w - d, d), (w, 0.0), (w, h - d), (w - d, h)])),
            ]
        }
        "flowChartDocument" => vec![filled(vec![
            Seg::Move(0.0, 0.0), Seg::Line(w, 0.0), Seg::Line(w, h * 0.83),
            Seg::Curve(w * 0.75, h * 0.73, w * 0.25, h * 1.05, 0.0, h * 0.9), Seg::Close,
        ])],
        "flowChartPredefinedProcess" => vec![
            filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)])),
            stroke_only(vec![Seg::Move(w * 0.125, 0.0), Seg::Line(w * 0.125, h), Seg::Move(w * 0.875, 0.0), Seg::Line(w * 0.875, h)]),
        ],
        "flowChartManualInput" => vec![filled(poly(&[(0.0, h * 0.2), (w, 0.0), (w, h), (0.0, h)]))],
        "flowChartOffpageConnector" => vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h * 0.8), (w / 2.0, h), (0.0, h * 0.8)]))],
        "flowChartPunchedCard" => vec![filled(poly(&[(w * 0.2, 0.0), (w, 0.0), (w, h), (0.0, h), (0.0, h * 0.2)]))],
        "foldedCorner" => {
            let d = ss * a("adj", 16667.0);
            vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h - d), (w - d, h), (0.0, h)])), filled(poly(&[(w - d, h), (w - d * 0.8, h - d * 0.8), (w, h - d)]))]
        }
        "heart" => vec![filled(vec![
            Seg::Move(w / 2.0, h * 0.25),
            Seg::Curve(w * 0.6, h * -0.05, w * 1.05, h * 0.05, w * 0.95, h * 0.4),
            Seg::Curve(w * 0.88, h * 0.62, w * 0.6, h * 0.8, w / 2.0, h),
            Seg::Curve(w * 0.4, h * 0.8, w * 0.12, h * 0.62, w * 0.05, h * 0.4),
            Seg::Curve(w * -0.05, h * 0.05, w * 0.4, h * -0.05, w / 2.0, h * 0.25),
            Seg::Close,
        ])],
        "lightningBolt" => vec![filled(poly(&[
            (w * 0.39, 0.0), (w * 0.62, h * 0.3), (w * 0.55, h * 0.33), (w * 0.8, h * 0.6), (w * 0.73, h * 0.63), (w, h),
            (w * 0.47, h * 0.7), (w * 0.55, h * 0.67), (w * 0.22, h * 0.45), (w * 0.3, h * 0.41), (0.0, h * 0.18),
        ]))],
        "moon" => vec![filled(vec![
            Seg::Move(w, 0.0), Seg::Curve(w * 0.45, 0.0, 0.0, h * 0.22, 0.0, h / 2.0),
            Seg::Curve(0.0, h * 0.78, w * 0.45, h, w, h),
            Seg::Curve(w * 0.7, h * 0.85, w * 0.5, h * 0.7, w * 0.5, h / 2.0),
            Seg::Curve(w * 0.5, h * 0.3, w * 0.7, h * 0.15, w, 0.0), Seg::Close,
        ])],
        "smileyFace" => vec![
            filled(ellipse(w / 2.0, h / 2.0, w / 2.0, h / 2.0)),
            filled(ellipse(w * 0.33, h * 0.37, w * 0.06, h * 0.06)),
            filled(ellipse(w * 0.67, h * 0.37, w * 0.06, h * 0.06)),
            stroke_only(vec![Seg::Move(w * 0.25, h * 0.62), Seg::Curve(w * 0.35, h * 0.8, w * 0.65, h * 0.8, w * 0.75, h * 0.62)]),
        ],
        "sun" => {
            let mut v2 = vec![filled(ellipse(w / 2.0, h / 2.0, w * 0.25, h * 0.25))];
            for i in 0..8 {
                let ang = i as f64 * std::f64::consts::FRAC_PI_4;
                let (c, s) = (ang.cos(), ang.sin());
                let tip = (w / 2.0 + c * w * 0.5, h / 2.0 + s * h * 0.5);
                let (px, py) = (-s * 0.07, c * 0.07);
                v2.push(filled(poly(&[(w / 2.0 + c * w * 0.34 + px * w, h / 2.0 + s * h * 0.34 + py * h), tip, (w / 2.0 + c * w * 0.34 - px * w, h / 2.0 + s * h * 0.34 - py * h)])));
            }
            v2
        }
        "cloud" | "cloudCallout" => {
            let mut v2 = Vec::new();
            for &(cx, cy, rx, ry) in &[(0.3, 0.35, 0.22, 0.22), (0.55, 0.28, 0.25, 0.25), (0.75, 0.45, 0.22, 0.22), (0.6, 0.68, 0.24, 0.22), (0.32, 0.65, 0.24, 0.22), (0.2, 0.5, 0.18, 0.18)] {
                v2.push(filled(ellipse(w * cx, h * cy, w * rx, h * ry)));
            }
            if g == "cloudCallout" {
                v2.push(filled(ellipse(w * 0.12, h * 0.95, w * 0.05, h * 0.05)));
                v2.push(filled(ellipse(w * 0.2, h * 0.86, w * 0.07, h * 0.07)));
            }
            v2
        }
        "wedgeRectCallout" | "wedgeRoundRectCallout" | "wedgeEllipseCallout" => {
            let tx = w / 2.0 + w * a("adj1", -20833.0);
            let ty = h / 2.0 + h * a("adj2", 62500.0);
            // the tail leaves the bottom edge, between a quarter and nearly half of the way along
            let (x1, x2) = (w * 0.25, w * 0.45);
            match g {
                "wedgeRectCallout" => vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (x2, h), (tx, ty), (x1, h), (0.0, h)]))],
                "wedgeRoundRectCallout" => {
                    let r = ss * 0.16667;
                    let k = r * (1.0 - K);
                    vec![filled(vec![
                        Seg::Move(r, 0.0), Seg::Line(w - r, 0.0), Seg::Curve(w - k, 0.0, w, k, w, r), Seg::Line(w, h - r),
                        Seg::Curve(w, h - k, w - k, h, w - r, h), Seg::Line(x2, h), Seg::Line(tx, ty), Seg::Line(x1, h),
                        Seg::Line(r, h), Seg::Curve(k, h, 0.0, h - k, 0.0, h - r), Seg::Line(0.0, r), Seg::Curve(0.0, k, k, 0.0, r, 0.0), Seg::Close,
                    ])]
                }
                _ => {
                    let mut body = ellipse(w / 2.0, h / 2.0, w / 2.0, h / 2.0);
                    body.extend(poly(&[(w * 0.3, h * 0.9), (tx, ty), (w * 0.45, h * 0.97)]));
                    vec![filled(body)]
                }
            }
        }
        "bracketPair" => vec![stroke_only(vec![
            Seg::Move(ss * 0.15, 0.0), Seg::Curve(0.0, 0.0, 0.0, 0.0, 0.0, ss * 0.15), Seg::Line(0.0, h - ss * 0.15),
            Seg::Curve(0.0, h, 0.0, h, ss * 0.15, h), Seg::Move(w - ss * 0.15, 0.0), Seg::Curve(w, 0.0, w, 0.0, w, ss * 0.15),
            Seg::Line(w, h - ss * 0.15), Seg::Curve(w, h, w, h, w - ss * 0.15, h),
        ])],
        "arc" => vec![stroke_only(vec![Seg::Move(w / 2.0, 0.0), Seg::Curve(w / 2.0 + w / 2.0 * K, 0.0, w, h / 2.0 - h / 2.0 * K, w, h / 2.0)])],
        "noSmoking" => {
            let d = ss * 0.18;
            let mut o = ellipse(w / 2.0, h / 2.0, w / 2.0, h / 2.0);
            o.extend(reverse_ellipse(w / 2.0, h / 2.0, w / 2.0 - d, h / 2.0 - d));
            let t = d / 2.0;
            vec![filled(o), filled(poly(&[(w * 0.2 - t, h * 0.25), (w * 0.25, h * 0.2 - t), (w * 0.8 + t, h * 0.75), (w * 0.75, h * 0.8 + t)]))]
        }
        "teardrop" => vec![filled(vec![
            Seg::Move(w, 0.0), Seg::Line(w, h / 2.0), Seg::Curve(w, h / 2.0 + h / 2.0 * K, w / 2.0 + w / 2.0 * K, h, w / 2.0, h),
            Seg::Curve(w / 2.0 - w / 2.0 * K, h, 0.0, h / 2.0 + h / 2.0 * K, 0.0, h / 2.0),
            Seg::Curve(0.0, h / 2.0 - h / 2.0 * K, w / 2.0 - w / 2.0 * K, 0.0, w / 2.0, 0.0), Seg::Close,
        ])],
        "pie" => vec![filled(vec![
            Seg::Move(w / 2.0, h / 2.0), Seg::Line(w, h / 2.0),
            Seg::Curve(w, h / 2.0 + h / 2.0 * K, w / 2.0 + w / 2.0 * K, h, w / 2.0, h),
            Seg::Curve(w / 2.0 - w / 2.0 * K, h, 0.0, h / 2.0 + h / 2.0 * K, 0.0, h / 2.0),
            Seg::Curve(0.0, h / 2.0 - h / 2.0 * K, w / 2.0 - w / 2.0 * K, 0.0, w / 2.0, 0.0), Seg::Close,
        ])],
        _ => vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]))],
    }
}

fn reverse_ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<Seg> {
    vec![
        Seg::Move(cx + rx, cy),
        Seg::Curve(cx + rx, cy - ry * K, cx + rx * K, cy - ry, cx, cy - ry),
        Seg::Curve(cx - rx * K, cy - ry, cx - rx, cy - ry * K, cx - rx, cy),
        Seg::Curve(cx - rx, cy + ry * K, cx - rx * K, cy + ry, cx, cy + ry),
        Seg::Curve(cx + rx * K, cy + ry, cx + rx, cy + ry * K, cx + rx, cy),
        Seg::Close,
    ]
}

/// The presets the Shapes gallery offers, by group, as the desktop spreadsheet arranges them.
pub const GALLERY: &[(&str, &[&str])] = &[
    ("Lines", &["line", "straightConnector1", "arc"]),
    ("Rectangles", &["rect", "roundRect", "snip1Rect"]),
    ("Basic Shapes", &["textBox", "ellipse", "triangle", "rtTriangle", "parallelogram", "trapezoid", "diamond", "pentagon",
                       "hexagon", "heptagon", "octagon", "decagon", "dodecagon", "pie", "teardrop", "frame", "plus", "can", "cube",
                       "donut", "noSmoking", "foldedCorner", "smileyFace", "heart", "lightningBolt", "sun", "moon", "cloud", "bracketPair"]),
    ("Block Arrows", &["rightArrow", "leftArrow", "upArrow", "downArrow", "leftRightArrow", "upDownArrow", "quadArrow",
                       "notchedRightArrow", "homePlate", "chevron"]),
    ("Equation Shapes", &["mathPlus", "mathMinus", "mathMultiply", "mathDivide", "mathEqual", "mathNotEqual"]),
    ("Flowchart", &["flowChartProcess", "flowChartAlternateProcess", "flowChartDecision", "flowChartInputOutput",
                    "flowChartPredefinedProcess", "flowChartDocument", "flowChartTerminator", "flowChartPreparation",
                    "flowChartManualInput", "flowChartManualOperation", "flowChartConnector", "flowChartOffpageConnector",
                    "flowChartPunchedCard", "flowChartExtract", "flowChartMerge", "flowChartMagneticDisk"]),
    ("Stars and Banners", &["star4", "star5", "star6", "star7", "star8", "star10", "star12", "star16"]),
    ("Callouts", &["wedgeRectCallout", "wedgeRoundRectCallout", "wedgeEllipseCallout", "cloudCallout"]),
];

/// The names the desktop spreadsheet gives new shapes ("Rectangle 3", "Oval 1" ...).
pub fn base_name(prst: &str) -> &'static str {
    match prst {
        "rect" => "Rectangle",
        "roundRect" => "Rectangle: Rounded Corners",
        "ellipse" => "Oval",
        "triangle" => "Isosceles Triangle",
        "rtTriangle" => "Right Triangle",
        "line" => "Straight Connector",
        "straightConnector1" => "Straight Arrow Connector",
        "textBox" => "TextBox",
        "rightArrow" => "Arrow: Right",
        "leftArrow" => "Arrow: Left",
        "upArrow" => "Arrow: Up",
        "downArrow" => "Arrow: Down",
        "star5" => "Star: 5 Points",
        "heart" => "Heart",
        "diamond" => "Diamond",
        "wedgeRectCallout" => "Speech Bubble: Rectangle",
        "cloud" => "Cloud",
        _ => "Shape",
    }
}

// ---------------------------------------------------------------------- new shapes

fn xfrm(cx: i64, cy: i64) -> String {
    format!("<a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>", cx, cy)
}

/// The anchor body of a new shape, line, text box or WordArt.
pub fn new_body(id: u32, name: &str, prst: &str, cx: i64, cy: i64, text: Option<&str>, wordart: Option<usize>) -> Vec<Node> {
    let name = xml::escape_attr(name);
    let s = if prst == "line" || prst == "straightConnector1" {
        let arrow = if prst == "straightConnector1" { "<a:tailEnd type=\"triangle\"/>" } else { "" };
        format!("<x><xdr:cxnSp macro=\"\"><xdr:nvCxnSpPr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvCxnSpPr/></xdr:nvCxnSpPr><xdr:spPr>{xf}<a:prstGeom prst=\"{p}\"><a:avLst/></a:prstGeom><a:ln>{arrow}</a:ln></xdr:spPr><xdr:style><a:lnRef idx=\"1\"><a:schemeClr val=\"accent1\"/></a:lnRef><a:fillRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:fillRef><a:effectRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:effectRef><a:fontRef idx=\"minor\"><a:schemeClr val=\"tx1\"/></a:fontRef></xdr:style></xdr:cxnSp><xdr:clientData/></x>",
            id = id, name = name, xf = xfrm(cx, cy), p = prst, arrow = arrow)
    } else if prst == "textBox" || wordart.is_some() {
        let t = text.unwrap_or("");
        let (fill, ln, rpr, body) = match wordart {
            Some(k) => {
                let (fill, outline) = WORDART[k.min(WORDART.len() - 1)].1;
                let outline = outline.map(|o| format!("<a:ln w=\"9525\"><a:solidFill>{}</a:solidFill></a:ln>", o)).unwrap_or_default();
                ("<a:noFill/>".to_string(), "<a:ln><a:noFill/></a:ln>".to_string(),
                 format!("<a:rPr lang=\"en-US\" sz=\"3600\" b=\"1\" cap=\"none\" spc=\"0\">{}<a:solidFill>{}</a:solidFill></a:rPr>", outline, fill),
                 "<a:bodyPr wrap=\"none\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\"><a:spAutoFit/></a:bodyPr>".to_string())
            }
            None => ("<a:solidFill><a:schemeClr val=\"lt1\"/></a:solidFill>".to_string(),
                     "<a:ln w=\"9525\" cmpd=\"sng\"><a:solidFill><a:schemeClr val=\"lt1\"><a:shade val=\"50000\"/></a:schemeClr></a:solidFill></a:ln>".to_string(),
                     "<a:rPr lang=\"en-US\" sz=\"1100\"/>".to_string(),
                     "<a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\" wrap=\"square\" rtlCol=\"0\" anchor=\"t\"/>".to_string()),
        };
        let paras: String = if t.is_empty() {
            format!("<a:p><a:endParaRPr lang=\"en-US\" sz=\"{}\"/></a:p>", if wordart.is_some() { 3600 } else { 1100 })
        } else {
            t.split('\n').map(|l| format!("<a:p>{}<a:r>{}<a:t>{}</a:t></a:r></a:p>", if wordart.is_some() { "<a:pPr algn=\"ctr\"/>" } else { "" }, rpr, xml::escape_text(l))).collect()
        };
        format!("<x><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvSpPr txBox=\"1\"/></xdr:nvSpPr><xdr:spPr>{xf}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>{fill}{ln}</xdr:spPr><xdr:style><a:lnRef idx=\"0\"><a:scrgbClr r=\"0\" g=\"0\" b=\"0\"/></a:lnRef><a:fillRef idx=\"0\"><a:scrgbClr r=\"0\" g=\"0\" b=\"0\"/></a:fillRef><a:effectRef idx=\"0\"><a:scrgbClr r=\"0\" g=\"0\" b=\"0\"/></a:effectRef><a:fontRef idx=\"minor\"><a:schemeClr val=\"tx1\"/></a:fontRef></xdr:style><xdr:txBody>{body}<a:lstStyle/>{paras}</xdr:txBody></xdr:sp><xdr:clientData/></x>",
            id = id, name = name, xf = xfrm(cx, cy), fill = fill, ln = ln, body = body, paras = paras)
    } else {
        format!("<x><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr>{xf}<a:prstGeom prst=\"{p}\"><a:avLst/></a:prstGeom></xdr:spPr><xdr:style><a:lnRef idx=\"2\"><a:schemeClr val=\"accent1\"><a:shade val=\"50000\"/></a:schemeClr></a:lnRef><a:fillRef idx=\"1\"><a:schemeClr val=\"accent1\"/></a:fillRef><a:effectRef idx=\"0\"><a:schemeClr val=\"accent1\"/></a:effectRef><a:fontRef idx=\"minor\"><a:schemeClr val=\"lt1\"/></a:fontRef></xdr:style><xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\" rtlCol=\"0\" anchor=\"ctr\"/><a:lstStyle/><a:p><a:pPr algn=\"ctr\"/><a:endParaRPr lang=\"en-US\" sz=\"1100\"/></a:p></xdr:txBody></xdr:sp><xdr:clientData/></x>",
            id = id, name = name, xf = xfrm(cx, cy), p = prst)
    };
    xml::parse(&s).map(|(_, e)| e.children).unwrap_or_default()
}

/// The WordArt styles offered (name, (text fill, outline)) as DrawingML colour elements.
pub const WORDART: &[(&str, (&str, Option<&str>))] = &[
    ("Fill: Black, Text color 1; Shadow", ("<a:schemeClr val=\"tx1\"/>", None)),
    ("Fill: Blue, Accent color 1; Shadow", ("<a:schemeClr val=\"accent1\"/>", None)),
    ("Fill: Orange, Accent color 2; Outline: Orange, Accent color 2", ("<a:schemeClr val=\"accent2\"><a:lumMod val=\"40000\"/><a:lumOff val=\"60000\"/></a:schemeClr>", Some("<a:schemeClr val=\"accent2\"/>"))),
    ("Fill: White; Outline: Blue, Accent color 1", ("<a:schemeClr val=\"bg1\"/>", Some("<a:schemeClr val=\"accent1\"/>"))),
    ("Fill: Gold, Accent color 4; Soft Bevel", ("<a:schemeClr val=\"accent4\"/>", None)),
    ("Fill: Gray-50%, Accent color 3; Sharp Bevel", ("<a:schemeClr val=\"accent3\"/>", None)),
    ("Fill: Black, Text color 1; Outline: White, Background color 1", ("<a:schemeClr val=\"tx1\"/>", Some("<a:schemeClr val=\"bg1\"/>"))),
    ("Fill: Green, Accent color 6", ("<a:schemeClr val=\"accent6\"/>", None)),
];

// ---------------------------------------------------------------------- edits

fn color_xml(c: u32) -> String {
    format!("<a:srgbClr val=\"{:06X}\"/>", c & 0xFFFFFF)
}

fn parse_frag(s: &str) -> Element {
    xml::parse(&format!("<x xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">{}</x>", s)).map(|(_, e)| e).unwrap()
}

fn sp_pr(sp: &mut Element) -> &mut Element {
    if sp.child("spPr").is_none() {
        let pfx = sp.name.split(':').next().filter(|_| sp.name.contains(':')).unwrap_or("xdr").to_string();
        sp.push(Element::new(&format!("{}:spPr", pfx)));
    }
    sp.child_mut("spPr").unwrap()
}

const FILLS: [&str; 6] = ["noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill"];

/// Where a fill goes in spPr: after the geometry.
fn fill_pos(pr: &Element) -> usize {
    let mut pos = 0;
    for (i, n) in pr.children.iter().enumerate() {
        if let Node::Elem(e) = n {
            if matches!(e.local(), "xfrm" | "custGeom" | "prstGeom") {
                pos = i + 1;
            }
        }
    }
    pos
}

/// Shape Fill: a colour, or None for No Fill.
pub fn set_fill(body: &mut [Node], color: Option<u32>) {
    if let Some(sp) = shape_elem_mut(body) {
        let pr = sp_pr(sp);
        pr.children.retain(|n| !matches!(n, Node::Elem(e) if FILLS.contains(&e.local())));
        let f = parse_frag(&match color {
            Some(c) => format!("<a:solidFill>{}</a:solidFill>", color_xml(c)),
            None => "<a:noFill/>".into(),
        });
        let pos = fill_pos(pr);
        for (k, n) in f.children.into_iter().enumerate() {
            pr.children.insert(pos + k, n);
        }
    }
}

/// Shape Outline: a colour (None: No Outline), and a weight in points.
pub fn set_line(body: &mut [Node], color: Option<Option<u32>>, weight_pt: Option<f64>, dashed: Option<bool>) {
    if let Some(sp) = shape_elem_mut(body) {
        let pr = sp_pr(sp);
        if pr.child("ln").is_none() {
            let pos = pr.children.iter().position(|n| matches!(n, Node::Elem(e) if matches!(e.local(), "effectLst" | "scene3d" | "sp3d" | "extLst"))).unwrap_or(pr.children.len());
            pr.children.insert(pos, Node::Elem(Element::new("a:ln")));
        }
        let ln = pr.child_mut("ln").unwrap();
        if let Some(w) = weight_pt {
            ln.set_attr("w", &((w * EMU_PER_PT).round() as i64).to_string());
        }
        if let Some(c) = color {
            ln.children.retain(|n| !matches!(n, Node::Elem(e) if FILLS.contains(&e.local())));
            let f = parse_frag(&match c {
                Some(c) => format!("<a:solidFill>{}</a:solidFill>", color_xml(c)),
                None => "<a:noFill/>".into(),
            });
            for (k, n) in f.children.into_iter().enumerate() {
                ln.children.insert(k, n);
            }
        }
        if let Some(d) = dashed {
            ln.children.retain(|n| !matches!(n, Node::Elem(e) if e.local() == "prstDash"));
            if d {
                let f = parse_frag("<a:prstDash val=\"dash\"/>");
                let pos = ln.children.iter().position(|n| matches!(n, Node::Elem(e) if matches!(e.local(), "round" | "bevel" | "miter" | "headEnd" | "tailEnd"))).unwrap_or(ln.children.len());
                ln.children.insert(pos, f.children.into_iter().next().unwrap());
            }
        }
    }
}

/// The shape's text, a line per paragraph; the first run's formatting is kept for all of it.
pub fn set_text(body: &mut [Node], text: &str) {
    let sp = match shape_elem_mut(body) {
        Some(sp) => sp,
        None => return,
    };
    if sp.local() == "cxnSp" {
        return;
    }
    if sp.child("txBody").is_none() {
        let pfx = sp.name.split(':').next().filter(|_| sp.name.contains(':')).unwrap_or("xdr").to_string();
        let t = parse_frag("<a:bodyPr rtlCol=\"0\" anchor=\"ctr\"/><a:lstStyle/>");
        let mut tb = Element::new(&format!("{}:txBody", pfx));
        tb.children = t.children;
        sp.push(tb);
    }
    let tb = sp.child_mut("txBody").unwrap();
    let first_p = tb.elems().find(|e| e.local() == "p").cloned();
    let ppr = first_p.as_ref().and_then(|p| p.child("pPr")).cloned();
    let rpr = first_p.as_ref().and_then(|p| p.elems().find(|r| r.local() == "r").and_then(|r| r.child("rPr")).cloned()
        .or_else(|| p.child("endParaRPr").cloned().map(|mut e| { e.name = e.name.replace("endParaRPr", "rPr"); e })));
    tb.children.retain(|n| !matches!(n, Node::Elem(e) if e.local() == "p"));
    for line in text.split('\n') {
        let mut p = Element::new("a:p");
        if let Some(pp) = &ppr {
            p.push(pp.clone());
        }
        if line.is_empty() {
            let mut end = rpr.clone().unwrap_or_else(|| Element::new("a:rPr"));
            end.name = "a:endParaRPr".into();
            end.children.retain(|n| matches!(n, Node::Elem(e) if e.local() != "ln"));
            p.push(end);
        } else {
            let mut r = Element::new("a:r");
            if let Some(rp) = &rpr {
                r.push(rp.clone());
            }
            let mut t = Element::new("a:t");
            t.children.push(Node::Text(line.to_string()));
            r.push(t);
            p.push(r);
        }
        tb.push(p);
    }
}

/// Bold, italic, size, colour or alignment for all of the shape's text.
pub fn set_text_format(body: &mut [Node], bold: Option<bool>, italic: Option<bool>, size: Option<f64>, color: Option<u32>, align: Option<i32>) {
    let sp = match shape_elem_mut(body) {
        Some(sp) => sp,
        None => return,
    };
    let tb = match sp.child_mut("txBody") {
        Some(t) => t,
        None => return,
    };
    for n in tb.children.iter_mut() {
        let p = match n {
            Node::Elem(e) if e.local() == "p" => e,
            _ => continue,
        };
        if let Some(a) = align {
            if p.child("pPr").is_none() {
                p.children.insert(0, Node::Elem(Element::new("a:pPr")));
            }
            p.child_mut("pPr").unwrap().set_attr("algn", match a {
                1 => "ctr",
                2 => "r",
                _ => "l",
            });
        }
        for c in p.children.iter_mut() {
            let r = match c {
                Node::Elem(e) if e.local() == "r" || e.local() == "endParaRPr" => e,
                _ => continue,
            };
            let rp = if r.local() == "endParaRPr" {
                r
            } else {
                if r.child("rPr").is_none() {
                    r.children.insert(0, Node::Elem(Element::new("a:rPr")));
                }
                r.child_mut("rPr").unwrap()
            };
            if let Some(b) = bold {
                rp.set_attr("b", if b { "1" } else { "0" });
            }
            if let Some(i) = italic {
                rp.set_attr("i", if i { "1" } else { "0" });
            }
            if let Some(sz) = size {
                rp.set_attr("sz", &((sz * 100.0).round() as i64).to_string());
            }
            if let Some(col) = color {
                rp.children.retain(|n| !matches!(n, Node::Elem(e) if FILLS.contains(&e.local())));
                let f = parse_frag(&format!("<a:solidFill>{}</a:solidFill>", color_xml(col)));
                let pos = rp.children.iter().position(|n| matches!(n, Node::Elem(e) if !matches!(e.local(), "ln"))).unwrap_or(rp.children.len());
                rp.children.insert(pos, f.children.into_iter().next().unwrap());
            }
        }
    }
}

/// The text's vertical alignment in the shape: 't', 'c' or 'b'.
pub fn set_text_anchor(body: &mut [Node], anchor: char) {
    if let Some(bp) = shape_elem_mut(body).and_then(|sp| sp.child_mut("txBody")).and_then(|t| t.child_mut("bodyPr")) {
        bp.set_attr("anchor", match anchor {
            'c' => "ctr",
            'b' => "b",
            _ => "t",
        });
    }
}

/// A different preset for the shape (Edit Shape › Change Shape).
pub fn set_geometry(body: &mut [Node], prst: &str) {
    if let Some(sp) = shape_elem_mut(body) {
        if let Some(g) = sp.child_mut("spPr").and_then(|p| p.child_mut("prstGeom")) {
            g.set_attr("prst", prst);
            g.children.retain(|n| !matches!(n, Node::Elem(e) if e.local() == "avLst"));
            g.push(Element::new("a:avLst"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> [u32; 12] {
        [0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47, 0x0563C1, 0x954F72]
    }

    #[test]
    fn a_new_shape_looks_like_the_desktop_spreadsheets() {
        let b = new_body(2, "Rectangle 1", "rect", 914400, 457200, None, None);
        let v = ShapeView::parse(&b, &theme()).unwrap();
        assert_eq!(v.geom, "rect");
        assert_eq!(v.fill, Some((0x4472C4, 1.0)));
        assert_eq!(v.line.map(|l| l.0), Some(0x223962));
        assert_eq!(v.anchor, 'c');
    }

    #[test]
    fn edits() {
        let mut b = new_body(2, "Oval 1", "ellipse", 914400, 914400, None, None);
        set_fill(&mut b, Some(0xFF0000));
        set_line(&mut b, Some(None), Some(3.0), None);
        set_text(&mut b, "Hello\nworld");
        set_text_format(&mut b, Some(true), None, Some(14.0), Some(0x00FF00), Some(2));
        let v = ShapeView::parse(&b, &theme()).unwrap();
        assert_eq!(v.fill, Some((0xFF0000, 1.0)));
        assert_eq!(v.line, None);
        assert_eq!(v.text(), "Hello\nworld");
        assert!(v.paras[0].runs[0].bold && v.paras[0].runs[0].size == 14.0 && v.paras[0].runs[0].color == 0x00FF00);
        assert_eq!(v.paras[1].align, 2);
        set_line(&mut b, Some(Some(0x0000FF)), None, Some(true));
        let v = ShapeView::parse(&b, &theme()).unwrap();
        assert_eq!(v.line, Some((0x0000FF, 3.0, true)));
        set_geometry(&mut b, "star5");
        assert_eq!(ShapeView::parse(&b, &theme()).unwrap().geom, "star5");
    }

    #[test]
    fn text_boxes_and_wordart() {
        let b = new_body(3, "TextBox 1", "textBox", 914400, 457200, Some("Note"), None);
        let v = ShapeView::parse(&b, &theme()).unwrap();
        assert!(v.textbox && v.text() == "Note" && v.fill == Some((0xFFFFFF, 1.0)));
        let w = new_body(4, "WordArt", "rect", 914400, 457200, Some("Title"), Some(2));
        let v = ShapeView::parse(&w, &theme()).unwrap();
        assert_eq!(v.fill, None);
        assert_eq!(v.paras[0].runs[0].outline.map(|o| o.0), Some(0xED7D31));
        assert_eq!(v.paras[0].runs[0].size, 36.0);
        assert!(!v.wrap);
    }

    #[test]
    fn every_gallery_shape_has_a_path() {
        for (_, list) in GALLERY {
            for p in list.iter() {
                let mut v = ShapeView::parse(&new_body(1, "x", p, 100, 100, None, None), &theme()).unwrap();
                v.geom = p.to_string();
                let paths = preset(&v, 100.0, 60.0);
                assert!(!paths.is_empty() && paths.iter().all(|sp| !sp.segs.is_empty()), "{}", p);
            }
        }
    }
}

/// Put an equation in the shape's text: one centred paragraph holding the Office Math.
pub fn set_math(body: &mut [Node], m: &crate::math::M, size_pt: f64) {
    set_text(body, "");
    let sp = match shape_elem_mut(body) {
        Some(sp) => sp,
        None => return,
    };
    let tb = match sp.child_mut("txBody") {
        Some(t) => t,
        None => return,
    };
    if let Some(bp) = tb.child_mut("bodyPr") {
        bp.set_attr("wrap", "none");
        bp.children.retain(|n| !matches!(n, Node::Elem(e) if matches!(e.local(), "spAutoFit" | "noAutofit" | "normAutofit")));
        bp.push(Element::new("a:spAutoFit"));
    }
    tb.children.retain(|n| !matches!(n, Node::Elem(e) if e.local() == "p"));
    let sz = (size_pt * 100.0).round() as u32;
    let x = format!("<a:p><a:pPr algn=\"ctr\"/>{}<a:endParaRPr lang=\"en-US\" sz=\"{}\"/></a:p>", crate::math::write_omml(m, Some(size_pt)), sz);
    if let Ok((_, p)) = crate::xml::parse(&x) {
        tb.push(p);
    }
}
