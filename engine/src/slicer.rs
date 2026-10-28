//! Slicers and timelines: buttons over a sheet that filter PivotTables by one
//! field (a slicer) or by a span of dates (a timeline). The filter itself is the
//! pivot's own (its field's hidden items); a slicer shows and changes it.

use crate::book::Book;
use crate::drawing::{Anchor, Obj};
use crate::ffi::*;
use crate::layout::FontSpec;
use crate::render::Pen;
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct Slicer {
    /// the slicer's name (unique in the workbook) and the cache it shares
    pub name: String,
    pub caption: String,
    pub cache: String,
    /// the PivotTables it filters, by name
    pub pivots: Vec<String>,
    /// the source field it filters
    pub field: String,
    pub timeline: bool,
    /// timeline: 0 years, 1 quarters, 2 months, 3 days
    pub level: u8,
    /// timeline: the dates chosen (serials, inclusive); None: all periods
    pub span: Option<(f64, f64)>,
    pub columns: u32,
    pub style: String,
    pub show_header: bool,
    /// the parts it was read from (kept when written back)
    pub part: Option<String>,
    pub cache_part: Option<String>,
}

impl Slicer {
    pub fn new(name: &str, field: &str, pivot: &str, timeline: bool) -> Slicer {
        let safe: String = field.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        Slicer {
            name: name.to_string(),
            caption: field.to_string(),
            cache: if timeline { format!("NativeTimeline_{}", safe) } else { format!("Slicer_{}", safe) },
            pivots: vec![pivot.to_string()],
            field: field.to_string(),
            timeline,
            level: 2,
            span: None,
            columns: 1,
            style: if timeline { "TimeSlicerStyleLight1".into() } else { "SlicerStyleLight1".into() },
            show_header: true,
            part: None,
            cache_part: None,
        }
    }
}

/// A slicer's or timeline's parts on screen, relative to its top left.
#[derive(Debug, Default)]
pub struct SlicerLayout {
    pub header: (f64, f64, f64, f64),
    pub clear: (f64, f64, f64, f64),
    /// (x, y, w, h, item index): slicer items, or timeline periods
    pub items: Vec<(f64, f64, f64, f64, usize)>,
    /// timeline: the level selector
    pub level: (f64, f64, f64, f64),
}

pub const LEVELS: [&str; 4] = ["YEARS", "QUARTERS", "MONTHS", "DAYS"];
const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

impl Book {
    pub fn slicer(&self, s: usize, i: usize) -> Option<&Slicer> {
        self.sheets.get(s)?.objects.get(i)?.slicer.as_deref()
    }

    /// The pivot (index) and field (index) a slicer filters.
    pub fn slicer_target(&self, sl: &Slicer) -> Option<(usize, usize)> {
        for name in &sl.pivots {
            if let Some(pi) = self.pivots.iter().position(|p| &p.name == name) {
                let f = self.pivots[pi].fields.iter().position(|f| f.name == sl.field)?;
                return Some((pi, f));
            }
        }
        None
    }

    /// Every PivotTable a slicer is connected to that has its field: (pivot, field).
    pub fn slicer_targets(&self, sl: &Slicer) -> Vec<(usize, usize)> {
        sl.pivots.iter().filter_map(|name| {
            let pi = self.pivots.iter().position(|p| &p.name == name)?;
            let f = self.pivots[pi].fields.iter().position(|f| f.name == sl.field)?;
            Some((pi, f))
        }).collect()
    }

    fn filter_targets(&mut self, sl: &Slicer, label: &str, hidden: HashSet<String>) -> Result<(), String> {
        let targets = self.slicer_targets(sl);
        if targets.is_empty() {
            return Err("The slicer's PivotTable isn't in the workbook.".into());
        }
        for (pi, f) in targets {
            let h = hidden.clone();
            self.edit_pivot(pi, label, move |p| {
                p.fields[f].hidden = h;
                for pg in p.pages.iter_mut().filter(|x| x.0 == f) {
                    pg.1 = None;
                }
                if !p.slicer_fields.contains(&f) {
                    p.slicer_fields.push(f);
                }
            })?;
        }
        Ok(())
    }

    /// The slicer's items: (text, selected).
    pub fn slicer_items(&self, s: usize, i: usize) -> Vec<(String, bool)> {
        let Some(sl) = self.slicer(s, i) else { return vec![] };
        let Some((pi, f)) = self.slicer_target(sl) else { return vec![] };
        let p = &self.pivots[pi];
        let page = p.pages.iter().find(|x| x.0 == f).and_then(|x| x.1.clone());
        self.pivot_field_items(pi, f).into_iter().map(|t| {
            let on = !p.fields[f].hidden.contains(&t) && page.as_ref().map_or(true, |x| x == &t);
            (t, on)
        }).collect()
    }

    /// A click on a slicer item: only that one (or, adding, it toggled).
    pub fn slicer_choose(&mut self, s: usize, i: usize, item: usize, add: bool) -> Result<(), String> {
        let sl = self.slicer(s, i).cloned().ok_or("No such slicer.")?;
        let (pi, f) = self.slicer_target(&sl).ok_or("The slicer's PivotTable isn't in the workbook.")?;
        let items = self.slicer_items(s, i);
        let Some((text, on)) = items.get(item).cloned() else { return Ok(()) };
        let all: Vec<String> = items.iter().map(|x| x.0.clone()).collect();
        let mut shown: HashSet<String> = items.iter().filter(|x| x.1).map(|x| x.0.clone()).collect();
        let everything = shown.len() == all.len();
        if add {
            if everything {
                shown = [text.clone()].into_iter().collect();
            } else if on {
                if shown.len() > 1 {
                    shown.remove(&text);
                }
            } else {
                shown.insert(text.clone());
            }
        } else {
            shown = [text.clone()].into_iter().collect();
        }
        let hidden: HashSet<String> = all.into_iter().filter(|t| !shown.contains(t)).collect();
        let _ = (pi, f);
        self.filter_targets(&sl, "Slicer", hidden)
    }

    /// Clear Filter: everything shown again.
    pub fn slicer_clear(&mut self, s: usize, i: usize) -> Result<(), String> {
        let sl = self.slicer(s, i).cloned().ok_or("No such slicer.")?;
        let (pi, f) = self.slicer_target(&sl).ok_or("The slicer's PivotTable isn't in the workbook.")?;
        if sl.timeline {
            self.set_slicer_object(s, i, "Clear Filter", |x| x.span = None)?;
        }
        let _ = (pi, f);
        self.filter_targets(&sl, "Clear Filter", HashSet::new())
    }

    /// Change a slicer's own settings (caption, columns, style, level...), as one undo step.
    pub fn set_slicer_object(&mut self, s: usize, i: usize, label: &str, f: impl FnOnce(&mut Slicer)) -> Result<(), String> {
        let mut objs = self.sheets.get(s).ok_or("No such sheet.")?.objects.clone();
        let o = objs.get_mut(i).ok_or("No such slicer.")?;
        let mut sl = o.slicer.as_deref().cloned().ok_or("No such slicer.")?;
        f(&mut sl);
        if sl.caption.trim().is_empty() {
            sl.caption = sl.field.clone();
        }
        sl.columns = sl.columns.clamp(1, 20);
        o.slicer = Some(Rc::new(sl));
        self.begin(label, s);
        self.record(crate::book::Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }

    /// Insert Slicer / Insert Timeline for a pivot's field, beside the pivot.
    pub fn insert_slicer(&mut self, pi: usize, f: usize, timeline: bool) -> Result<usize, String> {
        let p = self.pivots.get(pi).ok_or("No such PivotTable.")?.clone();
        let field = p.fields.get(f).ok_or("No such field.")?.name.clone();
        let s = p.sheet;
        if timeline && self.timeline_dates(pi, f).is_empty() {
            return Err(format!("The field \"{}\" doesn't hold dates, so there's no timeline to show.", field));
        }
        // a unique name, workbook-wide
        let mut name = field.clone();
        let taken: HashSet<String> = self.sheets.iter().flat_map(|sh| sh.objects.iter().filter_map(|o| o.slicer.as_ref().map(|x| x.name.clone()))).collect();
        let mut k = 1;
        while taken.contains(&name) {
            name = format!("{} {}", field, k);
            k += 1;
        }
        let mut sl = Slicer::new(&name, &field, &p.name, timeline);
        let caches: HashSet<String> = self.sheets.iter().flat_map(|sh| sh.objects.iter().filter_map(|o| o.slicer.as_ref().map(|x| x.cache.clone()))).collect();
        let base = sl.cache.clone();
        let mut k = 1;
        while caches.contains(&sl.cache) {
            sl.cache = format!("{}{}", base, k);
            k += 1;
        }
        // beside the pivot, staggered by the slicers already there
        let (x0, y0) = match p.out {
            Some(o) => (self.col_x(s, o.c2 + 2), self.row_y(s, o.r1)),
            None => (self.col_x(s, p.at.1 + 4), self.row_y(s, p.at.0)),
        };
        let n = self.sheets[s].objects.iter().filter(|o| o.slicer.is_some()).count() as f64;
        let (w, h) = if timeline { (432.0, 144.0) } else { (192.0, 252.0) };
        let (x, y) = (x0 + 20.0 * n, y0 + 20.0 * n);
        let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: Some("oneCell".into()) };
        let anchor = self.anchor_at(s, &like, x, y, w, h);
        let obj = Obj { anchor, body: Vec::new(), chart: None, name: name.clone(), part: None, picture: None, pic_part: None,
                        ink: None, ink_part: None, raw: None, slicer: Some(Rc::new(sl)), members: Vec::new() };
        let mut objs = self.sheets[s].objects.clone();
        objs.push(obj);
        let i = objs.len() - 1;
        self.begin(if timeline { "Insert Timeline" } else { "Insert Slicer" }, s);
        self.record(crate::book::Delta::Objects(s, objs));
        let mut all = self.pivots.clone();
        if !all[pi].slicer_fields.contains(&f) {
            all[pi].slicer_fields.push(f);
            all[pi].dirty = true;
            self.record(crate::book::Delta::Pivots(all));
        }
        self.commit();
        Ok(i)
    }

    // ------------------------------------------------------------ timelines
    /// The dates (serials) of a pivot field's items.
    pub fn timeline_dates(&self, pi: usize, f: usize) -> Vec<f64> {
        let Some(p) = self.pivots.get(pi) else { return vec![] };
        let Some(src) = self.pivot_source(p) else { return vec![] };
        let mut v: Vec<f64> = src.records.iter().filter_map(|r| match r.get(f) {
            Some(crate::value::Value::Num(x)) if *x >= 1.0 && *x < 2958466.0 => Some(x.floor()),
            _ => None,
        }).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup();
        // a date field: its texts are dates, not plain numbers (every value a whole day)
        let fmt = p.source_table.is_none() && self.sheet_index(&p.source_sheet).map(|s| {
            let c = p.source.c1 + f as u32;
            crate::numfmt::get(&self.number_format(s, p.source.r1 + 1, c)).is_date()
        }).unwrap_or(false);
        if !fmt && v.iter().any(|x| *x < 3000.0) {
            return vec![];
        }
        v
    }

    /// The periods a timeline shows at its level: (label, first day, last day, year label).
    pub fn timeline_periods(&self, s: usize, i: usize) -> Vec<(String, f64, f64, String)> {
        let Some(sl) = self.slicer(s, i) else { return vec![] };
        let Some((pi, f)) = self.slicer_target(sl) else { return vec![] };
        let dates = self.timeline_dates(pi, f);
        let (Some(&lo), Some(&hi)) = (dates.first(), dates.last()) else { return vec![] };
        let d1904 = self.date1904;
        let ymd = |x: f64| crate::dates::serial_to_ymd(x, d1904).unwrap_or((1900, 1, 1));
        let ser = |y: i64, m: i64, d: i64| crate::dates::ymd_to_serial(y, m, d, d1904).unwrap_or(0.0);
        let (y0, m0, _) = ymd(lo);
        let (y1, m1, _) = ymd(hi);
        let mut out = Vec::new();
        match sl.level {
            0 => {
                for y in y0..=y1 {
                    out.push((y.to_string(), ser(y, 1, 1), ser(y + 1, 1, 1) - 1.0, String::new()));
                }
            }
            1 => {
                let (q0, q1) = ((m0 as i64 - 1) / 3, (m1 as i64 - 1) / 3);
                let (mut y, mut q) = (y0, q0);
                while (y, q) <= (y1, q1) {
                    let start = ser(y, q * 3 + 1, 1);
                    let end = if q == 3 { ser(y + 1, 1, 1) } else { ser(y, q * 3 + 4, 1) } - 1.0;
                    out.push((format!("Q{}", q + 1), start, end, y.to_string()));
                    q += 1;
                    if q == 4 {
                        q = 0;
                        y += 1;
                    }
                }
            }
            2 => {
                let (mut y, mut m) = (y0, m0 as i64);
                while (y, m) <= (y1, m1 as i64) {
                    let start = ser(y, m, 1);
                    let end = if m == 12 { ser(y + 1, 1, 1) } else { ser(y, m + 1, 1) } - 1.0;
                    out.push((MONTHS[(m - 1) as usize].to_string(), start, end, y.to_string()));
                    m += 1;
                    if m == 13 {
                        m = 1;
                        y += 1;
                    }
                }
            }
            _ => {
                let mut d = lo;
                while d <= hi && out.len() < 3700 {
                    let (y, m, dd) = ymd(d);
                    out.push((dd.to_string(), d, d, format!("{} {}", MONTHS[(m - 1) as usize], y)));
                    d += 1.0;
                }
            }
        }
        out
    }

    /// A timeline's chosen span: periods first..=last (indexes into timeline_periods).
    pub fn timeline_select(&mut self, s: usize, i: usize, first: usize, last: usize) -> Result<(), String> {
        let per = self.timeline_periods(s, i);
        if per.is_empty() {
            return Ok(());
        }
        let (a, b) = (first.min(last).min(per.len() - 1), first.max(last).min(per.len() - 1));
        let span = (per[a].1, per[b].2);
        self.timeline_apply(s, i, Some(span), "Timeline")
    }

    pub fn timeline_apply(&mut self, s: usize, i: usize, span: Option<(f64, f64)>, label: &str) -> Result<(), String> {
        let sl = self.slicer(s, i).cloned().ok_or("No such timeline.")?;
        let (pi, f) = self.slicer_target(&sl).ok_or("The timeline's PivotTable isn't in the workbook.")?;
        let p = self.pivots[pi].clone();
        let src = self.pivot_source(&p).ok_or("No source data.")?;
        let mut hidden = HashSet::new();
        if let Some((lo, hi)) = span {
            for r in &src.records {
                if let Some(v) = r.get(f) {
                    let keep = matches!(v, crate::value::Value::Num(x) if x.floor() >= lo && x.floor() <= hi);
                    if !keep {
                        hidden.insert(crate::pivot::item_text(self, v));
                    }
                }
            }
        }
        self.begin(label, s);
        let mut objs = self.sheets[s].objects.clone();
        if let Some(o) = objs.get_mut(i) {
            let mut x = sl.clone();
            x.span = span;
            o.slicer = Some(Rc::new(x));
        }
        self.record(crate::book::Delta::Objects(s, objs));
        self.commit();
        let _ = pi;
        self.filter_targets(&sl, label, hidden)
    }

    // ------------------------------------------------------------ layout and drawing
    pub fn slicer_layout(&self, s: usize, i: usize, w: f64, h: f64, z: f64) -> SlicerLayout {
        let Some(sl) = self.slicer(s, i) else { return SlicerLayout::default() };
        let pad = 6.0 * z;
        let head_h = if sl.show_header { 26.0 * z } else { 0.0 };
        let mut l = SlicerLayout { header: (0.0, 0.0, w, head_h), clear: (w - pad - 18.0 * z, (head_h - 18.0 * z) / 2.0, 18.0 * z, 18.0 * z), ..Default::default() };
        if sl.timeline {
            let per = self.timeline_periods(s, i);
            let top = head_h + 22.0 * z;
            l.level = (w - pad - 90.0 * z, head_h + 2.0 * z, 90.0 * z, 18.0 * z);
            let cell_w = if sl.level == 3 { 22.0 } else { 40.0 } * z;
            let bar_h = (h - top - 30.0 * z).clamp(14.0 * z, 30.0 * z);
            // the periods from the start of the chosen span (or the first), as many as fit
            let first = match sl.span {
                Some((lo, _)) => per.iter().position(|p| p.2 >= lo).unwrap_or(0),
                None => 0,
            };
            let fit = (((w - 2.0 * pad) / cell_w).floor() as usize).max(1);
            let start = first.saturating_sub(fit / 3).min(per.len().saturating_sub(fit));
            for (k, idx) in (start..per.len()).take(fit).enumerate() {
                l.items.push((pad + k as f64 * cell_w, top + 14.0 * z, cell_w - 1.0, bar_h, idx));
            }
        } else {
            let n = self.slicer_items(s, i).len();
            let cols = sl.columns.max(1) as usize;
            let bh = 24.0 * z;
            let gap = 4.0 * z;
            let bw = ((w - 2.0 * pad - gap * (cols as f64 - 1.0)) / cols as f64).max(10.0);
            for k in 0..n {
                let (r, c) = (k / cols, k % cols);
                let y = head_h + pad + r as f64 * (bh + gap);
                if y + bh > h - 2.0 {
                    break;
                }
                l.items.push((pad + c as f64 * (bw + gap), y, bw, bh, k));
            }
        }
        l
    }

    /// What's under (x, y) in a slicer (screen offset from its top left):
    /// ("item" | "period", index), ("clear", 0), ("level", 0), ("header", 0).
    pub fn slicer_hit(&self, s: usize, i: usize, x: f64, y: f64, w: f64, h: f64, z: f64) -> Option<(&'static str, usize)> {
        let l = self.slicer_layout(s, i, w, h, z);
        let inside = |r: (f64, f64, f64, f64)| x >= r.0 && x <= r.0 + r.2 && y >= r.1 && y <= r.1 + r.3;
        if inside(l.clear) {
            return Some(("clear", 0));
        }
        let tl = self.slicer(s, i).map(|x| x.timeline).unwrap_or(false);
        if tl && inside(l.level) {
            return Some(("level", 0));
        }
        for it in &l.items {
            if inside((it.0, it.1, it.2, it.3)) {
                return Some((if tl { "period" } else { "item" }, it.4));
            }
        }
        if inside(l.header) {
            return Some(("header", 0));
        }
        None
    }

    pub(crate) unsafe fn paint_slicer(&self, s: usize, i: usize, cr: *mut cairo_t, x: f64, y: f64, w: f64, h: f64, z: f64, pen: &mut Pen) {
        let Some(sl) = self.slicer(s, i) else { return };
        // the style's accent: SlicerStyleLight3 / TimeSlicerStyleDark3 use accent 3
        let n = sl.style.chars().last().and_then(|c| c.to_digit(10)).unwrap_or(1).clamp(1, 6) as usize;
        let dark = sl.style.contains("Dark");
        let accent = self.styles.theme.get(3 + n).copied().unwrap_or(0x4472C4);
        let accent = if dark { crate::render::mix_pub(accent, 0x000000, 0.25) } else { accent };
        let rgb = |c: u32| cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
        let font = |size: f64, bold: bool, color: u32| FontSpec { family: self.styles.theme_fonts.1.clone(), size, bold, italic: false, underline: 0, strike: false, vert: 0, color };
        let l = self.slicer_layout(s, i, w, h, z);
        cairo_save(cr);
        cairo_rectangle(cr, x, y, w, h);
        cairo_clip(cr);
        rgb(0xFFFFFF);
        cairo_rectangle(cr, x, y, w, h);
        cairo_fill(cr);
        rgb(0xC8C8C8);
        cairo_set_line_width(cr, 1.0);
        cairo_rectangle(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0);
        cairo_stroke(cr);
        pen.set_align(PANGO_ALIGN_LEFT);
        if sl.show_header {
            let f = font(11.0, false, 0x262626);
            let (_tw, th) = pen.set(&sl.caption, &f, z, Some((w - 40.0 * z).max(10.0)));
            rgb(0x262626);
            pen.show(cr, x + 8.0 * z, y + (l.header.3 - th) / 2.0);
            // Clear Filter: a funnel with a cross, live when something is filtered
            let filtered = if sl.timeline { sl.span.is_some() } else { self.slicer_items(s, i).iter().any(|x| !x.1) };
            let (cx, cy, cw, ch) = l.clear;
            rgb(if filtered { 0x404040 } else { 0xC0C0C0 });
            cairo_set_line_width(cr, 1.2 * z);
            cairo_move_to(cr, x + cx + 2.0 * z, y + cy + 4.0 * z);
            cairo_line_to(cr, x + cx + cw - 6.0 * z, y + cy + 4.0 * z);
            cairo_line_to(cr, x + cx + cw / 2.0 - 1.0 * z, y + cy + ch / 2.0 + 1.0 * z);
            cairo_line_to(cr, x + cx + cw / 2.0 - 1.0 * z, y + cy + ch - 3.0 * z);
            cairo_line_to(cr, x + cx + cw / 2.0 - 4.0 * z, y + cy + ch - 5.0 * z);
            cairo_line_to(cr, x + cx + cw / 2.0 - 4.0 * z, y + cy + ch / 2.0 + 1.0 * z);
            cairo_close_path(cr);
            cairo_stroke(cr);
            if filtered {
                rgb(0xC00000);
                cairo_move_to(cr, x + cx + cw - 6.0 * z, y + cy + ch - 8.0 * z);
                cairo_line_to(cr, x + cx + cw - 1.0 * z, y + cy + ch - 3.0 * z);
                cairo_move_to(cr, x + cx + cw - 1.0 * z, y + cy + ch - 8.0 * z);
                cairo_line_to(cr, x + cx + cw - 6.0 * z, y + cy + ch - 3.0 * z);
                cairo_stroke(cr);
            }
        }
        if sl.timeline {
            let per = self.timeline_periods(s, i);
            let f = font(9.0, false, 0x595959);
            let label = match sl.span {
                None => "All Periods".to_string(),
                Some((a, b)) => {
                    let fmt = |v: f64| crate::dates::serial_to_ymd(v, self.date1904)
                        .map(|(y, m, d)| format!("{} {}, {}", MONTHS[(m - 1) as usize].chars().next().unwrap().to_string() + &MONTHS[(m - 1) as usize][1..].to_lowercase(), d, y))
                        .unwrap_or_default();
                    format!("{} - {}", fmt(a), fmt(b))
                }
            };
            pen.set(&label, &f, z, None);
            rgb(0x595959);
            pen.show(cr, x + 8.0 * z, y + l.header.3 + 4.0 * z);
            let (lx, ly, lw, _lh) = l.level;
            let lv = LEVELS[sl.level.min(3) as usize];
            let (tw, _) = pen.set(&format!("{} ▾", lv), &f, z, None);
            rgb(accent);
            pen.show(cr, x + lx + lw - tw, y + ly + 2.0 * z);
            let mut last_year = String::new();
            for &(px, py, pw, ph, idx) in &l.items {
                let Some(p) = per.get(idx) else { continue };
                if p.3 != last_year {
                    let fy = font(8.0, false, 0x808080);
                    pen.set(&p.3, &fy, z, None);
                    rgb(0x808080);
                    pen.show(cr, x + px, y + py - 13.0 * z);
                    last_year = p.3.clone();
                }
                let chosen = match sl.span { Some((a, b)) => p.2 >= a && p.1 <= b, None => true };
                rgb(if chosen && sl.span.is_some() { accent } else if chosen { crate::render::mix_pub(accent, 0xFFFFFF, 0.55) } else { 0xE8E8E8 });
                cairo_rectangle(cr, x + px, y + py, pw, ph);
                cairo_fill(cr);
                let fl = font(8.0, false, 0x404040);
                let (tw, _) = pen.set(&p.0, &fl, z, None);
                rgb(0x404040);
                pen.show(cr, x + px + (pw - tw) / 2.0, y + py + ph + 2.0 * z);
            }
        } else {
            let items = self.slicer_items(s, i);
            let f = font(9.0, false, 0x262626);
            for &(bx, by, bw, bh, k) in &l.items {
                let Some((text, on)) = items.get(k) else { continue };
                let all = items.iter().all(|x| x.1);
                let fill = if *on && !all { crate::render::mix_pub(accent, 0xFFFFFF, 0.45) } else if *on { crate::render::mix_pub(accent, 0xFFFFFF, 0.8) } else { 0xFFFFFF };
                rgb(fill);
                cairo_rectangle(cr, x + bx, y + by, bw, bh);
                cairo_fill(cr);
                rgb(if *on { crate::render::mix_pub(accent, 0xFFFFFF, 0.3) } else { 0xD0D0D0 });
                cairo_set_line_width(cr, 1.0);
                cairo_rectangle(cr, x + bx + 0.5, y + by + 0.5, bw - 1.0, bh - 1.0);
                cairo_stroke(cr);
                let (_tw, th) = pen.set(text, &f, z, None);
                rgb(if *on { 0x262626 } else { 0x9A9A9A });
                cairo_save(cr);
                cairo_rectangle(cr, x + bx, y + by, bw - 4.0 * z, bh);
                cairo_clip(cr);
                pen.show(cr, x + bx + 8.0 * z, y + by + (bh - th) / 2.0);
                cairo_restore(cr);
            }
        }
        cairo_restore(cr);
    }
}
