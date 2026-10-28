//! Charts: the model, reading and writing the DrawingML chart part, and the data a
//! chart shows (read from the cells its series refer to, or from the file's cache).

use crate::book::Book;
use crate::cellref::{parse_range, quote_sheet, Rect};
use crate::formula::{self, Adjust};
use crate::value::Value;
use crate::xml::{self, Element, Node};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bar,
    Line,
    Pie,
    Doughnut,
    Area,
    Scatter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Clustered,
    Stacked,
    Percent,
}

/// Where a piece of chart data comes from: a reference (with the file's cached values
/// as a fallback) or literal values.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Src {
    pub f: Option<String>,
    pub cache: Vec<CVal>,
    pub fmt: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CVal {
    Empty,
    Num(f64),
    Str(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Series {
    pub name: Option<Src>,
    pub cat: Option<Src>,
    pub val: Option<Src>,
    pub color: Option<u32>,
    pub labels: bool,
    pub smooth: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    pub kind: Kind,
    pub horizontal: bool,
    pub group: Group,
    pub markers: bool,
    /// scatter: lines between the points
    pub lines: bool,
    pub secondary: bool,
    pub vary: bool,
    pub gap: u32,
    pub overlap: i32,
    pub hole: u32,
    pub first_angle: u32,
    pub series: Vec<Series>,
    /// the element the file used when it isn't the plain one (bar3DChart...)
    pub file_tag: Option<String>,
}

impl Plot {
    pub fn new(kind: Kind) -> Plot {
        Plot {
            kind,
            horizontal: false,
            group: Group::Clustered,
            markers: false,
            lines: false,
            secondary: false,
            vary: matches!(kind, Kind::Pie | Kind::Doughnut),
            gap: 219,
            overlap: -27,
            hole: 75,
            first_angle: 0,
            series: Vec::new(),
            file_tag: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Title {
    None,
    /// the series name with one series, "Chart Title" otherwise
    Auto,
    Text(String),
    Ref(Src),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub title: Title,
    pub plots: Vec<Plot>,
    /// "b", "r", "t", "l", "tr"; None: no legend
    pub legend: Option<String>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub gridlines: bool,
    /// colour set: 0 colourful (theme accents), 1..=4 monochrome on accent 1..4
    pub palette: u8,
    /// the part as read; written back with only the references updated while the
    /// chart isn't changed in ways only the model describes
    pub orig: Option<(String, Element)>,
    /// a PivotChart: the PivotTable it plots (by name)
    pub pivot: Option<String>,
}

impl Default for Chart {
    fn default() -> Self {
        Chart { title: Title::Auto, plots: Vec::new(), legend: Some("b".into()), x_title: None, y_title: None, gridlines: true, palette: 0, orig: None, pivot: None }
    }
}

pub const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

// ---------------------------------------------------------------------- colours

/// A theme colour from a DrawingML colour element (srgbClr, schemeClr with lumMod/lumOff).
pub fn color_of(e: &Element, theme: &[u32; 12]) -> Option<u32> {
    let base = match e.local() {
        "srgbClr" => u32::from_str_radix(e.attr("val")?, 16).ok()?,
        "schemeClr" => {
            let i = match e.attr("val")? {
                "accent1" => 4,
                "accent2" => 5,
                "accent3" => 6,
                "accent4" => 7,
                "accent5" => 8,
                "accent6" => 9,
                "tx1" | "dk1" => 1,
                "bg1" | "lt1" => 0,
                "tx2" | "dk2" => 3,
                "bg2" | "lt2" => 2,
                "hlink" => 10,
                "folHlink" => 11,
                _ => return None,
            };
            theme[i]
        }
        "sysClr" => u32::from_str_radix(e.attr("lastClr").unwrap_or("000000"), 16).ok()?,
        "prstClr" => match e.attr("val")? {
            "black" => 0,
            "white" => 0xFFFFFF,
            "red" => 0xFF0000,
            "green" => 0x008000,
            "blue" => 0x0000FF,
            _ => return None,
        },
        _ => return None,
    };
    let mut c = base;
    let mut lm = 1.0;
    let mut lo = 0.0;
    for m in e.elems() {
        let v = m.attr("val").and_then(|v| v.parse::<f64>().ok()).unwrap_or(100000.0) / 100000.0;
        match m.local() {
            "lumMod" => lm = v,
            "lumOff" => lo = v,
            "shade" => c = scale(c, v),
            "tint" => c = tint(c, v),
            _ => {}
        }
    }
    if lm != 1.0 || lo != 0.0 {
        c = lum(c, lm, lo);
    }
    Some(c)
}

fn scale(c: u32, f: f64) -> u32 {
    let ch = |s: u32| (((c >> s) & 255) as f64 * f).round().clamp(0.0, 255.0) as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn tint(c: u32, f: f64) -> u32 {
    let ch = |s: u32| { let x = ((c >> s) & 255) as f64; (x + (255.0 - x) * (1.0 - f)).round().clamp(0.0, 255.0) as u32 };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Luminance modulation as DrawingML does it (in HSL).
pub fn lum(c: u32, m: f64, o: f64) -> u32 {
    let r = ((c >> 16) & 255) as f64 / 255.0;
    let g = ((c >> 8) & 255) as f64 / 255.0;
    let b = (c & 255) as f64 / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let mut l = (mx + mn) / 2.0;
    let (h, s) = if mx == mn {
        (0.0, 0.0)
    } else {
        let d = mx - mn;
        let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
        let h = if mx == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
        (h / 6.0, s)
    };
    l = (l * m + o).clamp(0.0, 1.0);
    let hue = |p: f64, q: f64, mut t: f64| {
        if t < 0.0 { t += 1.0 }
        if t > 1.0 { t -= 1.0 }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t } else if t < 0.5 { q } else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 } else { p }
    };
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        (hue(p, q, h + 1.0 / 3.0), hue(p, q, h), hue(p, q, h - 1.0 / 3.0))
    };
    let to = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u32;
    (to(r) << 16) | (to(g) << 8) | to(b)
}

/// The automatic colour of the i-th series (or pie slice).
pub fn auto_color(theme: &[u32; 12], palette: u8, i: usize) -> u32 {
    if palette >= 1 && palette <= 6 {
        // shades of one accent, darkest to lightest and back
        let base = theme[3 + palette as usize];
        let steps = [1.0, 0.6, 0.8, 0.5, 0.4, 0.75];
        let k = steps[i % steps.len()];
        return if k >= 0.75 && k != 1.0 { lum(base, k, 0.0) } else if k == 1.0 { base } else { lum(base, k, 1.0 - k) };
    }
    let base = theme[4 + i % 6];
    match (i / 6) % 4 {
        0 => base,
        1 => lum(base, 0.6, 0.0),
        2 => lum(base, 0.6, 0.4),
        _ => lum(base, 0.8, 0.2),
    }
}

// ---------------------------------------------------------------------- data

/// One point's value as the chart sees it, with its text.
#[derive(Clone, Debug)]
pub struct Pt {
    pub v: Option<f64>,
    pub text: String,
}

impl Book {
    /// The sheet and range a chart reference names ("Sheet1!$B$2:$B$5", '(a,b)' lists too).
    pub fn chart_ref_areas(&self, f: &str) -> Option<Vec<(usize, Rect)>> {
        let t = f.trim();
        let t = t.strip_prefix('=').unwrap_or(t);
        let inner = if t.starts_with('(') && t.ends_with(')') { &t[1..t.len() - 1] } else { t };
        let mut out = Vec::new();
        for part in split_top(inner) {
            let (s1, _, rest) = formula::split_sheet(part.trim());
            let s = self.sheet_index(&s1?)?;
            let rc = parse_range(&rest.replace('$', ""))?;
            out.push((s, self.sheets[s].clamp_rect(rc)));
        }
        if out.is_empty() { None } else { Some(out) }
    }

    /// The values a source shows: the cells it refers to (visible ones only), else the cache.
    pub fn chart_values(&self, src: &Src) -> Vec<Pt> {
        if let Some(f) = &src.f {
            if let Some(areas) = self.chart_ref_areas(f) {
                let mut out = Vec::new();
                for (s, rc) in areas {
                    let sh = &self.sheets[s];
                    // a block of several columns and rows reads row by row
                    for r in rc.r1..=rc.r2 {
                        if sh.row_hidden(r) {
                            continue;
                        }
                        for c in rc.c1..=rc.c2 {
                            if sh.col_hidden(c) {
                                continue;
                            }
                            let v = sh.value(r, c);
                            let text = self.display(s, r, c);
                            let n = match &v {
                                Value::Num(x) => Some(*x),
                                Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                                _ => None,
                            };
                            out.push(Pt { v: n, text });
                        }
                    }
                    if out.len() > 100_000 {
                        break;
                    }
                }
                return out;
            }
        }
        src.cache.iter().map(|c| match c {
            CVal::Empty => Pt { v: None, text: String::new() },
            CVal::Num(x) => Pt { v: Some(*x), text: crate::numfmt::format_number(*x, src.fmt.as_deref().unwrap_or("General"), self.date1904) },
            CVal::Str(s) => Pt { v: None, text: s.clone() },
        }).collect()
    }

    /// The number format of the first cell a source refers to.
    pub fn chart_format(&self, src: &Src) -> String {
        if let Some(f) = &src.f {
            if let Some(areas) = self.chart_ref_areas(f) {
                let (s, rc) = areas[0];
                return self.number_format(s, rc.r1, rc.c1);
            }
        }
        src.fmt.clone().unwrap_or_else(|| "General".into())
    }

    pub fn series_name(&self, ser: &Series, i: usize) -> String {
        match &ser.name {
            Some(src) => {
                let v = self.chart_values(src);
                let t: Vec<String> = v.into_iter().map(|p| p.text).filter(|t| !t.is_empty()).collect();
                if t.is_empty() { format!("Series{}", i + 1) } else { t.join(" ") }
            }
            None => format!("Series{}", i + 1),
        }
    }

    pub fn chart_title_text(&self, ch: &Chart) -> Option<String> {
        match &ch.title {
            Title::None => None,
            Title::Text(t) => Some(t.clone()),
            Title::Ref(src) => Some(self.chart_values(src).into_iter().map(|p| p.text).collect::<Vec<_>>().join(" ")),
            Title::Auto => {
                let all: Vec<&Series> = ch.plots.iter().flat_map(|p| p.series.iter()).collect();
                if all.len() == 1 {
                    Some(self.series_name(all[0], 0))
                } else {
                    Some("Chart Title".into())
                }
            }
        }
    }

    /// A new chart of the data in rc (Excel's rules: a text header row and column name
    /// the series and categories; series run down the columns unless there are more
    /// columns than rows).
    pub fn chart_from_range(&self, s: usize, rc: Rect, kind: &str) -> Result<Chart, String> {
        let sh = &self.sheets[s];
        let mut rc = sh.clamp_rect(rc);
        if rc.count() == 1 {
            rc = self.current_region(s, rc.r1, rc.c1);
        }
        let rc = sh.clamp_rect(rc);
        let used = self.used_rect(s).ok_or("Select the data for the chart first.")?;
        let rc = rc.intersect(&used).ok_or("Select the data for the chart first.")?;
        let is_text = |r: u32, c: u32| matches!(sh.value(r, c), Value::Str(_)) || matches!(sh.value(r, c), Value::Empty);
        let nonnum = |r: u32, c: u32| !matches!(sh.value(r, c), Value::Num(_));
        // a header row: every cell of the top row (but the corner) is text
        let header_row = rc.r2 > rc.r1 && (rc.c1..=rc.c2).all(|c| c == rc.c1 || is_text(rc.r1, c)) && (rc.c1..=rc.c2).any(|c| matches!(sh.value(rc.r1, c), Value::Str(_)));
        // a label column: text (or dates / years next to a header) down the left column
        let header_col = rc.c2 > rc.c1 && (rc.r1..=rc.r2).all(|r| (header_row && r == rc.r1) || nonnum(r, rc.c1))
            || (rc.c2 > rc.c1 && header_row && matches!(sh.value(rc.r1, rc.c1), Value::Empty));
        let d = Rect::new(rc.r1 + header_row as u32, rc.c1 + header_col as u32, rc.r2, rc.c2);
        if d.r1 > d.r2 || d.c1 > d.c2 {
            return Err("Select the data for the chart first.".into());
        }
        let by_rows = kind != "scatter" && (d.c2 - d.c1) > (d.r2 - d.r1);
        Ok(self.chart_from_parts(s, rc, d, header_row, header_col, by_rows, kind))
    }

    pub fn chart_from_parts(&self, s: usize, rc: Rect, d: Rect, header_row: bool, header_col: bool, by_rows: bool, kind: &str) -> Chart {
        let q = quote_sheet(&self.sheets[s].name);
        let a = |r1: u32, c1: u32, r2: u32, c2: u32| -> String {
            let one = |r: u32, c: u32| format!("${}${}", crate::cellref::col_name(c), r + 1);
            if (r1, c1) == (r2, c2) { format!("{}!{}", q, one(r1, c1)) } else { format!("{}!{}:{}", q, one(r1, c1), one(r2, c2)) }
        };
        let src = |f: String| Some(Src { f: Some(f), cache: vec![], fmt: None });
        let mut series = Vec::new();
        if kind == "scatter" {
            // the first column (or row) holds the x values
            let (xs, rest): (Option<String>, Vec<u32>) = if !by_rows {
                if d.c2 > d.c1 { (Some(a(d.r1, d.c1, d.r2, d.c1)), (d.c1 + 1..=d.c2).collect()) } else { (None, vec![d.c1]) }
            } else if d.r2 > d.r1 {
                (Some(a(d.r1, d.c1, d.r1, d.c2)), (d.r1 + 1..=d.r2).collect())
            } else {
                (None, vec![d.r1])
            };
            for k in rest {
                let (name, val) = if !by_rows {
                    (if header_row { src(a(rc.r1, k, rc.r1, k)) } else { None }, a(d.r1, k, d.r2, k))
                } else {
                    (if header_col { src(a(k, rc.c1, k, rc.c1)) } else { None }, a(k, d.c1, k, d.c2))
                };
                series.push(Series { name, cat: xs.clone().and_then(src), val: src(val), ..Default::default() });
            }
        } else if !by_rows {
            for c in d.c1..=d.c2 {
                series.push(Series {
                    name: if header_row { src(a(rc.r1, c, rc.r1, c)) } else { None },
                    cat: if header_col { src(a(d.r1, rc.c1, d.r2, rc.c1)) } else { None },
                    val: src(a(d.r1, c, d.r2, c)),
                    ..Default::default()
                });
            }
        } else {
            for r in d.r1..=d.r2 {
                series.push(Series {
                    name: if header_col { src(a(r, rc.c1, r, rc.c1)) } else { None },
                    cat: if header_row { src(a(rc.r1, d.c1, rc.r1, d.c2)) } else { None },
                    val: src(a(r, d.c1, r, d.c2)),
                    ..Default::default()
                });
            }
        }
        let mut ch = Chart::default();
        ch.plots = plots_for(kind, series);
        if matches!(kind, "pie" | "doughnut" | "pie-exploded") {
            ch.legend = Some("b".into());
        }
        ch
    }

    fn is_date_fmt(&self, s: usize, r: u32, c: u32) -> bool {
        crate::numfmt::get(&self.number_format(s, r, c)).is_date()
    }

    /// The smallest rectangle around the cells in use, if any.
    pub fn used_rect(&self, s: usize) -> Option<Rect> {
        let sh = &self.sheets[s];
        let mut it = sh.cells.iter().filter(|(_, c)| !c.is_blank());
        let ((r, c), _) = it.next()?;
        let mut out = Rect::cell(*r, *c);
        for ((r, c), _) in it {
            out.r1 = out.r1.min(*r);
            out.r2 = out.r2.max(*r);
            out.c1 = out.c1.min(*c);
            out.c2 = out.c2.max(*c);
        }
        for (r, c) in sh.ghosts.keys() {
            out.r2 = out.r2.max(*r);
            out.c2 = out.c2.max(*c);
        }
        Some(out)
    }
}

fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut q = false;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '\'' => q = !q,
            ',' if !q => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Chart kinds by the names the UI uses: col, col-stacked, col-percent, bar(-stacked/-percent),
/// line(-stacked/-percent)(-markers), pie, doughnut, area(-stacked/-percent), scatter(-lines), combo.
pub fn plots_for(kind: &str, mut series: Vec<Series>) -> Vec<Plot> {
    let group = if kind.contains("-percent") { Group::Percent } else if kind.contains("-stacked") { Group::Stacked } else { Group::Clustered };
    let base = kind.split('-').next().unwrap_or("col");
    let mut p = match base {
        "bar" => {
            let mut p = Plot::new(Kind::Bar);
            p.horizontal = true;
            p
        }
        "line" => {
            let mut p = Plot::new(Kind::Line);
            p.markers = kind.contains("-markers");
            p
        }
        "pie" => Plot::new(Kind::Pie),
        "doughnut" => Plot::new(Kind::Doughnut),
        "area" => Plot::new(Kind::Area),
        "radar" => {
            let mut p = Plot::new(Kind::Line);
            p.file_tag = Some("radarChart".into());
            p.markers = kind.contains("-markers");
            p.hole = if kind.contains("-filled") { 1 } else { 0 };
            p.series = series;
            return vec![p];
        }
        "stock" => {
            let mut p = Plot::new(Kind::Line);
            p.file_tag = Some("stockChart".into());
            p.markers = true;
            p.series = series;
            return vec![p];
        }
        "contour" => {
            let mut p = Plot::new(Kind::Line);
            p.file_tag = Some("surfaceChart".into());
            p.series = series;
            return vec![p];
        }
        "scatter" => {
            let mut p = Plot::new(Kind::Scatter);
            p.markers = true;
            p.lines = kind.contains("-lines") || kind.contains("-smooth");
            p
        }
        "combo" => {
            let n = series.len();
            if n >= 2 {
                let last = series.split_off(n - 1);
                let mut col = Plot::new(Kind::Bar);
                col.series = series;
                let mut line = Plot::new(Kind::Line);
                line.series = last;
                line.secondary = kind.contains("-secondary");
                return vec![col, line];
            }
            Plot::new(Kind::Bar)
        }
        _ => Plot::new(Kind::Bar),
    };
    p.group = group;
    if group != Group::Clustered && p.kind == Kind::Bar {
        p.overlap = 100;
        p.gap = 150;
    }
    if matches!(p.kind, Kind::Pie | Kind::Doughnut) {
        series.truncate(1);
    }
    p.series = series;
    if kind.contains("-smooth") {
        p.set_smooth(true);
    }
    vec![p]
}

impl Plot {
    pub fn smooth_any(&self) -> bool {
        self.series.iter().any(|s| s.smooth)
    }
}

impl Plot {
    pub fn set_smooth(&mut self, on: bool) {
        for s in self.series.iter_mut() {
            s.smooth = on;
        }
    }
}

// ---------------------------------------------------------------------- reading

fn val_attr(e: &Element, name: &str) -> Option<String> {
    e.child(name).and_then(|x| x.attr("val")).map(|s| s.to_string())
}

fn on(e: &Element, name: &str, default: bool) -> bool {
    match e.child(name) {
        Some(x) => !matches!(x.attr("val"), Some("0") | Some("false")),
        None => default,
    }
}

fn read_src(e: &Element) -> Option<Src> {
    // e is tx / cat / val / xVal / yVal: a strRef, numRef, multiLvlStrRef, strLit, numLit, or <c:v>
    for x in e.elems() {
        match x.local() {
            "strRef" | "numRef" | "multiLvlStrRef" => {
                let f = x.child("f").map(|f| f.text());
                let cache_el = x.child("strCache").or_else(|| x.child("numCache")).or_else(|| x.child("multiLvlStrCache"));
                let (cache, fmt) = cache_el.map(read_cache).unwrap_or_default();
                return Some(Src { f, cache, fmt });
            }
            "strLit" | "numLit" => {
                let (cache, fmt) = read_cache(x);
                return Some(Src { f: None, cache, fmt });
            }
            "v" => return Some(Src { f: None, cache: vec![CVal::Str(x.text())], fmt: None }),
            _ => {}
        }
    }
    None
}

fn read_cache(e: &Element) -> (Vec<CVal>, Option<String>) {
    let n: usize = val_attr(e, "ptCount").and_then(|v| v.parse().ok()).unwrap_or(0).min(100_000);
    let mut out = vec![CVal::Empty; n];
    let num = e.local() == "numCache" || e.local() == "numLit";
    // multi-level caches: use the innermost level
    let holder = if e.local() == "multiLvlStrCache" { e.child("lvl").unwrap_or(e) } else { e };
    for p in holder.elems_named("pt") {
        let i: usize = match p.attr("idx").and_then(|v| v.parse().ok()) {
            Some(i) => i,
            None => continue,
        };
        let t = p.child("v").map(|v| v.text()).unwrap_or_default();
        let v = if num { t.parse::<f64>().map(CVal::Num).unwrap_or(CVal::Str(t)) } else { CVal::Str(t) };
        if i >= out.len() {
            if i > 100_000 {
                continue;
            }
            out.resize(i + 1, CVal::Empty);
        }
        out[i] = v;
    }
    (out, e.child("formatCode").map(|f| f.text()))
}

fn rich_text(e: &Element) -> String {
    // tx/rich/a:p/a:r/a:t, paragraphs joined by newlines
    let mut out = Vec::new();
    for p in e.elems_named("p") {
        let mut s = String::new();
        for r in p.elems() {
            match r.local() {
                "r" | "fld" => s.push_str(&r.child("t").map(|t| t.text()).unwrap_or_default()),
                "br" => s.push('\n'),
                _ => {}
            }
        }
        out.push(s);
    }
    out.join("\n")
}

fn read_title(t: &Element) -> Title {
    match t.child("tx") {
        Some(tx) => {
            if let Some(rich) = tx.child("rich") {
                Title::Text(rich_text(rich))
            } else if let Some(src) = read_src(tx) {
                Title::Ref(src)
            } else {
                Title::Auto
            }
        }
        None => Title::Auto,
    }
}

fn sp_color(e: &Element, theme: &[u32; 12], line: bool) -> Option<u32> {
    let sp = e.child("spPr")?;
    let fill = if line { sp.child("ln")?.child("solidFill")? } else { sp.child("solidFill").or_else(|| sp.child("ln").and_then(|l| l.child("solidFill")))? };
    fill.elems().find_map(|c| scheme_ref(c).or_else(|| color_of(c, theme)))
}

/// A schemeClr that a theme reference says exactly (lighter/darker by lumMod/lumOff): kept as one,
/// so the colour follows the theme.
pub fn scheme_ref(e: &Element) -> Option<u32> {
    if e.local() != "schemeClr" {
        return None;
    }
    let i = match e.attr("val")? {
        "bg1" | "lt1" => 0, "tx1" | "dk1" => 1, "bg2" | "lt2" => 2, "tx2" | "dk2" => 3,
        "accent1" => 4, "accent2" => 5, "accent3" => 6, "accent4" => 7, "accent5" => 8, "accent6" => 9,
        "hlink" => 10, "folHlink" => 11,
        _ => return None,
    };
    let (mut lm, mut lo) = (1.0, 0.0);
    for m in e.elems() {
        let v = m.attr("val").and_then(|v| v.parse::<f64>().ok())? / 100000.0;
        match m.local() {
            "lumMod" => lm = v,
            "lumOff" => lo = v,
            _ => return None,
        }
    }
    let t = if lo > 0.0 && (lm + lo - 1.0).abs() < 0.001 { lo } else if lo == 0.0 { lm - 1.0 } else { return None };
    Some(crate::styles::theme_ref(i, t))
}

/// The automatic colour of the i-th series as a theme reference (what Office writes).
pub fn auto_ref(palette: u8, i: usize) -> u32 {
    use crate::styles::theme_ref;
    if palette >= 1 && palette <= 6 {
        let tints = [0.0, 0.4, -0.2, 0.5, 0.6, -0.25];
        return theme_ref(3 + palette as u32, tints[i % tints.len()]);
    }
    let slot = 4 + (i % 6) as u32;
    match (i / 6) % 4 {
        0 => theme_ref(slot, 0.0),
        1 => theme_ref(slot, -0.4),
        2 => theme_ref(slot, 0.4),
        _ => theme_ref(slot, 0.2),
    }
}

impl Chart {
    /// Read a chart part. Kinds the model doesn't draw natively are drawn as their
    /// nearest kind and always written back as read.
    pub fn parse(text: &str, theme: &[u32; 12]) -> Result<Chart, String> {
        let (prolog, root) = xml::parse(text)?;
        let chart = root.child("chart").ok_or("no chart element")?;
        let mut ch = Chart::default();
        ch.pivot = root.child("pivotSource").and_then(|ps| ps.child("name")).map(|n| n.text())
            .and_then(|t| t.rsplit('!').next().map(|x| x.to_string())).filter(|x| !x.is_empty());
        ch.title = match chart.child("title") {
            Some(t) => read_title(t),
            None => {
                if on(chart, "autoTitleDeleted", false) { Title::None } else { Title::Auto }
            }
        };
        ch.legend = chart.child("legend").map(|l| val_attr(l, "legendPos").unwrap_or_else(|| "r".into()));
        ch.gridlines = false;
        let pa = chart.child("plotArea").ok_or("no plot area")?;
        // axes by id
        let axes: Vec<&Element> = pa.elems().filter(|e| matches!(e.local(), "catAx" | "valAx" | "dateAx" | "serAx")).collect();
        let axis_title = |a: &Element| a.child("title").map(|t| match read_title(t) { Title::Text(s) => s, _ => String::new() });
        let mut primary_ids: Option<Vec<String>> = None;
        for p in pa.elems() {
            let tag = p.local().to_string();
            let (kind, native) = match tag.as_str() {
                "barChart" => (Kind::Bar, true),
                "bar3DChart" => (Kind::Bar, false),
                "lineChart" => (Kind::Line, true),
                "line3DChart" | "stockChart" | "radarChart" | "surfaceChart" | "surface3DChart" => (Kind::Line, false),
                "pieChart" => (Kind::Pie, true),
                "pie3DChart" | "ofPieChart" => (Kind::Pie, false),
                "doughnutChart" => (Kind::Doughnut, true),
                "areaChart" => (Kind::Area, true),
                "area3DChart" => (Kind::Area, false),
                "scatterChart" => (Kind::Scatter, true),
                "bubbleChart" => (Kind::Scatter, false),
                _ => continue,
            };
            let mut pl = Plot::new(kind);
            if !native {
                pl.file_tag = Some(tag.clone());
            }
            if tag == "radarChart" && val_attr(p, "radarStyle").as_deref() == Some("filled") {
                pl.hole = 1;
            }
            pl.horizontal = val_attr(p, "barDir").as_deref() == Some("bar");
            pl.group = match val_attr(p, "grouping").as_deref() {
                Some("stacked") => Group::Stacked,
                Some("percentStacked") => Group::Percent,
                _ => Group::Clustered,
            };
            pl.vary = on(p, "varyColors", false);
            if let Some(g) = val_attr(p, "gapWidth").and_then(|v| v.trim_end_matches('%').parse().ok()) {
                pl.gap = g;
            } else {
                pl.gap = 150;
            }
            pl.overlap = val_attr(p, "overlap").and_then(|v| v.trim_end_matches('%').parse().ok()).unwrap_or(if pl.group != Group::Clustered { 100 } else { 0 });
            pl.hole = if pl.hole == 1 && tag == "radarChart" { 1 } else { val_attr(p, "holeSize").and_then(|v| v.trim_end_matches('%').parse().ok()).unwrap_or(50) };
            pl.first_angle = val_attr(p, "firstSliceAng").and_then(|v| v.parse().ok()).unwrap_or(0);
            let scatter_style = val_attr(p, "scatterStyle").unwrap_or_default();
            pl.markers = match kind {
                Kind::Line => on(p, "marker", false),
                Kind::Scatter => true,
                _ => false,
            };
            pl.lines = kind == Kind::Scatter && scatter_style.starts_with("line") || scatter_style.starts_with("smooth");
            let mut any_line = false;
            let mut any_marker = false;
            for s in p.elems_named("ser") {
                let mut ser = Series::default();
                ser.name = s.child("tx").and_then(read_src);
                if kind == Kind::Scatter {
                    ser.cat = s.child("xVal").and_then(read_src);
                    ser.val = s.child("yVal").and_then(read_src);
                } else {
                    ser.cat = s.child("cat").and_then(read_src);
                    ser.val = s.child("val").and_then(read_src);
                }
                let line_kind = matches!(kind, Kind::Line | Kind::Scatter);
                ser.color = sp_color(s, theme, line_kind).or_else(|| if line_kind { sp_color(s, theme, false) } else { None });
                ser.labels = s.child("dLbls").map(|d| on(d, "showVal", false) || on(d, "showPercent", false)).unwrap_or(false);
                ser.smooth = on(s, "smooth", false);
                let ln_none = s.child("spPr").and_then(|sp| sp.child("ln")).map(|l| l.child("noFill").is_some()).unwrap_or(false);
                if !ln_none {
                    any_line = true;
                }
                match s.child("marker").and_then(|m| val_attr(m, "symbol")) {
                    Some(sym) if sym == "none" => {}
                    Some(_) => any_marker = true,
                    None => any_marker = any_marker || kind == Kind::Scatter || pl.markers,
                }
                pl.series.push(ser);
            }
            if kind == Kind::Scatter {
                pl.lines = any_line && !scatter_style.eq("marker");
                pl.markers = any_marker || !pl.lines;
            }
            if kind == Kind::Line && !pl.series.is_empty() {
                pl.markers = any_marker;
            }
            if let Some(l) = p.child("dLbls") {
                if on(l, "showVal", false) || on(l, "showPercent", false) {
                    for s in pl.series.iter_mut() {
                        s.labels = true;
                    }
                }
            }
            // axes this plot uses
            let ids: Vec<String> = p.elems_named("axId").filter_map(|a| a.attr("val").map(|s| s.to_string())).collect();
            match &primary_ids {
                None => primary_ids = Some(ids.clone()),
                Some(pi) => pl.secondary = !ids.is_empty() && ids != *pi,
            }
            for id in &ids {
                if let Some(ax) = axes.iter().find(|a| val_attr(a, "axId").as_deref() == Some(id)) {
                    if pl.secondary {
                        continue;
                    }
                    let pos = val_attr(ax, "axPos").unwrap_or_default();
                    let is_val = ax.local() == "valAx" && !(kind == Kind::Scatter && (pos == "b" || pos == "t"));
                    if ax.child("majorGridlines").is_some() && is_val {
                        ch.gridlines = true;
                    }
                    if let Some(t) = axis_title(ax) {
                        let vertical = pos == "l" || pos == "r";
                        if vertical { ch.y_title = Some(t) } else { ch.x_title = Some(t) }
                    }
                }
            }
            ch.plots.push(pl);
        }
        if ch.plots.is_empty() {
            return Err("the chart has no plot this program can show".into());
        }
        // without a title element only a one-series chart gets the automatic title
        if chart.child("title").is_none() && ch.title == Title::Auto
            && (ch.all_series().count() != 1 || ch.all_series().next().map(|s| s.name.is_none()).unwrap_or(true)) {
            ch.title = Title::None;
        }
        ch.orig = Some((prolog, root));
        Ok(ch)
    }

    /// Every reference in the chart, rewritten.
    pub fn adjust(&mut self, how: &Adjust) -> bool {
        let mut changed = false;
        let mut fix = |s: &mut Option<Src>| {
            if let Some(src) = s {
                if let Some(f) = &src.f {
                    let t = formula::adjust(f, how);
                    if &t != f {
                        src.f = Some(t);
                        changed = true;
                    }
                }
            }
        };
        for p in self.plots.iter_mut() {
            for s in p.series.iter_mut() {
                fix(&mut s.name);
                fix(&mut s.cat);
                fix(&mut s.val);
            }
        }
        if let Title::Ref(src) = &mut self.title {
            let mut o = Some(src.clone());
            fix(&mut o);
            *src = o.unwrap();
        }
        changed
    }

    pub fn all_series(&self) -> impl Iterator<Item = &Series> {
        self.plots.iter().flat_map(|p| p.series.iter())
    }

    /// Kinds a file used that this program draws only approximately.
    pub fn exotic(&self) -> bool {
        self.plots.iter().any(|p| p.file_tag.is_some() && !self.native_special())
    }

    /// Radar, stock (high-low-close) and contour charts: drawn and written here.
    pub fn native_special(&self) -> bool {
        self.plots.len() == 1 && matches!(self.plots[0].file_tag.as_deref(), Some("radarChart") | Some("stockChart") | Some("surfaceChart"))
    }

    pub fn special(&self) -> Option<&str> {
        if self.native_special() { self.plots[0].file_tag.as_deref() } else { None }
    }
}

// ---------------------------------------------------------------------- writing

fn esc(t: &str) -> String {
    xml::escape_text(t)
}

fn write_cache(book: &Book, src: &Src, num: bool, out: &mut String) {
    let pts = book.chart_values(src);
    if num {
        out.push_str("<c:numCache>");
        out.push_str(&format!("<c:formatCode>{}</c:formatCode>", esc(&book.chart_format(src))));
    } else {
        out.push_str("<c:strCache>");
    }
    out.push_str(&format!("<c:ptCount val=\"{}\"/>", pts.len()));
    for (i, p) in pts.iter().enumerate() {
        if num {
            if let Some(v) = p.v {
                out.push_str(&format!("<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>", i, crate::xlsx::num_text(v)));
            }
        } else if !p.text.is_empty() {
            out.push_str(&format!("<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>", i, esc(&p.text)));
        }
    }
    out.push_str(if num { "</c:numCache>" } else { "</c:strCache>" });
}

fn write_src(book: &Book, tag: &str, src: &Src, num: bool, out: &mut String) {
    out.push_str(&format!("<c:{}>", tag));
    match &src.f {
        Some(f) => {
            // categories of numbers are written as numbers
            let pts = book.chart_values(src);
            let num = num || (tag != "tx" && !pts.is_empty() && pts.iter().all(|p| p.v.is_some()));
            out.push_str(if num { "<c:numRef>" } else { "<c:strRef>" });
            out.push_str(&format!("<c:f>{}</c:f>", esc(f)));
            write_cache(book, src, num, out);
            out.push_str(if num { "</c:numRef>" } else { "</c:strRef>" });
        }
        None => {
            if tag == "tx" {
                let t = src.cache.iter().map(|c| match c { CVal::Str(s) => s.clone(), CVal::Num(n) => crate::xlsx::num_text(*n), CVal::Empty => String::new() }).collect::<Vec<_>>().join(" ");
                out.push_str(&format!("<c:v>{}</c:v>", esc(&t)));
            } else {
                let lit = if num { "numLit" } else { "strLit" };
                out.push_str(&format!("<c:{}>", lit));
                if num {
                    out.push_str("<c:formatCode>General</c:formatCode>");
                }
                out.push_str(&format!("<c:ptCount val=\"{}\"/>", src.cache.len()));
                for (i, c) in src.cache.iter().enumerate() {
                    match c {
                        CVal::Num(n) => out.push_str(&format!("<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>", i, crate::xlsx::num_text(*n))),
                        CVal::Str(s) => out.push_str(&format!("<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>", i, esc(s))),
                        CVal::Empty => {}
                    }
                }
                out.push_str(&format!("</c:{}>", lit));
            }
        }
    }
    out.push_str(&format!("</c:{}>", tag));
}

fn solid(c: u32) -> String {
    format!("<a:solidFill>{}</a:solidFill>", dml_color(c))
}

/// A DrawingML colour element: srgbClr, or schemeClr for a theme reference.
pub fn dml_color(c: u32) -> String {
    match crate::styles::split_ref(c) {
        Some((i, t)) => {
            let name = ["bg1", "tx1", "bg2", "tx2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"]
                .get(i as usize).copied().unwrap_or("accent1");
            let mods = if t > 0.0 {
                format!("<a:lumMod val=\"{}\"/><a:lumOff val=\"{}\"/>", ((1.0 - t) * 100000.0).round(), (t * 100000.0).round())
            } else if t < 0.0 {
                format!("<a:lumMod val=\"{}\"/>", ((1.0 + t) * 100000.0).round())
            } else {
                String::new()
            };
            format!("<a:schemeClr val=\"{}\">{}</a:schemeClr>", name, mods)
        }
        None => format!("<a:srgbClr val=\"{:06X}\"/>", c & 0xFFFFFF),
    }
}

const TEXT_GRAY: &str = "595959";
const LINE_GRAY: &str = "D9D9D9";

fn txpr(size_pt: u32, bold: bool) -> String {
    format!("<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz=\"{}\" b=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:defRPr></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>",
        size_pt * 100, bold as u8, TEXT_GRAY)
}

fn rich(text: &str, size_pt: u32, rot: Option<i32>) -> String {
    let body = match rot {
        Some(r) => format!("<a:bodyPr rot=\"{}\" vert=\"horz\"/>", r),
        None => "<a:bodyPr/>".to_string(),
    };
    let mut s = format!("<c:tx><c:rich>{}<a:lstStyle/>", body);
    for line in text.split('\n') {
        s.push_str(&format!("<a:p><a:pPr><a:defRPr sz=\"{}\" b=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:defRPr></a:pPr><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p>",
            size_pt * 100, TEXT_GRAY, esc(line)));
    }
    s.push_str("</c:rich></c:tx>");
    s
}

impl Chart {
    /// The chart part's XML.
    pub fn to_xml(&self, book: &Book) -> String {
        if let Some((prolog, root)) = &self.orig {
            let mut root = root.clone();
            self.patch_refs(&mut root);
            return xml::document(prolog, &root);
        }
        self.generate(book)
    }

    /// Write the model's references into the part as read, in the order they were read.
    fn patch_refs(&self, root: &mut Element) {
        let mut refs: Vec<Option<String>> = Vec::new();
        for p in &self.plots {
            for s in &p.series {
                refs.push(s.name.as_ref().and_then(|x| x.f.clone()));
                refs.push(s.cat.as_ref().and_then(|x| x.f.clone()));
                refs.push(s.val.as_ref().and_then(|x| x.f.clone()));
            }
        }
        let mut k = 0;
        let chart = match root.child_mut("chart") {
            Some(c) => c,
            None => return,
        };
        if let (Title::Ref(src), Some(t)) = (&self.title, chart.child_mut("title")) {
            if let Some(f) = t.child_mut("tx").and_then(|x| x.elems_mut().next()).and_then(|r| r.child_mut("f")) {
                if let Some(nf) = &src.f {
                    f.children = vec![Node::Text(nf.clone())];
                }
            }
        }
        let pa = match chart.child_mut("plotArea") {
            Some(p) => p,
            None => return,
        };
        for p in pa.elems_mut() {
            if !p.local().ends_with("Chart") {
                continue;
            }
            let scatter = matches!(p.local(), "scatterChart" | "bubbleChart");
            for s in p.elems_mut() {
                if s.local() != "ser" {
                    continue;
                }
                let tags: [&str; 3] = if scatter { ["tx", "xVal", "yVal"] } else { ["tx", "cat", "val"] };
                for t in tags {
                    let nf = refs.get(k).cloned().flatten();
                    k += 1;
                    if let Some(nf) = nf {
                        if let Some(f) = s.child_mut(t).and_then(|x| x.elems_mut().next()).and_then(|r| r.child_mut("f")) {
                            f.children = vec![Node::Text(nf)];
                        }
                    }
                }
            }
        }
    }

    fn generate(&self, book: &Book) -> String {
        let theme = &book.styles.theme;
        let mut o = String::from(xml::DECL);
        o.push_str(&format!("<c:chartSpace xmlns:c=\"{}\" xmlns:a=\"{}\" xmlns:r=\"{}\">", NS_C, NS_A, NS_R));
        o.push_str(&format!("<c:date1904 val=\"{}\"/><c:roundedCorners val=\"0\"/>", book.date1904 as u8));
        if let Some(pn) = &self.pivot {
            if let Some(p) = book.pivots.iter().find(|p| &p.name == pn) {
                let file = book.file.path.as_deref().and_then(|x| std::path::Path::new(x).file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
                    .unwrap_or_else(|| "Book1.xlsx".into());
                let sheet = book.sheets.get(p.sheet).map(|s| s.name.clone()).unwrap_or_default();
                let sheet = if sheet.chars().all(|c| c.is_alphanumeric() || c == '_') { sheet } else { format!("'{}'", sheet.replace('\'', "''")) };
                o.push_str(&format!("<c:pivotSource><c:name>[{}]{}!{}</c:name><c:fmtId val=\"0\"/></c:pivotSource>", xml::escape_text(&file), xml::escape_text(&sheet), xml::escape_text(pn)));
            }
        }
        o.push_str("<c:chart>");
        match &self.title {
            Title::None => o.push_str("<c:autoTitleDeleted val=\"1\"/>"),
            Title::Auto => {
                o.push_str(&format!("<c:title><c:overlay val=\"0\"/>{}</c:title><c:autoTitleDeleted val=\"0\"/>", txpr(14, false)));
            }
            Title::Text(t) => {
                o.push_str(&format!("<c:title>{}<c:overlay val=\"0\"/></c:title><c:autoTitleDeleted val=\"0\"/>", rich(t, 14, None)));
            }
            Title::Ref(src) => {
                o.push_str("<c:title>");
                write_src(book, "tx", src, false, &mut o);
                o.push_str(&format!("<c:overlay val=\"0\"/>{}</c:title><c:autoTitleDeleted val=\"0\"/>", txpr(14, false)));
            }
        }
        if self.special() == Some("surfaceChart") {
            o.push_str("<c:view3D><c:rotX val=\"90\"/><c:rotY val=\"0\"/><c:rAngAx val=\"0\"/><c:perspective val=\"0\"/></c:view3D>");
        }
        o.push_str("<c:plotArea><c:layout/>");
        let pie_only = self.plots.iter().all(|p| matches!(p.kind, Kind::Pie | Kind::Doughnut));
        let scatter = self.plots.first().map(|p| p.kind == Kind::Scatter).unwrap_or(false);
        let horizontal = self.plots.first().map(|p| p.horizontal).unwrap_or(false);
        let mut idx = 0usize;
        let mut has_secondary = false;
        for p in &self.plots {
            let (a1, a2) = if p.secondary { has_secondary = true; (3, 4) } else { (1, 2) };
            let grouping = match (p.kind, p.group) {
                (_, Group::Stacked) => "stacked",
                (_, Group::Percent) => "percentStacked",
                (Kind::Bar, _) => "clustered",
                _ => "standard",
            };
            let tag = p.file_tag.as_deref().filter(|t| matches!(*t, "radarChart" | "stockChart" | "surfaceChart"));
            match tag {
                Some("radarChart") => o.push_str(&format!("<c:radarChart><c:radarStyle val=\"{}\"/><c:varyColors val=\"0\"/>", if p.hole == 1 { "filled" } else { "marker" })),
                Some("stockChart") => o.push_str("<c:stockChart>"),
                Some(_) => o.push_str("<c:surfaceChart><c:wireframe val=\"0\"/>"),
                None => {}
            }
            if tag.is_none() { match p.kind {
                Kind::Bar => o.push_str(&format!("<c:barChart><c:barDir val=\"{}\"/><c:grouping val=\"{}\"/><c:varyColors val=\"{}\"/>", if p.horizontal { "bar" } else { "col" }, grouping, p.vary as u8)),
                Kind::Line => o.push_str(&format!("<c:lineChart><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>", grouping)),
                Kind::Area => o.push_str(&format!("<c:areaChart><c:grouping val=\"{}\"/><c:varyColors val=\"0\"/>", grouping)),
                Kind::Pie => o.push_str("<c:pieChart><c:varyColors val=\"1\"/>"),
                Kind::Doughnut => o.push_str("<c:doughnutChart><c:varyColors val=\"1\"/>"),
                Kind::Scatter => o.push_str(&format!("<c:scatterChart><c:scatterStyle val=\"{}\"/><c:varyColors val=\"0\"/>", if p.lines { if p.smooth_any() { "smoothMarker" } else { "lineMarker" } } else { "lineMarker" })),
            } }
            for s in &p.series {
                let color = s.color.unwrap_or_else(|| auto_ref(self.palette, idx));
                let _ = theme;
                o.push_str(&format!("<c:ser><c:idx val=\"{}\"/><c:order val=\"{}\"/>", idx, idx));
                if let Some(n) = &s.name {
                    write_src(book, "tx", n, false, &mut o);
                }
                match p.kind {
                    _ if tag == Some("surfaceChart") || (tag == Some("radarChart") && p.hole == 1) => {
                        o.push_str(&format!("<c:spPr>{}<a:ln><a:noFill/></a:ln></c:spPr>", solid(color)));
                    }
                    _ if tag == Some("stockChart") => {
                        o.push_str("<c:spPr><a:ln w=\"19050\"><a:noFill/></a:ln></c:spPr><c:marker><c:symbol val=\"none\"/></c:marker>");
                    }
                    Kind::Line | Kind::Scatter => {
                        let draw_line = p.kind == Kind::Line || p.lines;
                        if draw_line {
                            o.push_str(&format!("<c:spPr><a:ln w=\"28575\" cap=\"rnd\">{}<a:round/></a:ln></c:spPr>", solid(color)));
                        } else {
                            o.push_str("<c:spPr><a:ln w=\"25400\" cap=\"rnd\"><a:noFill/><a:round/></a:ln></c:spPr>");
                        }
                        if p.markers {
                            o.push_str(&format!("<c:marker><c:symbol val=\"circle\"/><c:size val=\"5\"/><c:spPr>{}<a:ln w=\"9525\">{}</a:ln></c:spPr></c:marker>", solid(color), solid(color)));
                        } else {
                            o.push_str("<c:marker><c:symbol val=\"none\"/></c:marker>");
                        }
                    }
                    Kind::Pie | Kind::Doughnut => {
                        o.push_str("<c:spPr><a:ln w=\"19050\"><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill></a:ln></c:spPr>");
                        let n = s.val.as_ref().map(|v| book.chart_values(v).len()).unwrap_or(0).min(255);
                        for i in 0..n {
                            o.push_str(&format!("<c:dPt><c:idx val=\"{}\"/><c:bubble3D val=\"0\"/><c:spPr>{}<a:ln w=\"19050\"><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill></a:ln></c:spPr></c:dPt>",
                                i, solid(auto_ref(self.palette, i))));
                        }
                    }
                    _ => {
                        o.push_str(&format!("<c:spPr>{}<a:ln><a:noFill/></a:ln></c:spPr>", solid(color)));
                        if p.kind == Kind::Bar {
                            o.push_str("<c:invertIfNegative val=\"0\"/>");
                        }
                    }
                }
                if s.labels {
                    o.push_str(&format!("<c:dLbls><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}<c:showLegendKey val=\"0\"/><c:showVal val=\"{}\"/><c:showCatName val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent val=\"{}\"/><c:showBubbleSize val=\"0\"/></c:dLbls>",
                        txpr(9, false), (!matches!(p.kind, Kind::Pie | Kind::Doughnut)) as u8, matches!(p.kind, Kind::Pie | Kind::Doughnut) as u8));
                }
                let (ct, vt) = if p.kind == Kind::Scatter { ("xVal", "yVal") } else { ("cat", "val") };
                if let Some(c) = &s.cat {
                    write_src(book, ct, c, p.kind == Kind::Scatter, &mut o);
                }
                if let Some(v) = &s.val {
                    write_src(book, vt, v, true, &mut o);
                }
                if matches!(p.kind, Kind::Line | Kind::Scatter) && tag.is_none() || tag == Some("stockChart") {
                    o.push_str(&format!("<c:smooth val=\"{}\"/>", s.smooth as u8));
                }
                o.push_str("</c:ser>");
                idx += 1;
            }
            match tag {
                Some("radarChart") => o.push_str(&format!("<c:axId val=\"{}\"/><c:axId val=\"{}\"/></c:radarChart>", a1, a2)),
                Some("stockChart") => o.push_str(&format!("<c:hiLowLines><c:spPr><a:ln w=\"9525\">{}</a:ln></c:spPr></c:hiLowLines><c:axId val=\"{}\"/><c:axId val=\"{}\"/></c:stockChart>", solid(0x404040), a1, a2)),
                Some(_) => o.push_str(&format!("<c:bandFmts/><c:axId val=\"{}\"/><c:axId val=\"{}\"/><c:axId val=\"5\"/></c:surfaceChart>", a1, a2)),
                None => {}
            }
            if tag.is_none() { match p.kind {
                Kind::Bar => o.push_str(&format!("<c:gapWidth val=\"{}\"/><c:overlap val=\"{}\"/><c:axId val=\"{}\"/><c:axId val=\"{}\"/></c:barChart>", p.gap, p.overlap, a1, a2)),
                Kind::Line => o.push_str(&format!("<c:marker val=\"1\"/><c:axId val=\"{}\"/><c:axId val=\"{}\"/></c:lineChart>", a1, a2)),
                Kind::Area => o.push_str(&format!("<c:axId val=\"{}\"/><c:axId val=\"{}\"/></c:areaChart>", a1, a2)),
                Kind::Pie => o.push_str(&format!("<c:firstSliceAng val=\"{}\"/></c:pieChart>", p.first_angle)),
                Kind::Doughnut => o.push_str(&format!("<c:firstSliceAng val=\"{}\"/><c:holeSize val=\"{}\"/></c:doughnutChart>", p.first_angle, p.hole)),
                Kind::Scatter => o.push_str(&format!("<c:axId val=\"{}\"/><c:axId val=\"{}\"/></c:scatterChart>", a1, a2)),
            } }
        }
        if !pie_only {
            let line = format!("<c:spPr><a:noFill/><a:ln w=\"9525\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">{}<a:round/></a:ln></c:spPr>", solid(0xD9D9D9));
            let grid = format!("<c:majorGridlines><c:spPr><a:ln w=\"9525\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:round/></a:ln></c:spPr></c:majorGridlines>", LINE_GRAY);
            let title = |t: &Option<String>, vertical: bool| -> String {
                match t {
                    Some(t) if !t.is_empty() => format!("<c:title>{}<c:overlay val=\"0\"/></c:title>", rich(t, 10, if vertical { Some(-5400000) } else { None })),
                    _ => String::new(),
                }
            };
            let (cat_pos, val_pos) = if horizontal { ("l", "b") } else { ("b", "l") };
            let fmt = self.plots.first().and_then(|p| p.series.first()).and_then(|s| s.val.as_ref()).map(|v| book.chart_format(v)).unwrap_or_else(|| "General".into());
            if scatter {
                o.push_str(&format!("<c:valAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/>{}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{}{}<c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"midCat\"/></c:valAx>",
                    title(&self.x_title, false), line, txpr(9, false)));
            } else {
                o.push_str(&format!("<c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"{}\"/>{}<c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/>{}{}<c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/><c:noMultiLvlLbl val=\"0\"/></c:catAx>",
                    cat_pos, title(&if horizontal { self.y_title.clone() } else { self.x_title.clone() }, horizontal), line, txpr(9, false)));
            }
            let percent = self.plots.first().map(|p| p.group == Group::Percent).unwrap_or(false);
            o.push_str(&format!("<c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"{}\"/>{}{}<c:numFmt formatCode=\"{}\" sourceLinked=\"{}\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}<c:crossAx val=\"1\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"{}\"/></c:valAx>",
                if scatter { "l" } else { val_pos }, if self.gridlines { grid.as_str() } else { "" },
                title(&if horizontal && !scatter { self.x_title.clone() } else { self.y_title.clone() }, !(horizontal && !scatter)),
                if percent { "0%".to_string() } else { esc(&fmt) }, (!percent) as u8, txpr(9, false), if scatter { "midCat" } else { "between" }));
            if self.special() == Some("surfaceChart") {
                o.push_str("<c:serAx><c:axId val=\"5\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"1\"/><c:axPos val=\"b\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"2\"/><c:crosses val=\"autoZero\"/></c:serAx>");
            }
            if has_secondary {
                o.push_str("<c:catAx><c:axId val=\"3\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"1\"/><c:axPos val=\"b\"/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crossAx val=\"4\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/><c:noMultiLvlLbl val=\"0\"/></c:catAx>");
                o.push_str(&format!("<c:valAx><c:axId val=\"4\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"r\"/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:majorTickMark val=\"none\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}<c:crossAx val=\"3\"/><c:crosses val=\"max\"/><c:crossBetween val=\"between\"/></c:valAx>", txpr(9, false)));
            }
        }
        o.push_str("<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr></c:plotArea>");
        if let Some(pos) = &self.legend {
            o.push_str(&format!("<c:legend><c:legendPos val=\"{}\"/><c:overlay val=\"0\"/><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}</c:legend>", esc(pos), txpr(9, false)));
        }
        o.push_str("<c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart>");
        o.push_str(&format!("<c:spPr><a:solidFill><a:srgbClr val=\"FFFFFF\"/></a:solidFill><a:ln w=\"9525\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">{}<a:round/></a:ln></c:spPr>", solid(0xD9D9D9)));
        o.push_str(&format!("<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr/></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p></c:txPr><c:printSettings><c:headerFooter/><c:pageMargins b=\"0.75\" l=\"0.7\" r=\"0.7\" t=\"0.75\" header=\"0.3\" footer=\"0.3\"/><c:pageSetup/></c:printSettings></c:chartSpace>"));
        o
    }
}
