//! The few cairo and pango calls the engine makes (measuring and drawing text and the grid).
#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_double, c_int, c_uint, c_void};

pub type cairo_t = c_void;
pub type cairo_surface_t = c_void;
pub type cairo_pattern_t = c_void;
pub type PangoLayout = c_void;
pub type PangoContext = c_void;
pub type PangoFontMap = c_void;
pub type PangoFontDescription = c_void;
pub type PangoAttrList = c_void;
pub type PangoAttribute = c_void;
pub type PangoFontMetrics = c_void;
pub type PangoLanguage = c_void;
pub type PangoFont = c_void;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct PangoRectangle {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

pub const PANGO_SCALE: f64 = 1024.0;
pub const CAIRO_FORMAT_ARGB32: c_int = 0;
pub const PANGO_WRAP_WORD_CHAR: c_int = 2;
pub const PANGO_ALIGN_LEFT: c_int = 0;
pub const PANGO_ALIGN_CENTER: c_int = 1;
pub const PANGO_ALIGN_RIGHT: c_int = 2;
pub const PANGO_STYLE_NORMAL: c_int = 0;
pub const PANGO_STYLE_ITALIC: c_int = 2;
pub const PANGO_UNDERLINE_SINGLE: c_int = 1;
pub const PANGO_UNDERLINE_DOUBLE: c_int = 2;
pub const CAIRO_ANTIALIAS_NONE: c_int = 1;
pub const CAIRO_FILL_RULE_EVEN_ODD: c_int = 1;
pub const CAIRO_LINE_CAP_BUTT: c_int = 0;

#[link(name = "cairo")]
extern "C" {
    pub fn cairo_create(target: *mut cairo_surface_t) -> *mut cairo_t;
    pub fn cairo_destroy(cr: *mut cairo_t);
    pub fn cairo_reference(cr: *mut cairo_t) -> *mut cairo_t;
    pub fn cairo_status(cr: *mut cairo_t) -> c_int;
    pub fn cairo_save(cr: *mut cairo_t);
    pub fn cairo_restore(cr: *mut cairo_t);
    pub fn cairo_rectangle(cr: *mut cairo_t, x: c_double, y: c_double, w: c_double, h: c_double);
    pub fn cairo_clip(cr: *mut cairo_t);
    pub fn cairo_reset_clip(cr: *mut cairo_t);
    pub fn cairo_fill(cr: *mut cairo_t);
    pub fn cairo_fill_preserve(cr: *mut cairo_t);
    pub fn cairo_stroke(cr: *mut cairo_t);
    pub fn cairo_paint(cr: *mut cairo_t);
    pub fn cairo_new_path(cr: *mut cairo_t);
    pub fn cairo_close_path(cr: *mut cairo_t);
    pub fn cairo_move_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_line_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_rel_line_to(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_arc(cr: *mut cairo_t, xc: c_double, yc: c_double, r: c_double, a1: c_double, a2: c_double);
    pub fn cairo_set_source_rgb(cr: *mut cairo_t, r: c_double, g: c_double, b: c_double);
    pub fn cairo_set_source_rgba(cr: *mut cairo_t, r: c_double, g: c_double, b: c_double, a: c_double);
    pub fn cairo_set_line_width(cr: *mut cairo_t, w: c_double);
    pub fn cairo_set_dash(cr: *mut cairo_t, dashes: *const c_double, n: c_int, offset: c_double);
    pub fn cairo_set_antialias(cr: *mut cairo_t, a: c_int);
    pub fn cairo_set_fill_rule(cr: *mut cairo_t, r: c_int);
    pub fn cairo_set_line_cap(cr: *mut cairo_t, c: c_int);
    pub fn cairo_translate(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_rotate(cr: *mut cairo_t, a: c_double);
    pub fn cairo_scale(cr: *mut cairo_t, x: c_double, y: c_double);
    pub fn cairo_image_surface_create(format: c_int, w: c_int, h: c_int) -> *mut cairo_surface_t;
    pub fn cairo_image_surface_get_data(s: *mut cairo_surface_t) -> *mut u8;
    pub fn cairo_image_surface_get_stride(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_image_surface_get_width(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_image_surface_get_height(s: *mut cairo_surface_t) -> c_int;
    pub fn cairo_surface_destroy(s: *mut cairo_surface_t);
    pub fn cairo_surface_flush(s: *mut cairo_surface_t);
    pub fn cairo_surface_write_to_png(s: *mut cairo_surface_t, path: *const c_char) -> c_int;
    pub fn cairo_pattern_create_linear(x0: c_double, y0: c_double, x1: c_double, y1: c_double) -> *mut cairo_pattern_t;
    pub fn cairo_pattern_add_color_stop_rgb(p: *mut cairo_pattern_t, off: c_double, r: c_double, g: c_double, b: c_double);
    pub fn cairo_pattern_destroy(p: *mut cairo_pattern_t);
    pub fn cairo_set_source(cr: *mut cairo_t, p: *mut cairo_pattern_t);
}

#[link(name = "pango-1.0")]
extern "C" {
    pub fn pango_layout_new(ctx: *mut PangoContext) -> *mut PangoLayout;
    pub fn pango_layout_set_text(l: *mut PangoLayout, text: *const c_char, len: c_int);
    pub fn pango_layout_set_font_description(l: *mut PangoLayout, d: *const PangoFontDescription);
    pub fn pango_layout_set_width(l: *mut PangoLayout, w: c_int);
    pub fn pango_layout_set_wrap(l: *mut PangoLayout, w: c_int);
    pub fn pango_layout_set_alignment(l: *mut PangoLayout, a: c_int);
    pub fn pango_layout_set_attributes(l: *mut PangoLayout, a: *mut PangoAttrList);
    pub fn pango_layout_get_pixel_size(l: *mut PangoLayout, w: *mut c_int, h: *mut c_int);
    pub fn pango_layout_get_extents(l: *mut PangoLayout, ink: *mut PangoRectangle, logical: *mut PangoRectangle);
    pub fn pango_layout_get_line_count(l: *mut PangoLayout) -> c_int;
    pub fn pango_layout_get_baseline(l: *mut PangoLayout) -> c_int;
    pub fn pango_layout_get_context(l: *mut PangoLayout) -> *mut PangoContext;
    pub fn pango_layout_context_changed(l: *mut PangoLayout);
    pub fn pango_font_description_new() -> *mut PangoFontDescription;
    pub fn pango_font_description_free(d: *mut PangoFontDescription);
    pub fn pango_font_description_set_family(d: *mut PangoFontDescription, f: *const c_char);
    pub fn pango_font_description_set_absolute_size(d: *mut PangoFontDescription, s: c_double);
    pub fn pango_font_description_set_weight(d: *mut PangoFontDescription, w: c_int);
    pub fn pango_font_description_set_style(d: *mut PangoFontDescription, s: c_int);
    pub fn pango_font_map_create_context(m: *mut PangoFontMap) -> *mut PangoContext;
    pub fn pango_context_load_font(c: *mut PangoContext, d: *const PangoFontDescription) -> *mut PangoFont;
    pub fn pango_context_get_metrics(c: *mut PangoContext, d: *const PangoFontDescription, lang: *mut PangoLanguage) -> *mut PangoFontMetrics;
    pub fn pango_font_metrics_get_ascent(m: *mut PangoFontMetrics) -> c_int;
    pub fn pango_font_metrics_get_descent(m: *mut PangoFontMetrics) -> c_int;
    pub fn pango_font_metrics_get_height(m: *mut PangoFontMetrics) -> c_int;
    pub fn pango_font_metrics_unref(m: *mut PangoFontMetrics);
    pub fn pango_attr_list_new() -> *mut PangoAttrList;
    pub fn pango_attr_list_insert(l: *mut PangoAttrList, a: *mut PangoAttribute);
    pub fn pango_attr_list_unref(l: *mut PangoAttrList);
    pub fn pango_attr_underline_new(u: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_strikethrough_new(s: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_rise_new(r: c_int) -> *mut PangoAttribute;
    pub fn pango_attr_scale_new(s: c_double) -> *mut PangoAttribute;
}

#[link(name = "pangocairo-1.0")]
extern "C" {
    pub fn pango_cairo_font_map_get_default() -> *mut PangoFontMap;
    pub fn pango_cairo_create_layout(cr: *mut cairo_t) -> *mut PangoLayout;
    pub fn pango_cairo_show_layout(cr: *mut cairo_t, l: *mut PangoLayout);
    pub fn pango_cairo_update_layout(cr: *mut cairo_t, l: *mut PangoLayout);
    pub fn pango_cairo_context_set_resolution(c: *mut PangoContext, dpi: c_double);
}

#[link(name = "gobject-2.0")]
extern "C" {
    pub fn g_object_unref(o: *mut c_void);
}

pub fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s.replace('\0', " ")).unwrap()
}

pub unsafe fn set_text(l: *mut PangoLayout, s: &str) {
    pango_layout_set_text(l, s.as_ptr() as *const c_char, s.len() as c_int);
}

pub fn unused(_: c_uint) {}
