//! Ink drawn with the Draw tab (SH-26): pen, pencil and highlighter strokes, kept in the file as
//! Office does, as InkML content parts (xl/ink/inkN.xml) the drawing points to.

use crate::xml::{self, Element};

/// pen, pencil, highlighter
pub const PEN: u8 = 0;
pub const PENCIL: u8 = 1;
pub const HIGHLIGHTER: u8 = 2;

/// Himetric (1/1000 cm) per pixel at 96 dpi.
pub const HIMETRIC: f64 = 2540.0 / 96.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// points in the ink's own space (0..w, 0..h pixels at 100 %)
    pub pts: Vec<(f64, f64)>,
    pub color: u32,
    /// pixels at 100 %
    pub width: f64,
    pub kind: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ink {
    pub strokes: Vec<Stroke>,
    /// the space the points are in (the object is drawn scaled from it to its frame)
    pub w: f64,
    pub h: f64,
}

impl Ink {
    /// Strokes in sheet pixels made into an ink object: (its box x, y, w, h, the ink).
    pub fn from_sheet(strokes: Vec<Stroke>) -> Option<(f64, f64, f64, f64, Ink)> {
        let mut x1 = f64::MAX;
        let mut y1 = f64::MAX;
        let mut x2 = f64::MIN;
        let mut y2 = f64::MIN;
        for s in &strokes {
            for &(x, y) in &s.pts {
                x1 = x1.min(x);
                y1 = y1.min(y);
                x2 = x2.max(x);
                y2 = y2.max(y);
            }
        }
        if x1 > x2 {
            return None;
        }
        let w = (x2 - x1).max(1.0);
        let h = (y2 - y1).max(1.0);
        let strokes = strokes.into_iter().map(|s| Stroke { pts: s.pts.iter().map(|&(x, y)| (x - x1, y - y1)).collect(), ..s }).collect();
        Some((x1, y1, w, h, Ink { strokes, w, h }))
    }
}

fn hex(c: u32) -> String {
    format!("#{:06X}", c & 0xFFFFFF)
}

/// InkML for the strokes, as Office writes it: coordinates in himetric.
pub fn write_inkml(ink: &Ink) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<inkml:ink xmlns:inkml=\"http://www.w3.org/2003/InkML\">");
    s.push_str("<inkml:definitions><inkml:context xml:id=\"ctx0\"><inkml:inkSource xml:id=\"inkSrc0\"><inkml:traceFormat>");
    s.push_str("<inkml:channel name=\"X\" type=\"integer\" max=\"32767\" units=\"cm\"/><inkml:channel name=\"Y\" type=\"integer\" max=\"32767\" units=\"cm\"/>");
    s.push_str("</inkml:traceFormat><inkml:channelProperties><inkml:channelProperty channel=\"X\" name=\"resolution\" value=\"1000\" units=\"1/cm\"/>");
    s.push_str("<inkml:channelProperty channel=\"Y\" name=\"resolution\" value=\"1000\" units=\"1/cm\"/></inkml:channelProperties></inkml:inkSource></inkml:context>");
    for (i, st) in ink.strokes.iter().enumerate() {
        let cm = st.width / 96.0 * 2.54;
        s.push_str(&format!("<inkml:brush xml:id=\"br{}\"><inkml:brushProperty name=\"width\" value=\"{:.5}\" units=\"cm\"/><inkml:brushProperty name=\"height\" value=\"{:.5}\" units=\"cm\"/><inkml:brushProperty name=\"color\" value=\"{}\"/>",
            i, cm, cm, hex(st.color)));
        if st.kind == HIGHLIGHTER {
            s.push_str("<inkml:brushProperty name=\"tip\" value=\"rectangle\"/><inkml:brushProperty name=\"rasterOp\" value=\"maskPen\"/><inkml:brushProperty name=\"transparency\" value=\"127\"/>");
        }
        if st.kind == PENCIL {
            s.push_str("<inkml:brushProperty name=\"inkEffects\" value=\"pencil\"/>");
        }
        s.push_str("</inkml:brush>");
    }
    s.push_str("</inkml:definitions>");
    for (i, st) in ink.strokes.iter().enumerate() {
        s.push_str(&format!("<inkml:trace contextRef=\"#ctx0\" brushRef=\"#br{}\">", i));
        let pts: Vec<String> = st.pts.iter().map(|&(x, y)| format!("{} {}", (x * HIMETRIC).round() as i64, (y * HIMETRIC).round() as i64)).collect();
        s.push_str(&pts.join(", "));
        s.push_str("</inkml:trace>");
    }
    s.push_str("</inkml:ink>");
    s
}

/// One trace's points (X and Y of each), decoding InkML's difference encoding: a value after '
/// is the change from the last one, after " the change of that change; the last qualifier of a
/// channel holds until another is given.
fn trace_points(text: &str, channels: usize) -> Vec<(f64, f64)> {
    let channels = channels.max(2);
    let mut out = Vec::new();
    let mut val = vec![0.0f64; channels];
    let mut vel = vec![0.0f64; channels];
    let mut mode = vec![b'!'; channels];
    for point in text.split(',') {
        let b = point.as_bytes();
        let mut i = 0;
        let mut ch = 0;
        while i < b.len() && ch < channels {
            let c = b[i];
            if c.is_ascii_whitespace() {
                i += 1;
                continue;
            }
            let mut q = None;
            if c == b'!' || c == b'\'' || c == b'"' {
                q = Some(c);
                i += 1;
                while i < b.len() && b[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
            let start = i;
            if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
                i += 1;
            }
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b'e' || b[i] == b'E'
                                  || ((b[i] == b'-' || b[i] == b'+') && i > start && (b[i - 1] == b'e' || b[i - 1] == b'E'))) {
                i += 1;
            }
            let num: f64 = match std::str::from_utf8(&b[start..i]).ok().and_then(|t| t.parse().ok()) {
                Some(v) => v,
                None => {
                    i = i.max(start + 1);
                    continue;
                }
            };
            if let Some(q) = q {
                mode[ch] = q;
            }
            match mode[ch] {
                b'\'' => {
                    vel[ch] = num;
                    val[ch] += num;
                }
                b'"' => {
                    vel[ch] += num;
                    val[ch] += vel[ch];
                }
                _ => {
                    vel[ch] = num - val[ch];
                    val[ch] = num;
                }
            }
            ch += 1;
        }
        if ch > 0 {
            out.push((val[0], val[1]));
        }
    }
    out
}

fn parse_color(v: &str) -> u32 {
    u32::from_str_radix(v.trim_start_matches('#'), 16).unwrap_or(0) & 0xFFFFFF
}

/// The strokes of an InkML part, in its own units (himetric for Office's ink).
pub fn parse_inkml(text: &str) -> Option<Vec<Stroke>> {
    let (_, root) = xml::parse(text).ok()?;
    struct Brush {
        color: u32,
        width: f64,
        kind: u8,
    }
    let mut brushes: Vec<(String, Brush)> = Vec::new();
    let mut channels = 2usize;
    fn walk<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
        out.push(e);
        for c in e.elems() {
            walk(c, out);
        }
    }
    let mut all = Vec::new();
    walk(&root, &mut all);
    for e in &all {
        match e.local() {
            "traceFormat" => channels = e.elems().filter(|c| c.local() == "channel").count().max(2),
            "brush" => {
                let id = e.attrs.iter().find(|(k, _)| xml::local(k) == "id").map(|(_, v)| v.clone()).unwrap_or_default();
                let mut b = Brush { color: 0, width: 1.0, kind: PEN };
                let mut mask = false;
                for p in e.elems().filter(|c| c.local() == "brushProperty") {
                    let v = p.attr("value").unwrap_or("");
                    match p.attr("name").unwrap_or("") {
                        "color" => b.color = parse_color(v),
                        "width" => {
                            let w: f64 = v.parse().unwrap_or(0.05);
                            let units = p.attr("units").unwrap_or("cm");
                            b.width = match units {
                                "mm" => w / 25.4 * 96.0,
                                "in" => w * 96.0,
                                "himetric" => w / HIMETRIC,
                                _ => w / 2.54 * 96.0,
                            };
                        }
                        "rasterOp" => mask = v == "maskPen",
                        "inkEffects" if v.eq_ignore_ascii_case("pencil") => b.kind = PENCIL,
                        _ => {}
                    }
                }
                if mask {
                    b.kind = HIGHLIGHTER;
                }
                brushes.push((id, b));
            }
            _ => {}
        }
    }
    let mut strokes = Vec::new();
    for e in &all {
        if e.local() != "trace" {
            continue;
        }
        let bref = e.attr("brushRef").unwrap_or("").trim_start_matches('#').to_string();
        let b = brushes.iter().find(|(id, _)| *id == bref).map(|(_, b)| b).or_else(|| brushes.first().map(|(_, b)| b));
        let pts = trace_points(&e.text(), channels);
        if pts.is_empty() {
            continue;
        }
        strokes.push(Stroke {
            pts,
            color: b.map(|b| b.color).unwrap_or(0),
            width: b.map(|b| b.width).unwrap_or(1.0),
            kind: b.map(|b| b.kind).unwrap_or(PEN),
        });
    }
    Some(strokes)
}

/// An ink part read into an object of this size (pixels): the strokes' bounds fill the frame.
pub fn ink_for_frame(strokes: Vec<Stroke>, w: f64, h: f64) -> Ink {
    let mut x1 = f64::MAX;
    let mut y1 = f64::MAX;
    let mut x2 = f64::MIN;
    let mut y2 = f64::MIN;
    for s in &strokes {
        for &(x, y) in &s.pts {
            x1 = x1.min(x);
            y1 = y1.min(y);
            x2 = x2.max(x);
            y2 = y2.max(y);
        }
    }
    if x1 > x2 {
        return Ink { strokes: vec![], w: w.max(1.0), h: h.max(1.0) };
    }
    let sx = if x2 > x1 { w / (x2 - x1) } else { 1.0 / HIMETRIC };
    let sy = if y2 > y1 { h / (y2 - y1) } else { 1.0 / HIMETRIC };
    let strokes = strokes.into_iter().map(|s| Stroke { pts: s.pts.iter().map(|&(x, y)| ((x - x1) * sx, (y - y1) * sy)).collect(), ..s }).collect();
    Ink { strokes, w: w.max(1.0), h: h.max(1.0) }
}

/// Distance from a point to the stroke's centre line.
pub fn distance(s: &Stroke, px: f64, py: f64) -> f64 {
    let mut best = f64::MAX;
    if s.pts.len() == 1 {
        let (x, y) = s.pts[0];
        return ((x - px).powi(2) + (y - py).powi(2)).sqrt();
    }
    for w in s.pts.windows(2) {
        let (ax, ay) = w[0];
        let (bx, by) = w[1];
        let (dx, dy) = (bx - ax, by - ay);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let (cx, cy) = (ax + t * dx, ay + t * dy);
        best = best.min(((cx - px).powi(2) + (cy - py).powi(2)).sqrt());
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difference_encoding() {
        // explicit, then first differences, then second differences
        let p = trace_points("100 200, '10 '20, '10 '20, \"0 \"0, \"5\"-5", 2);
        assert_eq!(p, vec![(100.0, 200.0), (110.0, 220.0), (120.0, 240.0), (130.0, 260.0), (145.0, 275.0)]);
        // extra channels are read past
        let p = trace_points("1 2 3, 4 5 6", 3);
        assert_eq!(p, vec![(1.0, 2.0), (4.0, 5.0)]);
    }

    #[test]
    fn round_trip() {
        let ink = Ink {
            strokes: vec![
                Stroke { pts: vec![(0.0, 0.0), (50.0, 20.0), (100.0, 40.0)], color: 0xE71224, width: 2.0, kind: PEN },
                Stroke { pts: vec![(0.0, 40.0), (100.0, 40.0)], color: 0xFFFF00, width: 12.0, kind: HIGHLIGHTER },
                Stroke { pts: vec![(10.0, 10.0), (20.0, 30.0)], color: 0x333333, width: 1.0, kind: PENCIL },
            ],
            w: 100.0,
            h: 40.0,
        };
        let xml = write_inkml(&ink);
        let back = parse_inkml(&xml).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].color, 0xE71224);
        assert_eq!(back[1].kind, HIGHLIGHTER);
        assert_eq!(back[2].kind, PENCIL);
        assert!((back[1].width - 12.0).abs() < 0.01);
        let f = ink_for_frame(back, 100.0, 40.0);
        let (x, y) = f.strokes[0].pts[2];
        assert!((x - 100.0).abs() < 0.1 && (y - 40.0).abs() < 0.1, "{:?}", f.strokes[0].pts);
    }

    #[test]
    fn office_style_trace() {
        let t = "<inkml:ink xmlns:inkml=\"http://www.w3.org/2003/InkML\"><inkml:definitions><inkml:context xml:id=\"ctx0\"><inkml:inkSource xml:id=\"inkSrc0\"><inkml:traceFormat><inkml:channel name=\"X\" type=\"integer\" max=\"32767\" units=\"cm\"/><inkml:channel name=\"Y\" type=\"integer\" max=\"32767\" units=\"cm\"/><inkml:channel name=\"F\" name2=\"\" type=\"integer\" max=\"32767\" units=\"dev\"/></inkml:traceFormat></inkml:inkSource></inkml:context><inkml:brush xml:id=\"br0\"><inkml:brushProperty name=\"width\" value=\"0.05\" units=\"cm\"/><inkml:brushProperty name=\"height\" value=\"0.05\" units=\"cm\"/><inkml:brushProperty name=\"color\" value=\"#004F8B\"/></inkml:brush></inkml:definitions><inkml:trace contextRef=\"#ctx0\" brushRef=\"#br0\">11590 4699 0,'0'0,0 0</inkml:trace></inkml:ink>";
        let s = parse_inkml(t).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].color, 0x004F8B);
        assert_eq!(s[0].pts[0], (11590.0, 4699.0));
        assert!((s[0].width - 0.05 / 2.54 * 96.0).abs() < 1e-9);
    }

    #[test]
    fn near_a_stroke() {
        let s = Stroke { pts: vec![(0.0, 0.0), (10.0, 0.0)], color: 0, width: 1.0, kind: PEN };
        assert!((distance(&s, 5.0, 3.0) - 3.0).abs() < 1e-9);
        assert!((distance(&s, 13.0, 4.0) - 5.0).abs() < 1e-9);
    }
}
