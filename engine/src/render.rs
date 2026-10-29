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

pub(crate) fn mix_pub(a: u32, b: u32, t: f64) -> u32 {
    mix(a, b, t)
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

pub(crate) struct Pen {
    pub(crate) layout: *mut PangoLayout,
    pub(crate) fonts: HashMap<(String, u64, bool, bool, u8), crate::layout::FontDesc>,
}

impl Pen {
    fn font(&mut self, f: &FontSpec, zoom: f64) -> *mut PangoFontDescription {
        let key = (f.family.clone(), (f.size * zoom * 100.0).round() as u64, f.bold, f.italic, f.vert);
        self.fonts.entry(key).or_insert_with(|| crate::layout::FontDesc::new(f, zoom)).0
    }

    /// Lay out text in a font; attributes for underline/strike. Returns the logical size.
    pub(crate) fn set(&mut self, text: &str, f: &FontSpec, zoom: f64, wrap: Option<f64>) -> (f64, f64) {
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

    pub(crate) fn width(&mut self, text: &str, f: &FontSpec, zoom: f64) -> f64 {
        self.set(text, f, zoom, None).0
    }

    pub(crate) fn show(&mut self, cr: *mut cairo_t, x: f64, y: f64) {
        unsafe {
            cairo_move_to(cr, x, y);
            pango_cairo_show_layout(cr, self.layout);
        }
    }

    pub(crate) fn set_align(&mut self, a: i32) {
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
        if p.page_layout {
            let f = self.file.path.as_deref().map(|x| x.rsplit('/').next().unwrap_or("").to_string()).unwrap_or_default();
            return self.pl_render(s, cr, p, &f);
        }
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
                if !p.print {
                    self.paint_background(s, cr, p, &g, (*x0, *y0, *x1, *y1));
                }
                self.paint_cells(s, cr, p, &g, &bands, &rows, &cols, &mut pen);
                if !p.print {
                    self.paint_filter_buttons(s, cr, p, &rows, &cols);
                    self.paint_pivot_placeholders(s, cr, p, &g, &mut pen);
                    if p.page_breaks {
                        self.paint_page_breaks(s, cr, p, &g, &rows, &cols, &mut pen);
                    }
                    self.paint_overlays(s, cr, p, &g, &rows, &cols);
                    self.paint_totals_button(s, cr, p, &rows, &cols);
                    self.paint_dv(s, cr, p, &rows, &cols);
                }
                self.paint_objects(s, cr, p, &g, &mut pen);
                self.paint_controls(s, cr, p, &g, &mut pen);
                cairo_restore(cr);
            }
        }
        // the freeze lines, or a split's bars (across the headers too)
        if p.freeze_rows > 0 && g.row_spans.len() > 1 {
            if p.split {
                rgb(cr, 0xD6D6D6);
                rect(cr, 0.0, g.row_spans[0].1 - 2.0, p.width, 4.0);
                rgb(cr, 0xA6A6A6);
                rect(cr, 0.0, g.row_spans[0].1 - 2.0, p.width, 1.0);
                rect(cr, 0.0, g.row_spans[0].1 + 1.0, p.width, 1.0);
            } else {
                rgb(cr, 0x9A9A9A);
                rect(cr, g.hw, g.row_spans[0].1 - 1.0, p.width - g.hw, 1.0);
            }
        }
        if p.freeze_cols > 0 && g.col_spans.len() > 1 {
            if p.split {
                rgb(cr, 0xD6D6D6);
                rect(cr, g.col_spans[0].1 - 2.0, 0.0, 4.0, p.height);
                rgb(cr, 0xA6A6A6);
                rect(cr, g.col_spans[0].1 - 2.0, 0.0, 1.0, p.height);
                rect(cr, g.col_spans[0].1 + 1.0, 0.0, 1.0, p.height);
            } else {
                rgb(cr, 0x9A9A9A);
                rect(cr, g.col_spans[0].1 - 1.0, g.hh, 1.0, p.height - g.hh);
            }
        }
        if !p.print {
            self.paint_notes(s, cr, p, &g, &mut pen);
            self.paint_object_selection(s, cr, p, &g);
        }
        if p.show_headers {
            self.paint_headers(s, cr, p, &g, &mut pen);
        }
        cairo_restore(cr);
    }

    /// Page Break Preview: gray outside what prints, blue lines between pages, page numbers.
    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_page_breaks(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)], pen: &mut Pen) {
        let pages = self.pages(s);
        let ps = self.page_setup(s);
        let z = p.zoom;
        let inside = |r: u32, c: u32| pages.iter().any(|pg| r >= pg.rows.0 && r <= pg.rows.1 && c >= pg.cols.0 && c <= pg.cols.1);
        for &(r, y, h) in rows {
            for &(c, x, w) in cols {
                if !inside(r, c) {
                    rgba(cr, 0x808080, 0.35);
                    rect(cr, x, y, w, h);
                }
            }
        }
        let f = FontSpec { family: self.styles.theme_fonts.1.clone(), size: 36.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: 0 };
        for pg in &pages {
            let x1 = self.screen_x(s, p, g, pg.cols.0);
            let x2 = self.screen_x(s, p, g, pg.cols.1 + 1);
            let y1 = self.screen_y(s, p, g, pg.rows.0);
            let y2 = self.screen_y(s, p, g, pg.rows.1 + 1);
            if x2 < g.hw || y2 < g.hh || x1 > p.width || y1 > p.height {
                continue;
            }
            // the page number, faint, in the middle of the page
            let label = format!("Page {}", pg.number);
            let (tw, th) = pen.set(&label, &f, z, None);
            rgba(cr, 0x7F7F7F, 0.35);
            pen.show(cr, (x1 + x2 - tw) / 2.0, (y1 + y2 - th) / 2.0);
            // manual breaks solid, automatic dashed
            let manual_r = ps.row_breaks.contains(&pg.rows.0) || ps.row_breaks.contains(&(pg.rows.1 + 1));
            let manual_c = ps.col_breaks.contains(&pg.cols.0) || ps.col_breaks.contains(&(pg.cols.1 + 1));
            rgb(cr, 0x2F6DB5);
            cairo_set_line_width(cr, 2.0 * z.max(0.5));
            let dash = [6.0f64, 3.0];
            cairo_set_dash(cr, dash.as_ptr(), if manual_c { 0 } else { 2 }, 0.0);
            cairo_move_to(cr, x1, y1);
            cairo_line_to(cr, x1, y2);
            cairo_move_to(cr, x2, y1);
            cairo_line_to(cr, x2, y2);
            cairo_stroke(cr);
            cairo_set_dash(cr, dash.as_ptr(), if manual_r { 0 } else { 2 }, 0.0);
            cairo_move_to(cr, x1, y1);
            cairo_line_to(cr, x2, y1);
            cairo_move_to(cr, x1, y2);
            cairo_line_to(cr, x2, y2);
            cairo_stroke(cr);
            cairo_set_dash(cr, dash.as_ptr(), 0, 0.0);
        }
    }

    /// Where a note's box goes on screen (x, y, w, h), beside its cell.
    pub fn note_box(&self, s: usize, p: &ViewParams, g: &Geometry, r: u32, c: u32) -> (f64, f64, f64, f64) {
        let z = p.zoom;
        let n = self.sheets[s].notes.get(&(r, c));
        let (w, h) = n.map(|n| (n.w, n.h)).unwrap_or((crate::notes::NOTE_W, crate::notes::NOTE_H));
        let x = self.screen_x(s, p, g, c + 1) + 15.0 * z;
        let y = (self.screen_y(s, p, g, r) - 10.0 * z).max(g.hh + 2.0);
        (x, y, w * z, h * z)
    }

    unsafe fn paint_notes(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, pen: &mut Pen) {
        let sh = &self.sheets[s];
        if sh.notes.is_empty() && sh.threads.is_empty() {
            return;
        }
        let z = p.zoom;
        cairo_save(cr);
        cairo_rectangle(cr, g.hw, g.hh, p.width - g.hw, p.height - g.hh);
        cairo_clip(cr);
        let on_screen = |r: u32, c: u32| g.rows.iter().any(|x| x.0 == r) && g.cols.iter().any(|x| x.0 == c);
        // the corner markers
        let mark = |r: u32, c: u32, col: u32| {
            if !on_screen(r, c) {
                return;
            }
            let (_, y, _) = *g.rows.iter().find(|x| x.0 == r).unwrap();
            let (_, x, w) = *g.cols.iter().find(|x| x.0 == c).unwrap();
            let k = (6.0 * z).max(4.0);
            rgb(cr, col);
            cairo_new_path(cr);
            cairo_move_to(cr, x + w - 1.0 - k, y);
            cairo_line_to(cr, x + w - 1.0, y);
            cairo_line_to(cr, x + w - 1.0, y + k);
            cairo_close_path(cr);
            cairo_fill(cr);
        };
        for (r, c) in sh.notes.keys() {
            mark(*r, *c, 0xE81123);
        }
        for ((r, c), t) in sh.threads.iter() {
            mark(*r, *c, if t.resolved { 0x9A9A9A } else { 0x7719AA });
        }
        // shown notes, and the one under the pointer
        let f = FontSpec { family: self.styles.theme_fonts.1.clone(), size: 9.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: 0 };
        let fb = FontSpec { bold: true, ..f.clone() };
        for ((r, c), n) in sh.notes.iter() {
            let show = n.visible || p.hover_note == Some((*r, *c));
            if !show || p.edit_note == Some((*r, *c)) {
                continue;
            }
            let (bx, by, bw, bh) = self.note_box(s, p, g, *r, *c);
            // the line to the cell
            let cx = self.screen_x(s, p, g, *c + 1) - 1.0;
            let cy = self.screen_y(s, p, g, *r);
            rgb(cr, 0x000000);
            cairo_set_line_width(cr, 1.0);
            cairo_move_to(cr, cx, cy + 1.0);
            cairo_line_to(cr, bx, by + 6.0 * z);
            cairo_stroke(cr);
            {
                let (dx, dy) = (cx - bx, cy + 1.0 - (by + 6.0 * z));
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let (ux, uy) = (dx / len, dy / len);
                let k = 6.0 * z;
                cairo_new_path(cr);
                cairo_move_to(cr, cx, cy + 1.0);
                cairo_line_to(cr, cx - ux * k - uy * k * 0.4, cy + 1.0 - uy * k + ux * k * 0.4);
                cairo_line_to(cr, cx - ux * k + uy * k * 0.4, cy + 1.0 - uy * k - ux * k * 0.4);
                cairo_close_path(cr);
                cairo_fill(cr);
            }
            rgba(cr, 0x000000, 0.25);
            rect(cr, bx + 3.0, by + 3.0, bw, bh);
            rgb(cr, 0xFFFFE1);
            rect(cr, bx, by, bw, bh);
            rgb(cr, 0x000000);
            cairo_rectangle(cr, bx.round() + 0.5, by.round() + 0.5, bw.round() - 1.0, bh.round() - 1.0);
            cairo_stroke(cr);
            cairo_save(cr);
            cairo_rectangle(cr, bx + 1.0, by + 1.0, bw - 2.0, bh - 2.0);
            cairo_clip(cr);
            let prefix = format!("{}:", n.author);
            let mut y = by + 3.0 * z;
            let (head, body) = if !n.author.is_empty() && n.text.starts_with(&prefix) { (Some(prefix.clone()), &n.text[prefix.len()..]) } else { (None, n.text.as_str()) };
            if let Some(hd) = head {
                let (_, th) = pen.set(&hd, &fb, z, Some(bw - 8.0 * z));
                rgb(cr, 0x000000);
                pen.show(cr, bx + 4.0 * z, y);
                y += th;
            }
            let body = body.strip_prefix('\n').unwrap_or(body);
            pen.set(body, &f, z, Some(bw - 8.0 * z));
            rgb(cr, 0x000000);
            pen.show(cr, bx + 4.0 * z, y);
            cairo_restore(cr);
        }
        cairo_restore(cr);
    }

    /// A form control's rectangle on screen.
    pub fn control_screen_rect(&self, s: usize, p: &ViewParams, g: &Geometry, c: &crate::controls::FormControl) -> (f64, f64, f64, f64) {
        let z = p.zoom;
        let x1 = self.screen_x(s, p, g, c.from.col) + c.from.col_off as f64 * z;
        let y1 = self.screen_y(s, p, g, c.from.row) + c.from.row_off as f64 * z;
        let x2 = self.screen_x(s, p, g, c.to.col) + c.to.col_off as f64 * z;
        let y2 = self.screen_y(s, p, g, c.to.row) + c.to.row_off as f64 * z;
        (x1, y1, (x2 - x1).max(1.0), (y2 - y1).max(1.0))
    }

    /// The form controls, as the desktop spreadsheet draws them.
    unsafe fn paint_controls(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, pen: &mut Pen) {
        let list = &self.sheets[s].controls;
        if list.is_empty() {
            return;
        }
        let z = p.zoom;
        let f = FontSpec { family: self.styles.theme_fonts.1.clone(), size: 9.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: 0 };
        let stroke_box = |x: f64, y: f64, w: f64, h: f64, col: u32| {
            rgb(cr, col);
            cairo_set_line_width(cr, 1.0);
            cairo_rectangle(cr, x.round() + 0.5, y.round() + 0.5, w.round() - 1.0, h.round() - 1.0);
            cairo_stroke(cr);
        };
        let arrow = |x: f64, y: f64, w: f64, h: f64, dir: u8| {
            // a small solid triangle in the box: 0 up, 1 down, 2 left, 3 right
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let k = (w.min(h) * 0.22).max(2.0);
            rgb(cr, 0x404040);
            cairo_new_path(cr);
            match dir {
                0 => { cairo_move_to(cr, cx - k, cy + k / 2.0); cairo_line_to(cr, cx + k, cy + k / 2.0); cairo_line_to(cr, cx, cy - k / 2.0); }
                1 => { cairo_move_to(cr, cx - k, cy - k / 2.0); cairo_line_to(cr, cx + k, cy - k / 2.0); cairo_line_to(cr, cx, cy + k / 2.0); }
                2 => { cairo_move_to(cr, cx + k / 2.0, cy - k); cairo_line_to(cr, cx + k / 2.0, cy + k); cairo_line_to(cr, cx - k / 2.0, cy); }
                _ => { cairo_move_to(cr, cx - k / 2.0, cy - k); cairo_line_to(cr, cx - k / 2.0, cy + k); cairo_line_to(cr, cx + k / 2.0, cy); }
            }
            cairo_close_path(cr);
            cairo_fill(cr);
        };
        for (i, c) in list.iter().enumerate() {
            let (x, y, w, h) = self.control_screen_rect(s, p, g, c);
            if x > p.width || y > p.height || x + w < 0.0 || y + h < 0.0 {
                continue;
            }
            if p.print && (c.kind == "Button" || c.kind == "GBox" || c.kind == "Label") {
                // Print Object is off for these by default
            }
            let v = self.control_value(s, i);
            cairo_save(cr);
            cairo_rectangle(cr, x - 1.0, y - 1.0, w + 2.0, h + 2.0);
            cairo_clip(cr);
            let text_at = |pen: &mut Pen, t: &str, tx: f64, ty_mid: f64, maxw: f64| {
                let (_, th) = pen.set(t, &f, z, Some(maxw.max(1.0)));
                rgb(cr, 0x000000);
                pen.show(cr, tx, ty_mid - th / 2.0);
            };
            match c.kind.as_str() {
                "Button" => {
                    rgb(cr, 0xE1E1E1);
                    rect(cr, x, y, w, h);
                    stroke_box(x, y, w, h, 0xADADAD);
                    let tw = pen.width(&c.text, &f, z);
                    text_at(pen, &c.text, x + ((w - tw) / 2.0).max(2.0 * z), y + h / 2.0, w - 4.0 * z);
                }
                "Checkbox" | "Radio" => {
                    let k = (13.0 * z).min(h - 2.0).max(6.0);
                    let bx = x + 2.0 * z;
                    let by = y + (h - k) / 2.0;
                    rgb(cr, 0xFFFFFF);
                    if c.kind == "Checkbox" {
                        rect(cr, bx, by, k, k);
                        stroke_box(bx, by, k, k, 0x333333);
                        if v == 1 {
                            rgb(cr, 0x000000);
                            cairo_set_line_width(cr, (1.6 * z).max(1.2));
                            cairo_move_to(cr, bx + k * 0.22, by + k * 0.52);
                            cairo_line_to(cr, bx + k * 0.42, by + k * 0.72);
                            cairo_line_to(cr, bx + k * 0.78, by + k * 0.28);
                            cairo_stroke(cr);
                        } else if v == 2 {
                            rgb(cr, 0x808080);
                            rect(cr, bx + k * 0.25, by + k * 0.25, k * 0.5, k * 0.5);
                        }
                    } else {
                        cairo_new_path(cr);
                        cairo_arc(cr, bx + k / 2.0, by + k / 2.0, k / 2.0 - 0.5, 0.0, std::f64::consts::TAU);
                        cairo_fill_preserve(cr);
                        rgb(cr, 0x333333);
                        cairo_set_line_width(cr, 1.0);
                        cairo_stroke(cr);
                        if v == 1 {
                            cairo_new_path(cr);
                            cairo_arc(cr, bx + k / 2.0, by + k / 2.0, k * 0.22, 0.0, std::f64::consts::TAU);
                            rgb(cr, 0x000000);
                            cairo_fill(cr);
                        }
                    }
                    text_at(pen, &c.text, bx + k + 4.0 * z, y + h / 2.0, w - k - 8.0 * z);
                }
                "Label" => {
                    let (_, _th) = pen.set(&c.text, &f, z, Some(w.max(1.0)));
                    rgb(cr, 0x000000);
                    pen.show(cr, x + 1.0, y + 1.0);
                }
                "GBox" => {
                    let (tw, th) = pen.set(&c.text, &f, z, None);
                    let top = y + th / 2.0;
                    rgb(cr, 0xA0A0A0);
                    cairo_set_line_width(cr, 1.0);
                    cairo_move_to(cr, x + 6.0 * z + tw + 3.0 * z, top.round() + 0.5);
                    cairo_line_to(cr, x + w - 0.5, top.round() + 0.5);
                    cairo_line_to(cr, x + w - 0.5, y + h - 0.5);
                    cairo_line_to(cr, x + 0.5, y + h - 0.5);
                    cairo_line_to(cr, x + 0.5, top.round() + 0.5);
                    cairo_line_to(cr, x + 4.0 * z, top.round() + 0.5);
                    cairo_stroke(cr);
                    rgb(cr, 0x000000);
                    pen.show(cr, x + 6.0 * z, y);
                }
                "Drop" => {
                    rgb(cr, 0xFFFFFF);
                    rect(cr, x, y, w, h);
                    stroke_box(x, y, w, h, 0x7A7A7A);
                    let bw = (16.0 * z).min(w / 2.0);
                    rgb(cr, 0xF0F0F0);
                    rect(cr, x + w - bw, y + 1.0, bw - 1.0, h - 2.0);
                    arrow(x + w - bw, y, bw, h, 1);
                    if v >= 1 {
                        let items = self.control_items(s, c);
                        if let Some(t) = items.get(v as usize - 1) {
                            text_at(pen, t, x + 3.0 * z, y + h / 2.0, w - bw - 6.0 * z);
                        }
                    }
                }
                "List" => {
                    rgb(cr, 0xFFFFFF);
                    rect(cr, x, y, w, h);
                    let items = self.control_items(s, c);
                    let (_, lh) = pen.set("Ag", &f, z, None);
                    let lh = lh.max(1.0);
                    let rows = (h / lh).floor().max(1.0) as usize;
                    let first = if v as usize > rows { v as usize - rows } else { 0 };
                    let sbw = if items.len() > rows { 16.0 * z } else { 0.0 };
                    for (k, t) in items.iter().enumerate().skip(first).take(rows + 1) {
                        let ty = y + (k - first) as f64 * lh;
                        if k as i64 + 1 == v {
                            rgb(cr, 0x0078D7);
                            rect(cr, x + 1.0, ty, w - sbw - 2.0, lh);
                            let (_, _) = pen.set(t, &f, z, Some((w - sbw - 6.0 * z).max(1.0)));
                            rgb(cr, 0xFFFFFF);
                            pen.show(cr, x + 3.0 * z, ty);
                        } else {
                            let (_, _) = pen.set(t, &f, z, Some((w - sbw - 6.0 * z).max(1.0)));
                            rgb(cr, 0x000000);
                            pen.show(cr, x + 3.0 * z, ty);
                        }
                    }
                    if sbw > 0.0 {
                        rgb(cr, 0xF0F0F0);
                        rect(cr, x + w - sbw, y, sbw, h);
                        arrow(x + w - sbw, y, sbw, sbw, 0);
                        arrow(x + w - sbw, y + h - sbw, sbw, sbw, 1);
                    }
                    stroke_box(x, y, w, h, 0x7A7A7A);
                }
                "Spin" => {
                    rgb(cr, 0xF0F0F0);
                    rect(cr, x, y, w, h);
                    if c.horiz || w > h * 1.5 {
                        stroke_box(x, y, w / 2.0, h, 0xADADAD);
                        stroke_box(x + w / 2.0, y, w / 2.0, h, 0xADADAD);
                        arrow(x, y, w / 2.0, h, 2);
                        arrow(x + w / 2.0, y, w / 2.0, h, 3);
                    } else {
                        stroke_box(x, y, w, h / 2.0, 0xADADAD);
                        stroke_box(x, y + h / 2.0, w, h / 2.0, 0xADADAD);
                        arrow(x, y, w, h / 2.0, 0);
                        arrow(x, y + h / 2.0, w, h / 2.0, 1);
                    }
                }
                "Scroll" => {
                    rgb(cr, 0xF0F0F0);
                    rect(cr, x, y, w, h);
                    stroke_box(x, y, w, h, 0xC8C8C8);
                    let horiz = c.horiz || w > h;
                    let span = (c.max - c.min).max(1) as f64;
                    let t = ((v - c.min.min(c.max)) as f64 / span).clamp(0.0, 1.0);
                    if horiz {
                        let k = h.min(w / 3.0);
                        arrow(x, y, k, h, 2);
                        arrow(x + w - k, y, k, h, 3);
                        let track = w - 2.0 * k;
                        let thumb = (track * 0.15).max(8.0 * z).min(track);
                        rgb(cr, 0xC1C1C1);
                        rect(cr, x + k + (track - thumb) * t, y + 2.0, thumb, h - 4.0);
                    } else {
                        let k = w.min(h / 3.0);
                        arrow(x, y, w, k, 0);
                        arrow(x, y + h - k, w, k, 1);
                        let track = h - 2.0 * k;
                        let thumb = (track * 0.15).max(8.0 * z).min(track);
                        rgb(cr, 0xC1C1C1);
                        rect(cr, x + 2.0, y + k + (track - thumb) * t, w - 4.0, thumb);
                    }
                }
                _ => {}
            }
            cairo_restore(cr);
        }
    }

    /// An object's rectangle on screen.
    pub fn obj_screen_rect(&self, s: usize, p: &ViewParams, g: &Geometry, o: &crate::drawing::Obj) -> (f64, f64, f64, f64) {
        use crate::drawing::{Anchor, EMU};
        let z = p.zoom;
        let pt = |m: &crate::drawing::Marker| (self.screen_x(s, p, g, m.col) + m.col_off as f64 / EMU * z, self.screen_y(s, p, g, m.row) + m.row_off as f64 / EMU * z);
        match &o.anchor {
            Anchor::Two { from, to, .. } => {
                let (x1, y1) = pt(from);
                let (x2, y2) = pt(to);
                (x1, y1, (x2 - x1).max(0.0), (y2 - y1).max(0.0))
            }
            Anchor::One { from, cx, cy } => {
                let (x1, y1) = pt(from);
                (x1, y1, *cx as f64 / EMU * z, *cy as f64 / EMU * z)
            }
            Anchor::Abs { x, y, cx, cy } => (g.hw + *x as f64 / EMU * z, g.hh + *y as f64 / EMU * z, *cx as f64 / EMU * z, *cy as f64 / EMU * z),
        }
    }

    /// The sheet's background picture, tiled from the sheet's top left (not printed).
    unsafe fn paint_background(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, band: (f64, f64, f64, f64)) {
        let pic = match &self.sheets[s].background {
            Some(x) => x.clone(),
            None => return,
        };
        let (px, pw, ph) = match pic.argb() {
            Some(x) => x,
            None => return,
        };
        let z = p.zoom;
        let (tw, th) = (pw as f64 * z, ph as f64 * z);
        if tw < 1.0 || th < 1.0 {
            return;
        }
        let ox = self.screen_x(s, p, g, 0);
        let oy = self.screen_y(s, p, g, 0);
        let surf = cairo_image_surface_create_for_data(px.as_ptr() as *mut u8, 0, pw as i32, ph as i32, (pw * 4) as i32);
        let (bx0, by0, bx1, by1) = band;
        let mut ty = oy + ((by0 - oy) / th).floor() * th;
        let mut n = 0;
        while ty < by1 && n < 4000 {
            let mut tx = ox + ((bx0 - ox) / tw).floor() * tw;
            while tx < bx1 && n < 4000 {
                cairo_save(cr);
                cairo_translate(cr, tx, ty);
                cairo_scale(cr, z, z);
                cairo_set_source_surface(cr, surf, 0.0, 0.0);
                cairo_rectangle(cr, 0.0, 0.0, pw as f64, ph as f64);
                cairo_fill(cr);
                cairo_restore(cr);
                tx += tw;
                n += 1;
            }
            ty += th;
        }
        cairo_surface_destroy(surf);
    }

    unsafe fn paint_objects(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, pen: &mut Pen) {
        for (oi, o) in self.sheets[s].objects.iter().enumerate() {
            let (x, y, w, h) = self.obj_screen_rect(s, p, g, o);
            if x > p.width || y > p.height || x + w < 0.0 || y + h < 0.0 || w < 1.0 || h < 1.0 {
                continue;
            }
            if crate::objects_ext::is_hidden(o) || (p.hide_ink && o.ink.is_some()) {
                continue;
            }
            self.paint_obj(s, oi, o, cr, x, y, w, h, p, pen);
        }
    }

    /// One object in its frame (a group: its members inside it).
    pub(crate) unsafe fn paint_obj(&self, s: usize, oi: usize, o: &crate::drawing::Obj, cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64,
                                   p: &ViewParams, pen: &mut Pen) {
        if !o.members.is_empty() {
            let (cx0, cy0, ccx, ccy, rot, fh, fv) = crate::objects_ext::group_frame(o);
            cairo_save(cr);
            cairo_translate(cr, x + w / 2.0, y + h / 2.0);
            if rot != 0.0 {
                cairo_rotate(cr, rot.to_radians());
            }
            cairo_scale(cr, if fh { -1.0 } else { 1.0 }, if fv { -1.0 } else { 1.0 });
            cairo_translate(cr, -w / 2.0, -h / 2.0);
            let sx = w / (ccx.max(1) as f64);
            let sy = h / (ccy.max(1) as f64);
            for m in &o.members {
                if crate::objects_ext::is_hidden(m) {
                    continue;
                }
                if let crate::drawing::Anchor::Abs { x: mx, y: my, cx, cy } = m.anchor {
                    let (qx, qy) = ((mx - cx0) as f64 * sx, (my - cy0) as f64 * sy);
                    self.paint_obj(s, oi, m, cr, qx, qy, cx as f64 * sx, cy as f64 * sy, p, pen);
                }
            }
            cairo_restore(cr);
            return;
        }
        if o.slicer.is_some() {
            self.paint_slicer(s, oi, cr, x, y, w, h, p.zoom, pen);
            return;
        }
        if let Some(pic) = &o.picture {
            if let Some((px, pw, ph)) = pic.argb() {
                cairo_save(cr);
                let surf = cairo_image_surface_create_for_data(px.as_ptr() as *mut u8, 0, pw as i32, ph as i32, (pw * 4) as i32);
                cairo_rectangle(cr, x, y, w, h);
                cairo_clip(cr);
                cairo_translate(cr, x, y);
                cairo_scale(cr, w / pw as f64, h / ph as f64);
                cairo_set_source_surface(cr, surf, 0.0, 0.0);
                cairo_paint(cr);
                cairo_restore(cr);
                cairo_surface_destroy(surf);
                return;
            }
        }
        if let Some(ink) = &o.ink {
            let (sx, sy) = (w / ink.w.max(1e-9), h / ink.h.max(1e-9));
            cairo_save(cr);
            for st in &ink.strokes {
                let lw = (st.width * p.zoom).max(0.5);
                let c = st.color;
                let (r, g2, b) = (((c >> 16) & 0xFF) as f64 / 255.0, ((c >> 8) & 0xFF) as f64 / 255.0, (c & 0xFF) as f64 / 255.0);
                // the highlighter darkens what is under it, so text stays readable
                cairo_set_operator(cr, if st.kind == crate::ink::HIGHLIGHTER { 14 } else { 2 });
                let alpha = match st.kind {
                    crate::ink::HIGHLIGHTER => 1.0,
                    crate::ink::PENCIL => 0.85,
                    _ => 1.0,
                };
                cairo_set_source_rgba(cr, r, g2, b, alpha);
                cairo_set_line_width(cr, lw);
                cairo_set_line_cap(cr, if st.kind == crate::ink::HIGHLIGHTER { 2 } else { 1 });
                cairo_set_line_join(cr, 1);
                for (k, &(px, py)) in st.pts.iter().enumerate() {
                    let (qx, qy) = (x + px * sx, y + py * sy);
                    if k == 0 {
                        cairo_move_to(cr, qx, qy);
                    } else {
                        cairo_line_to(cr, qx, qy);
                    }
                }
                if st.pts.len() == 1 {
                    cairo_rel_line_to(cr, 0.01, 0.0);
                }
                cairo_stroke(cr);
            }
            cairo_set_operator(cr, 2);
            cairo_restore(cr);
            return;
        }
        if o.chart.is_none() && o.raw.is_none() {
            if let Some(v) = crate::shape::ShapeView::parse(&o.body, &self.styles.theme) {
                let tfx = crate::shape::theme_effects_cached(self.styles.theme_xml.as_deref(), &self.styles.theme);
                let v = v.with_effects(&o.body, &tfx, &self.styles.theme);
                self.paint_shape(cr, &v, x, y, w, h, p.zoom, pen);
                return;
            }
        }
        if let Some(cx) = &o.chartex {
            self.paint_chartex(cx, cr, x, y, w, h, p.zoom, pen);
            return;
        }
        match &o.chart {
            Some(ch) => self.paint_chart(ch, cr, x, y, w, h, p.zoom, pen),
            None => {
                // pictures and shapes aren't drawn yet: a placeholder frame
                rgba(cr, 0x808080, 0.08);
                rect(cr, x, y, w, h);
                rgb(cr, 0xA0A0A0);
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0);
                cairo_stroke(cr);
            }
        }
    }

    /// A shape, text box or WordArt in its frame on screen.
    pub(crate) unsafe fn paint_shape(&self, cr: *mut cairo_t, v: &crate::shape::ShapeView, x: f64, y: f64, w: f64, h: f64, z: f64, pen: &mut Pen) {
        use crate::shape::Seg;
        cairo_save(cr);
        cairo_translate(cr, x + w / 2.0, y + h / 2.0);
        if v.rot != 0.0 {
            cairo_rotate(cr, v.rot.to_radians());
        }
        cairo_scale(cr, if v.flip_h { -1.0 } else { 1.0 }, if v.flip_v { -1.0 } else { 1.0 });
        cairo_translate(cr, -w / 2.0, -h / 2.0);
        let lw = v.line.map(|l| (l.1 * 96.0 / 72.0 * z).max(0.5)).unwrap_or(0.0);
        let mut last_dir: Option<((f64, f64), (f64, f64))> = None;
        let mut first_dir: Option<((f64, f64), (f64, f64))> = None;
        let subs = crate::shape::preset(v, w, h);
        let trace = |sub: &crate::shape::SubPath| {
            cairo_new_path(cr);
            for sg in &sub.segs {
                match *sg {
                    Seg::Move(a, b) => cairo_move_to(cr, a, b),
                    Seg::Line(a, b) => cairo_line_to(cr, a, b),
                    Seg::Curve(a, b, c, d, e, f) => cairo_curve_to(cr, a, b, c, d, e, f),
                    Seg::Close => cairo_close_path(cr),
                }
            }
        };
        // shapes made of overlapping parts (a cloud) show one outline round all of them
        if matches!(v.geom.as_str(), "cloud" | "cloudCallout" | "ellipseCallout") {
            if let Some((c, _, _)) = v.line {
                rgb(cr, c);
                cairo_set_line_width(cr, lw * 2.0);
                for sub in &subs {
                    trace(sub);
                    cairo_stroke(cr);
                }
            }
            for sub in &subs {
                trace(sub);
                if let Some((c, a)) = v.fill {
                    rgba(cr, c, a);
                    cairo_fill(cr);
                }
            }
            cairo_new_path(cr);
        }
        let union = matches!(v.geom.as_str(), "cloud" | "cloudCallout" | "ellipseCallout");
        // effects under the shape: its shadow (softened in a few passes) and glow
        let pxz = 96.0 / 72.0 * z;
        if let Some((blur, dist, dir, alpha)) = v.fx.shadow {
            let (dx, dy) = (dist * pxz * dir.to_radians().cos(), dist * pxz * dir.to_radians().sin());
            let b = (blur * pxz).max(0.0);
            let passes = if b > 1.0 { 4 } else { 1 };
            cairo_save(cr);
            cairo_translate(cr, dx, dy);
            for k in 0..passes {
                let grow = b * (passes - k) as f64 / passes as f64;
                for sub in subs.iter().filter(|s| s.fill || v.fill.is_none()) {
                    trace(sub);
                    rgba(cr, 0x000000, alpha / passes as f64);
                    if grow > 0.5 {
                        cairo_set_line_width(cr, grow);
                        cairo_set_line_join(cr, 1);
                        cairo_fill_preserve(cr);
                        cairo_stroke(cr);
                    } else {
                        cairo_fill(cr);
                    }
                }
            }
            cairo_restore(cr);
        }
        if let Some((rad, col, alpha)) = v.fx.glow {
            let col = col.or(v.fill.map(|f| f.0)).or(v.line.map(|l| l.0)).unwrap_or(0x4472C4);
            let r = rad * pxz;
            for k in 1..=4 {
                for sub in &subs {
                    trace(sub);
                    rgba(cr, col, alpha / 4.0);
                    cairo_set_line_width(cr, 2.0 * r * k as f64 / 4.0);
                    cairo_set_line_join(cr, 1);
                    cairo_stroke(cr);
                }
            }
        }
        let fx_inner = v.fx.inner;
        let fx_edge = v.fx.soft_edge;
        let fx_bevel = v.fx.bevel;
        for sub in subs.into_iter().filter(|_| !union) {
            cairo_new_path(cr);
            let mut prev = (0.0, 0.0);
            for sg in &sub.segs {
                match *sg {
                    Seg::Move(a, b) => {
                        cairo_move_to(cr, a, b);
                        prev = (a, b);
                    }
                    Seg::Line(a, b) => {
                        cairo_line_to(cr, a, b);
                        if first_dir.is_none() {
                            first_dir = Some(((a, b), prev));
                        }
                        last_dir = Some((prev, (a, b)));
                        prev = (a, b);
                    }
                    Seg::Curve(a, b, c, d, e, f) => {
                        cairo_curve_to(cr, a, b, c, d, e, f);
                        last_dir = Some(((c, d), (e, f)));
                        prev = (e, f);
                    }
                    Seg::Close => cairo_close_path(cr),
                }
            }
            if sub.fill {
                if let Some((c, a)) = v.fill {
                    cairo_set_fill_rule(cr, 1);
                    rgba(cr, c, a);
                    cairo_fill_preserve(cr);
                    // inner shadow, bevel light and soft edges, inside the shape
                    if fx_inner.is_some() || fx_bevel || fx_edge.is_some() {
                        cairo_save(cr);
                        cairo_clip_preserve(cr);
                        if let Some((blur, dist, dir, alpha)) = fx_inner {
                            let (dx, dy) = (dist * pxz * dir.to_radians().cos(), dist * pxz * dir.to_radians().sin());
                            let bw = (blur * pxz).max(2.0);
                            for k in 1..=3 {
                                cairo_save(cr);
                                cairo_translate(cr, dx, dy);
                                rgba(cr, 0x000000, alpha / 3.0);
                                cairo_set_line_width(cr, bw * k as f64 / 1.5);
                                cairo_stroke_preserve(cr);
                                cairo_restore(cr);
                            }
                        }
                        if let Some(r) = fx_edge {
                            let r = (r * pxz).max(1.0);
                            for k in 1..=3 {
                                rgba(cr, 0xFFFFFF, 0.3);
                                cairo_set_line_width(cr, 2.0 * r * k as f64 / 3.0);
                                cairo_stroke_preserve(cr);
                            }
                        }
                        if fx_bevel {
                            // light from the top: a highlight fading down, a shade along the bottom
                            cairo_new_path(cr);
                            let bh = (h * 0.18).max(2.0);
                            for k in 0..4 {
                                let t = k as f64 / 4.0;
                                rgba(cr, 0xFFFFFF, 0.12 * (1.0 - t));
                                rect(cr, 0.0, bh * t, w, bh / 4.0);
                                rgba(cr, 0x000000, 0.07 * (1.0 - t));
                                rect(cr, 0.0, h - bh * t - bh / 4.0, w, bh / 4.0);
                            }
                        }
                        cairo_restore(cr);
                        // the outline goes round the shape's own path again
                        trace(&sub);
                    }
                }
            }
            if let Some((c, _, dashed)) = v.line {
                rgb(cr, c);
                cairo_set_line_width(cr, lw);
                cairo_set_line_join(cr, 1);
                if dashed {
                    let d = [lw * 4.0, lw * 3.0];
                    cairo_set_dash(cr, d.as_ptr(), 2, 0.0);
                }
                cairo_stroke(cr);
                cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
            } else {
                cairo_new_path(cr);
            }
        }
        // arrowheads on lines
        if let Some((c, _, _)) = v.line {
            let mut head = |from: (f64, f64), to: (f64, f64)| {
                let (dx, dy) = (to.0 - from.0, to.1 - from.1);
                let l = (dx * dx + dy * dy).sqrt();
                if l < 1e-6 {
                    return;
                }
                let (ux, uy) = (dx / l, dy / l);
                let s = (lw * 3.0).max(6.0 * z);
                rgb(cr, c);
                cairo_new_path(cr);
                cairo_move_to(cr, to.0, to.1);
                cairo_line_to(cr, to.0 - ux * s - uy * s * 0.6, to.1 - uy * s + ux * s * 0.6);
                cairo_line_to(cr, to.0 - ux * s + uy * s * 0.6, to.1 - uy * s - ux * s * 0.6);
                cairo_close_path(cr);
                cairo_fill(cr);
            };
            if v.tail {
                if let Some((a, b)) = last_dir {
                    head(a, b);
                }
            }
            if v.head {
                if let Some((a, b)) = first_dir {
                    head(a, b);
                }
            }
        }
        // an equation
        if let Some((m, size)) = &v.math {
            let k = 96.0 / 72.0 * z;
            let (l, t, r, b) = (v.insets.0 * k, v.insets.1 * k, v.insets.2 * k, v.insets.3 * k);
            let mut ms = crate::math::PangoMeasure::new(cr);
            let bx = crate::math::layout(m, &mut ms, size * k);
            let color = v.paras.iter().flat_map(|p| p.runs.first()).map(|r| r.color).next().unwrap_or(0);
            let avail = (w - l - r).max(1.0);
            let align = v.paras.first().map(|p| p.align).unwrap_or(1);
            let tx = l + (avail - bx.w) * align as f64 / 2.0;
            let hh = bx.asc + bx.desc;
            let ty = match v.anchor {
                'c' => (h - hh) / 2.0,
                'b' => h - b - hh,
                _ => t,
            } + bx.asc;
            crate::math::paint(cr, &mut ms, &bx, tx, ty, color);
        } else
        // the text
        if !v.paras.is_empty() && v.paras.iter().any(|p| p.runs.iter().any(|r| !r.text.is_empty())) {
            let k = 96.0 / 72.0 * z;
            let (l, t, r, b) = (v.insets.0 * k, v.insets.1 * k, v.insets.2 * k, v.insets.3 * k);
            let avail = (w - l - r).max(1.0);
            let mut lines: Vec<(f64, f64, i32, crate::layout::FontSpec, String, Option<(u32, f64)>)> = Vec::new();
            let mut total = 0.0;
            for para in &v.paras {
                let text: String = para.runs.iter().map(|r| r.text.as_str()).collect();
                let run = para.runs.first();
                let f = crate::layout::FontSpec {
                    family: run.and_then(|r| r.font.clone()).unwrap_or_else(|| self.styles.theme_fonts.1.clone()),
                    size: run.map(|r| r.size).unwrap_or(11.0),
                    bold: run.map(|r| r.bold).unwrap_or(false),
                    italic: run.map(|r| r.italic).unwrap_or(false),
                    underline: if run.map(|r| r.underline).unwrap_or(false) { 1 } else { 0 },
                    strike: false,
                    vert: 0,
                    color: run.map(|r| r.color).unwrap_or(0),
                };
                let (tw, th) = pen.set(if text.is_empty() { " " } else { &text }, &f, z, if v.wrap { Some(avail) } else { None });
                total += th;
                lines.push((tw, th, para.align, f, text, run.and_then(|r| r.outline)));
            }
            let mut ty = match v.anchor {
                'c' => (h - total) / 2.0,
                'b' => h - b - total,
                _ => t,
            };
            for (tw, th, align, f, text, outline) in lines {
                if !text.is_empty() {
                    pen.set(&text, &f, z, if v.wrap { Some(avail) } else { None });
                    let tx = if v.wrap {
                        pen.set_align(align);
                        l
                    } else {
                        pen.set_align(0);
                        l + (avail - tw) * align as f64 / 2.0
                    };
                    cairo_move_to(cr, tx, ty);
                    match outline {
                        Some((oc, ow)) => {
                            pango_cairo_layout_path(cr, pen.layout);
                            rgb(cr, f.color);
                            cairo_fill_preserve(cr);
                            rgb(cr, oc);
                            cairo_set_line_width(cr, (ow * k).max(0.6));
                            cairo_stroke(cr);
                        }
                        None => {
                            rgb(cr, f.color);
                            pango_cairo_show_layout(cr, pen.layout);
                        }
                    }
                    pen.set_align(0);
                }
                ty += th;
            }
        }
        cairo_restore(cr);
    }

    unsafe fn paint_object_selection(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry) {
        let sh = &self.sheets[s];
        cairo_save(cr);
        cairo_rectangle(cr, g.hw, g.hh, p.width - g.hw, p.height - g.hh);
        cairo_clip(cr);
        for &i in &p.sel_objects {
            if let Some(o) = sh.objects.get(i) {
                let (x, y, w, h) = self.obj_screen_rect(s, p, g, o);
                rgb(cr, ACCENT);
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, x.round() + 0.5, y.round() + 0.5, w.round() - 1.0, h.round() - 1.0);
                cairo_stroke(cr);
                for (hx, hy) in handles(x, y, w, h) {
                    rgb(cr, 0xFFFFFF);
                    cairo_arc(cr, hx, hy, 4.0, 0.0, 2.0 * std::f64::consts::PI);
                    cairo_fill_preserve(cr);
                    rgb(cr, ACCENT);
                    cairo_stroke(cr);
                }
            }
        }
        if let Some((x, y, w, h)) = p.obj_preview {
            rgb(cr, 0x404040);
            cairo_set_line_width(cr, 1.0);
            let d = [3.0f64, 3.0];
            cairo_set_dash(cr, d.as_ptr(), 2, 0.0);
            cairo_rectangle(cr, x.round() + 0.5, y.round() + 0.5, w.round(), h.round());
            cairo_stroke(cr);
            cairo_set_dash(cr, d.as_ptr(), 0, 0.0);
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
        // ---- conditional formats of the cells in view
        let blocks = self.cf_blocks(s);
        let mut cfm: HashMap<(u32, u32), crate::cf::CfOut> = HashMap::new();
        if blocks.iter().any(|b| b.ranges.iter().any(|x| x.intersects(&view))) {
            for &(r, _, _) in rows {
                for &(c, _, _) in cols {
                    if let Some(o) = self.cf_at(s, r, c, &blocks) {
                        cfm.insert((r, c), o);
                    }
                }
            }
        }
        // ---- table styles of the cells in view
        let mut tcm: HashMap<(u32, u32), crate::tables::TCell> = HashMap::new();
        for tb in sh.tables.iter().filter(|x| x.rect.intersects(&view) && x.style.is_some()) {
            for &(r, _, _) in rows {
                for &(c, _, _) in cols {
                    if tb.rect.contains(r, c) {
                        tcm.insert((r, c), self.table_cell(tb, r, c));
                    }
                }
            }
        }
        // ---- pivot styles
        for pv in self.pivots.iter().filter(|x| x.sheet == s && x.out.map(|o| o.intersects(&view)).unwrap_or(false)) {
            for &(r, _, _) in rows {
                for &(c, _, _) in cols {
                    if pv.roles.contains_key(&(r, c)) {
                        if let Some(t) = self.pivot_cell(pv, r, c) {
                            tcm.insert((r, c), t);
                        }
                    }
                }
            }
        }
        // ---- fills
        let mut filled: HashMap<(u32, u32), u32> = HashMap::new();
        for &(r, y, h) in rows {
            for &(c, x, w) in cols {
                let (xf, (fx, fy, fw, fh)) = match merge_of(r, c) {
                    Some(m) => (sh.style_at(m.r1, m.c1), mrect(&m)),
                    None => (sh.style_at(r, c), (x, y, w, h)),
                };
                let cf_fill = cfm.get(&match merge_of(r, c) { Some(m) => (m.r1, m.c1), None => (r, c) }).and_then(|o| o.scale.or(o.dxf.fill));
                let t_fill = tcm.get(&(r, c)).and_then(|x| x.fill);
                if xf == 0 && cf_fill.is_none() && t_fill.is_none() {
                    continue;
                }
                if let Some(col) = cf_fill.or_else(|| self.fill_color(self.styles.xf(xf))).or(t_fill) {
                    filled.insert((r, c), col);
                    if merge_of(r, c).map(|m| (m.r1.max(rmin), m.c1.max(cmin)) == (r, c)).unwrap_or(true) {
                        rgb(cr, col);
                        rect(cr, fx, fy, fw, fh);
                    }
                }
            }
        }
        // ---- data bars and icons
        for &(r, y, h) in rows {
            for &(c, x, w) in cols {
                let o = match cfm.get(&(r, c)) {
                    Some(o) => o,
                    None => continue,
                };
                if let Some((a, b, bar, neg, axis)) = &o.bar {
                    let (bx, bw) = (x + 1.0 * z, (w - 3.0 * z).max(1.0));
                    let (by, bh) = (y + 2.0 * z, (h - 5.0 * z).max(1.0));
                    let (fa, fb) = if bar.rtl { (1.0 - b, 1.0 - a) } else { (*a, *b) };
                    let x0 = bx + fa * bw;
                    let x1 = bx + fb * bw;
                    let col = if *neg { bar.neg_color.unwrap_or(bar.color) } else { bar.color };
                    if bar.gradient {
                        let pat = cairo_pattern_create_linear(if neg ^ bar.rtl { x1 } else { x0 }, 0.0, if neg ^ bar.rtl { x0 } else { x1 }, 0.0);
                        let ch = |c: u32, k: u32| ((c >> k) & 255) as f64 / 255.0;
                        cairo_pattern_add_color_stop_rgb(pat, 0.0, ch(col, 16), ch(col, 8), ch(col, 0));
                        let light = crate::cf::mix(col, 0xFFFFFF, 0.9);
                        cairo_pattern_add_color_stop_rgb(pat, 1.0, ch(light, 16), ch(light, 8), ch(light, 0));
                        cairo_set_source(cr, pat);
                        cairo_rectangle(cr, x0, by, (x1 - x0).max(0.0), bh);
                        cairo_fill(cr);
                        cairo_pattern_destroy(pat);
                    } else {
                        rgb(cr, col);
                        rect(cr, x0, by, (x1 - x0).max(0.0), bh);
                    }
                    let border = if *neg { bar.neg_border.or(bar.border) } else { bar.border };
                    if let Some(bc) = border.or(if bar.gradient && bar.min_len == 10.0 { None } else { None }) {
                        rgb(cr, bc);
                        cairo_set_line_width(cr, 1.0);
                        cairo_rectangle(cr, x0.round() + 0.5, by.round() + 0.5, (x1 - x0 - 1.0).max(0.0), bh.round() - 1.0);
                        cairo_stroke(cr);
                    }
                    if let Some(ax) = axis {
                        let axx = (bx + if bar.rtl { 1.0 - ax } else { *ax } * bw).round() + 0.5;
                        rgb(cr, bar.axis_color);
                        cairo_set_line_width(cr, 1.0);
                        let d = [2.0f64, 1.0];
                        cairo_set_dash(cr, d.as_ptr(), 2, 0.0);
                        cairo_move_to(cr, axx, y);
                        cairo_line_to(cr, axx, y + h - 1.0);
                        cairo_stroke(cr);
                        cairo_set_dash(cr, d.as_ptr(), 0, 0.0);
                    }
                }
                if let Some((set, i)) = &o.icon {
                    let sz = (h - 4.0 * z).min(16.0 * z).max(4.0);
                    crate::cf_icons::draw_icon(cr, set, *i, x + 2.0 * z, y + (h - sz) / 2.0, sz);
                }
            }
        }
        // ---- sparklines
        let sgroups = self.sparkline_groups_cached(s);
        for gr in sgroups.iter() {
            let shared = if gr.min_type == "group" || gr.max_type == "group" { self.group_bounds(s, gr) } else { None };
            let xs = self.date_positions(s, gr);
            let rg = gr.resolved(|c| self.styles.resolve(c));
            for (f, (r, c)) in &gr.items {
                let (yy, hh) = match rows.iter().find(|x| x.0 == *r) { Some(x) => (x.1, x.2), None => continue };
                let (xx, ww) = match cols.iter().find(|x| x.0 == *c) { Some(x) => (x.1, x.2), None => continue };
                let vals = self.group_values(s, gr, f);
                let range = crate::sparkline::bounds(gr, &vals, shared);
                cairo_save(cr);
                cairo_rectangle(cr, xx, yy, ww, hh);
                cairo_clip(cr);
                crate::sparkline::paint_with(cr, &rg, &vals, range, xs.as_deref(), xx, yy, ww, hh, z);
                cairo_restore(cr);
            }
        }
        // ---- text (decides which gridlines overflowing text hides)
        let mut hide_v: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        let mut texts: Vec<TextJob> = Vec::new();
        for &(r, y, h) in rows {
            // cells on this row, including some to the left whose text may run into view
            let from = cmin.saturating_sub(64);
            let mut to = (cmax + 64).min(MAX_COLS - 1);
            if p.freeze_cols > 0 && cmax < p.freeze_cols {
                // the frozen (left) pane: columns of the scrolling pane belong there, not here
                to = to.min(p.freeze_cols - 1);
            }
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
                let mut cfo = if cfm.is_empty() { None } else { cfm.get(&(r, c)).cloned().or_else(|| if bands.cols.contains_key(&c) { None } else { self.cf_at(s, r, c, &blocks) }) };
                if let Some(tc) = tcm.get(&(r, c)) {
                    // the table style under the cell's own font colour and the rules
                    let mut o = cfo.take().unwrap_or_default();
                    let own = crate::layout::font_of(&self.styles, xf).color;
                    if o.dxf.color.is_none() && own == 0x000000 {
                        o.dxf.color = tc.color;
                    }
                    if o.dxf.bold.is_none() && tc.bold {
                        o.dxf.bold = Some(true);
                    }
                    cfo = Some(o);
                }
                if cfo.as_ref().map(|o| o.hide_value).unwrap_or(false) {
                    continue;
                }
                if self.styles.xf(xf).checkbox && matches!(v, Value::Bool(_) | Value::Empty) {
                    // a cell check box (Insert > Checkbox): the box in place of TRUE / FALSE
                    let fs = font_of(&self.styles, xf);
                    let k = (fs.size * 1.25 * z).min(bh - 2.0).min(bw - 2.0).max(4.0);
                    let x0 = match self.styles.xf(xf).halign.as_str() {
                        "left" => bx + 3.0 * z,
                        "right" => bx + bw - k - 3.0 * z,
                        _ => bx + (bw - k) / 2.0,
                    };
                    let y0 = match self.styles.xf(xf).valign.as_str() {
                        "top" => by + 2.0 * z,
                        "center" => by + (bh - k) / 2.0,
                        _ => by + bh - k - 2.0 * z,
                    };
                    let on = matches!(v, Value::Bool(true));
                    let col = if fs.color == 0 { 0x000000 } else { fs.color };
                    if on {
                        rgb(cr, col);
                        rect(cr, x0, y0, k, k);
                        rgb(cr, 0xFFFFFF);
                        cairo_set_line_width(cr, (k * 0.13).max(1.2));
                        cairo_move_to(cr, x0 + k * 0.22, y0 + k * 0.52);
                        cairo_line_to(cr, x0 + k * 0.42, y0 + k * 0.72);
                        cairo_line_to(cr, x0 + k * 0.78, y0 + k * 0.3);
                        cairo_stroke(cr);
                    } else {
                        rgb(cr, col);
                        cairo_set_line_width(cr, 1.0);
                        cairo_rectangle(cr, x0.round() + 0.5, y0.round() + 0.5, k.round() - 1.0, k.round() - 1.0);
                        cairo_stroke(cr);
                    }
                    continue;
                }
                let icon_pad = if cfo.as_ref().map(|o| o.icon.is_some()).unwrap_or(false) { ((h - 4.0 * z).min(16.0 * z) + 3.0 * z).max(0.0) } else { 0.0 };
                let job = self.layout_text(s, r, c, &v, xf, is_f, p, (bx + icon_pad, by, (bw - icon_pad).max(1.0), bh), merged.is_some(), cfo.as_ref(), pen, &|cc: u32| {
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
        // ---- table borders (the cells' own come over them)
        for (&(r, c), tc) in tcm.iter() {
            let (x, w) = match bands.cols.get(&c) { Some(v) => *v, None => continue };
            let (y, h) = match bands.rows.get(&r) { Some(v) => *v, None => continue };
            let b = &tc.borders;
            if let Some((st, col)) = b[0] { line(cr, st, col, x - 0.5, y, x - 0.5, y + h, z); }
            if let Some((st, col)) = b[1] { line(cr, st, col, x + w - 0.5, y, x + w - 0.5, y + h, z); }
            if let Some((st, col)) = b[2] { line(cr, st, col, x - 1.0, y - 0.5, x + w, y - 0.5, z); }
            if let Some((st, col)) = b[3] { line(cr, st, col, x - 1.0, y + h - 0.5, x + w, y + h - 0.5, z); }
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
        // conditional borders
        for (&(r, c), o) in cfm.iter() {
            if o.dxf.borders.iter().all(|b| b.is_none()) {
                continue;
            }
            let (x, w) = match bands.cols.get(&c) { Some(v) => *v, None => continue };
            let (y, h) = match bands.rows.get(&r) { Some(v) => *v, None => continue };
            let b = &o.dxf.borders;
            if let Some((st, col)) = &b[0] { line(cr, st, *col, x - 0.5, y, x - 0.5, y + h, z); }
            if let Some((st, col)) = &b[1] { line(cr, st, *col, x + w - 0.5, y, x + w - 0.5, y + h, z); }
            if let Some((st, col)) = &b[2] { line(cr, st, *col, x - 1.0, y - 0.5, x + w, y - 0.5, z); }
            if let Some((st, col)) = &b[3] { line(cr, st, *col, x - 1.0, y + h - 0.5, x + w, y + h - 0.5, z); }
        }
    }

    /// The AutoFilter drop-down buttons in the header row.
    /// The filter drop-downs: (header row, first column, last column, filtered columns).
    pub fn filter_headers(&self, s: usize) -> Vec<(u32, u32, u32, Vec<u32>)> {
        let sh = &self.sheets[s];
        let mut out = Vec::new();
        if let Some(af) = &sh.autofilter {
            out.push((af.rect.r1, af.rect.c1, af.rect.c2, af.cols.iter().map(|f| af.rect.c1 + f.col).collect()));
        }
        for t in &sh.tables {
            if t.filter_button && t.header {
                let f = t.filter.as_ref().map(|f| f.cols.iter().map(|x| t.rect.c1 + x.col).collect()).unwrap_or_default();
                out.push((t.rect.r1, t.rect.c1, t.rect.c2, f));
            }
        }
        out
    }

    unsafe fn paint_totals_button(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)]) {
        // the drop-down of the active cell in a table's total row
        let (ar, ac) = p.active;
        if let Some(t) = self.sheets[s].tables.iter().find(|t| t.totals_row() == Some(ar) && ac >= t.rect.c1 && ac <= t.rect.c2) {
            let _ = t;
            if let (Some(row), Some(col)) = (rows.iter().find(|x| x.0 == ar), cols.iter().find(|x| x.0 == ac)) {
                let (_, by, bs) = filter_button(0.0, row.1, 40.0, row.2, p.zoom);
                let bx = col.1 + col.2 + 2.0;
                rgb(cr, 0xFFFFFF);
                rect(cr, bx, by, bs, bs);
                rgb(cr, 0xA0A0A0);
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, bx + 0.5, by + 0.5, bs - 1.0, bs - 1.0);
                cairo_stroke(cr);
                rgb(cr, 0x444444);
                let (cx, cy) = (bx + bs / 2.0, by + bs / 2.0);
                cairo_move_to(cr, cx - bs * 0.25, cy - bs * 0.1);
                cairo_line_to(cr, cx + bs * 0.25, cy - bs * 0.1);
                cairo_line_to(cr, cx, cy + bs * 0.18);
                cairo_close_path(cr);
                cairo_fill(cr);
            }
        }
    }

    /// The validation list's drop-down beside the active cell, and Circle Invalid Data's rings.
    unsafe fn paint_dv(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)]) {
        let (ar, ac) = p.active;
        if p.editing.is_none() && self.dv_dropdown_at(s, ar, ac) {
            if let (Some(row), Some(col)) = (rows.iter().find(|x| x.0 == ar), cols.iter().find(|x| x.0 == ac)) {
                let (_, by, bs) = filter_button(0.0, row.1, 40.0, row.2, p.zoom);
                let bx = col.1 + col.2 + 2.0;
                rgb(cr, 0xF3F3F3);
                rect(cr, bx, by, bs, bs);
                rgb(cr, 0xA0A0A0);
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, bx + 0.5, by + 0.5, bs - 1.0, bs - 1.0);
                cairo_stroke(cr);
                rgb(cr, 0x444444);
                let (cx, cy) = (bx + bs / 2.0, by + bs / 2.0);
                cairo_move_to(cr, cx - bs * 0.25, cy - bs * 0.1);
                cairo_line_to(cr, cx + bs * 0.25, cy - bs * 0.1);
                cairo_line_to(cr, cx, cy + bs * 0.18);
                cairo_close_path(cr);
                cairo_fill(cr);
            }
        }
        if !p.dv_circles.is_empty() {
            rgb(cr, 0xE00000);
            cairo_set_line_width(cr, 1.5 * p.zoom.max(0.5));
            for (r, c) in &p.dv_circles {
                if let (Some(row), Some(col)) = (rows.iter().find(|x| x.0 == *r), cols.iter().find(|x| x.0 == *c)) {
                    let (x, y, w, h) = (col.1, row.1, col.2, row.2);
                    cairo_save(cr);
                    cairo_translate(cr, x + w / 2.0, y + h / 2.0);
                    cairo_scale(cr, w / 2.0 + 3.0, h / 2.0 + 3.0);
                    cairo_arc(cr, 0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
                    cairo_restore(cr);
                    cairo_stroke(cr);
                }
            }
        }
    }

    /// An empty PivotTable: the box that asks for fields.
    unsafe fn paint_pivot_placeholders(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, g: &Geometry, pen: &mut Pen) {
        for pv in self.pivots.iter().filter(|x| x.sheet == s && x.rows.is_empty() && x.cols.is_empty() && x.values.is_empty()) {
            let t = match pv.table_rect() {
                Some(t) => t,
                None => continue,
            };
            let (x0, x1) = (self.screen_x(s, p, g, t.c1), self.screen_x(s, p, g, t.c2 + 1));
            let (y0, y1) = (self.screen_y(s, p, g, t.r1), self.screen_y(s, p, g, t.r2 + 1));
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            rgb(cr, 0xFFFFFF);
            cairo_rectangle(cr, x0, y0, x1 - x0, y1 - y0);
            cairo_fill(cr);
            rgb(cr, 0xA0A0A0);
            cairo_set_line_width(cr, 1.0);
            cairo_rectangle(cr, x0.round() + 0.5, y0.round() + 0.5, (x1 - x0).round() - 1.0, (y1 - y0).round() - 1.0);
            cairo_stroke(cr);
            let f = FontSpec { family: self.styles.theme_fonts.1.clone(), size: 11.0, bold: true, italic: false, underline: 0, strike: false, vert: 0, color: 0 };
            let w = (x1 - x0 - 16.0).max(20.0);
            let (_, h1) = pen.set(&pv.name, &f, p.zoom, Some(w));
            rgb(cr, 0x404040);
            let top = y0 + (y1 - y0) * 0.3;
            cairo_move_to(cr, x0 + 8.0, top);
            pango_cairo_show_layout(cr, pen.layout);
            let f2 = FontSpec { bold: false, size: 9.0, ..f };
            pen.set("To build a report, choose fields from the PivotTable Field List", &f2, p.zoom, Some(w));
            cairo_move_to(cr, x0 + 8.0, top + h1 + 8.0 * p.zoom);
            pango_cairo_show_layout(cr, pen.layout);
        }
    }

    unsafe fn paint_filter_buttons(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, rows: &[(u32, f64, f64)], cols: &[(u32, f64, f64)]) {
        let mut heads = self.filter_headers(s);
        // a PivotTable's buttons, one cell each
        for (r, c, f, _, _) in self.pivot_buttons(s) {
            heads.push((r, c, c, if f { vec![c] } else { vec![] }));
        }
        for (hr, c1, c2, filtered_cols) in heads {
        let row = match rows.iter().find(|(r, _, _)| *r == hr) {
            Some(x) => *x,
            None => continue,
        };
        for &(c, x, w) in cols {
            if c < c1 || c > c2 {
                continue;
            }
            let (bx, by, bs) = filter_button(x, row.1, w, row.2, p.zoom);
            let filtered = filtered_cols.contains(&c);
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
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn layout_text(&self, s: usize, r: u32, c: u32, v: &Value, xf_i: u32, is_formula: bool, p: &ViewParams,
                          cell: (f64, f64, f64, f64), merged: bool, cf: Option<&crate::cf::CfOut>, pen: &mut Pen,
                          empty: &dyn Fn(u32) -> bool, colpos: &dyn Fn(u32) -> (f64, f64)) -> Option<TextJob> {
        let z = p.zoom;
        let xf = self.styles.xf(xf_i).clone();
        let mut font = font_of(&self.styles, xf_i);
        let cf_code = cf.and_then(|o| o.dxf.numfmt.clone());
        if let Some(o) = cf {
            let d = &o.dxf;
            if let Some(c) = d.color { font.color = c }
            if let Some(b) = d.bold { font.bold = b }
            if let Some(i) = d.italic { font.italic = i }
            if let Some(st) = d.strike { font.strike = st }
            if let Some(u) = d.underline { font.underline = u }
        }
        let fmtv = |v: &Value, k: usize| match &cf_code {
            Some(code) => self.format_value_code(v, code, k),
            None => self.format_value(v, xf_i, k),
        };
        let (bx, by, bw, bh) = cell;
        let pad = (3.0 * z).round().max(1.0);
        let mdw = self.mdw(s);
        let indent = xf.indent as f64 * 3.0 * mdw * z
            + self.pivots.iter().find(|q| q.sheet == s && q.out.map(|o| o.contains(r, c)).unwrap_or(false)).map(|q| self.pivot_indent(q, r, c) * z).unwrap_or(0.0);
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
                Value::Num(_) if cf_code.as_deref().map(|c| c.eq_ignore_ascii_case("General")).unwrap_or_else(|| self.styles.xf_numfmt(xf_i).eq_ignore_ascii_case("General")) && !xf.wrap => {
                    // General shows as many digits as fit (11 at most)
                    let mut best = fmtv(v, 11);
                    let mut k = 11;
                    while k > 1 && pen.width(&best.text, &font, z) > avail {
                        k -= 1;
                        best = fmtv(v, k);
                    }
                    best
                }
                _ => fmtv(v, 255),
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
        // dark mode (the desktop is dark): headers go dark as in Excel; the cells keep their colours
        let (hbg, hsel, hwhole, htext, haccent, hgrid, hcorner) = if p.dark {
            (0x2E2E2Eu32, 0x474747u32, 0x5C5030u32, 0xD6D6D6u32, 0xE8C35Au32, 0x4A4A4Au32, 0x6E6E6Eu32)
        } else {
            (HEADER_BG, HEADER_SEL, 0xD2C9A8, HEADER_TEXT, ACCENT_TEXT, GRID, 0xC8C8C8)
        };
        let font = FontSpec { family: header_family(&self.styles), size: 11.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: htext };
        // column headers
        rgb(cr, hbg);
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
                rgb(cr, if whole_col(c) { hwhole } else { hsel });
                rect(cr, x, 0.0, w, g.hh);
                rgb(cr, ACCENT);
                rect(cr, x, g.hh - 2.0, w, 2.0);
            }
            rgb(cr, hgrid);
            rect(cr, (x + w - 1.0).round(), 0.0, 1.0, g.hh);
            let name = col_name(c);
            let (tw, th) = pen.set(&name, &font, z, None);
            rgb(cr, if on { haccent } else { htext });
            if tw < w {
                pen.show(cr, x + (w - tw) / 2.0, (g.hh - th) / 2.0);
            }
        }
        cairo_restore(cr);
        rgb(cr, hgrid);
        rect(cr, 0.0, g.hh - 1.0, p.width, 1.0);
        // row headers
        cairo_save(cr);
        cairo_rectangle(cr, 0.0, g.hh, g.hw, p.height - g.hh);
        cairo_clip(cr);
        for &(r, y, h) in &g.rows {
            let on = sel_rows(r);
            if on {
                rgb(cr, if whole_row(r) { hwhole } else { hsel });
                rect(cr, 0.0, y, g.hw, h);
                rgb(cr, ACCENT);
                rect(cr, g.hw - 2.0, y, 2.0, h);
            }
            rgb(cr, hgrid);
            rect(cr, 0.0, (y + h - 1.0).round(), g.hw, 1.0);
            let label = (r + 1).to_string();
            let (tw, th) = pen.set(&label, &font, z, None);
            rgb(cr, if on { haccent } else { htext });
            if th <= h + 2.0 {
                pen.show(cr, (g.hw - tw) / 2.0, y + (h - th) / 2.0);
            }
        }
        cairo_restore(cr);
        rgb(cr, hgrid);
        rect(cr, g.hw - 1.0, 0.0, 1.0, p.height);
        // the corner's select-all triangle
        rgb(cr, hcorner);
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

impl Book {
    /// Draw a chart on its own (dialog previews, printing).
    pub unsafe fn paint_chart_at(&self, ch: &crate::chart::Chart, cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64, z: f64) {
        let mut pen = Pen { layout: pango_cairo_create_layout(cr), fonts: HashMap::new() };
        self.paint_chart(ch, cr, x, y, w, h, z, &mut pen);
    }
}

/// Where a cell's filter button goes: (x, y, size).
pub fn filter_button(x: f64, y: f64, w: f64, h: f64, z: f64) -> (f64, f64, f64) {
    let bs = (16.0 * z).round().min(h - 2.0).min(w - 2.0).max(6.0);
    (x + w - bs - 2.0, y + h - bs - 2.0, bs)
}

/// The eight sizing handles: nw, n, ne, e, se, s, sw, w.
pub fn handles(x: f64, y: f64, w: f64, h: f64) -> [(f64, f64); 8] {
    [(x, y), (x + w / 2.0, y), (x + w, y), (x + w, y + h / 2.0), (x + w, y + h), (x + w / 2.0, y + h), (x, y + h), (x, y + h / 2.0)]
}

pub const HANDLE_NAMES: [&str; 8] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
