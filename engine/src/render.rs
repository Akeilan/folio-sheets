//! Drawing the grid onto a cairo context: fills, gridlines, text, borders, then the
//! selection, copy marquee, formula references, tracer arrows and headers on top.

use crate::book::Book;
use crate::cellref::{col_name, Rect, MAX_COLS};
use crate::ffi::*;
use crate::layout::{family_list, font_of, FontSpec};
use crate::styles::Xf;
use crate::value::Value;
use crate::view::{Geometry, ViewParams};
use std::collections::HashMap;

const GRID: u32 = 0xD4D4D4;
const HEADER_BG: u32 = 0xF5F5F5;
const HEADER_SEL: u32 = 0xE1E1E1;
const HEADER_TEXT: u32 = 0x444444;
const ACCENT: u32 = 0xA67C00;
const ACCENT_TEXT: u32 = 0x7D5E06;
const SPILL: u32 = 0x4472C4;

fn rgb(cr: *mut cairo_t, c: u32) {
    unsafe {
        cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
    }
}

fn rgba(cr: *mut cairo_t, c: u32, a: f64) {
    unsafe {
        cairo_set_source_rgba(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0, a);
    }
}

fn rect(cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64) {
    unsafe {
        cairo_rectangle(cr, x, y, w, h);
        cairo_fill(cr);
    }
}

fn mix(a: u32, b: u32, t: f64) -> u32 {
    let ch = |s: u32| -> (f64, f64) { (((a >> s) & 255) as f64, ((b >> s) & 255) as f64) };
    let mut out = 0;
    for s in [16, 8, 0] {
        let (x, y) = ch(s);
        out |= ((x * (1.0 - t) + y * t).round() as u32) << s;
    }
    out
}

struct Pen {
    layout: *mut PangoLayout,
    fonts: HashMap<(String, u64, bool, bool, u8), crate::layout::FontDesc>,
}

impl Pen {
    fn font(&mut self, f: &FontSpec, zoom: f64) -> *mut PangoFontDescription {
        let key = (f.family.clone(), (f.size * zoom * 100.0).round() as u64, f.bold, f.italic, f.vert);
        self.fonts.entry(key).or_insert_with(|| crate::layout::FontDesc::new(f, zoom)).0
    }

    /// Lay out text in a font; attributes for underline/strike. Returns the logical size.
    fn set(&mut self, text: &str, f: &FontSpec, zoom: f64, wrap: Option<f64>) -> (f64, f64) {
        let d = self.font(f, zoom);
        unsafe {
            let l = self.layout;
            pango_layout_set_font_description(l, d);
            let attrs = pango_attr_list_new();
            if f.underline != 0 {
                pango_attr_list_insert(attrs, pango_attr_underline_new(if f.underline == 2 || f.underline == 4 { PANGO_UNDERLINE_DOUBLE } else { PANGO_UNDERLINE_SINGLE }));
            }
            if f.strike {
                pango_attr_list_insert(attrs, pango_attr_strikethrough_new(1));
            }
            pango_layout_set_attributes(l, attrs);
            pango_attr_list_unref(attrs);
            match wrap {
                Some(w) => {
                    pango_layout_set_width(l, (w.max(1.0) * PANGO_SCALE) as i32);
                    pango_layout_set_wrap(l, PANGO_WRAP_WORD_CHAR);
                }
                None => pango_layout_set_width(l, -1),
            }
            set_text(l, text);
            let mut ink = PangoRectangle::default();
            let mut lg = PangoRectangle::default();
            pango_layout_get_extents(l, &mut ink, &mut lg);
            (lg.width as f64 / PANGO_SCALE, lg.height as f64 / PANGO_SCALE)
        }
    }

    fn width(&mut self, text: &str, f: &FontSpec, zoom: f64) -> f64 {
        self.set(text, f, zoom, None).0
    }

    fn show(&mut self, cr: *mut cairo_t, x: f64, y: f64) {
        unsafe {
            cairo_move_to(cr, x, y);
            pango_cairo_show_layout(cr, self.layout);
        }
    }

    fn set_align(&mut self, a: i32) {
        unsafe { pango_layout_set_alignment(self.layout, a) }
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        self.fonts.clear();
        unsafe { g_object_unref(self.layout) }
    }
}

/// Screen positions of the rows and columns on screen.
struct Bands {
    rows: HashMap<u32, (f64, f64)>,
    cols: HashMap<u32, (f64, f64)>,
}

impl Book {
    /// Paint a sheet. `cr` is a cairo context the caller owns.
    pub unsafe fn render(&self, s: usize, cr: *mut cairo_t, p: &ViewParams) {
        let g = self.geometry(s, p);
        let mut pen = Pen { layout: pango_cairo_create_layout(cr), fonts: HashMap::new() };
        cairo_save(cr);
        rgb(cr, 0xFFFFFF);
        rect(cr, 0.0, 0.0, p.width, p.height);
        let bands = Bands {
            rows: g.rows.iter().map(|(r, y, h)| (*r, (*y, *h))).collect(),
            cols: g.cols.iter().map(|(c, x, w)| (*c, (*x, *w))).collect(),
        };
        // every combination of row band and column band
        for (bi, (y0, y1)) in g.row_spans.iter().enumerate() {
            for (bj, (x0, x1)) in g.col_spans.iter().enumerate() {
                if y1 <= y0 || x1 <= x0 {
                    continue;
                }
                let frozen_r = g.row_spans.len() > 1 && bi == 0;
                let frozen_c = g.col_spans.len() > 1 && bj == 0;
                let rows: Vec<(u32, f64, f64)> = g.rows.iter().copied().filter(|(r, _, _)| (*r < p.freeze_rows) == frozen_r || g.row_spans.len() == 1).collect();
                let cols: Vec<(u32, f64, f64)> = g.cols.iter().copied().filter(|(c, _, _)| (*c < p.freeze_cols) == frozen_c || g.col_spans.len() == 1).collect();
                cairo_save(cr);
                cairo_rectangle(cr, *x0, *y0, x1 - x0, y1 - y0);
                cairo_clip(cr);
                self.paint_cells(s, cr, p, &g, &bands, &rows, &cols, &mut pen);
                self.paint_filter_buttons(s, cr, p, &rows, &cols);
                self.paint_overlays(s, cr, p, &g, &rows, &cols);
                cairo_restore(cr);
            }
        }
        // the freeze lines
        if p.freeze_rows > 0 && g.row_spans.len() > 1 {
            rgb(cr, 0x9A9A9A);
            rect(cr, g.hw, g.row_spans[0].1 - 1.0, p.width - g.hw, 1.0);
        }
        if p.freeze_cols > 0 && g.col_spans.len() > 1 {
            rgb(cr, 0x9A9A9A);
            rect(cr, g.col_spans[0].1 - 1.0, g.hh, 1.0, p.height - g.hh);
        }
        if p.show_headers {
            self.paint_headers(s, cr, p, &g, &mut pen);
        }
        cairo_restore(cr);
    }

    pub fn fill_color(&self, xf: &Xf) -> Option<u32> {
        let f = self.styles.fill(xf.fill);
        let fg = f.fg.as_ref().map(|c| self.styles.rgb(c, 0x000000));
        let bg = f.bg.as_ref().map(|c| self.styles.rgb(c, 0xFFFFFF)).unwrap_or(0xFFFFFF);
        match f.pattern.as_str() {
            "none" | "" => None,
            "solid" => Some(fg.unwrap_or(0x000000)),
            "gray125" => Some(mix(bg, fg.unwrap_or(0), 0.125)),
            "gray0625" => Some(mix(bg, fg.unwrap_or(0), 0.0625)),
            "lightGray" => Some(mix(bg, fg.unwrap_or(0), 0.25)),
            "mediumGray" => Some(mix(bg, fg.unwrap_or(0), 0.5)),
            "darkGray" => Some(mix(bg, fg.unwrap_or(0), 0.75)),
            _ => Some(mix(bg, fg.unwrap_or(0), 0.4)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_cells(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, bands: &Bands,
                          rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)], pen: &mut Pen) {
        if rows.is_empty() || cols.is_empty() {
            return;
        }
        let sh = &self.sheets[s];
        let z = p.zoom;
        let (rmin, rmax) = (rows[0].0, rows[rows.len() - 1].0);
        let (cmin, cmax) = (cols[0].0, cols[cols.len() - 1].0);
        let view = Rect::new(rmin, cmin, rmax, cmax);
        let merges: Vec<Rect> = sh.merges.iter().filter(|m| m.intersects(&view)).copied().collect();
        let merge_of = |r: u32, c: u32| merges.iter().find(|m| m.contains(r, c)).copied();
        let mrect = |m: &Rect| -> (f64, f64, f64, f64) {
            let x1 = self.screen_x(s, p, g, m.c1);
            let x2 = self.screen_x(s, p, g, m.c2) + self.col_px(s, m.c2) * z;
            let y1 = self.screen_y(s, p, g, m.r1);
            let y2 = self.screen_y(s, p, g, m.r2) + self.row_px(s, m.r2) * z;
            (x1, y1, x2 - x1, y2 - y1)
        };
        // ---- fills
        let mut filled: HashMap<(u32, u32), u32> = HashMap::new();
        for &(r, y, h) in rows {
            for &(c, x, w) in cols {
                let (xf, (fx, fy, fw, fh)) = match merge_of(r, c) {
                    Some(m) => (sh.style_at(m.r1, m.c1), mrect(&m)),
                    None => (sh.style_at(r, c), (x, y, w, h)),
                };
                if xf == 0 {
                    continue;
                }
                if let Some(col) = self.fill_color(self.styles.xf(xf)) {
                    filled.insert((r, c), col);
                    if merge_of(r, c).map(|m| (m.r1.max(rmin), m.c1.max(cmin)) == (r, c)).unwrap_or(true) {
                        rgb(cr, col);
                        rect(cr, fx, fy, fw, fh);
                    }
                }
            }
        }
        // ---- text (decides which gridlines overflowing text hides)
        let mut hide_v: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        let mut texts: Vec<TextJob> = Vec::new();
        for &(r, y, h) in rows {
            // cells on this row, including some to the left whose text may run into view
            let from = cmin.saturating_sub(64);
            let to = (cmax + 64).min(MAX_COLS - 1);
            let mut items: Vec<(u32, Value, u32, bool)> = Vec::new();
            for ((_, c), cell) in sh.cells.range((r, from)..=(r, to)) {
                let v = sh.value(r, *c);
                let is_f = cell.f.is_some();
                if matches!(v, Value::Empty) && !(p.show_formulas && is_f) {
                    continue;
                }
                items.push((*c, v, cell.s, is_f));
            }
            for ((_, c), v) in sh.ghosts.range((r, from)..=(r, to)) {
                if !sh.cells.get(&(r, *c)).map(|x| !x.is_blank()).unwrap_or(false) {
                    items.push((*c, v.clone(), sh.style_at(r, *c), false));
                }
            }
            items.sort_by_key(|x| x.0);
            for (c, v, xf, is_f) in items {
                if p.editing == Some((r, c)) {
                    continue;
                }
                if let Some(m) = merge_of(r, c) {
                    if (m.r1, m.c1) != (r, c) {
                        continue;
                    }
                } else if let Some(m) = sh.merge_at(r, c) {
                    if (m.r1, m.c1) != (r, c) {
                        continue;
                    }
                }
                let (cx, cw) = match bands.cols.get(&c) {
                    Some(v) => *v,
                    None => {
                        let x = self.screen_x(s, p, g, c);
                        (x, self.col_px(s, c) * z)
                    }
                };
                let merged = sh.merge_at(r, c).filter(|m| m.r1 != m.r2 || m.c1 != m.c2);
                let (bx, by, bw, bh) = match merged {
                    Some(m) => mrect(&m),
                    None => (cx, y, cw, h),
                };
                if bw <= 0.0 || bh <= 0.0 {
                    continue;
                }
                let job = self.layout_text(s, r, c, &v, xf, is_f, p, (bx, by, bw, bh), merged.is_some(), pen, &|cc: u32| {
                    // an empty neighbour that text may run into
                    !sh.has_content(r, cc) && sh.merge_at(r, cc).is_none() && p.editing != Some((r, cc))
                }, &|cc: u32| bands.cols.get(&cc).copied().unwrap_or_else(|| (self.screen_x(s, p, g, cc), self.col_px(s, cc) * z)));
                if let Some(job) = job {
                    for cc in job.covers.0..job.covers.1 {
                        hide_v.insert((r, cc));
                    }
                    texts.push(job);
                }
            }
        }
        // ---- gridlines
        if p.show_grid {
            rgb(cr, GRID);
            for &(r, y, h) in rows {
                for &(c, x, w) in cols {
                    let here_m = merge_of(r, c);
                    // right edge
                    let right_m = merge_of(r, c + 1);
                    let inside = here_m.is_some() && here_m == right_m;
                    let fill_r = filled.contains_key(&(r, c)) || filled.contains_key(&(r, c + 1));
                    if !inside && !fill_r && !hide_v.contains(&(r, c)) {
                        rect(cr, (x + w - 1.0).round(), y, 1.0, h);
                    }
                    let below_m = merge_of(r + 1, c);
                    let inside_b = here_m.is_some() && here_m == below_m;
                    let fill_b = filled.contains_key(&(r, c)) || filled.contains_key(&(r + 1, c));
                    if !inside_b && !fill_b {
                        rect(cr, x, (y + h - 1.0).round(), w, 1.0);
                    }
                }
            }
        }
        for t in texts {
            t.draw(cr, pen);
        }
        // ---- borders
        for &(r, y, h) in rows {
            for &(c, x, w) in cols {
                let xf_i = match merge_of(r, c) {
                    Some(m) => {
                        // a merged area takes the borders of its edge cells
                        sh.style_at(r, c).max(0).min(u32::MAX).max(if (r, c) == (m.r1, m.c1) { 0 } else { 0 })
                    }
                    None => sh.style_at(r, c),
                };
                if xf_i == 0 {
                    continue;
                }
                let xf = self.styles.xf(xf_i);
                if xf.border == 0 {
                    continue;
                }
                let b = self.styles.border(xf.border);
                let m = merge_of(r, c);
                let edge = |side: u8| -> bool {
                    match m {
                        None => true,
                        Some(m) => match side {
                            0 => c == m.c1,
                            1 => c == m.c2,
                            2 => r == m.r1,
                            _ => r == m.r2,
                        },
                    }
                };
                if let Some((st, col)) = &b.left {
                    if edge(0) {
                        line(cr, st, self.styles.rgb(col, 0), x - 0.5, y, x - 0.5, y + h, z);
                    }
                }
                if let Some((st, col)) = &b.right {
                    if edge(1) {
                        line(cr, st, self.styles.rgb(col, 0), x + w - 0.5, y, x + w - 0.5, y + h, z);
                    }
                }
                if let Some((st, col)) = &b.top {
                    if edge(2) {
                        line(cr, st, self.styles.rgb(col, 0), x - 1.0, y - 0.5, x + w, y - 0.5, z);
                    }
                }
                if let Some((st, col)) = &b.bottom {
                    if edge(3) {
                        line(cr, st, self.styles.rgb(col, 0), x - 1.0, y + h - 0.5, x + w, y + h - 0.5, z);
                    }
                }
                if let Some((st, col)) = &b.diagonal {
                    let (dx, dy, dw, dh) = match m {
                        Some(m) => mrect(&m),
                        None => (x, y, w, h),
                    };
                    if m.map(|m| (m.r1, m.c1) == (r.max(rmin).min(m.r1.max(rmin)), c)).unwrap_or(true) {
                        let colr = self.styles.rgb(col, 0);
                        if b.diag_down {
                            line(cr, st, colr, dx, dy, dx + dw, dy + dh, z);
                        }
                        if b.diag_up {
                            line(cr, st, colr, dx, dy + dh, dx + dw, dy, z);
                        }
                    }
                }
            }
        }
    }

    /// The AutoFilter drop-down buttons in the header row.
    unsafe fn paint_filter_buttons(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)]) {
        let af = match &self.sheets[s].autofilter {
            Some(a) => a,
            None => return,
        };
        let row = match rows.iter().find(|(r, _, _)| *r == af.rect.r1) {
            Some(x) => *x,
            None => return,
        };
        for &(c, x, w) in cols {
            if c < af.rect.c1 || c > af.rect.c2 {
                continue;
            }
            let (bx, by, bs) = filter_button(x, row.1, w, row.2, p.zoom);
            let filtered = af.cols.iter().any(|f| f.col == c - af.rect.c1);
            rgb(cr, 0xFFFFFF);
            rect(cr, bx, by, bs, bs);
            rgb(cr, 0xA0A0A0);
            cairo_set_line_width(cr, 1.0);
            cairo_rectangle(cr, bx + 0.5, by + 0.5, bs - 1.0, bs - 1.0);
            cairo_stroke(cr);
            rgb(cr, 0x444444);
            let (cx, cy) = (bx + bs / 2.0, by + bs / 2.0);
            if filtered {
                // a funnel and a small arrow
                cairo_move_to(cr, cx - bs * 0.32, cy - bs * 0.28);
                cairo_line_to(cr, cx + bs * 0.12, cy - bs * 0.28);
                cairo_line_to(cr, cx - 0.5, cy + 0.0);
                cairo_line_to(cr, cx - 0.5, cy + bs * 0.3);
                cairo_line_to(cr, cx - bs * 0.2, cy + bs * 0.2);
                cairo_line_to(cr, cx - bs * 0.2, cy);
                cairo_close_path(cr);
                cairo_fill(cr);
                cairo_move_to(cr, cx + bs * 0.12, cy + bs * 0.05);
                cairo_line_to(cr, cx + bs * 0.36, cy + bs * 0.05);
                cairo_line_to(cr, cx + bs * 0.24, cy + bs * 0.2);
                cairo_close_path(cr);
                cairo_fill(cr);
            } else {
                cairo_move_to(cr, cx - bs * 0.25, cy - bs * 0.1);
                cairo_line_to(cr, cx + bs * 0.25, cy - bs * 0.1);
                cairo_line_to(cr, cx, cy + bs * 0.18);
                cairo_close_path(cr);
                cairo_fill(cr);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn layout_text(&self, s: usize, r: u32, c: u32, v: &Value, xf_i: u32, is_formula: bool, p: &ViewParams,
                          cell: (f64, f64, f64, f64), merged: bool, pen: &mut Pen,
                          empty: &dyn Fn(u32) -> bool, colpos: &dyn Fn(u32) -> (f64, f64)) -> Option<TextJob> {
        let z = p.zoom;
        let xf = self.styles.xf(xf_i).clone();
        let font = font_of(&self.styles, xf_i);
        let (bx, by, bw, bh) = cell;
        let pad = (3.0 * z).round().max(1.0);
        let mdw = self.mdw(s);
        let indent = xf.indent as f64 * 3.0 * mdw * z;
        // the text and its colour
        let mut color = font.color;
        let mut fill_char: Option<(usize, char)> = None;
        let mut hash = false;
        let mut text;
        let mut halign = xf.halign.clone();
        if p.show_formulas && is_formula {
            text = format!("={}", self.sheets[s].get(r, c).and_then(|x| x.f.as_ref()).map(|f| f.text.clone()).unwrap_or_default());
            if halign.is_empty() || halign == "general" {
                halign = "left".into();
            }
        } else {
            // numbers may use the cell up to its right padding
            let avail = bw - pad - 2.0 - indent;
            let f = match v {
                Value::Num(_) if self.styles.xf_numfmt(xf_i).eq_ignore_ascii_case("General") && !xf.wrap => {
                    // General shows as many digits as fit (11 at most)
                    let mut best = self.format_value(v, xf_i, 11);
                    let mut k = 11;
                    while k > 1 && pen.width(&best.text, &font, z) > avail {
                        k -= 1;
                        best = self.format_value(v, xf_i, k);
                    }
                    best
                }
                _ => self.format_value(v, xf_i, 255),
            };
            text = f.text.clone();
            if let Some(col) = f.color {
                color = col;
            }
            fill_char = f.fill;
            hash = f.hash_on_overflow || matches!(v, Value::Num(_));
            // pads (_x): leave the width of a character
            if !f.pads.is_empty() {
                let mut t = String::new();
                let mut last = 0;
                let mut shift = 0;
                for (pos, ch) in &f.pads {
                    if *pos >= last && *pos <= text.len() && text.is_char_boundary(*pos) {
                        t.push_str(&text[last..*pos]);
                        let pc = if *ch == ')' || *ch == '(' { '\u{2007}' } else { '\u{2002}' };
                        t.push(pc);
                        if let Some((fp, _)) = fill_char {
                            if *pos <= fp {
                                shift += pc.len_utf8();
                            }
                        }
                        last = *pos;
                    }
                }
                t.push_str(&text[last..]);
                text = t;
                if let Some((fp, fc)) = fill_char {
                    fill_char = Some((fp + shift, fc));
                }
            }
        }
        if text.is_empty() {
            return None;
        }
        if halign.is_empty() || halign == "general" {
            halign = match v {
                Value::Num(_) => "right".into(),
                Value::Bool(_) | Value::Err(_) => "center".into(),
                _ => "left".into(),
            };
            if p.show_formulas && is_formula {
                halign = "left".into();
            }
        }
        let wrap = xf.wrap || halign == "justify" || halign == "distributed" || xf.valign == "justify";
        if wrap {
            let w = bw - 2.0 * pad - indent;
            let (tw, th) = pen.set(&text, &font, z, Some(w));
            let _ = tw;
            let y = match xf.valign.as_str() {
                "top" => by + 1.0,
                "center" | "justify" | "distributed" => by + (bh - th) / 2.0,
                _ => by + bh - th - 1.0,
            };
            let align = match halign.as_str() {
                "center" | "centerContinuous" => PANGO_ALIGN_CENTER,
                "right" => PANGO_ALIGN_RIGHT,
                _ => PANGO_ALIGN_LEFT,
            };
            return Some(TextJob { text, font, zoom: z, color, x: bx + pad + if halign == "left" { indent } else { 0.0 }, y, clip: (bx, by, bw, bh), wrap: Some(w), align, covers: (c, c), fill: None, rotate: 0 });
        }
        let one_line = text.replace('\n', " ");
        let (mut tw, th) = pen.set(&one_line, &font, z, None);
        let avail = bw - 2.0 * pad - indent;
        let mut text = one_line;
        // numbers and dates that don't fit show #
        if hash && !matches!(v, Value::Str(_)) && tw > bw - 2.0 && !(p.show_formulas && is_formula) {
            let hw = pen.width("#", &font, z).max(1.0);
            let n = ((bw - 2.0) / hw).floor().max(0.0) as usize;
            text = "#".repeat(n);
            tw = n as f64 * hw;
            if n == 0 {
                return None;
            }
            halign = "center".into();
        }
        if let Some((pos, ch)) = fill_char {
            let cw = pen.width(&ch.to_string(), &font, z).max(1.0);
            let room = avail - tw;
            if room > cw && pos <= text.len() && text.is_char_boundary(pos) {
                let n = (room / cw).floor() as usize;
                text = format!("{}{}{}", &text[..pos], ch.to_string().repeat(n), &text[pos..]);
                tw += n as f64 * cw;
            }
        }
        if halign == "fill" {
            let n = ((avail / tw.max(1.0)).floor() as usize).max(1);
            text = text.repeat(n);
            tw *= n as f64;
            halign = "left".into();
        }
        let y = match xf.valign.as_str() {
            "top" => by + 1.0,
            "center" | "distributed" | "justify" => by + (bh - th) / 2.0,
            _ => by + bh - th - (1.0 * z).round(),
        };
        let rotate = xf.rotation;
        // where the text goes, and how far it may run into empty neighbours
        let is_text = matches!(v, Value::Str(_)) || (p.show_formulas && is_formula);
        let mut clip = (bx, by, bw, bh);
        let mut covers = (c, c);
        let x = match halign.as_str() {
            "right" => bx + bw - pad - tw - indent,
            "center" | "centerContinuous" => bx + (bw - tw) / 2.0,
            _ => bx + pad + indent,
        };
        if is_text && !merged && rotate == 0 {
            let need_right = x + tw > bx + bw;
            let need_left = x < bx;
            if need_right && halign != "right" {
                let mut cc = c + 1;
                let mut right = bx + bw;
                while right < x + tw && cc < MAX_COLS && empty(cc) {
                    let (_, w) = colpos(cc);
                    right += w;
                    covers.1 = cc;
                    cc += 1;
                }
                clip.2 = right - clip.0;
            }
            if need_left && halign != "left" {
                let mut cc = c;
                let mut left = bx;
                while left > x && cc > 0 && empty(cc - 1) {
                    cc -= 1;
                    let (cx, _) = colpos(cc);
                    left = cx;
                    covers.0 = cc;
                }
                clip.2 += clip.0 - left;
                clip.0 = left;
            }
        }
        // gridlines to hide: between the covered cells
        let covers = if covers.0 < covers.1 { (covers.0, covers.1) } else { (c, c) };
        Some(TextJob { text, font, zoom: z, color, x, y, clip, wrap: None, align: PANGO_ALIGN_LEFT, covers, fill: None, rotate })
    }

    unsafe fn paint_overlays(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)]) {
        if rows.is_empty() || cols.is_empty() {
            return;
        }
        let z = p.zoom;
        let sh = &self.sheets[s];
        let area = |rg: &Rect| -> (f64, f64, f64, f64) {
            let r2 = rg.r2.min(crate::cellref::MAX_ROWS - 1);
            let c2 = rg.c2.min(MAX_COLS - 1);
            // whole rows/columns: clamp the far edge to the screen
            let x1 = self.screen_x(s, p, g, rg.c1);
            let y1 = self.screen_y(s, p, g, rg.r1);
            let last_c = cols[cols.len() - 1].0;
            let last_r = rows[rows.len() - 1].0;
            let x2 = if c2 > last_c + 1 { p.width + 10.0 } else { self.screen_x(s, p, g, c2) + self.col_px(s, c2) * z };
            let y2 = if r2 > last_r + 1 { p.height + 10.0 } else { self.screen_y(s, p, g, r2) + self.row_px(s, r2) * z };
            (x1.max(-10.0), y1.max(-10.0), x2 - x1.max(-10.0), y2 - y1.max(-10.0))
        };
        let expand = |rg: &Rect| -> Rect {
            let mut rg = *rg;
            for m in &sh.merges {
                if m.intersects(&rg) {
                    rg = rg.union(m);
                }
            }
            rg
        };
        // spill range around the active cell's formula
        let (ar, ac) = p.active;
        for (a, rc) in sh.spills.iter() {
            if rc.contains(ar, ac) || *a == (ar, ac) {
                let (x, y, w, h) = area(rc);
                rgb(cr, SPILL);
                cairo_set_line_width(cr, 1.0);
                let d = [3.0f64, 2.0];
                cairo_set_dash(cr, d.as_ptr(), 2, 0.0);
                cairo_rectangle(cr, x.round() + 0.5, y.round() - 0.5, w - 1.0, h);
                cairo_stroke(cr);
                cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
            }
        }
        // the selection: shaded except the active cell, outlined
        let active_rect = expand(&Rect::cell(ar, ac));
        for rg in &p.sel {
            let rg = expand(rg);
            let (x, y, w, h) = area(&rg);
            cairo_save(cr);
            cairo_rectangle(cr, x, y, w, h);
            if rg.contains(ar, ac) {
                let (ax, ay, aw, ah) = area(&active_rect);
                cairo_rectangle(cr, ax, ay, aw, ah);
                cairo_set_fill_rule(cr, CAIRO_FILL_RULE_EVEN_ODD);
            }
            rgba(cr, 0x000000, 0.078);
            cairo_fill(cr);
            cairo_restore(cr);
        }
        if p.sel.len() == 1 {
            let rg = expand(&p.sel[0]);
            let (x, y, w, h) = area(&rg);
            rgb(cr, ACCENT);
            let t = (2.0 * z).round().max(2.0);
            rect(cr, x - t, y - t, w + t, t);
            rect(cr, x - t, y + h - t, w + t, t);
            rect(cr, x - t, y - t, t, h + t);
            rect(cr, x + w - t, y - t, t, h + t);
            // the fill handle
            let (hx, hy) = (x + w - t / 2.0, y + h - t / 2.0);
            rgb(cr, 0xFFFFFF);
            rect(cr, hx - 4.0, hy - 4.0, 8.0, 8.0);
            rgb(cr, ACCENT);
            rect(cr, hx - 3.0, hy - 3.0, 6.0, 6.0);
        } else {
            for rg in &p.sel {
                let (x, y, w, h) = area(&expand(rg));
                rgb(cr, ACCENT);
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, x - 0.5, y - 0.5, w, h);
                cairo_stroke(cr);
            }
            let (x, y, w, h) = area(&active_rect);
            rgb(cr, ACCENT);
            cairo_set_line_width(cr, 2.0);
            cairo_rectangle(cr, x, y, w - 1.0, h - 1.0);
            cairo_stroke(cr);
        }
        // where the fill handle is being dragged
        if let Some(ft) = &p.fill_target {
            let (x, y, w, h) = area(&expand(ft));
            rgb(cr, 0x7F7F7F);
            cairo_set_line_width(cr, 1.0);
            let d = [3.0f64, 2.0];
            cairo_set_dash(cr, d.as_ptr(), 2, 0.0);
            cairo_rectangle(cr, x - 0.5, y - 0.5, w, h);
            cairo_stroke(cr);
            cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
        }
        // the copy marquee
        if let Some(cp) = &p.copy {
            let (x, y, w, h) = area(&expand(cp));
            cairo_set_line_width(cr, 2.0);
            rgb(cr, 0xFFFFFF);
            cairo_rectangle(cr, x - 1.0, y - 1.0, w, h);
            cairo_stroke(cr);
            rgb(cr, ACCENT);
            let d = [4.0f64, 4.0];
            cairo_set_dash(cr, d.as_ptr(), 2, p.ants_phase);
            cairo_rectangle(cr, x - 1.0, y - 1.0, w, h);
            cairo_stroke(cr);
            cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
        }
        // references of the formula being edited
        for (rg, col) in &p.refs {
            let (x, y, w, h) = area(&expand(rg));
            rgba(cr, *col, 0.10);
            rect(cr, x, y, w, h);
            rgb(cr, *col);
            cairo_set_line_width(cr, (1.5 * z).max(1.5));
            cairo_rectangle(cr, x - 0.25, y - 0.25, w - 0.5, h - 0.5);
            cairo_stroke(cr);
            for (cx, cy) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
                rect(cr, cx - 2.5, cy - 2.5, 5.0, 5.0);
            }
        }
        // tracer arrows
        for (src, (tr, tc), col) in &p.arrows {
            let (sx, sy, sw, shh) = area(src);
            let (tx, ty, tw, th) = area(&expand(&Rect::cell(*tr, *tc)));
            if src.count() > 1 {
                rgb(cr, *col);
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, sx + 0.5, sy + 0.5, sw - 1.0, shh - 1.0);
                cairo_stroke(cr);
            }
            let (x0, y0) = if src.count() > 1 { (sx + 6.0, sy + 6.0) } else { (sx + sw / 2.0, sy + shh / 2.0) };
            let (x1, y1) = (tx + tw / 2.0, ty + th / 2.0);
            arrow(cr, *col, x0, y0, x1, y1);
        }
    }

    unsafe fn paint_headers(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, pen: &mut Pen) {
        let z = p.zoom;
        let sh = &self.sheets[s];
        let font = FontSpec { family: header_family(&self.styles), size: 11.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: HEADER_TEXT };
        // column headers
        rgb(cr, HEADER_BG);
        rect(cr, 0.0, 0.0, p.width, g.hh);
        rect(cr, 0.0, 0.0, g.hw, p.height);
        let sel_cols = |c: u32| p.sel.iter().any(|r| r.c1 <= c && c <= r.c2);
        let sel_rows = |r: u32| p.sel.iter().any(|x| x.r1 <= r && r <= x.r2);
        let whole_col = |c: u32| p.sel.iter().any(|r| r.c1 <= c && c <= r.c2 && r.r1 == 0 && r.r2 >= crate::cellref::MAX_ROWS - 1);
        let whole_row = |r: u32| p.sel.iter().any(|x| x.r1 <= r && r <= x.r2 && x.c1 == 0 && x.c2 >= MAX_COLS - 1);
        cairo_save(cr);
        cairo_rectangle(cr, g.hw, 0.0, p.width - g.hw, g.hh);
        cairo_clip(cr);
        for &(c, x, w) in &g.cols {
            let on = sel_cols(c);
            if on {
                rgb(cr, if whole_col(c) { 0xD2C9A8 } else { HEADER_SEL });
                rect(cr, x, 0.0, w, g.hh);
                rgb(cr, ACCENT);
                rect(cr, x, g.hh - 2.0, w, 2.0);
            }
            rgb(cr, GRID);
            rect(cr, (x + w - 1.0).round(), 0.0, 1.0, g.hh);
            let name = col_name(c);
            let (tw, th) = pen.set(&name, &font, z, None);
            rgb(cr, if on { ACCENT_TEXT } else { HEADER_TEXT });
            if tw < w {
                pen.show(cr, x + (w - tw) / 2.0, (g.hh - th) / 2.0);
            }
        }
        cairo_restore(cr);
        rgb(cr, GRID);
        rect(cr, 0.0, g.hh - 1.0, p.width, 1.0);
        // row headers
        cairo_save(cr);
        cairo_rectangle(cr, 0.0, g.hh, g.hw, p.height - g.hh);
        cairo_clip(cr);
        for &(r, y, h) in &g.rows {
            let on = sel_rows(r);
            if on {
                rgb(cr, if whole_row(r) { 0xD2C9A8 } else { HEADER_SEL });
                rect(cr, 0.0, y, g.hw, h);
                rgb(cr, ACCENT);
                rect(cr, g.hw - 2.0, y, 2.0, h);
            }
            rgb(cr, GRID);
            rect(cr, 0.0, (y + h - 1.0).round(), g.hw, 1.0);
            let label = (r + 1).to_string();
            let (tw, th) = pen.set(&label, &font, z, None);
            rgb(cr, if on { ACCENT_TEXT } else { HEADER_TEXT });
            if th <= h + 2.0 {
                pen.show(cr, (g.hw - tw) / 2.0, y + (h - th) / 2.0);
            }
        }
        cairo_restore(cr);
        rgb(cr, GRID);
        rect(cr, g.hw - 1.0, 0.0, 1.0, p.height);
        // the corner's select-all triangle
        rgb(cr, 0xC8C8C8);
        cairo_move_to(cr, g.hw - 4.0, g.hh - 4.0);
        cairo_line_to(cr, g.hw - 4.0, g.hh - 4.0 - 10.0 * z.min(1.5));
        cairo_line_to(cr, g.hw - 4.0 - 10.0 * z.min(1.5), g.hh - 4.0);
        cairo_close_path(cr);
        cairo_fill(cr);
        let _ = sh;
    }
}

fn header_family(styles: &crate::styles::Styles) -> String {
    let f = crate::layout::normal_font(styles);
    f.family
}

struct TextJob {
    text: String,
    font: FontSpec,
    zoom: f64,
    color: u32,
    x: f64,
    y: f64,
    clip: (f64, f64, f64, f64),
    wrap: Option<f64>,
    align: i32,
    covers: (u32, u32),
    fill: Option<char>,
    rotate: u32,
}

impl TextJob {
    unsafe fn draw(&self, cr: *mut cairo_t, pen: &mut Pen) {
        cairo_save(cr);
        let (cx, cy, cw, ch) = self.clip;
        cairo_rectangle(cr, cx, cy, cw - 1.0, ch - 1.0);
        cairo_clip(cr);
        rgb(cr, self.color);
        let (tw, th) = pen.set(&self.text, &self.font, self.zoom, self.wrap);
        pen.set_align(self.align);
        let mut y = self.y;
        if self.font.vert == 1 {
            y -= th * 0.35;
        } else if self.font.vert == 2 {
            y += th * 0.2;
        }
        if self.rotate != 0 && self.rotate <= 180 {
            let ang = if self.rotate <= 90 { -(self.rotate as f64) } else { (self.rotate - 90) as f64 };
            let (mx, my) = (cx + cw / 2.0, cy + ch / 2.0);
            cairo_translate(cr, mx, my);
            cairo_rotate(cr, ang.to_radians());
            pen.show(cr, -tw / 2.0, -th / 2.0);
        } else if self.rotate == 255 {
            // stacked letters
            let stacked: String = self.text.chars().map(|c| c.to_string()).collect::<Vec<_>>().join("\n");
            let (sw, _) = pen.set(&stacked, &self.font, self.zoom, None);
            pen.set_align(PANGO_ALIGN_CENTER);
            pen.show(cr, cx + (cw - sw) / 2.0, cy + 1.0);
        } else {
            pen.show(cr, self.x, y);
        }
        pen.set_align(PANGO_ALIGN_LEFT);
        let _ = self.fill;
        cairo_restore(cr);
    }
}

/// A border line in one of the spreadsheet line styles.
#[allow(clippy::too_many_arguments)]
unsafe fn line(cr: *mut cairo_t, style: &str, color: u32, x0: f64, y0: f64, x1: f64, y1: f64, z: f64) {
    let (w, dash): (f64, &[f64]) = match style {
        "thin" => (1.0, &[]),
        "medium" => (2.0, &[]),
        "thick" => (3.0, &[]),
        "hair" => (1.0, &[1.0, 1.0]),
        "dotted" => (1.0, &[1.0, 2.0]),
        "dashed" => (1.0, &[3.0, 1.0]),
        "mediumDashed" => (2.0, &[6.0, 2.0]),
        "dashDot" => (1.0, &[6.0, 2.0, 2.0, 2.0]),
        "mediumDashDot" | "slantDashDot" => (2.0, &[6.0, 2.0, 2.0, 2.0]),
        "dashDotDot" => (1.0, &[6.0, 2.0, 2.0, 2.0, 2.0, 2.0]),
        "mediumDashDotDot" => (2.0, &[6.0, 2.0, 2.0, 2.0, 2.0, 2.0]),
        "double" => (1.0, &[]),
        _ => (1.0, &[]),
    };
    rgb(cr, color);
    let w = (w * z).round().max(1.0);
    cairo_set_line_width(cr, w);
    if !dash.is_empty() {
        let d: Vec<f64> = dash.iter().map(|x| x * z.max(1.0)).collect();
        cairo_set_dash(cr, d.as_ptr(), d.len() as i32, 0.0);
    }
    let odd = (w as i64) % 2 == 1;
    let adj = |v: f64| if odd { v } else { v + 0.5 };
    if style == "double" {
        let (dx, dy) = if (y0 - y1).abs() < 0.01 { (0.0, 1.0) } else { (1.0, 0.0) };
        for k in [-1.0, 1.0] {
            cairo_move_to(cr, x0 + dx * k, y0 + dy * k);
            cairo_line_to(cr, x1 + dx * k, y1 + dy * k);
            cairo_stroke(cr);
        }
    } else {
        let (ax, ay, bx, by) = if (y0 - y1).abs() < 0.01 { (x0, adj(y0), x1, adj(y1)) } else if (x0 - x1).abs() < 0.01 { (adj(x0), y0, adj(x1), y1) } else { (x0, y0, x1, y1) };
        cairo_move_to(cr, ax, ay);
        cairo_line_to(cr, bx, by);
        cairo_stroke(cr);
    }
    cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
}

unsafe fn arrow(cr: *mut cairo_t, col: u32, x0: f64, y0: f64, x1: f64, y1: f64) {
    rgb(cr, col);
    cairo_set_line_width(cr, 1.5);
    cairo_arc(cr, x0, y0, 3.0, 0.0, std::f64::consts::TAU);
    cairo_fill(cr);
    let a = (y1 - y0).atan2(x1 - x0);
    let (hx, hy) = (x1 - 8.0 * a.cos(), y1 - 8.0 * a.sin());
    cairo_move_to(cr, x0, y0);
    cairo_line_to(cr, hx, hy);
    cairo_stroke(cr);
    cairo_move_to(cr, x1, y1);
    cairo_line_to(cr, x1 - 10.0 * (a - 0.4).cos(), y1 - 10.0 * (a - 0.4).sin());
    cairo_line_to(cr, x1 - 10.0 * (a + 0.4).cos(), y1 - 10.0 * (a + 0.4).sin());
    cairo_close_path(cr);
    cairo_fill(cr);
}

/// Render a sheet to a PNG file (tests and thumbnails).
pub fn render_png(book: &Book, s: usize, p: &ViewParams, path: &str) -> Result<(), String> {
    unsafe {
        let surf = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, p.width as i32, p.height as i32);
        let cr = cairo_create(surf);
        book.render(s, cr, p);
        cairo_destroy(cr);
        let c = cstr(path);
        let st = cairo_surface_write_to_png(surf, c.as_ptr());
        cairo_surface_destroy(surf);
        if st != 0 {
            return Err(format!("could not write {}", path));
        }
    }
    let _ = family_list("");
    Ok(())
}

/// Where a cell's filter button goes: (x, y, size).
pub fn filter_button(x: f64, y: f64, w: f64, h: f64, z: f64) -> (f64, f64, f64) {
    let bs = (16.0 * z).round().min(h - 2.0).min(w - 2.0).max(6.0);
    (x + w - bs - 2.0, y + h - bs - 2.0, bs)
}
