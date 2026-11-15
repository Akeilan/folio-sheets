//! Drawing a chart with cairo: title, legend, axes, gridlines and the series.

use crate::book::Book;
use crate::chart::{auto_color, Chart, Group, Kind, Plot};
use crate::ffi::*;
use crate::layout::FontSpec;
use crate::render::Pen;
use std::f64::consts::PI;

const TEXT: u32 = 0x595959;
const LINE: u32 = 0xD9D9D9;

unsafe fn rgb(cr: *mut cairo_t, c: u32) {
    cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
}

fn shade(c: u32, k: f64) -> u32 {
    crate::render::mix_pub(c, 0x000000, k)
}

fn lighten(c: u32, k: f64) -> u32 {
    crate::render::mix_pub(c, 0xFFFFFF, k)
}

/// A box seen from the front, its top and right side going back by (dx, dy).
unsafe fn prism(cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64, dx: f64, dy: f64, c: u32) {
    rgb(cr, shade(c, 0.3));
    cairo_move_to(cr, x + w, y);
    cairo_line_to(cr, x + w + dx, y - dy);
    cairo_line_to(cr, x + w + dx, y + h - dy);
    cairo_line_to(cr, x + w, y + h);
    cairo_close_path(cr);
    cairo_fill(cr);
    rgb(cr, lighten(c, 0.25));
    cairo_move_to(cr, x, y);
    cairo_line_to(cr, x + dx, y - dy);
    cairo_line_to(cr, x + w + dx, y - dy);
    cairo_line_to(cr, x + w, y);
    cairo_close_path(cr);
    cairo_fill(cr);
    rgb(cr, c);
    cairo_rectangle(cr, x, y, w, h);
    cairo_fill(cr);
}

fn luma(c: u32) -> f64 {
    0.299 * ((c >> 16) & 255) as f64 + 0.587 * ((c >> 8) & 255) as f64 + 0.114 * (c & 255) as f64
}

/// The axis Excel would pick for data from min to max: (low, high, major step).
pub fn nice_axis(mut min: f64, mut max: f64, ticks: f64, from_zero: bool) -> (f64, f64, f64) {
    if !min.is_finite() || !max.is_finite() {
        return (0.0, 1.0, 0.2);
    }
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if from_zero {
        // a range far from zero doesn't start at zero
        if min > 0.0 && (max - min) / max >= 1.0 / 6.0 {
            min = 0.0;
        } else if max < 0.0 && (max - min) / -min >= 1.0 / 6.0 {
            max = 0.0;
        } else if min <= 0.0 && max >= 0.0 {
        }
    }
    if min == max {
        if min == 0.0 {
            return (0.0, 1.0, 0.2);
        }
        let d = min.abs() / 2.0;
        if min > 0.0 { min = 0.0 } else { max = 0.0 }
        let _ = d;
    }
    let span = max - min;
    // headroom over the largest value
    let hi_raw = if max > 0.0 { max + span * 0.05 } else { max };
    let lo_raw = if min < 0.0 { min - span * 0.05 } else { min };
    let raw = (hi_raw - lo_raw) / ticks.max(2.0);
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = mag * if norm <= 1.0 { 1.0 } else if norm <= 2.0 { 2.0 } else if norm <= 2.5 { 2.5 } else if norm <= 5.0 { 5.0 } else { 10.0 };
    let lo = (lo_raw / step).floor() * step;
    let mut hi = (hi_raw / step).ceil() * step;
    if hi <= lo {
        hi = lo + step;
    }
    (clean(lo), clean(hi), step)
}

fn clean(x: f64) -> f64 {
    let r = (x * 1e9).round() / 1e9;
    if r == 0.0 { 0.0 } else { r }
}

struct Entry {
    text: String,
    color: u32,
    line: bool,
    marker: bool,
}

struct SerData {
    vals: Vec<Option<f64>>,
    xs: Vec<Option<f64>>,
    /// bubble sizes
    sizes: Vec<Option<f64>>,
    color: u32,
    labels: bool,
    smooth: bool,
    texts: Vec<String>,
}

impl Book {
    fn chart_font(&self, size: f64, bold: bool) -> FontSpec {
        FontSpec { family: self.styles.theme_fonts.1.clone(), size, bold, italic: false, underline: 0, strike: false, vert: 0, color: TEXT }
    }

    /// Paint a chart into the rectangle (x, y, w, h) on screen, at zoom z.
    pub(crate) unsafe fn paint_chart(&self, ch: &Chart, cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64, z: f64, pen: &mut Pen) {
        if w < 4.0 || h < 4.0 {
            return;
        }
        let theme = &self.styles.theme;
        cairo_save(cr);
        cairo_rectangle(cr, x, y, w, h);
        cairo_clip(cr);
        rgb(cr, 0xFFFFFF);
        cairo_rectangle(cr, x, y, w, h);
        cairo_fill(cr);
        rgb(cr, LINE);
        cairo_set_line_width(cr, 1.0);
        cairo_rectangle(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0);
        cairo_stroke(cr);
        let pad = (8.0 * z).max(2.0);
        let (mut l, mut t, mut r, mut b) = (x + pad, y + pad, x + w - pad, y + h - pad);
        pen.set_align(PANGO_ALIGN_CENTER);
        // ---- title
        if let Some(title) = self.chart_title_text(ch) {
            if !title.is_empty() {
                let f = self.chart_font(14.0, false);
                let (_, th) = pen.set(&title, &f, z, Some(r - l));
                rgb(cr, TEXT);
                pen.show(cr, l, t);
                t += th + 4.0 * z;
            }
        }
        pen.set_align(PANGO_ALIGN_LEFT);
        if let Some(tag) = ch.special().filter(|t| matches!(*t, "radarChart" | "stockChart" | "surfaceChart" | "surface3DChart")) {
            self.paint_special(ch, tag, cr, l, t, r, b, z, pen);
            cairo_restore(cr);
            return;
        }
        // ---- data
        let pie = ch.plots.iter().all(|p| matches!(p.kind, Kind::Pie | Kind::Doughnut));
        let mut idx = 0usize;
        let mut data: Vec<Vec<SerData>> = Vec::new();
        let mut entries: Vec<Entry> = Vec::new();
        let mut cats: Vec<String> = Vec::new();
        for p in &ch.plots {
            let mut list = Vec::new();
            for se in &p.series {
                let pts = se.val.as_ref().map(|v| self.chart_values(v)).unwrap_or_default();
                let xpts = se.cat.as_ref().map(|v| self.chart_values(v)).unwrap_or_default();
                if cats.is_empty() && !xpts.is_empty() && p.kind != Kind::Scatter {
                    cats = xpts.iter().map(|p| p.text.clone()).collect();
                }
                let color = se.color.map(|c| self.styles.resolve(c)).unwrap_or_else(|| auto_color(theme, ch.palette, idx));
                if !pie {
                    entries.push(Entry { text: self.series_name(se, idx), color, line: matches!(p.kind, Kind::Line) || p.kind == Kind::Scatter && p.lines, marker: p.markers });
                }
                list.push(SerData {
                    vals: pts.iter().map(|p| p.v).collect(),
                    texts: pts.iter().map(|p| p.text.clone()).collect(),
                    xs: xpts.iter().map(|p| p.v).collect(),
                    sizes: se.size.as_ref().map(|v| self.chart_values(v).into_iter().map(|p| p.v).collect()).unwrap_or_default(),
                    color,
                    labels: se.labels,
                    smooth: se.smooth,
                });
                idx += 1;
            }
            data.push(list);
        }
        if pie {
            if let Some(s0) = data.first().and_then(|d| d.first()) {
                for i in 0..s0.vals.len() {
                    let text = cats.get(i).cloned().filter(|t| !t.is_empty()).unwrap_or_else(|| format!("{}", i + 1));
                    entries.push(Entry { text, color: auto_color(theme, ch.palette, i), line: false, marker: false });
                }
            }
        }
        // ---- legend
        if let Some(pos) = &ch.legend {
            let f = self.chart_font(9.0, false);
            let key = 7.0 * z;
            let gap = 4.0 * z;
            let sizes: Vec<(f64, f64)> = entries.iter().map(|e| {
                let (tw, th) = pen.set(&e.text, &f, z, None);
                ((if e.line { key * 2.5 } else { key }) + gap + tw, th)
            }).collect();
            let lh = sizes.iter().map(|s| s.1).fold(12.0 * z, f64::max);
            let place: Vec<(f64, f64)>;
            match pos.as_str() {
                "r" | "l" | "tr" => {
                    let lw = sizes.iter().map(|s| s.0).fold(0.0, f64::max).min((r - l) * 0.4);
                    let total = lh * entries.len() as f64;
                    let x0 = if pos == "l" { l } else { r - lw };
                    let y0 = if pos == "tr" { t } else { t + ((b - t) - total).max(0.0) / 2.0 };
                    place = (0..entries.len()).map(|i| (x0, y0 + i as f64 * lh)).collect();
                    if pos == "l" { l += lw + 8.0 * z } else { r -= lw + 8.0 * z }
                }
                _ => {
                    // rows of entries, centred
                    let avail = r - l;
                    let mut rows: Vec<Vec<usize>> = vec![vec![]];
                    let mut cur = 0.0;
                    for (i, s) in sizes.iter().enumerate() {
                        let need = s.0 + 12.0 * z;
                        if cur + need > avail && !rows.last().unwrap().is_empty() {
                            rows.push(vec![]);
                            cur = 0.0;
                        }
                        rows.last_mut().unwrap().push(i);
                        cur += need;
                    }
                    let rows: Vec<Vec<usize>> = rows.into_iter().take(((b - t) / 3.0 / lh).max(1.0) as usize).collect();
                    let total_h = rows.len() as f64 * lh;
                    let y0 = if pos == "t" { t } else { b - total_h };
                    let mut pl = vec![(-1e9, -1e9); entries.len()];
                    for (k, row) in rows.iter().enumerate() {
                        let rw: f64 = row.iter().map(|i| sizes[*i].0 + 12.0 * z).sum::<f64>() - 12.0 * z;
                        let mut xx = l + (avail - rw).max(0.0) / 2.0;
                        for i in row {
                            pl[*i] = (xx, y0 + k as f64 * lh);
                            xx += sizes[*i].0 + 12.0 * z;
                        }
                    }
                    place = pl;
                    if pos == "t" { t += total_h + 6.0 * z } else { b -= total_h + 6.0 * z }
                }
            }
            for (i, e) in entries.iter().enumerate() {
                let (ex, ey) = place[i];
                if ex < -1e8 {
                    continue;
                }
                let cy = ey + lh / 2.0;
                rgb(cr, e.color);
                let kw = if e.line {
                    cairo_set_line_width(cr, 2.25 * z);
                    cairo_move_to(cr, ex, cy);
                    cairo_line_to(cr, ex + key * 2.5, cy);
                    cairo_stroke(cr);
                    if e.marker {
                        cairo_arc(cr, ex + key * 1.25, cy, 2.5 * z, 0.0, 2.0 * PI);
                        cairo_fill(cr);
                    }
                    key * 2.5
                } else {
                    cairo_rectangle(cr, ex, cy - key / 2.0, key, key);
                    cairo_fill(cr);
                    key
                };
                pen.set(&e.text, &f, z, None);
                rgb(cr, TEXT);
                pen.show(cr, ex + kw + gap, ey);
            }
        }
        if r - l < 10.0 || b - t < 10.0 {
            cairo_restore(cr);
            return;
        }
        if pie {
            self.paint_pie(ch, &data, cr, l, t, r, b, z, pen);
        } else {
            self.paint_axes_chart(ch, &data, &cats, cr, l, t, r, b, z, pen);
        }
        cairo_restore(cr);
    }

    unsafe fn paint_pie(&self, ch: &Chart, data: &[Vec<SerData>], cr: *mut cairo_t, l: f64, t: f64, r: f64, b: f64, z: f64, pen: &mut Pen) {
        let p = &ch.plots[0];
        let s0 = match data.first().and_then(|d| d.first()) {
            Some(s) => s,
            None => return,
        };
        let total: f64 = s0.vals.iter().map(|v| v.unwrap_or(0.0).abs()).sum();
        if total <= 0.0 {
            return;
        }
        match p.file_tag.as_deref() {
            Some("pie3DChart") => return self.paint_pie_3d(ch, s0, total, cr, l, t, r, b, z, pen),
            Some("ofPieChart") => return self.paint_of_pie(ch, s0, cr, l, t, r, b, z, pen),
            _ => {}
        }
        let cx = (l + r) / 2.0;
        let cy = (t + b) / 2.0;
        let rad = ((r - l).min(b - t) / 2.0) * 0.92;
        let hole = if p.kind == Kind::Doughnut { rad * (p.hole as f64 / 100.0).clamp(0.1, 0.9) } else { 0.0 };
        let mut a = -PI / 2.0 + p.first_angle as f64 * PI / 180.0;
        let theme = &self.styles.theme;
        let f = self.chart_font(9.0, false);
        let mut labels = Vec::new();
        for (i, v) in s0.vals.iter().enumerate() {
            let v = v.unwrap_or(0.0).abs();
            if v == 0.0 {
                continue;
            }
            let sweep = v / total * 2.0 * PI;
            let color = auto_color(theme, ch.palette, i);
            rgb(cr, color);
            cairo_new_path(cr);
            if hole > 0.0 {
                cairo_arc(cr, cx, cy, rad, a, a + sweep);
                cairo_arc_negative(cr, cx, cy, hole, a + sweep, a);
            } else {
                cairo_move_to(cr, cx, cy);
                cairo_arc(cr, cx, cy, rad, a, a + sweep);
            }
            cairo_close_path(cr);
            cairo_fill_preserve(cr);
            rgb(cr, 0xFFFFFF);
            cairo_set_line_width(cr, 1.5 * z);
            cairo_stroke(cr);
            if s0.labels {
                let mid = a + sweep / 2.0;
                let rr = if hole > 0.0 { (rad + hole) / 2.0 } else { rad * 0.65 };
                labels.push((format!("{:.0}%", v / total * 100.0), cx + rr * mid.cos(), cy + rr * mid.sin(), color));
            }
            a += sweep;
        }
        for (text, x, y, color) in labels {
            let (tw, th) = pen.set(&text, &f, z, None);
            rgb(cr, if luma(color) < 140.0 { 0xFFFFFF } else { 0x404040 });
            pen.show(cr, x - tw / 2.0, y - th / 2.0);
        }
    }

    /// A pie tilted back, with its edge showing at the front.
    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_pie_3d(&self, ch: &Chart, s0: &SerData, total: f64, cr: *mut cairo_t, l: f64, t: f64, r: f64, b: f64, z: f64, pen: &mut Pen) {
        let theme = &self.styles.theme;
        let rad = ((r - l) / 2.0).min((b - t) / 2.0 / 0.7) * 0.92;
        let ry = rad * 0.55;
        let depth = rad * 0.16;
        let (cx, cy) = ((l + r) / 2.0, (t + b) / 2.0 - depth / 2.0);
        let p = &ch.plots[0];
        let mut slices = Vec::new();
        let mut a = -PI / 2.0 + p.first_angle as f64 * PI / 180.0;
        for (i, v) in s0.vals.iter().enumerate() {
            let v = v.unwrap_or(0.0).abs();
            if v == 0.0 {
                continue;
            }
            let sweep = v / total * 2.0 * PI;
            slices.push((a, a + sweep, auto_color(theme, ch.palette, i), v));
            a += sweep;
        }
        let ell = |ang: f64, dy: f64| (cx + rad * ang.cos(), cy + ry * ang.sin() + dy);
        // the edge: the parts of each slice in the front half (angles 0..pi on screen)
        for (a0, a1, c, _) in &slices {
            for k in -1..=2 {
                let lo = (k as f64) * 2.0 * PI;
                let (s1, e1) = (a0.max(lo), a1.min(lo + PI));
                if e1 <= s1 {
                    continue;
                }
                rgb(cr, shade(*c, 0.3));
                cairo_new_path(cr);
                let steps = (((e1 - s1) / 0.05).ceil() as usize).max(2);
                for st in 0..=steps {
                    let q = ell(s1 + (e1 - s1) * st as f64 / steps as f64, 0.0);
                    if st == 0 { cairo_move_to(cr, q.0, q.1) } else { cairo_line_to(cr, q.0, q.1) }
                }
                for st in (0..=steps).rev() {
                    let q = ell(s1 + (e1 - s1) * st as f64 / steps as f64, depth);
                    cairo_line_to(cr, q.0, q.1);
                }
                cairo_close_path(cr);
                cairo_fill(cr);
            }
        }
        // the top
        let f = self.chart_font(9.0, false);
        for (a0, a1, c, v) in &slices {
            cairo_save(cr);
            cairo_translate(cr, cx, cy);
            cairo_scale(cr, 1.0, ry / rad);
            cairo_new_path(cr);
            cairo_move_to(cr, 0.0, 0.0);
            cairo_arc(cr, 0.0, 0.0, rad, *a0, *a1);
            cairo_close_path(cr);
            cairo_restore(cr);
            rgb(cr, *c);
            cairo_fill_preserve(cr);
            rgb(cr, 0xFFFFFF);
            cairo_set_line_width(cr, 1.0 * z);
            cairo_stroke(cr);
            if s0.labels {
                let mid = (a0 + a1) / 2.0;
                let text = format!("{:.0}%", v / total * 100.0);
                let (tw, th) = pen.set(&text, &f, z, None);
                rgb(cr, if luma(*c) < 140.0 { 0xFFFFFF } else { 0x404040 });
                pen.show(cr, cx + rad * 0.62 * mid.cos() - tw / 2.0, cy + ry * 0.62 * mid.sin() - th / 2.0);
            }
        }
    }

    /// Pie-of-pie and bar-of-pie: the last points taken out of the pie into a second pie or a stacked bar.
    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_of_pie(&self, ch: &Chart, s0: &SerData, cr: *mut cairo_t, l: f64, t: f64, r: f64, b: f64, z: f64, pen: &mut Pen) {
        let theme = &self.styles.theme;
        let p = &ch.plots[0];
        let vals: Vec<f64> = s0.vals.iter().map(|v| v.unwrap_or(0.0).abs()).collect();
        let n = vals.len();
        let k = crate::chart::of_split(p, n);
        let (main, second) = vals.split_at(n - k);
        let other: f64 = second.iter().sum();
        let total: f64 = main.iter().sum::<f64>() + other;
        if total <= 0.0 {
            return;
        }
        let w = r - l;
        let cy = (t + b) / 2.0;
        let rad = (w * 0.26).min((b - t) / 2.0 * 0.9);
        let cx1 = l + w * 0.3;
        let cx2 = l + w * 0.78;
        let f = self.chart_font(9.0, false);
        let mut labels: Vec<(String, f64, f64, u32)> = Vec::new();
        let other_sweep = other / total * 2.0 * PI;
        // the main pie: the combined slice faces the second plot
        let mut a = other_sweep / 2.0;
        let slice = |a0: f64, a1: f64, c: u32, cx: f64, rr: f64| {
            rgb(cr, c);
            cairo_new_path(cr);
            cairo_move_to(cr, cx, cy);
            cairo_arc(cr, cx, cy, rr, a0, a1);
            cairo_close_path(cr);
            cairo_fill_preserve(cr);
            rgb(cr, 0xFFFFFF);
            cairo_set_line_width(cr, 1.5 * z);
            cairo_stroke(cr);
        };
        for (i, v) in main.iter().enumerate() {
            if *v == 0.0 {
                continue;
            }
            let sw = v / total * 2.0 * PI;
            let c = auto_color(theme, ch.palette, i);
            slice(a, a + sw, c, cx1, rad);
            if s0.labels {
                let m = a + sw / 2.0;
                labels.push((format!("{:.0}%", v / total * 100.0), cx1 + rad * 0.65 * m.cos(), cy + rad * 0.65 * m.sin(), c));
            }
            a += sw;
        }
        if other > 0.0 {
            let c = auto_color(theme, ch.palette, n);
            slice(-other_sweep / 2.0, other_sweep / 2.0, c, cx1, rad);
            if s0.labels {
                labels.push((format!("{:.0}%", other / total * 100.0), cx1 + rad * 0.65, cy, c));
            }
        }
        // the second plot and the lines joining it to the combined slice
        let (top_pt, bot_pt) = ((cx1 + rad * (other_sweep / 2.0).cos(), cy - rad * (other_sweep / 2.0).sin()),
                                (cx1 + rad * (other_sweep / 2.0).cos(), cy + rad * (other_sweep / 2.0).sin()));
        let (t2, b2, left2);
        if p.of_bar {
            let bh = rad * 2.0 * 0.85;
            let bw = rad * 0.45;
            let x0 = cx2 - bw / 2.0;
            let mut y = cy - bh / 2.0;
            for (j, v) in second.iter().enumerate() {
                let hh = if other > 0.0 { v / other * bh } else { 0.0 };
                let c = auto_color(theme, ch.palette, n - k + j);
                rgb(cr, c);
                cairo_rectangle(cr, x0, y, bw, hh);
                cairo_fill_preserve(cr);
                rgb(cr, 0xFFFFFF);
                cairo_set_line_width(cr, 1.5 * z);
                cairo_stroke(cr);
                if s0.labels {
                    labels.push((format!("{:.0}%", v / total * 100.0), cx2, y + hh / 2.0, c));
                }
                y += hh;
            }
            t2 = (x0, cy - bh / 2.0);
            b2 = (x0, cy + bh / 2.0);
            left2 = x0;
        } else {
            let rad2 = rad * 0.75;
            let mut a2 = PI - (if other > 0.0 { second[0] / other } else { 0.0 }) * PI;
            for (j, v) in second.iter().enumerate() {
                if *v == 0.0 {
                    continue;
                }
                let sw = if other > 0.0 { v / other * 2.0 * PI } else { 0.0 };
                let c = auto_color(theme, ch.palette, n - k + j);
                slice(a2, a2 + sw, c, cx2, rad2);
                if s0.labels {
                    let m = a2 + sw / 2.0;
                    labels.push((format!("{:.0}%", v / total * 100.0), cx2 + rad2 * 0.62 * m.cos(), cy + rad2 * 0.62 * m.sin(), c));
                }
                a2 += sw;
            }
            t2 = (cx2, cy - rad2);
            b2 = (cx2, cy + rad2);
            left2 = cx2 - rad2;
        }
        let _ = left2;
        rgb(cr, 0xA6A6A6);
        cairo_set_line_width(cr, 1.0 * z);
        cairo_move_to(cr, top_pt.0, top_pt.1);
        cairo_line_to(cr, t2.0, t2.1);
        cairo_move_to(cr, bot_pt.0, bot_pt.1);
        cairo_line_to(cr, b2.0, b2.1);
        cairo_stroke(cr);
        for (text, x, y, c) in labels {
            let (tw, th) = pen.set(&text, &f, z, None);
            rgb(cr, if luma(c) < 140.0 { 0xFFFFFF } else { 0x404040 });
            pen.show(cr, x - tw / 2.0, y - th / 2.0);
        }
    }

    unsafe fn paint_axes_chart(&self, ch: &Chart, data: &[Vec<SerData>], cats: &[String], cr: *mut cairo_t, l: f64, t: f64, r: f64, b: f64, z: f64, pen: &mut Pen) {
        let f = self.chart_font(9.0, false);
        let ft = self.chart_font(10.0, false);
        let horizontal = ch.plots.first().map(|p| p.horizontal && p.kind == Kind::Bar).unwrap_or(false);
        let scatter = ch.plots.first().map(|p| p.kind == Kind::Scatter).unwrap_or(false);
        let ncat = data.iter().flatten().map(|s| s.vals.len()).max().unwrap_or(0).max(cats.len()).max(1);
        // ---- value ranges per axis
        let range = |secondary: bool| -> Option<(f64, f64, bool)> {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            let mut percent = false;
            for (p, ser) in ch.plots.iter().zip(data) {
                if p.secondary != secondary {
                    continue;
                }
                match p.group {
                    Group::Clustered => {
                        for s in ser {
                            for v in s.vals.iter().flatten() {
                                lo = lo.min(*v);
                                hi = hi.max(*v);
                            }
                        }
                    }
                    Group::Stacked | Group::Percent => {
                        percent |= p.group == Group::Percent;
                        for i in 0..ncat {
                            let (mut pos, mut neg, mut tot) = (0.0, 0.0, 0.0);
                            for s in ser {
                                let v = s.vals.get(i).copied().flatten().unwrap_or(0.0);
                                if v >= 0.0 { pos += v } else { neg += v }
                                tot += v.abs();
                            }
                            if p.group == Group::Percent && tot > 0.0 {
                                pos /= tot;
                                neg /= tot;
                            }
                            lo = lo.min(neg);
                            hi = hi.max(pos);
                        }
                    }
                }
            }
            if lo > hi { None } else { Some((lo, hi, percent)) }
        };
        let fmt_of = |secondary: bool| -> String {
            ch.plots.iter().find(|p| p.secondary == secondary).and_then(|p| p.series.first()).and_then(|s| s.val.as_ref())
                .map(|v| self.chart_format(v)).unwrap_or_else(|| "General".into())
        };
        let (_, th) = pen.set("0", &f, z, None);
        let len_val = if horizontal || false { r - l } else { b - t };
        let ticks = (len_val / (th * 1.8)).clamp(2.0, 12.0);
        let mk = |rg: Option<(f64, f64, bool)>| -> Option<(f64, f64, f64, bool)> {
            let (lo, hi, pc) = rg?;
            let (a, b2, st) = if pc {
                let (a, b2, st) = nice_axis(lo, hi, ticks, true);
                (a.max(-1.0), b2.min(1.0), st)
            } else {
                nice_axis(lo, hi, ticks, true)
            };
            Some((a, b2, st, pc))
        };
        let prim = mk(range(false)).unwrap_or((0.0, 1.0, 0.2, false));
        let sec = mk(range(true));
        // x values (scatter)
        let xax = if scatter {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for s in data.iter().flatten() {
                for i in 0..s.vals.len() {
                    let xv = if s.xs.is_empty() { Some(i as f64 + 1.0) } else { s.xs.get(i).copied().flatten() };
                    if let Some(xv) = xv {
                        lo = lo.min(xv);
                        hi = hi.max(xv);
                    }
                }
            }
            let xticks = ((r - l) / 60.0 / z).clamp(2.0, 10.0);
            if lo > hi { Some((0.0, 1.0, 0.2)) } else { Some(nice_axis(lo, hi, xticks, true)) }
        } else {
            None
        };
        let label = |v: f64, pc: bool, fmt: &str| -> String {
            if pc { format!("{:.0}%", v * 100.0) } else { crate::numfmt::format_number(clean(v), fmt, self.date1904) }
        };
        let ticks_of = |(lo, hi, st): (f64, f64, f64)| -> Vec<f64> {
            let mut out = Vec::new();
            let n = ((hi - lo) / st).round() as i64;
            for k in 0..=n.min(200) {
                out.push(clean(lo + k as f64 * st));
            }
            out
        };
        let pfmt = fmt_of(false);
        let sfmt = fmt_of(true);
        let prim_labels: Vec<(f64, String)> = ticks_of((prim.0, prim.1, prim.2)).into_iter().map(|v| (v, label(v, prim.3, &pfmt))).collect();
        let sec_labels: Vec<(f64, String)> = sec.map(|s| ticks_of((s.0, s.1, s.2)).into_iter().map(|v| (v, label(v, s.3, &sfmt))).collect()).unwrap_or_default();
        let x_labels: Vec<(f64, String)> = xax.map(|a| ticks_of(a).into_iter().map(|v| (v, crate::numfmt::format_number(v, "General", self.date1904))).collect()).unwrap_or_default();
        let maxw = |v: &[(f64, String)], pen: &mut Pen| v.iter().map(|(_, s)| pen.set(s, &f, z, None).0).fold(0.0, f64::max);
        // ---- room for labels and titles
        let mut pl = l;
        let mut pr = r;
        let pt = t + th / 2.0;
        let mut pb = b;
        // axis titles
        let (xt, yt) = if horizontal { (ch.y_title.clone(), ch.x_title.clone()) } else { (ch.x_title.clone(), ch.y_title.clone()) };
        let yt_h = yt.as_ref().filter(|s| !s.is_empty()).map(|s| pen.set(s, &ft, z, None).1 + 4.0 * z).unwrap_or(0.0);
        let xt_h = xt.as_ref().filter(|s| !s.is_empty()).map(|s| pen.set(s, &ft, z, None).1 + 4.0 * z).unwrap_or(0.0);
        pl += yt_h;
        pb -= xt_h;
        let cat_texts: Vec<String> = (0..ncat).map(|i| cats.get(i).cloned().unwrap_or_else(|| format!("{}", i + 1))).collect();
        if horizontal {
            let cw = cat_texts.iter().map(|s| pen.set(s, &f, z, None).0).fold(0.0, f64::max).min((r - l) * 0.4);
            pl += cw + 6.0 * z;
            pb -= th + 4.0 * z;
            let lastw = prim_labels.last().map(|x| pen.set(&x.1, &f, z, None).0).unwrap_or(0.0);
            pr -= lastw / 2.0;
        } else {
            pl += maxw(&prim_labels, pen) + 6.0 * z;
            pb -= th + 4.0 * z;
            if !sec_labels.is_empty() {
                pr -= maxw(&sec_labels, pen) + 6.0 * z;
            } else {
                pr -= 4.0 * z;
            }
            if scatter {
                let lastw = x_labels.last().map(|x| pen.set(&x.1, &f, z, None).0).unwrap_or(0.0);
                pr -= lastw / 2.0;
            }
        }
        if pr - pl < 10.0 || pb - pt < 10.0 {
            return;
        }
        // ---- 3-D: room for the depth, one row per series where they stand one behind another
        let p3 = ch.plots.first().filter(|p| p.is_3d());
        let nser = data.first().map(|d| d.len()).unwrap_or(1).max(1);
        let rows3 = match p3 {
            Some(p) if p.standard3d || p.file_tag.as_deref() == Some("line3DChart")
                || (p.file_tag.as_deref() == Some("area3DChart") && p.group == Group::Clustered) => nser,
            Some(_) => 1,
            None => 0,
        };
        let (ddx, ddy) = if rows3 > 0 {
            let len = if horizontal { pb - pt } else { pr - pl };
            let unit = (len / ncat as f64 * 0.45).min(len * 0.25 / rows3 as f64).min(40.0 * z).max(3.0 * z);
            (unit * 0.85, unit * 0.5)
        } else {
            (0.0, 0.0)
        };
        let (dxt, dyt) = (ddx * rows3 as f64, ddy * rows3 as f64);
        let pr = pr - dxt;
        let pt = pt + dyt;
        if pr - pl < 10.0 || pb - pt < 10.0 {
            return;
        }
        // value -> screen
        let vmap = |v: f64, ax: (f64, f64, f64, bool)| -> f64 {
            let k = (v - ax.0) / (ax.1 - ax.0);
            if horizontal { pl + k * (pr - pl) } else { pb - k * (pb - pt) }
        };
        let xmap = |v: f64| -> f64 {
            let a = xax.unwrap();
            pl + (v - a.0) / (a.1 - a.0) * (pr - pl)
        };
        // ---- gridlines and value labels
        cairo_set_line_width(cr, z.max(1.0));
        for (v, s) in &prim_labels {
            let p = vmap(*v, prim).round() + 0.5;
            if ch.gridlines {
                rgb(cr, LINE);
                if rows3 > 0 {
                    // on the back wall, joined to the front along the side wall (floor)
                    if horizontal {
                        cairo_move_to(cr, p, pb);
                        cairo_line_to(cr, p + dxt, pb - dyt);
                        cairo_line_to(cr, p + dxt, pt - dyt);
                    } else {
                        cairo_move_to(cr, pl, p);
                        cairo_line_to(cr, pl + dxt, p - dyt);
                        cairo_line_to(cr, pr + dxt, p - dyt);
                    }
                } else if horizontal {
                    cairo_move_to(cr, p, pt);
                    cairo_line_to(cr, p, pb);
                } else {
                    cairo_move_to(cr, pl, p);
                    cairo_line_to(cr, pr, p);
                }
                cairo_stroke(cr);
            }
            let (tw, h2) = pen.set(s, &f, z, None);
            rgb(cr, TEXT);
            if horizontal {
                pen.show(cr, p - tw / 2.0, pb + 4.0 * z);
            } else {
                pen.show(cr, pl - 6.0 * z - tw, p - h2 / 2.0);
            }
        }
        if let Some(sa) = sec {
            for (v, s) in &sec_labels {
                let p = vmap(*v, sa);
                let (_, h2) = pen.set(s, &f, z, None);
                rgb(cr, TEXT);
                pen.show(cr, pr + 6.0 * z, p - h2 / 2.0);
            }
        }
        // ---- category / x axis
        if scatter {
            for (v, s) in &x_labels {
                let p = xmap(*v).round() + 0.5;
                if ch.gridlines && false {
                    rgb(cr, LINE);
                    cairo_move_to(cr, p, pt);
                    cairo_line_to(cr, p, pb);
                    cairo_stroke(cr);
                }
                let (tw, _) = pen.set(s, &f, z, None);
                rgb(cr, TEXT);
                pen.show(cr, p - tw / 2.0, pb + 4.0 * z);
            }
        } else {
            let slot = if horizontal { (pb - pt) / ncat as f64 } else { (pr - pl) / ncat as f64 };
            let widest = cat_texts.iter().map(|s| pen.set(s, &f, z, None).0).fold(0.0, f64::max);
            let every = if horizontal { ((th + 2.0) / slot).ceil().max(1.0) as usize } else { ((widest + 6.0 * z) / slot).ceil().max(1.0) as usize };
            for (i, s) in cat_texts.iter().enumerate() {
                if i % every != 0 {
                    continue;
                }
                let (tw, h2) = pen.set(s, &f, z, None);
                rgb(cr, TEXT);
                if horizontal {
                    // the first category at the bottom
                    let cy = pb - (i as f64 + 0.5) * slot;
                    pen.show(cr, pl - 6.0 * z - tw.min((r - l) * 0.4), cy - h2 / 2.0);
                } else {
                    let cx = pl + (i as f64 + 0.5) * slot;
                    pen.show(cr, cx - tw / 2.0, pb + 4.0 * z);
                }
            }
        }
        // the category axis line, at zero
        let zero = vmap(if prim.0 > 0.0 { prim.0 } else if prim.1 < 0.0 { prim.1 } else { 0.0 }, prim).round() + 0.5;
        rgb(cr, LINE);
        if horizontal {
            cairo_move_to(cr, zero, pt);
            cairo_line_to(cr, zero, pb);
        } else if scatter {
            cairo_move_to(cr, pl, zero);
            cairo_line_to(cr, pr, zero);
        } else {
            cairo_move_to(cr, pl, zero);
            cairo_line_to(cr, pr, zero);
        }
        cairo_stroke(cr);
        // ---- titles
        if let Some(s) = xt.filter(|s| !s.is_empty()) {
            let (tw, _) = pen.set(&s, &ft, z, None);
            rgb(cr, TEXT);
            pen.show(cr, (pl + pr) / 2.0 - tw / 2.0, b - xt_h + 2.0 * z);
        }
        if let Some(s) = yt.filter(|s| !s.is_empty()) {
            let (tw, _) = pen.set(&s, &ft, z, None);
            rgb(cr, TEXT);
            cairo_save(cr);
            cairo_translate(cr, l, (pt + pb) / 2.0 + tw / 2.0);
            cairo_rotate(cr, -PI / 2.0);
            pango_cairo_update_layout(cr, pen.layout);
            pen.show(cr, 0.0, 0.0);
            cairo_restore(cr);
            pango_cairo_update_layout(cr, pen.layout);
        }
        if rows3 > 0 {
            rgb(cr, LINE);
            cairo_move_to(cr, pl, pb);
            cairo_line_to(cr, pl + dxt, pb - dyt);
            cairo_line_to(cr, pr + dxt, pb - dyt);
            cairo_line_to(cr, pr, pb);
            cairo_move_to(cr, pl + dxt, pb - dyt);
            cairo_line_to(cr, pl + dxt, pt - dyt);
            cairo_stroke(cr);
        }
        // ---- series
        cairo_save(cr);
        cairo_rectangle(cr, pl - 6.0 * z, pt - dyt - 6.0 * z, pr - pl + dxt + 12.0 * z, pb - pt + dyt + 12.0 * z);
        cairo_clip(cr);
        cairo_set_line_join(cr, 1);
        cairo_set_line_cap(cr, 1);
        let nbar = ch.plots.iter().filter(|p| p.kind == Kind::Bar).count().max(1);
        let _ = nbar;
        let mut labels: Vec<(String, f64, f64)> = Vec::new();
        for (p, ser) in ch.plots.iter().zip(data) {
            let ax = if p.secondary { sec.unwrap_or(prim) } else { prim };
            let fmt = if p.secondary { &sfmt } else { &pfmt };
            match p.kind {
                Kind::Bar if p.is_3d() => self.paint_bars_3d(p, ser, ncat, ax, horizontal, pl, pt, pr, pb, cr, &vmap, (ddx, ddy), fmt, &mut labels),
                Kind::Bar => self.paint_bars(p, ser, ncat, ax, horizontal, pl, pt, pr, pb, cr, &vmap, fmt, &mut labels),
                Kind::Line | Kind::Area if p.is_3d() => self.paint_lines_3d(p, ser, ncat, ax, pl, pr, cr, &vmap, z, (ddx, ddy), rows3 > 1),
                Kind::Line | Kind::Area => self.paint_lines(p, ser, ncat, ax, pl, pr, cr, &vmap, z, fmt, &mut labels),
                Kind::Scatter if p.file_tag.as_deref() == Some("bubbleChart") => {
                    let maxsz = ser.iter().flat_map(|s| s.sizes.iter().flatten()).fold(0.0f64, |m, v| m.max(v.abs()));
                    let rmax = 0.125 * (pr - pl).min(pb - pt);
                    for s in ser {
                        for i in 0..s.vals.len() {
                            let xv = if s.xs.is_empty() { Some(i as f64 + 1.0) } else { s.xs.get(i).copied().flatten() };
                            let (xv, yv) = match (xv, s.vals[i]) {
                                (Some(a), Some(b)) => (a, b),
                                _ => continue,
                            };
                            let sz = s.sizes.get(i).copied().flatten().unwrap_or(if s.sizes.is_empty() { 1.0 } else { 0.0 });
                            if sz <= 0.0 || maxsz <= 0.0 && !s.sizes.is_empty() {
                                continue;
                            }
                            let rr = if maxsz > 0.0 { (sz / maxsz).sqrt() * rmax } else { rmax * 0.5 };
                            let (x, y) = (xmap(xv), vmap(yv, ax));
                            if p.bubble3d {
                                let g = cairo_pattern_create_radial(x - rr * 0.35, y - rr * 0.35, rr * 0.05, x, y, rr);
                                let hi = lighten(s.color, 0.6);
                                let ch2 = |c: u32, k: u32| ((c >> k) & 255) as f64 / 255.0;
                                cairo_pattern_add_color_stop_rgb(g, 0.0, ch2(hi, 16), ch2(hi, 8), ch2(hi, 0));
                                cairo_pattern_add_color_stop_rgb(g, 1.0, ch2(shade(s.color, 0.2), 16), ch2(shade(s.color, 0.2), 8), ch2(shade(s.color, 0.2), 0));
                                cairo_set_source(cr, g);
                                cairo_arc(cr, x, y, rr, 0.0, 2.0 * PI);
                                cairo_fill(cr);
                                cairo_pattern_destroy(g);
                            } else {
                                rgb(cr, s.color);
                                cairo_arc(cr, x, y, rr, 0.0, 2.0 * PI);
                                cairo_fill_preserve(cr);
                                rgb(cr, 0xFFFFFF);
                                cairo_set_line_width(cr, 0.75 * z);
                                cairo_stroke(cr);
                            }
                            if s.labels {
                                labels.push((label(yv, false, fmt), x, y + 4.0 * z));
                            }
                        }
                    }
                }
                Kind::Scatter => {
                    for s in ser {
                        let pts: Vec<Option<(f64, f64)>> = (0..s.vals.len()).map(|i| {
                            let xv = if s.xs.is_empty() { Some(i as f64 + 1.0) } else { s.xs.get(i).copied().flatten() };
                            Some((xmap(xv?), vmap(s.vals[i]?, ax)))
                        }).collect();
                        rgb(cr, s.color);
                        if p.lines {
                            cairo_set_line_width(cr, 2.25 * z);
                            stroke_runs(cr, &pts, s.smooth);
                        }
                        if p.markers || !p.lines {
                            for (x, y) in pts.iter().flatten() {
                                cairo_arc(cr, *x, *y, 3.0 * z, 0.0, 2.0 * PI);
                                cairo_fill(cr);
                            }
                        }
                        if s.labels {
                            for (i, pt2) in pts.iter().enumerate() {
                                if let (Some((x, y)), Some(v)) = (pt2, s.vals[i]) {
                                    labels.push((label(v, false, fmt), *x, *y - 4.0 * z));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        cairo_restore(cr);
        for (text, x, y) in labels {
            let (tw, h2) = pen.set(&text, &f, z, None);
            rgb(cr, 0x404040);
            pen.show(cr, x - tw / 2.0, y - h2);
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_bars(&self, p: &Plot, ser: &[SerData], ncat: usize, ax: (f64, f64, f64, bool), horizontal: bool,
                         pl: f64, pt: f64, pr: f64, pb: f64, cr: *mut cairo_t, vmap: &dyn Fn(f64, (f64, f64, f64, bool)) -> f64,
                         fmt: &str, labels: &mut Vec<(String, f64, f64)>) {
        let slot = if horizontal { (pb - pt) / ncat as f64 } else { (pr - pl) / ncat as f64 };
        let g = p.gap as f64 / 100.0;
        let n = ser.len().max(1) as f64;
        let stacked = p.group != Group::Clustered;
        let ov = if stacked { 1.0 } else { (p.overlap as f64 / 100.0).clamp(-1.0, 1.0) };
        let bw = slot / (n - (n - 1.0) * ov + g);
        let group_w = bw * (n - (n - 1.0) * ov);
        let base_v = if ax.0 > 0.0 { ax.0 } else if ax.1 < 0.0 { ax.1 } else { 0.0 };
        for i in 0..ncat {
            let center = if horizontal { pb - (i as f64 + 0.5) * slot } else { pl + (i as f64 + 0.5) * slot };
            let start = center - group_w / 2.0;
            let tot: f64 = ser.iter().map(|s| s.vals.get(i).copied().flatten().unwrap_or(0.0).abs()).sum();
            let (mut pos, mut neg) = (0.0, 0.0);
            for (j, s) in ser.iter().enumerate() {
                let v = match s.vals.get(i).copied().flatten() {
                    Some(v) => v,
                    None => continue,
                };
                let v2 = if p.group == Group::Percent && tot > 0.0 { v / tot } else { v };
                let (a, bb) = if stacked {
                    if v2 >= 0.0 { let a = pos; pos += v2; (a, pos) } else { let a = neg; neg += v2; (a, neg) }
                } else {
                    (base_v, v2)
                };
                let off = if stacked { 0.0 } else { j as f64 * bw * (1.0 - ov) };
                let p0 = vmap(a, ax);
                let p1 = vmap(bb, ax);
                rgb(cr, s.color);
                if horizontal {
                    // series run top to bottom in a category, as Excel draws them
                    let y0 = start + group_w - off - bw;
                    cairo_rectangle(cr, p0.min(p1), y0, (p1 - p0).abs(), bw);
                    cairo_fill(cr);
                    if s.labels {
                        labels.push((self.fmt_label(v, fmt), p1.max(p0) + 12.0, y0 + bw / 2.0 + 6.0));
                    }
                } else {
                    let x0 = start + off;
                    cairo_rectangle(cr, x0, p0.min(p1), bw, (p1 - p0).abs());
                    cairo_fill(cr);
                    if s.labels {
                        labels.push((self.fmt_label(v, fmt), x0 + bw / 2.0, if stacked { (p0 + p1) / 2.0 + 6.0 } else { p1.min(p0) - 2.0 }));
                    }
                }
            }
        }
    }

    /// 3-D column and bar charts: boxes with depth; "3-D Column" puts each series in its own row going back.
    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_bars_3d(&self, p: &Plot, ser: &[SerData], ncat: usize, ax: (f64, f64, f64, bool), horizontal: bool,
                            pl: f64, pt: f64, pr: f64, pb: f64, cr: *mut cairo_t, vmap: &dyn Fn(f64, (f64, f64, f64, bool)) -> f64,
                            (ddx, ddy): (f64, f64), fmt: &str, labels: &mut Vec<(String, f64, f64)>) {
        let slot = if horizontal { (pb - pt) / ncat as f64 } else { (pr - pl) / ncat as f64 };
        let g = p.gap as f64 / 100.0;
        let base_v = if ax.0 > 0.0 { ax.0 } else if ax.1 < 0.0 { ax.1 } else { 0.0 };
        if p.standard3d && !horizontal {
            let bw = slot / (1.0 + g);
            for (j, s) in ser.iter().enumerate().rev() {
                let (ox, oy) = (ddx * j as f64, -ddy * j as f64);
                for i in 0..ncat {
                    let v = match s.vals.get(i).copied().flatten() {
                        Some(v) => v,
                        None => continue,
                    };
                    let x0 = pl + (i as f64 + 0.5) * slot - bw / 2.0 + ox + ddx * 0.1;
                    let (p0, p1) = (vmap(base_v, ax) + oy, vmap(v, ax) + oy);
                    prism(cr, x0, p0.min(p1), bw * 0.8, (p1 - p0).abs(), ddx * 0.8, ddy * 0.8, s.color);
                    if s.labels {
                        labels.push((self.fmt_label(v, fmt), x0 + bw * 0.4, p1.min(p0) - ddy - 2.0));
                    }
                }
            }
            return;
        }
        let n = ser.len().max(1) as f64;
        let stacked = p.group != Group::Clustered;
        let bw = if stacked { slot / (1.0 + g) } else { slot / (n + g) };
        let order: Vec<usize> = if horizontal { (0..ncat).rev().collect() } else { (0..ncat).collect() };
        for i in order {
            let center = if horizontal { pb - (i as f64 + 0.5) * slot } else { pl + (i as f64 + 0.5) * slot };
            let start = center - if stacked { bw / 2.0 } else { bw * n / 2.0 };
            let tot: f64 = ser.iter().map(|s| s.vals.get(i).copied().flatten().unwrap_or(0.0).abs()).sum();
            let (mut pos, mut neg) = (0.0, 0.0);
            for (j, s) in ser.iter().enumerate() {
                let v = match s.vals.get(i).copied().flatten() {
                    Some(v) => v,
                    None => continue,
                };
                let v2 = if p.group == Group::Percent && tot > 0.0 { v / tot } else { v };
                let (a, bb) = if stacked {
                    if v2 >= 0.0 { let a = pos; pos += v2; (a, pos) } else { let a = neg; neg += v2; (a, neg) }
                } else {
                    (base_v, v2)
                };
                let off = if stacked { 0.0 } else { j as f64 * bw };
                let (p0, p1) = (vmap(a, ax), vmap(bb, ax));
                if horizontal {
                    let y0 = start + if stacked { 0.0 } else { (n - 1.0) * bw - off };
                    prism(cr, p0.min(p1), y0 + ddy * 0.2, (p1 - p0).abs(), bw * 0.8, ddx * 0.8, ddy * 0.8, s.color);
                    if s.labels {
                        labels.push((self.fmt_label(v, fmt), p1.max(p0) + ddx + 12.0, y0 + bw / 2.0 + 6.0));
                    }
                } else {
                    let x0 = start + off;
                    prism(cr, x0, p0.min(p1), bw * 0.9, (p1 - p0).abs(), ddx * 0.8, ddy * 0.8, s.color);
                    if s.labels {
                        labels.push((self.fmt_label(v, fmt), x0 + bw / 2.0, if stacked { (p0 + p1) / 2.0 + 6.0 } else { p1.min(p0) - ddy - 2.0 }));
                    }
                }
            }
        }
    }

    /// 3-D lines (ribbons, one row per series) and 3-D areas (slabs; a row per series when not stacked).
    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_lines_3d(&self, p: &Plot, ser: &[SerData], ncat: usize, ax: (f64, f64, f64, bool), pl: f64, pr: f64,
                             cr: *mut cairo_t, vmap: &dyn Fn(f64, (f64, f64, f64, bool)) -> f64, z: f64, (ddx, ddy): (f64, f64), rows: bool) {
        let stacked = p.group != Group::Clustered;
        let area = p.kind == Kind::Area;
        let tot: Vec<f64> = (0..ncat).map(|i| ser.iter().map(|s| s.vals.get(i).copied().flatten().unwrap_or(0.0).abs()).sum()).collect();
        let base_v = if ax.0 > 0.0 { ax.0 } else if ax.1 < 0.0 { ax.1 } else { 0.0 };
        let xs: Vec<f64> = if area {
            (0..ncat).map(|i| if ncat == 1 { (pl + pr) / 2.0 } else { pl + i as f64 * (pr - pl) / (ncat - 1) as f64 }).collect()
        } else {
            let slot = (pr - pl) / ncat as f64;
            (0..ncat).map(|i| pl + (i as f64 + 0.5) * slot).collect()
        };
        // the tops (and bottoms) of each series
        let mut below = vec![0.0f64; ncat];
        let mut layers: Vec<(Vec<f64>, Vec<f64>, u32)> = Vec::new();
        for s in ser {
            let top: Vec<f64> = (0..ncat).map(|i| {
                let v = s.vals.get(i).copied().flatten().unwrap_or(0.0);
                let v = if p.group == Group::Percent && tot[i] > 0.0 { v / tot[i] } else { v };
                if stacked { below[i] + v } else { v }
            }).collect();
            let bot: Vec<f64> = if stacked { below.clone() } else { vec![base_v; ncat] };
            if stacked {
                below = top.clone();
            }
            layers.push((top, bot, s.color));
        }
        let depth = if rows { 0.75 } else { 1.0 };
        let (rdx, rdy) = (ddx * depth, ddy * depth);
        // back rows first; the first series stands at the front
        let order: Vec<usize> = if rows { (0..layers.len()).rev().collect() } else { (0..layers.len()).collect() };
        for j in order {
            let (top, bot, c) = &layers[j];
            let (ox, oy) = if rows { (ddx * j as f64, -ddy * j as f64) } else { (0.0, 0.0) };
            let pt_of = |i: usize, v: f64| (xs[i] + ox, vmap(v, ax) + oy);
            if area {
                // the top surface, strip by strip, then the right end and the front
                for i in 0..ncat.saturating_sub(1) {
                    let (a, b) = (pt_of(i, top[i]), pt_of(i + 1, top[i + 1]));
                    rgb(cr, lighten(*c, 0.25));
                    cairo_move_to(cr, a.0, a.1);
                    cairo_line_to(cr, b.0, b.1);
                    cairo_line_to(cr, b.0 + rdx, b.1 - rdy);
                    cairo_line_to(cr, a.0 + rdx, a.1 - rdy);
                    cairo_close_path(cr);
                    cairo_fill(cr);
                }
                let (e1, e0) = (pt_of(ncat - 1, top[ncat - 1]), pt_of(ncat - 1, bot[ncat - 1]));
                rgb(cr, shade(*c, 0.3));
                cairo_move_to(cr, e1.0, e1.1);
                cairo_line_to(cr, e1.0 + rdx, e1.1 - rdy);
                cairo_line_to(cr, e0.0 + rdx, e0.1 - rdy);
                cairo_line_to(cr, e0.0, e0.1);
                cairo_close_path(cr);
                cairo_fill(cr);
                rgb(cr, *c);
                cairo_new_path(cr);
                for i in 0..ncat {
                    let q = pt_of(i, top[i]);
                    if i == 0 { cairo_move_to(cr, q.0, q.1) } else { cairo_line_to(cr, q.0, q.1) }
                }
                for i in (0..ncat).rev() {
                    let q = pt_of(i, bot[i]);
                    cairo_line_to(cr, q.0, q.1);
                }
                cairo_close_path(cr);
                cairo_fill(cr);
            } else {
                // a ribbon: each segment as a band going back, its front edge darker
                for i in 0..ncat.saturating_sub(1) {
                    let (a, b) = (pt_of(i, top[i]), pt_of(i + 1, top[i + 1]));
                    rgb(cr, if b.1 < a.1 { lighten(*c, 0.15) } else { *c });
                    cairo_move_to(cr, a.0, a.1);
                    cairo_line_to(cr, b.0, b.1);
                    cairo_line_to(cr, b.0 + rdx, b.1 - rdy);
                    cairo_line_to(cr, a.0 + rdx, a.1 - rdy);
                    cairo_close_path(cr);
                    cairo_fill(cr);
                    rgb(cr, shade(*c, 0.3));
                    cairo_set_line_width(cr, z.max(1.0));
                    cairo_move_to(cr, a.0, a.1);
                    cairo_line_to(cr, b.0, b.1);
                    cairo_stroke(cr);
                }
            }
        }
    }

    fn fmt_label(&self, v: f64, fmt: &str) -> String {
        crate::numfmt::format_number(v, fmt, self.date1904)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn paint_lines(&self, p: &Plot, ser: &[SerData], ncat: usize, ax: (f64, f64, f64, bool), pl: f64, pr: f64,
                          cr: *mut cairo_t, vmap: &dyn Fn(f64, (f64, f64, f64, bool)) -> f64, z: f64, fmt: &str,
                          labels: &mut Vec<(String, f64, f64)>) {
        let slot = (pr - pl) / ncat as f64;
        let stacked = p.group != Group::Clustered;
        let tot: Vec<f64> = (0..ncat).map(|i| ser.iter().map(|s| s.vals.get(i).copied().flatten().unwrap_or(0.0).abs()).sum()).collect();
        let mut below = vec![0.0f64; ncat];
        let base_v = if ax.0 > 0.0 { ax.0 } else if ax.1 < 0.0 { ax.1 } else { 0.0 };
        // areas are drawn back to front: the first series is behind
        for s in ser {
            let mut vals: Vec<Option<f64>> = (0..ncat).map(|i| {
                let v = s.vals.get(i).copied().flatten();
                let v = if p.group == Group::Percent { v.map(|x| if tot[i] > 0.0 { x / tot[i] } else { 0.0 }) } else { v };
                if stacked { Some(below[i] + v.unwrap_or(0.0)) } else { v }
            }).collect();
            if p.kind == Kind::Area && !stacked {
                for v in vals.iter_mut() {
                    v.get_or_insert(0.0);
                }
            }
            let pts: Vec<Option<(f64, f64)>> = vals.iter().enumerate().map(|(i, v)| v.map(|v| (pl + (i as f64 + 0.5) * slot, vmap(v, ax)))).collect();
            rgb(cr, s.color);
            if p.kind == Kind::Area {
                // from the series below (or the axis) up to this one
                let xs: Vec<f64> = (0..ncat).map(|i| if ncat == 1 { (pl + pr) / 2.0 } else { pl + i as f64 * (pr - pl) / (ncat - 1) as f64 }).collect();
                cairo_new_path(cr);
                for i in 0..ncat {
                    let y = vmap(vals[i].unwrap_or(base_v), ax);
                    if i == 0 { cairo_move_to(cr, xs[i], y) } else { cairo_line_to(cr, xs[i], y) }
                }
                for i in (0..ncat).rev() {
                    let y = vmap(if stacked { below[i] } else { base_v }, ax);
                    cairo_line_to(cr, xs[i], y);
                }
                cairo_close_path(cr);
                cairo_fill(cr);
            } else {
                cairo_set_line_width(cr, 2.25 * z);
                stroke_runs(cr, &pts, s.smooth);
                if p.markers {
                    for (x, y) in pts.iter().flatten() {
                        cairo_arc(cr, *x, *y, 3.0 * z, 0.0, 2.0 * PI);
                        cairo_fill(cr);
                    }
                }
            }
            if s.labels {
                for (i, pt) in pts.iter().enumerate() {
                    if let (Some((x, y)), Some(v)) = (pt, s.vals.get(i).copied().flatten()) {
                        labels.push((self.fmt_label(v, fmt), *x, *y - 4.0 * z));
                    }
                }
            }
            if stacked {
                for i in 0..ncat {
                    below[i] = vals[i].unwrap_or(below[i]);
                }
            }
        }
    }
}

/// Stroke the unbroken runs of points (gaps for blanks), straight or smoothed.
unsafe fn stroke_runs(cr: *mut cairo_t, pts: &[Option<(f64, f64)>], smooth: bool) {
    let mut run: Vec<(f64, f64)> = Vec::new();
    let flush = |run: &mut Vec<(f64, f64)>| {
        if run.len() >= 2 {
            cairo_new_path(cr);
            cairo_move_to(cr, run[0].0, run[0].1);
            for k in 1..run.len() {
                if smooth {
                    let p0 = if k >= 2 { run[k - 2] } else { run[k - 1] };
                    let p1 = run[k - 1];
                    let p2 = run[k];
                    let p3 = if k + 1 < run.len() { run[k + 1] } else { run[k] };
                    let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
                    let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
                    cairo_curve_to(cr, c1.0, c1.1, c2.0, c2.1, p2.0, p2.1);
                } else {
                    cairo_line_to(cr, run[k].0, run[k].1);
                }
            }
            cairo_stroke(cr);
        }
        run.clear();
    };
    for p in pts {
        match p {
            Some(x) => run.push(*x),
            None => flush(&mut run),
        }
    }
    flush(&mut run);
}

#[cfg(test)]
mod tests {
    use super::nice_axis;
    #[test]
    fn axes() {
        assert_eq!(nice_axis(10.0, 40.0, 10.0, true), (0.0, 45.0, 5.0));
        let (lo, hi, _) = nice_axis(95.0, 100.0, 6.0, true);
        assert!(lo > 0.0 && lo <= 95.0 && hi >= 100.0);
        let (lo, hi, _) = nice_axis(-20.0, 30.0, 6.0, true);
        assert!(lo <= -20.0 && hi >= 30.0);
    }
}
