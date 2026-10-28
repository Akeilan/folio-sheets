//! Radar, stock (high-low-close) and contour charts drawn with cairo.
use crate::book::Book;
use crate::chart::{auto_color, Chart};
use crate::ffi::*;
use crate::render::Pen;
use std::f64::consts::PI;

const TEXT: u32 = 0x595959;
const LINE: u32 = 0xD9D9D9;

unsafe fn rgb(cr: *mut cairo_t, c: u32) {
    cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
}

unsafe fn rgba(cr: *mut cairo_t, c: u32, a: f64) {
    cairo_set_source_rgba(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0, a);
}

impl Book {
    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn paint_special(&self, ch: &Chart, tag: &str, cr: *mut cairo_t, l: f64, t: f64, r: f64, b: f64, z: f64, pen: &mut Pen) {
        let theme = self.styles.theme;
        let small = crate::layout::FontSpec { family: self.styles.theme_fonts.1.clone(), size: 9.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: TEXT };
        let p = &ch.plots[0];
        let series: Vec<(String, Vec<Option<f64>>, u32)> = p.series.iter().enumerate().map(|(k, se)| {
            let vals = se.val.as_ref().map(|v| self.chart_values(v).into_iter().map(|x| x.v).collect()).unwrap_or_default();
            (self.series_name(se, k), vals, se.color.map(|c| self.styles.resolve(c)).unwrap_or_else(|| auto_color(&theme, ch.palette, k)))
        }).collect();
        let cats: Vec<String> = p.series.first().and_then(|s| s.cat.as_ref()).map(|c| self.chart_values(c).into_iter().map(|x| x.text).collect())
            .unwrap_or_else(|| (1..=series.first().map(|s| s.1.len()).unwrap_or(0)).map(|i| i.to_string()).collect());
        let all: Vec<f64> = series.iter().flat_map(|s| s.1.iter().filter_map(|v| *v)).collect();
        let lo = all.iter().cloned().fold(f64::MAX, f64::min);
        let hi = all.iter().cloned().fold(f64::MIN, f64::max);
        let (lo, hi) = if all.is_empty() { (0.0, 1.0) } else { (lo, hi) };
        // the legend along the bottom: the series, or a contour's value bands
        let mut entries: Vec<(String, u32)> = series.iter().map(|s| (s.0.clone(), s.2)).collect();
        if tag == "surfaceChart" {
            let (amin, amax, step) = crate::chart_render::nice_axis(lo, hi, 6.0, false);
            entries.clear();
            let mut v = amin;
            let mut k = 0;
            while v < amax - step * 1e-6 && k < 12 {
                let f = |x: f64| crate::numfmt::num_to_text((x * 1e9).round() / 1e9);
                entries.push((format!("{}-{}", f(v), f(v + step)), auto_color(&theme, 0, k)));
                v += step;
                k += 1;
            }
        }
        let mut b = b;
        if ch.legend.is_some() && tag != "stockChart" {
            let lh = 16.0 * z;
            let mut x = l;
            for (name, c) in &entries {
                rgb(cr, *c);
                cairo_rectangle(cr, x, b - lh / 2.0 - 4.0 * z, 8.0 * z, 8.0 * z);
                cairo_fill(cr);
                let (tw, th) = pen.set(name, &small, z, None);
                rgb(cr, TEXT);
                pen.show(cr, x + 11.0 * z, b - lh / 2.0 - th / 2.0);
                x += tw + 26.0 * z;
            }
            b -= lh + 4.0 * z;
        }
        match tag {
            "radarChart" => {
                let (amin, amax, step) = crate::chart_render::nice_axis(lo.min(0.0), hi, 5.0, true);
                let n = cats.len().max(3);
                let (ox, oy) = ((l + r) / 2.0, (t + b) / 2.0 + 4.0 * z);
                let rad = ((r - l).min(b - t) / 2.0 - 18.0 * z).max(4.0);
                let ang = |i: usize| -PI / 2.0 + 2.0 * PI * i as f64 / n as f64;
                let rv = |v: f64| (v - amin) / (amax - amin) * rad;
                // the web and the scale
                cairo_set_line_width(cr, 1.0);
                let mut v = amin;
                while v <= amax + step * 1e-6 {
                    rgb(cr, LINE);
                    for i in 0..=n {
                        let (x, y) = (ox + rv(v) * ang(i % n).cos(), oy + rv(v) * ang(i % n).sin());
                        if i == 0 { cairo_move_to(cr, x, y) } else { cairo_line_to(cr, x, y) }
                    }
                    cairo_stroke(cr);
                    let (tw, th) = pen.set(&crate::numfmt::num_to_text((v * 1e9).round() / 1e9), &small, z, None);
                    rgb(cr, TEXT);
                    pen.show(cr, ox - tw - 3.0 * z, oy - rv(v) - th / 2.0);
                    v += step;
                }
                for i in 0..n {
                    rgb(cr, LINE);
                    cairo_move_to(cr, ox, oy);
                    cairo_line_to(cr, ox + rad * ang(i).cos(), oy + rad * ang(i).sin());
                    cairo_stroke(cr);
                    if let Some(c) = cats.get(i) {
                        let (tw, th) = pen.set(c, &small, z, None);
                        let (x, y) = (ox + (rad + 10.0 * z) * ang(i).cos(), oy + (rad + 10.0 * z) * ang(i).sin());
                        rgb(cr, TEXT);
                        pen.show(cr, x - tw / 2.0, y - th / 2.0);
                    }
                }
                for (_, vals, c) in &series {
                    cairo_new_path(cr);
                    for i in 0..n {
                        let v = vals.get(i).copied().flatten().unwrap_or(amin);
                        let (x, y) = (ox + rv(v) * ang(i).cos(), oy + rv(v) * ang(i).sin());
                        if i == 0 { cairo_move_to(cr, x, y) } else { cairo_line_to(cr, x, y) }
                    }
                    cairo_close_path(cr);
                    if p.hole == 1 {
                        rgba(cr, *c, 0.55);
                        cairo_fill(cr);
                    } else {
                        rgb(cr, *c);
                        cairo_set_line_width(cr, 2.25 * z);
                        cairo_stroke(cr);
                        if p.markers {
                            for i in 0..n {
                                let v = vals.get(i).copied().flatten().unwrap_or(amin);
                                cairo_arc(cr, ox + rv(v) * ang(i).cos(), oy + rv(v) * ang(i).sin(), 3.0 * z, 0.0, 2.0 * PI);
                                cairo_fill(cr);
                            }
                        }
                    }
                }
            }
            "stockChart" => {
                let (amin, amax, step) = crate::chart_render::nice_axis(lo, hi, 6.0, false);
                let lab_h = 16.0 * z;
                let (pl, pt, pr, pb) = self.cx_value_axis(cr, pen, l, t, r, b - lab_h, amin, amax, step, z, &small);
                let n = cats.len().max(1);
                let slot = (pr - pl) / n as f64;
                let yv = |v: f64| pb - (v - amin) / (amax - amin) * (pb - pt);
                // series: high, low, close (as the original expects them, in that order)
                let get = |k: usize, i: usize| series.get(k).and_then(|s| s.1.get(i).copied().flatten());
                for i in 0..n {
                    let x = pl + slot * (i as f64 + 0.5);
                    if let (Some(h), Some(lw)) = (get(0, i), get(1, i)) {
                        rgb(cr, 0x404040);
                        cairo_set_line_width(cr, 1.0);
                        cairo_move_to(cr, x.floor() + 0.5, yv(h));
                        cairo_line_to(cr, x.floor() + 0.5, yv(lw));
                        cairo_stroke(cr);
                    }
                    if let Some(c) = get(2, i) {
                        rgb(cr, auto_color(&theme, ch.palette, 2));
                        cairo_set_line_width(cr, 2.0 * z);
                        cairo_move_to(cr, x, yv(c));
                        cairo_line_to(cr, x + slot * 0.25, yv(c));
                        cairo_stroke(cr);
                    }
                    if let Some(c) = cats.get(i) {
                        if slot > 14.0 {
                            let (tw, th) = pen.set(c, &small, z, Some(slot));
                            rgb(cr, TEXT);
                            pen.show(cr, x - tw / 2.0, pb + lab_h / 2.0 - th / 2.0);
                        }
                    }
                }
            }
            _ => {
                // contour: categories across, series down, colour bands by value
                let (amin, amax, step) = crate::chart_render::nice_axis(lo, hi, 6.0, false);
                let bands = (((amax - amin) / step).round() as usize).max(1);
                let lab_h = 16.0 * z;
                let (pl, pt, pr, pb) = self.cx_value_axis(cr, pen, l, t, r, b - lab_h, 0.0, series.len().max(1) as f64, 1.0, z, &small);
                let nc = cats.len().max(2);
                let ns = series.len().max(1);
                let cw = (pr - pl) / (nc - 1) as f64;
                let rh = (pb - pt) / ns as f64;
                let band_color = |v: f64| auto_color(&theme, 0, (((v - amin) / step).floor().max(0.0) as usize).min(bands - 1));
                // each cell of the grid, split finely, coloured by the value there (bilinear)
                let fine = 8;
                for k in 0..ns {
                    for i in 0..nc - 1 {
                        let v = |kk: usize, ii: usize| series.get(kk.min(ns - 1)).and_then(|s| s.1.get(ii).copied().flatten()).unwrap_or(amin);
                        let (a, bb, c, d) = (v(k, i), v(k, i + 1), v(k + 1, i), v(k + 1, i + 1));
                        for fy in 0..fine {
                            for fx in 0..fine {
                                let (u, w) = ((fx as f64 + 0.5) / fine as f64, (fy as f64 + 0.5) / fine as f64);
                                let val = a * (1.0 - u) * (1.0 - w) + bb * u * (1.0 - w) + c * (1.0 - u) * w + d * u * w;
                                rgb(cr, band_color(val));
                                cairo_rectangle(cr, pl + cw * (i as f64 + fx as f64 / fine as f64), pb - rh * (k as f64 + (fy + 1) as f64 / fine as f64),
                                                cw / fine as f64 + 0.6, rh / fine as f64 + 0.6);
                                cairo_fill(cr);
                            }
                        }
                    }
                }
                for (i, c) in cats.iter().enumerate() {
                    let (tw, th) = pen.set(c, &small, z, None);
                    rgb(cr, TEXT);
                    pen.show(cr, pl + cw * i as f64 - tw / 2.0, pb + lab_h / 2.0 - th / 2.0);
                }
            }
        }
    }
}
