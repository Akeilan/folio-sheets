//! Shapes, text boxes and WordArt on a sheet (SH-21): inserting them and the Shape Format edits.

use crate::book::{Book, Delta};
use crate::drawing::{Anchor, Obj, EMU};
use crate::shape::{self, ShapeView};

impl Book {
    fn new_shape(&mut self, s: usize, prst: &str, x: f64, y: f64, w: f64, h: f64, text: Option<&str>, wordart: Option<usize>,
                 flip: (bool, bool), label: &str) -> Result<usize, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("Shapes can be inserted on worksheets only.".into());
        }
        let base = if wordart.is_some() { "TextBox" } else { shape::base_name(prst) };
        let (id, name) = self.next_object_name(s, base);
        let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: None };
        let (w, h) = (w.max(1.0), h.max(1.0));
        let anchor = self.anchor_at(s, &like, x, y, w, h);
        let mut body = shape::new_body(id, &name, prst, (w * EMU).round() as i64, (h * EMU).round() as i64, text, wordart);
        if flip.0 || flip.1 {
            if let Some(x) = shape::shape_elem_mut(&mut body).and_then(|sp| sp.child_mut("spPr")).and_then(|p| p.child_mut("xfrm")) {
                if flip.0 {
                    x.set_attr("flipH", "1");
                }
                if flip.1 {
                    x.set_attr("flipV", "1");
                }
            }
        }
        let obj = Obj { anchor, body, chart: None, name, part: None, picture: None, pic_part: None, ink: None, ink_part: None, raw: None, slicer: None };
        let mut objs = self.sheets[s].objects.clone();
        objs.push(obj);
        let i = objs.len() - 1;
        self.begin(label, s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(i)
    }

    /// A preset shape (rect, ellipse, rightArrow, line ...) in the box at sheet pixels; lines run
    /// corner to corner, flipped to go the way they were drawn.
    pub fn insert_shape(&mut self, s: usize, prst: &str, x: f64, y: f64, w: f64, h: f64, flip_h: bool, flip_v: bool) -> Result<usize, String> {
        if prst == "textBox" {
            return self.insert_text_box(s, x, y, w, h, "");
        }
        self.new_shape(s, prst, x, y, w, h, None, None, (flip_h, flip_v), "Insert Shape")
    }

    pub fn insert_text_box(&mut self, s: usize, x: f64, y: f64, w: f64, h: f64, text: &str) -> Result<usize, String> {
        self.new_shape(s, "textBox", x, y, w, h, Some(text), None, (false, false), "Insert Text Box")
    }

    /// WordArt in one of the shape::WORDART styles, sized to its text.
    pub fn insert_wordart(&mut self, s: usize, style: usize, text: &str, x: f64, y: f64) -> Result<usize, String> {
        let longest = text.split('\n').map(|l| l.chars().count()).max().unwrap_or(1).max(1) as f64;
        let lines = text.split('\n').count().max(1) as f64;
        let (w, h) = (longest * 30.0 + 24.0, lines * 64.0 + 12.0);
        self.new_shape(s, "rect", x - w / 2.0, y - h / 2.0, w, h, Some(text), Some(style.min(shape::WORDART.len() - 1)), (false, false), "Insert WordArt")
    }

    pub fn shape_view(&self, s: usize, i: usize) -> Option<ShapeView> {
        let o = self.sheets.get(s)?.objects.get(i)?;
        if o.raw.is_some() || o.chart.is_some() || o.picture.is_some() || o.ink.is_some() {
            return None;
        }
        ShapeView::parse(&o.body, &self.styles.theme)
    }

    /// Change a shape's XML as one undo step.
    pub fn edit_shape(&mut self, s: usize, which: &[usize], label: &str, f: impl Fn(&mut Vec<crate::xml::Node>)) -> Result<(), String> {
        let sh = self.sheets.get(s).ok_or("No such sheet.")?;
        let mut objs = sh.objects.clone();
        let mut any = false;
        for &i in which {
            if let Some(o) = objs.get_mut(i) {
                if shape::shape_elem(&o.body).is_some() && o.raw.is_none() {
                    f(&mut o.body);
                    any = true;
                }
            }
        }
        if !any {
            return Err("Select a shape first.".into());
        }
        self.begin(label, s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }

    /// Bring Forward (+1), Send Backward (-1), Bring to Front (+big), Send to Back (-big).
    pub fn reorder_object(&mut self, s: usize, i: usize, by: i64) -> Result<usize, String> {
        let sh = self.sheets.get(s).ok_or("No such sheet.")?;
        let n = sh.objects.len();
        if i >= n {
            return Err("No such object.".into());
        }
        let to = (i as i64 + by).clamp(0, n as i64 - 1) as usize;
        if to == i {
            return Ok(i);
        }
        let mut objs = sh.objects.clone();
        let o = objs.remove(i);
        objs.insert(to, o);
        self.begin(if by > 0 { "Bring Forward" } else { "Send Backward" }, s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(to)
    }

    /// Insert › Equation: a text box holding an equation (typed in the linear format), sized to it.
    pub fn insert_equation(&mut self, s: usize, linear: &str, x: f64, y: f64) -> Result<usize, String> {
        let m = crate::math::parse_linear(linear);
        let (w, h) = equation_box(&m, 11.0);
        self.begin_group("Insert Equation", s);
        let i = match self.new_shape(s, "textBox", x, y, w, h, Some(""), None, (false, false), "Insert Equation") {
            Ok(i) => i,
            Err(e) => {
                self.end_group();
                return Err(e);
            }
        };
        let r = self.edit_shape(s, &[i], "Insert Equation", |b| {
            shape::set_fill(b, None);
            shape::set_line(b, Some(None), None, None);
            shape::set_math(b, &m, 11.0);
        });
        self.end_group();
        r.map(|_| i)
    }

    /// The equation in a text box, in the linear format.
    pub fn equation_linear(&self, s: usize, i: usize) -> Option<String> {
        self.shape_view(s, i).and_then(|v| v.math).map(|(m, _)| crate::math::to_linear(&m))
    }

    /// Change an equation (typed in the linear format); its box grows or shrinks to fit.
    pub fn set_equation(&mut self, s: usize, i: usize, linear: &str) -> Result<(), String> {
        let v = self.shape_view(s, i).ok_or("Select an equation first.")?;
        let size = v.math.as_ref().map(|x| x.1).unwrap_or(11.0);
        let m = crate::math::parse_linear(linear);
        let (w, h) = equation_box(&m, size);
        let o = &self.sheets[s].objects[i];
        let (x, y, _, _) = self.obj_rect(s, o);
        let like = o.anchor.clone();
        let anchor = self.anchor_at(s, &like, x, y, w, h);
        let mut objs = self.sheets[s].objects.clone();
        shape::set_math(&mut objs[i].body, &m, size);
        if let Some(xf) = shape::shape_elem_mut(&mut objs[i].body).and_then(|sp| sp.child_mut("spPr")).and_then(|p| p.child_mut("xfrm")) {
            if let Some(ext) = xf.child_mut("ext") {
                ext.set_attr("cx", &((w * EMU).round() as i64).to_string());
                ext.set_attr("cy", &((h * EMU).round() as i64).to_string());
            }
        }
        objs[i].anchor = anchor;
        self.begin("Equation", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }
}

/// The size (sheet px) of a text box that fits an equation.
pub fn equation_box(m: &crate::math::M, size_pt: f64) -> (f64, f64) {
    let (w, a, d) = equation_extent(m, size_pt * 96.0 / 72.0);
    let ins = 96.0 / 72.0;
    ((w + 14.4 * ins + 4.0).ceil(), (a + d + 7.2 * ins + 4.0).ceil())
}

/// (width, ascent, descent) of an equation at a size in px.
pub fn equation_extent(m: &crate::math::M, size_px: f64) -> (f64, f64, f64) {
    unsafe {
        use crate::ffi::*;
        let surf = cairo_image_surface_create(0, 4, 4);
        let cr = cairo_create(surf);
        let r = {
            let mut ms = crate::math::PangoMeasure::new(cr);
            let b = crate::math::layout(m, &mut ms, size_px);
            (b.w, b.asc, b.desc)
        };
        cairo_destroy(cr);
        cairo_surface_destroy(surf);
        r
    }
}
