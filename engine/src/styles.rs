//! The style sheet: number formats, fonts, fills, borders and cell formats (xf).
//!
//! The XML tree is kept (so whatever isn't understood is written back unchanged) along
//! with a decoded copy of each entry for drawing. Changes intern new entries instead of
//! editing shared ones, so undo never has to touch this table.

use crate::xml::{Element, Node};
use std::collections::HashMap;

pub const STYLE_ORDER: [&str; 11] = ["numFmts", "fonts", "fills", "borders", "cellStyleXfs", "cellXfs", "cellStyles",
    "dxfs", "tableStyles", "colors", "extLst"];
pub const BORDER_ORDER: [&str; 9] = ["start", "left", "end", "right", "top", "bottom", "diagonal", "vertical", "horizontal"];

pub const BUILTIN_FORMATS: [(u32, &str); 28] = [
    (0, "General"), (1, "0"), (2, "0.00"), (3, "#,##0"), (4, "#,##0.00"), (9, "0%"), (10, "0.00%"),
    (11, "0.00E+00"), (12, "# ?/?"), (13, "# ??/??"), (14, "m/d/yyyy"), (15, "d-mmm-yy"), (16, "d-mmm"),
    (17, "mmm-yy"), (18, "h:mm AM/PM"), (19, "h:mm:ss AM/PM"), (20, "h:mm"), (21, "h:mm:ss"), (22, "m/d/yyyy h:mm"),
    (37, "#,##0 ;(#,##0)"), (38, "#,##0 ;[Red](#,##0)"), (39, "#,##0.00;(#,##0.00)"), (40, "#,##0.00;[Red](#,##0.00)"),
    (45, "mm:ss"), (46, "[h]:mm:ss"), (47, "mmss.0"), (48, "##0.0E+0"), (49, "@"),
];
const BUILTIN_EXTRA: [(u32, &str); 4] = [
    (41, "_(* #,##0_);_(* \\(#,##0\\);_(* \"-\"_);_(@_)"),
    (42, "_(\"$\"* #,##0_);_(\"$\"* \\(#,##0\\);_(\"$\"* \"-\"_);_(@_)"),
    (43, "_(* #,##0.00_);_(* \\(#,##0.00\\);_(* \"-\"??_);_(@_)"),
    (44, "_(\"$\"* #,##0.00_);_(\"$\"* \\(#,##0.00\\);_(\"$\"* \"-\"??_);_(@_)"),
];

pub fn builtin_format(id: u32) -> Option<&'static str> {
    BUILTIN_FORMATS.iter().chain(BUILTIN_EXTRA.iter()).find(|(i, _)| *i == id).map(|(_, c)| *c)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Auto,
    Rgb(u32),
    Theme(u32, f64),
    Indexed(u32),
}

impl Color {
    pub fn from_el(e: &Element) -> Color {
        if let Some(rgb) = e.attr("rgb") {
            let h = if rgb.len() >= 6 { &rgb[rgb.len() - 6..] } else { rgb };
            if let Ok(v) = u32::from_str_radix(h, 16) {
                return Color::Rgb(v);
            }
        }
        let tint = e.attr("tint").and_then(|t| t.parse().ok()).unwrap_or(0.0);
        if let Some(t) = e.attr("theme").and_then(|t| t.parse().ok()) {
            return Color::Theme(t, tint);
        }
        if let Some(i) = e.attr("indexed").and_then(|t| t.parse().ok()) {
            return Color::Indexed(i);
        }
        Color::Auto
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub name: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: u8, // 0 none, 1 single, 2 double, 3 single accounting, 4 double accounting
    pub strike: bool,
    pub color: Color,
    pub vert: u8, // 0 baseline, 1 superscript, 2 subscript
    pub scheme: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    pub pattern: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Border {
    pub left: Option<(String, Color)>,
    pub right: Option<(String, Color)>,
    pub top: Option<(String, Color)>,
    pub bottom: Option<(String, Color)>,
    pub diagonal: Option<(String, Color)>,
    pub diag_up: bool,
    pub diag_down: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Xf {
    pub numfmt: u32,
    pub font: u32,
    pub fill: u32,
    pub border: u32,
    pub halign: String,
    pub valign: String,
    pub wrap: bool,
    pub shrink: bool,
    pub indent: u32,
    pub rotation: u32,
    pub reading: u32,
    pub locked: bool,
    pub hidden: bool,
    pub quote_prefix: bool,
    /// Insert > Checkbox: the cell shows a check box for its TRUE/FALSE value
    pub checkbox: bool,
}

pub const XFPB_URI: &str = "{C7286773-470A-42A8-94C5-96B5CB345126}";
pub const NS_XFPB: &str = "http://schemas.microsoft.com/office/spreadsheetml/2022/featurepropertybag";

pub struct Styles {
    pub prolog: String,
    pub root: Element,
    pub numfmts: HashMap<u32, String>,
    pub fonts: Vec<Font>,
    pub fills: Vec<Fill>,
    pub borders: Vec<Border>,
    pub xfs: Vec<Xf>,
    pub dirty: bool,
    index: HashMap<(u8, String), u32>,
    pub theme: [u32; 12],
    pub theme_fonts: (String, String),
    pub indexed: Vec<u32>,
    /// the theme part's XML (None: the default theme)
    pub theme_xml: Option<String>,
}

/// Colours picked from the theme's columns of the palette are kept as theme references, so they
/// follow the theme: a u32 with this bit set holds a theme slot (bits 24..28) and a tint (low bits).
pub const THEME_BIT: u32 = 0x8000_0000;

pub fn theme_ref(idx: u32, tint: f64) -> u32 {
    THEME_BIT | ((idx & 15) << 24) | (((tint.clamp(-1.0, 1.0) * 10000.0).round() as i32 + 10000) as u32 & 0xFFFF)
}

pub fn split_ref(c: u32) -> Option<(u32, f64)> {
    if c & THEME_BIT != 0 {
        Some(((c >> 24) & 15, ((c & 0xFFFF) as i32 - 10000) as f64 / 10000.0))
    } else {
        None
    }
}

/// A tint as Office writes it (its own rounding of the palette's steps).
pub fn tint_text(t: f64) -> String {
    let known: [(f64, &str); 18] = [(0.8, "0.79998168889431442"), (0.6, "0.59999389629810485"), (0.4, "0.39997558519241921"),
        (-0.25, "-0.249977111117893"), (-0.5, "-0.499984740745262"), (-0.35, "-0.34998626667073579"),
        (-0.15, "-0.14999847407452621"), (-0.05, "-4.9989318521683403E-2"), (0.5, "0.499984740745262"),
        (0.35, "0.34998626667073579"), (0.25, "0.249977111117893"), (0.15, "0.14999847407452621"),
        (0.05, "4.9989318521683403E-2"), (-0.1, "-9.9978637043366805E-2"), (-0.75, "-0.749992370372631"),
        (-0.9, "-0.89999084444715716"), (0.9, "0.89999084444715716"), (0.75, "0.749992370372631")];
    for (v, txt) in known {
        if (t - v).abs() < 0.0002 {
            return txt.to_string();
        }
    }
    format!("{}", (t * 10000.0).round() / 10000.0)
}

/// The attributes of a spreadsheet colour element (rgb, or theme and tint).
pub fn color_attrs(c: u32) -> Vec<(&'static str, String)> {
    match split_ref(c) {
        Some((i, t)) => {
            let mut v = vec![("theme", i.to_string())];
            if t != 0.0 {
                v.push(("tint", tint_text(t)));
            }
            v
        }
        None => vec![("rgb", format!("FF{:06X}", c & 0xFFFFFF))],
    }
}

pub fn color_el(name: &str, c: u32) -> Element {
    let mut e = Element::new(name);
    for (k, v) in color_attrs(c) {
        e.set_attr(k, &v);
    }
    e
}

fn flag(e: &Element, name: &str) -> bool {
    match e.child(name) {
        Some(c) => !matches!(c.attr("val"), Some("0") | Some("false")),
        None => false,
    }
}

impl Font {
    pub fn from_el(e: &Element) -> Font {
        let u = e.child("u").map(|c| match c.attr("val").unwrap_or("single") {
            "double" => 2,
            "singleAccounting" => 3,
            "doubleAccounting" => 4,
            "none" => 0,
            _ => 1,
        }).unwrap_or(0);
        Font {
            name: e.child("name").and_then(|c| c.attr("val")).unwrap_or("Calibri").to_string(),
            size: e.child("sz").and_then(|c| c.attr("val")).and_then(|v| v.parse().ok()).unwrap_or(11.0),
            bold: flag(e, "b"),
            italic: flag(e, "i"),
            underline: u,
            strike: flag(e, "strike"),
            color: e.child("color").map(Color::from_el).unwrap_or(Color::Auto),
            vert: match e.child("vertAlign").and_then(|c| c.attr("val")) {
                Some("superscript") => 1,
                Some("subscript") => 2,
                _ => 0,
            },
            scheme: e.child("scheme").and_then(|c| c.attr("val")).map(|s| s.to_string()),
        }
    }
}

impl Fill {
    pub fn from_el(e: &Element) -> Fill {
        if let Some(p) = e.child("patternFill") {
            let fg = p.child("fgColor").map(Color::from_el);
            let bg = p.child("bgColor").map(Color::from_el);
            let pattern = p.attr("patternType").map(|s| s.to_string()).unwrap_or_else(|| if fg.is_some() { "solid".into() } else { "none".into() });
            return Fill { pattern, fg, bg };
        }
        if let Some(g) = e.child("gradientFill") {
            // drawn as its first stop colour
            let c = g.elems_named("stop").next().and_then(|s| s.child("color")).map(Color::from_el);
            return Fill { pattern: "solid".into(), fg: c, bg: None };
        }
        Fill { pattern: "none".into(), fg: None, bg: None }
    }
}

impl Border {
    pub fn from_el(e: &Element) -> Border {
        let side = |names: &[&str]| -> Option<(String, Color)> {
            for n in names {
                if let Some(s) = e.child(n) {
                    if let Some(st) = s.attr("style") {
                        if st != "none" {
                            return Some((st.to_string(), s.child("color").map(Color::from_el).unwrap_or(Color::Auto)));
                        }
                    }
                }
            }
            None
        };
        Border {
            left: side(&["left", "start"]),
            right: side(&["right", "end"]),
            top: side(&["top"]),
            bottom: side(&["bottom"]),
            diagonal: side(&["diagonal"]),
            diag_up: matches!(e.attr("diagonalUp"), Some("1") | Some("true")),
            diag_down: matches!(e.attr("diagonalDown"), Some("1") | Some("true")),
        }
    }
}

impl Xf {
    pub fn from_el(e: &Element) -> Xf {
        let n = |k: &str| e.attr(k).and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut x = Xf { numfmt: n("numFmtId"), font: n("fontId"), fill: n("fillId"), border: n("borderId"), locked: true, ..Default::default() };
        x.quote_prefix = matches!(e.attr("quotePrefix"), Some("1") | Some("true"));
        x.checkbox = e.child("extLst").map(|l| l.elems().any(|x| x.attr("uri").map(|u| u.eq_ignore_ascii_case(XFPB_URI)).unwrap_or(false))).unwrap_or(false);
        if let Some(a) = e.child("alignment") {
            x.halign = a.attr("horizontal").unwrap_or("").to_string();
            x.valign = a.attr("vertical").unwrap_or("").to_string();
            x.wrap = matches!(a.attr("wrapText"), Some("1") | Some("true"));
            x.shrink = matches!(a.attr("shrinkToFit"), Some("1") | Some("true"));
            x.indent = a.attr("indent").and_then(|v| v.parse().ok()).unwrap_or(0);
            x.rotation = a.attr("textRotation").and_then(|v| v.parse().ok()).unwrap_or(0);
            x.reading = a.attr("readingOrder").and_then(|v| v.parse().ok()).unwrap_or(0);
        }
        if let Some(p) = e.child("protection") {
            x.locked = !matches!(p.attr("locked"), Some("0") | Some("false"));
            x.hidden = matches!(p.attr("hidden"), Some("1") | Some("true"));
        }
        x
    }
}

pub const DEFAULT_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><color theme="1"/><name val="Calibri"/><family val="2"/><scheme val="minor"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles><dxfs count="0"/><tableStyles count="0" defaultTableStyle="TableStyleMedium2" defaultPivotStyle="PivotStyleLight16"/></styleSheet>"#;

pub const DEFAULT_THEME: [u32; 12] = [0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000,
    0x5B9BD5, 0x70AD47, 0x0563C1, 0x954F72];

fn key(e: &Element) -> String {
    e.to_string()
}

impl Styles {
    pub fn parse(text: Option<&str>) -> Result<Styles, String> {
        let (prolog, mut root) = crate::xml::parse(text.unwrap_or(DEFAULT_STYLES))?;
        for name in ["fonts", "fills", "borders", "cellXfs"] {
            if root.child(name).is_none() {
                root.insert_ordered(Element::new(name), &STYLE_ORDER);
            }
        }
        let mut st = Styles {
            prolog,
            root,
            numfmts: HashMap::new(),
            fonts: vec![],
            fills: vec![],
            borders: vec![],
            xfs: vec![],
            dirty: false,
            index: HashMap::new(),
            theme: DEFAULT_THEME,
            theme_fonts: ("Calibri Light".into(), "Calibri".into()),
            indexed: crate::numfmt::PALETTE.to_vec(),
            theme_xml: None,
        };
        st.ensure_defaults();
        st.reload();
        Ok(st)
    }

    fn ensure_defaults(&mut self) {
        let r = &mut self.root;
        if r.child("fonts").map(|f| f.elems().count()).unwrap_or(0) == 0 {
            let f = r.child_mut("fonts").unwrap();
            let mut font = Element::new(&name_like(f, "font"));
            font.push(Element::with_attrs(&name_like(f, "sz"), &[("val", "11")]));
            font.push(Element::with_attrs(&name_like(f, "name"), &[("val", "Calibri")]));
            f.push(font);
        }
        if r.child("fills").map(|f| f.elems().count()).unwrap_or(0) == 0 {
            let f = r.child_mut("fills").unwrap();
            for p in ["none", "gray125"] {
                let mut fill = Element::new(&name_like(f, "fill"));
                fill.push(Element::with_attrs(&name_like(f, "patternFill"), &[("patternType", p)]));
                f.push(fill);
            }
        }
        if r.child("borders").map(|f| f.elems().count()).unwrap_or(0) == 0 {
            let f = r.child_mut("borders").unwrap();
            let mut b = Element::new(&name_like(f, "border"));
            for s in ["left", "right", "top", "bottom", "diagonal"] {
                b.push(Element::new(&name_like(f, s)));
            }
            f.push(b);
        }
        if r.child("cellXfs").map(|f| f.elems().count()).unwrap_or(0) == 0 {
            let f = r.child_mut("cellXfs").unwrap();
            f.push(Element::with_attrs(&name_like(f, "xf"), &[("numFmtId", "0"), ("fontId", "0"), ("fillId", "0"), ("borderId", "0"), ("xfId", "0")]));
        }
    }

    /// Decode every entry again from the tree.
    pub fn reload(&mut self) {
        self.numfmts.clear();
        if let Some(n) = self.root.child("numFmts") {
            for f in n.elems_named("numFmt") {
                if let (Some(id), Some(code)) = (f.attr("numFmtId").and_then(|v| v.parse().ok()), f.attr("formatCode")) {
                    self.numfmts.insert(id, code.to_string());
                }
            }
        }
        self.fonts = self.root.child("fonts").map(|c| c.elems().map(Font::from_el).collect()).unwrap_or_default();
        self.fills = self.root.child("fills").map(|c| c.elems().map(Fill::from_el).collect()).unwrap_or_default();
        self.borders = self.root.child("borders").map(|c| c.elems().map(Border::from_el).collect()).unwrap_or_default();
        self.xfs = self.root.child("cellXfs").map(|c| c.elems().map(Xf::from_el).collect()).unwrap_or_default();
        self.index.clear();
        for (kind, coll) in [(0u8, "fonts"), (1, "fills"), (2, "borders"), (3, "cellXfs")] {
            if let Some(c) = self.root.child(coll) {
                for (i, e) in c.elems().enumerate() {
                    self.index.entry((kind, key(e))).or_insert(i as u32);
                }
            }
        }
        if let Some(c) = self.root.child("colors").and_then(|c| c.child("indexedColors")) {
            let v: Vec<u32> = c.elems().filter_map(|e| e.attr("rgb")).filter_map(|h| u32::from_str_radix(&h[h.len().saturating_sub(6)..], 16).ok()).collect();
            if !v.is_empty() {
                self.indexed = v;
            }
        }
    }

    /// A colour as it shows: theme references resolved.
    pub fn resolve(&self, c: u32) -> u32 {
        match split_ref(c) {
            Some((i, t)) => self.rgb(&Color::Theme(i, t), 0),
            None => c & 0xFFFFFF,
        }
    }

    /// Use a theme (None: the default one).
    pub fn use_theme(&mut self, theme_xml: Option<&str>) {
        self.theme = DEFAULT_THEME;
        self.theme_fonts = ("Calibri Light".into(), "Calibri".into());
        self.theme_xml = None;
        if let Some(x) = theme_xml {
            self.set_theme(x);
        }
    }

    pub fn set_theme(&mut self, theme_xml: &str) {
        self.theme_xml = Some(theme_xml.to_string());
        if let Ok((_, root)) = crate::xml::parse(theme_xml) {
            // a:theme/a:themeElements/a:clrScheme
            let scheme = root.child("themeElements").and_then(|t| t.child("clrScheme"));
            if let Some(s) = scheme {
                let order = ["lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"];
                // theme index order used by cell colours: 0 lt1, 1 dk1, 2 lt2, 3 dk2, 4.. accents
                for (i, n) in order.iter().enumerate() {
                    if let Some(c) = s.child(n) {
                        let v = c.child("srgbClr").and_then(|x| x.attr("val")).or_else(|| c.child("sysClr").and_then(|x| x.attr("lastClr")));
                        if let Some(v) = v.and_then(|h| u32::from_str_radix(h, 16).ok()) {
                            self.theme[i] = v;
                        }
                    }
                }
            }
            if let Some(fs) = root.child("themeElements").and_then(|t| t.child("fontScheme")) {
                let major = fs.child("majorFont").and_then(|m| m.child("latin")).and_then(|l| l.attr("typeface"));
                let minor = fs.child("minorFont").and_then(|m| m.child("latin")).and_then(|l| l.attr("typeface"));
                if let (Some(a), Some(b)) = (major, minor) {
                    self.theme_fonts = (a.to_string(), b.to_string());
                }
            }
        }
    }

    pub fn numfmt_code(&self, id: u32) -> String {
        if let Some(c) = self.numfmts.get(&id) {
            return c.clone();
        }
        builtin_format(id).unwrap_or("General").to_string()
    }

    pub fn xf(&self, i: u32) -> &Xf {
        self.xfs.get(i as usize).unwrap_or(&self.xfs[0])
    }
    pub fn font(&self, i: u32) -> &Font {
        self.fonts.get(i as usize).unwrap_or(&self.fonts[0])
    }
    pub fn fill(&self, i: u32) -> &Fill {
        self.fills.get(i as usize).unwrap_or(&self.fills[0])
    }
    pub fn border(&self, i: u32) -> &Border {
        self.borders.get(i as usize).unwrap_or(&self.borders[0])
    }
    pub fn xf_numfmt(&self, xf: u32) -> String {
        self.numfmt_code(self.xf(xf).numfmt)
    }

    /// RGB for a colour (auto = the given default).
    pub fn rgb(&self, c: &Color, auto: u32) -> u32 {
        match c {
            Color::Auto => auto,
            Color::Rgb(v) => *v,
            Color::Indexed(i) => {
                if *i == 64 || *i == 65 {
                    auto
                } else {
                    *self.indexed.get(*i as usize).unwrap_or(&auto)
                }
            }
            Color::Theme(t, tint) => {
                // cell colours count theme slots as dk1/lt1 swapped against the scheme order
                let idx = match t {
                    0 => 0,
                    1 => 1,
                    2 => 2,
                    3 => 3,
                    n => *n as usize,
                };
                let base = *self.theme.get(idx).unwrap_or(&auto);
                apply_tint(base, *tint)
            }
        }
    }

    /// The display font name (theme slot names resolved).
    pub fn font_name(&self, f: &Font) -> String {
        match f.scheme.as_deref() {
            Some("major") => self.theme_fonts.0.clone(),
            // a theme font follows the theme (the name in the file is a cached copy)
            Some("minor") => self.theme_fonts.1.clone(),
            _ => f.name.clone(),
        }
    }

    fn coll_mut(&mut self, name: &str) -> &mut Element {
        if self.root.child(name).is_none() {
            self.root.insert_ordered(Element::new(name), &STYLE_ORDER);
        }
        self.root.child_mut(name).unwrap()
    }

    fn intern(&mut self, kind: u8, coll: &str, e: Element) -> u32 {
        let k = key(&e);
        if let Some(i) = self.index.get(&(kind, k.clone())) {
            return *i;
        }
        let c = self.coll_mut(coll);
        c.push(e);
        let n = c.elems().count() as u32;
        c.set_attr("count", &n.to_string());
        let idx = n - 1;
        self.index.insert((kind, k), idx);
        self.dirty = true;
        idx
    }

    fn get_el(&self, coll: &str, i: u32) -> Element {
        self.root.child(coll).and_then(|c| c.elems().nth(i as usize).or_else(|| c.elems().next())).cloned().unwrap_or_else(|| Element::new("x"))
    }

    /// The id of a number format already in the book (or built in).
    pub fn numfmt_id_ro(&self, code: &str) -> Option<u32> {
        if code.eq_ignore_ascii_case("General") {
            return Some(0);
        }
        for (id, c) in BUILTIN_FORMATS.iter().chain(BUILTIN_EXTRA.iter()) {
            if *c == code {
                return Some(*id);
            }
        }
        self.numfmts.iter().find(|(_, c)| *c == code).map(|(id, _)| *id)
    }

    pub fn numfmt_id(&mut self, code: &str) -> u32 {
        if code.eq_ignore_ascii_case("General") {
            return 0;
        }
        for (id, c) in BUILTIN_FORMATS.iter().chain(BUILTIN_EXTRA.iter()) {
            if *c == code {
                return *id;
            }
        }
        for (id, c) in &self.numfmts {
            if c == code {
                return *id;
            }
        }
        let id = self.numfmts.keys().copied().max().unwrap_or(163).max(163) + 1;
        let nf = self.coll_mut("numFmts");
        let tag = name_like(nf, "numFmt");
        nf.push(Element::with_attrs(&tag, &[("numFmtId", &id.to_string()), ("formatCode", code)]));
        let n = nf.elems().count();
        nf.set_attr("count", &n.to_string());
        self.numfmts.insert(id, code.to_string());
        self.dirty = true;
        id
    }

    /// cellXfs[xi] with changes applied (see `Change`); returns the index to use.
    pub fn derive(&mut self, xi: u32, ch: &Change) -> u32 {
        let base = if ch.reset { 0 } else { xi };
        let mut xf = self.get_el("cellXfs", base);
        if let Some(fc) = &ch.font {
            let mut font = self.get_el("fonts", xf.attr("fontId").and_then(|v| v.parse().ok()).unwrap_or(0));
            // "@major"/"@minor": the theme's headings or body font (it follows the theme)
            let slot = fc.name.as_deref().and_then(|n| n.strip_prefix('@')).map(|n| n.to_string());
            match slot.as_deref() {
                Some(which @ ("major" | "minor")) => {
                    let mut f2 = fc.clone();
                    f2.name = Some(if which == "major" { self.theme_fonts.0.clone() } else { self.theme_fonts.1.clone() });
                    edit_font(&mut font, &f2);
                    font.remove_children("scheme");
                    font.push(Element::with_attrs(&name_like_tag(&font.name, "scheme"), &[("val", which)]));
                }
                _ => edit_font(&mut font, fc),
            }
            let id = self.intern(0, "fonts", font);
            xf.set_attr("fontId", &id.to_string());
            xf.set_attr("applyFont", "1");
        }
        if let Some(fill) = &ch.fill {
            let tag = name_like(self.root.child("fills").unwrap(), "fill");
            let pf_tag = name_like(self.root.child("fills").unwrap(), "patternFill");
            let mut fe = Element::new(&tag);
            match fill {
                None => fe.push(Element::with_attrs(&pf_tag, &[("patternType", "none")])),
                Some((pattern, fg, bg)) => {
                    let mut pf = Element::with_attrs(&pf_tag, &[("patternType", pattern)]);
                    if let Some(c) = fg {
                        pf.push(color_el(&name_like_tag(&pf_tag, "fgColor"), *c));
                    }
                    match bg {
                        Some(c) => pf.push(color_el(&name_like_tag(&pf_tag, "bgColor"), *c)),
                        None => pf.push(Element::with_attrs(&name_like_tag(&pf_tag, "bgColor"), &[("indexed", "64")])),
                    }
                    fe.push(pf);
                }
            }
            let id = self.intern(1, "fills", fe);
            xf.set_attr("fillId", &id.to_string());
            xf.set_attr("applyFill", "1");
        }
        if !ch.border.is_empty() || ch.diag.is_some() {
            let mut b = self.get_el("borders", xf.attr("borderId").and_then(|v| v.parse().ok()).unwrap_or(0));
            edit_border(&mut b, &ch.border, ch.diag);
            let id = self.intern(2, "borders", b);
            xf.set_attr("borderId", &id.to_string());
            xf.set_attr("applyBorder", "1");
        }
        if let Some(a) = &ch.align {
            edit_align(&mut xf, a);
            xf.set_attr("applyAlignment", "1");
        }
        if ch.locked.is_some() || ch.hidden.is_some() {
            edit_protection(&mut xf, ch.locked, ch.hidden);
            xf.set_attr("applyProtection", "1");
        }
        if let Some(code) = &ch.numfmt {
            let id = self.numfmt_id(code);
            xf.set_attr("numFmtId", &id.to_string());
            xf.set_attr("applyNumberFormat", "1");
        }
        if let Some(q) = ch.quote_prefix {
            if q {
                xf.set_attr("quotePrefix", "1");
            } else {
                xf.remove_attr("quotePrefix");
            }
        }
        if let Some(cb) = ch.checkbox {
            let tag = name_like_tag(&xf.name, "extLst");
            if let Some(l) = xf.child_mut("extLst") {
                l.children.retain(|n| !matches!(n, Node::Elem(x) if x.attr("uri").map(|u| u.eq_ignore_ascii_case(XFPB_URI)).unwrap_or(false)));
            }
            if xf.child("extLst").map(|l| l.elems().next().is_none()).unwrap_or(false) {
                xf.remove_children("extLst");
            }
            if cb {
                if xf.child("extLst").is_none() {
                    xf.push(Element::new(&tag));
                }
                let mut ext = Element::with_attrs(&name_like_tag(&xf.name, "ext"), &[("uri", XFPB_URI), ("xmlns:xfpb", NS_XFPB)]);
                ext.push(Element::with_attrs("xfpb:xfComplement", &[("i", "0")]));
                xf.child_mut("extLst").unwrap().push(ext);
            }
        }
        let id = self.intern(3, "cellXfs", xf);
        if id as usize >= self.xfs.len() {
            self.reload();
        }
        id
    }

    pub fn to_xml(&self) -> String {
        crate::xml::document(&self.prolog, &self.root)
    }
}

pub fn apply_tint(rgb: u32, tint: f64) -> u32 {
    if tint == 0.0 {
        return rgb;
    }
    let (r, g, b) = (((rgb >> 16) & 255) as f64 / 255.0, ((rgb >> 8) & 255) as f64 / 255.0, (rgb & 255) as f64 / 255.0);
    let (h, l, s) = rgb_to_hls(r, g, b);
    let l = if tint < 0.0 { l * (1.0 + tint) } else { l * (1.0 - tint) + tint };
    let (r, g, b) = hls_to_rgb(h, l.clamp(0.0, 1.0), s);
    (((r * 255.0).round() as u32) << 16) | (((g * 255.0).round() as u32) << 8) | (b * 255.0).round() as u32
}

fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, l, 0.0);
    }
    let d = mx - mn;
    let s = if l <= 0.5 { d / (mx + mn) } else { d / (2.0 - mx - mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, l, s)
}

fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let m2 = if l <= 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let m1 = 2.0 * l - m2;
    let f = |hh: f64| -> f64 {
        let hh = hh.rem_euclid(1.0);
        if hh < 1.0 / 6.0 {
            m1 + (m2 - m1) * hh * 6.0
        } else if hh < 0.5 {
            m2
        } else if hh < 2.0 / 3.0 {
            m1 + (m2 - m1) * (2.0 / 3.0 - hh) * 6.0
        } else {
            m1
        }
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

/// A tag name with the same prefix as the parent collection uses ("x:font" in prefixed files).
fn name_like(parent: &Element, local: &str) -> String {
    name_like_tag(&parent.name, local)
}

fn name_like_tag(tag: &str, local: &str) -> String {
    match tag.rfind(':') {
        Some(i) => format!("{}:{}", &tag[..i], local),
        None => local.to_string(),
    }
}

/// A style change, as the formatting commands and Format Cells make them.
#[derive(Clone, Debug, Default)]
pub struct Change {
    pub reset: bool,
    pub font: Option<FontChange>,
    /// Some(None) = no fill; Some(Some(pattern, fg, bg))
    pub fill: Option<Option<(String, Option<u32>, Option<u32>)>>,
    /// side name -> Some((style, rgb or None for automatic)) or None to remove
    pub border: Vec<(String, Option<(String, Option<u32>)>)>,
    /// (diagonal up, diagonal down)
    pub diag: Option<(bool, bool)>,
    pub align: Option<AlignChange>,
    pub locked: Option<bool>,
    pub hidden: Option<bool>,
    pub numfmt: Option<String>,
    pub quote_prefix: Option<bool>,
    pub checkbox: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct FontChange {
    pub name: Option<String>,
    pub size: Option<f64>,
    pub grow: i32,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<Option<String>>,
    pub strike: Option<bool>,
    /// Some(None) = automatic
    pub color: Option<Option<u32>>,
    pub vert: Option<Option<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct AlignChange {
    pub horizontal: Option<String>,
    pub vertical: Option<String>,
    pub wrap: Option<bool>,
    pub shrink: Option<bool>,
    pub indent: Option<u32>,
    pub indent_step: i32,
    pub rotation: Option<u32>,
    pub reading: Option<u32>,
}

const FONT_STEPS: [f64; 16] = [8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0];

pub fn step_size(size: f64, d: i32) -> f64 {
    if d > 0 {
        FONT_STEPS.iter().copied().find(|s| *s > size).unwrap_or((size + 10.0).min(409.0))
    } else {
        FONT_STEPS.iter().rev().copied().find(|s| *s < size).unwrap_or((size - 1.0).max(1.0))
    }
}

fn set_flag(f: &mut Element, tag: &str, on: bool) {
    let full = name_like_tag(&f.name, tag);
    let pos = f.children.iter().position(|n| matches!(n, Node::Elem(e) if e.local() == tag));
    match (on, pos) {
        (true, Some(p)) => {
            if let Node::Elem(e) = &mut f.children[p] {
                e.remove_attr("val");
            }
        }
        (true, None) => f.children.insert(0, Node::Elem(Element::new(&full))),
        (false, Some(p)) => {
            f.children.remove(p);
        }
        (false, None) => {}
    }
}

fn edit_font(f: &mut Element, c: &FontChange) {
    if let Some(b) = c.bold {
        set_flag(f, "b", b);
    }
    if let Some(i) = c.italic {
        set_flag(f, "i", i);
    }
    if let Some(s) = c.strike {
        set_flag(f, "strike", s);
    }
    if let Some(u) = &c.underline {
        f.remove_children("u");
        if let Some(u) = u {
            let mut e = Element::new(&name_like_tag(&f.name, "u"));
            if u != "single" {
                e.set_attr("val", u);
            }
            f.children.insert(0, Node::Elem(e));
        }
    }
    if let Some(v) = &c.vert {
        f.remove_children("vertAlign");
        if let Some(v) = v {
            f.children.insert(0, Node::Elem(Element::with_attrs(&name_like_tag(&f.name, "vertAlign"), &[("val", v)])));
        }
    }
    if c.size.is_some() || c.grow != 0 {
        let cur: f64 = f.child("sz").and_then(|s| s.attr("val")).and_then(|v| v.parse().ok()).unwrap_or(11.0);
        let mut size = c.size.unwrap_or(cur);
        if c.grow != 0 {
            size = step_size(size, c.grow);
        }
        let txt = crate::numfmt::num_to_text(size);
        match f.child_mut("sz") {
            Some(s) => s.set_attr("val", &txt),
            None => f.push(Element::with_attrs(&name_like_tag(&f.name, "sz"), &[("val", &txt)])),
        }
    }
    if let Some(name) = &c.name {
        match f.child_mut("name") {
            Some(n) => n.set_attr("val", name),
            None => f.push(Element::with_attrs(&name_like_tag(&f.name, "name"), &[("val", name)])),
        }
        // an explicit font replaces the theme font slot
        f.remove_children("scheme");
    }
    if let Some(col) = &c.color {
        f.remove_children("color");
        if let Some(rgb) = col {
            f.push(color_el(&name_like_tag(&f.name, "color"), *rgb));
        }
    }
}

fn edit_border(b: &mut Element, sides: &[(String, Option<(String, Option<u32>)>)], diag: Option<(bool, bool)>) {
    let mut map: Vec<(String, Element)> = Vec::new();
    for n in b.children.drain(..) {
        if let Node::Elem(e) = n {
            let local = match e.local() {
                "start" => "left".to_string(),
                "end" => "right".to_string(),
                x => x.to_string(),
            };
            map.push((local, e));
        }
    }
    for (side, v) in sides {
        let mut e = Element::new(&name_like_tag(&b.name, side));
        if let Some((style, color)) = v {
            if style != "none" {
                e.set_attr("style", style);
                let mut ce = Element::new(&name_like_tag(&b.name, "color"));
                match color {
                    Some(rgb) => {
                        for (k, v) in color_attrs(*rgb) {
                            ce.set_attr(k, &v);
                        }
                    }
                    None => ce.set_attr("indexed", "64"),
                }
                e.push(ce);
            }
        }
        if let Some(slot) = map.iter_mut().find(|(k, _)| k == side) {
            slot.1 = e;
        } else {
            map.push((side.clone(), e));
        }
    }
    for s in ["left", "right", "top", "bottom", "diagonal"] {
        if !map.iter().any(|(k, _)| k == s) {
            map.push((s.to_string(), Element::new(&name_like_tag(&b.name, s))));
        }
    }
    for s in BORDER_ORDER {
        if let Some(i) = map.iter().position(|(k, _)| k == s) {
            let (_, mut e) = map.remove(i);
            // keep the element's name in sync with the side (start/end become left/right)
            e.name = name_like_tag(&b.name, s);
            b.children.push(Node::Elem(e));
        }
    }
    if let Some((up, down)) = diag {
        if up {
            b.set_attr("diagonalUp", "1");
        } else {
            b.remove_attr("diagonalUp");
        }
        if down {
            b.set_attr("diagonalDown", "1");
        } else {
            b.remove_attr("diagonalDown");
        }
    }
}

fn edit_align(xf: &mut Element, a: &AlignChange) {
    let tag = name_like_tag(&xf.name, "alignment");
    if xf.child("alignment").is_none() {
        xf.children.insert(0, Node::Elem(Element::new(&tag)));
    }
    let al = xf.child_mut("alignment").unwrap();
    let put = |al: &mut Element, k: &str, v: Option<String>, default: &str| match v {
        Some(v) if v != default => al.set_attr(k, &v),
        _ => al.remove_attr(k),
    };
    if let Some(h) = &a.horizontal {
        put(al, "horizontal", Some(h.clone()), "general");
    }
    if let Some(v) = &a.vertical {
        put(al, "vertical", Some(v.clone()), "bottom");
    }
    if let Some(w) = a.wrap {
        put(al, "wrapText", if w { Some("1".into()) } else { None }, "");
    }
    if let Some(s) = a.shrink {
        put(al, "shrinkToFit", if s { Some("1".into()) } else { None }, "");
    }
    if let Some(r) = a.rotation {
        put(al, "textRotation", Some(r.to_string()), "0");
    }
    if let Some(r) = a.reading {
        put(al, "readingOrder", Some(r.to_string()), "0");
    }
    let mut indent: i64 = al.attr("indent").and_then(|v| v.parse().ok()).unwrap_or(0);
    if let Some(i) = a.indent {
        indent = i as i64;
    }
    if a.indent_step != 0 {
        indent = (indent + a.indent_step as i64).clamp(0, 250);
        let h = al.attr("horizontal").unwrap_or("").to_string();
        if indent > 0 && !matches!(h.as_str(), "left" | "right" | "distributed") {
            al.set_attr("horizontal", "left");
        }
    }
    put(al, "indent", Some(indent.to_string()), "0");
    if al.attrs.is_empty() {
        xf.remove_children("alignment");
    }
}

fn edit_protection(xf: &mut Element, locked: Option<bool>, hidden: Option<bool>) {
    let tag = name_like_tag(&xf.name, "protection");
    if xf.child("protection").is_none() {
        let pos = if xf.child("alignment").is_some() { 1 } else { 0 };
        xf.children.insert(pos.min(xf.children.len()), Node::Elem(Element::new(&tag)));
    }
    let p = xf.child_mut("protection").unwrap();
    if let Some(l) = locked {
        if l {
            p.remove_attr("locked");
        } else {
            p.set_attr("locked", "0");
        }
    }
    if let Some(h) = hidden {
        if h {
            p.set_attr("hidden", "1");
        } else {
            p.remove_attr("hidden");
        }
    }
    if p.attrs.is_empty() {
        xf.remove_children("protection");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derive_bold() {
        let mut s = Styles::parse(None).unwrap();
        let ch = Change { font: Some(FontChange { bold: Some(true), ..Default::default() }), ..Default::default() };
        let a = s.derive(0, &ch);
        assert_eq!(a, 1);
        assert!(s.font(s.xf(a).font).bold);
        assert_eq!(s.derive(0, &ch), 1);
        let back = s.derive(a, &Change { font: Some(FontChange { bold: Some(false), ..Default::default() }), ..Default::default() });
        assert!(!s.font(s.xf(back).font).bold);
        let n = s.numfmt_id("0.000\" kg\"");
        assert_eq!(n, 164);
        assert_eq!(s.numfmt_id("0.00"), 2);
        assert_eq!(apply_tint(0x4472C4, 0.0), 0x4472C4);
    }
}
