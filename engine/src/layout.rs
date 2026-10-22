//! Text measurement with pango: the maximum digit width that column widths are counted
//! in, text widths for AutoFit, and line heights for row heights.

use crate::ffi::*;
use crate::styles::Styles;
use std::cell::{Cell as StdCell, RefCell};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub family: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: u8,
    pub strike: bool,
    pub vert: u8,
    pub color: u32,
}

impl FontSpec {
    pub fn key(&self) -> (String, u64, bool, bool) {
        (self.family.clone(), (self.size * 100.0).round() as u64, self.bold, self.italic)
    }
}

pub struct Layout {
    ctx: StdCell<*mut PangoContext>,
    layout: StdCell<*mut PangoLayout>,
    mdw: RefCell<HashMap<(String, u64), f64>>,
    heights: RefCell<HashMap<(String, u64, bool, bool), (f64, f64)>>,
    widths: RefCell<HashMap<(String, u64, bool, bool, String), f64>>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            ctx: StdCell::new(std::ptr::null_mut()),
            layout: StdCell::new(std::ptr::null_mut()),
            mdw: RefCell::new(HashMap::new()),
            heights: RefCell::new(HashMap::new()),
            widths: RefCell::new(HashMap::new()),
        }
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        unsafe {
            if !self.layout.get().is_null() {
                g_object_unref(self.layout.get());
            }
            if !self.ctx.get().is_null() {
                g_object_unref(self.ctx.get());
            }
        }
    }
}

/// A pango font description for a font at a zoom (pixels: points * 96/72).
pub struct FontDesc(pub *mut PangoFontDescription);

impl FontDesc {
    pub fn new(f: &FontSpec, zoom: f64) -> FontDesc {
        unsafe {
            let d = pango_font_description_new();
            let fam = cstr(&family_list(&f.family));
            pango_font_description_set_family(d, fam.as_ptr());
            let px = f.size * 96.0 / 72.0 * zoom * if f.vert != 0 { 0.66 } else { 1.0 };
            pango_font_description_set_absolute_size(d, px.max(0.5) * PANGO_SCALE);
            pango_font_description_set_weight(d, if f.bold { 700 } else { 400 });
            pango_font_description_set_style(d, if f.italic { PANGO_STYLE_ITALIC } else { PANGO_STYLE_NORMAL });
            FontDesc(d)
        }
    }
}

impl Drop for FontDesc {
    fn drop(&mut self) {
        unsafe { pango_font_description_free(self.0) }
    }
}

/// Office fonts and their metric-compatible stand-ins, for systems without them.
pub fn family_list(name: &str) -> String {
    let sub = match name.to_lowercase().as_str() {
        "calibri" => "Carlito",
        "cambria" => "Caladea",
        "arial" => "Liberation Sans",
        "times new roman" => "Liberation Serif",
        "courier new" => "Liberation Mono",
        "aptos" | "aptos narrow" => "Carlito",
        _ => "",
    };
    if sub.is_empty() {
        name.to_string()
    } else {
        format!("{},{}", name, sub)
    }
}

impl Layout {
    pub fn layout(&self) -> *mut PangoLayout {
        if self.layout.get().is_null() {
            unsafe {
                let map = pango_cairo_font_map_get_default();
                let ctx = pango_font_map_create_context(map);
                pango_cairo_context_set_resolution(ctx, 96.0);
                self.ctx.set(ctx);
                self.layout.set(pango_layout_new(ctx));
            }
        }
        self.layout.get()
    }

    /// Width and height in pixels of one line of text.
    pub fn measure(&self, text: &str, f: &FontSpec, zoom: f64) -> (f64, f64) {
        let l = self.layout();
        let d = FontDesc::new(f, zoom);
        unsafe {
            pango_layout_set_attributes(l, std::ptr::null_mut());
            pango_layout_set_width(l, -1);
            pango_layout_set_font_description(l, d.0);
            set_text(l, text);
            let mut ink = PangoRectangle::default();
            let mut logical = PangoRectangle::default();
            pango_layout_get_extents(l, &mut ink, &mut logical);
            (logical.width as f64 / PANGO_SCALE, logical.height as f64 / PANGO_SCALE)
        }
    }

    /// Size of text wrapped at `width` pixels.
    pub fn measure_wrapped(&self, text: &str, f: &FontSpec, zoom: f64, width: f64) -> (f64, f64) {
        let l = self.layout();
        let d = FontDesc::new(f, zoom);
        unsafe {
            pango_layout_set_attributes(l, std::ptr::null_mut());
            pango_layout_set_font_description(l, d.0);
            pango_layout_set_width(l, (width.max(1.0) * PANGO_SCALE) as i32);
            pango_layout_set_wrap(l, PANGO_WRAP_WORD_CHAR);
            set_text(l, text);
            let mut ink = PangoRectangle::default();
            let mut logical = PangoRectangle::default();
            pango_layout_get_extents(l, &mut ink, &mut logical);
            pango_layout_set_width(l, -1);
            (logical.width as f64 / PANGO_SCALE, logical.height as f64 / PANGO_SCALE)
        }
    }

    pub fn text_width(&self, text: &str, f: &FontSpec) -> f64 {
        let k = (f.family.clone(), (f.size * 100.0).round() as u64, f.bold, f.italic, text.to_string());
        if let Some(w) = self.widths.borrow().get(&k) {
            return *w;
        }
        let w = self.measure(text, f, 1.0).0;
        let mut m = self.widths.borrow_mut();
        if m.len() > 20000 {
            m.clear();
        }
        m.insert(k, w);
        w
    }

    /// (ascent + descent, line height) in pixels at 100 %.
    pub fn line_height(&self, f: &FontSpec) -> (f64, f64) {
        let k = f.key();
        if let Some(h) = self.heights.borrow().get(&k) {
            return *h;
        }
        let (_, h) = self.measure("Ag", f, 1.0);
        let v = (h, h);
        self.heights.borrow_mut().insert(k, v);
        v
    }

    /// The maximum digit width of the Normal style font, in pixels: what column widths
    /// are counted in.
    pub fn mdw(&self, styles: &Styles) -> f64 {
        let f = normal_font(styles);
        let k = (f.family.clone(), (f.size * 100.0).round() as u64);
        if let Some(w) = self.mdw.borrow().get(&k) {
            return *w;
        }
        let w = mdw_for(&f.family, f.size).unwrap_or_else(|| {
            let mut m: f64 = 0.0;
            for d in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] {
                m = m.max(self.measure(d, &f, 1.0).0);
            }
            m.round().max(1.0)
        });
        self.mdw.borrow_mut().insert(k, w);
        w
    }

    /// Default row height in points for a font (what Excel uses for rows without a set
    /// height).
    pub fn row_height_pt(&self, f: &FontSpec) -> f64 {
        if let Some(h) = known_height(&f.family, f.size) {
            return h;
        }
        let (h, _) = self.line_height(f);
        // a line plus a little room, in whole pixels, in quarter points
        let px = (h * 1.03).ceil() + 1.0;
        (px * 0.75 * 4.0).round() / 4.0
    }
}

/// Excel's maximum digit widths (96 dpi) for the usual fonts and sizes.
fn mdw_for(family: &str, size: f64) -> Option<f64> {
    let f = family.to_lowercase();
    let table: &[(f64, f64)] = match f.as_str() {
        "calibri" | "carlito" => &[(8.0, 5.0), (9.0, 6.0), (10.0, 6.0), (11.0, 7.0), (12.0, 8.0), (14.0, 9.0), (16.0, 11.0), (18.0, 12.0), (20.0, 13.0)],
        "arial" | "liberation sans" => &[(8.0, 5.0), (9.0, 6.0), (10.0, 7.0), (11.0, 8.0), (12.0, 9.0), (14.0, 10.0)],
        "aptos narrow" => &[(11.0, 7.0)],
        "cambria" => &[(11.0, 7.0)],
        _ => &[],
    };
    table.iter().find(|(s, _)| (*s - size).abs() < 0.01).map(|(_, w)| *w)
}

/// Excel's default row heights (points) for the usual fonts.
fn known_height(family: &str, size: f64) -> Option<f64> {
    let f = family.to_lowercase();
    let table: &[(f64, f64)] = match f.as_str() {
        "calibri" | "carlito" | "aptos narrow" | "aptos" => &[(8.0, 11.25), (9.0, 12.0), (10.0, 12.75), (11.0, 15.0), (12.0, 15.75), (14.0, 18.75),
            (16.0, 21.0), (18.0, 23.25), (20.0, 26.25), (22.0, 28.5), (24.0, 31.5), (26.0, 33.75), (28.0, 36.75), (36.0, 46.5), (48.0, 63.0), (72.0, 93.75)],
        "calibri light" => &[(11.0, 15.0), (15.0, 19.5), (18.0, 23.25)],
        "cambria" => &[(11.0, 14.25), (18.0, 23.25)],
        "arial" | "liberation sans" => &[(8.0, 11.25), (9.0, 12.0), (10.0, 12.75), (11.0, 14.25), (12.0, 15.0), (14.0, 18.0), (16.0, 20.25), (18.0, 23.25)],
        _ => &[],
    };
    table.iter().find(|(s, _)| (*s - size).abs() < 0.01).map(|(_, h)| *h)
}

pub fn normal_font(styles: &Styles) -> FontSpec {
    font_of(styles, 0)
}

/// The font of a cell format.
pub fn font_of(styles: &Styles, xf: u32) -> FontSpec {
    let x = styles.xf(xf);
    let f = styles.font(x.font);
    FontSpec {
        family: styles.font_name(f),
        size: if f.size > 0.0 { f.size } else { 11.0 },
        bold: f.bold,
        italic: f.italic,
        underline: f.underline,
        strike: f.strike,
        vert: f.vert,
        color: styles.rgb(&f.color, 0x000000),
    }
}
