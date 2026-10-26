//! Charts and other objects on a sheet: inserting, moving, sizing, deleting, changing.

use crate::book::{rebase, Book, Delta};
use crate::cellref::{parse_range, Rect};
use crate::chart::{plots_for, Chart, Group, Kind, Src, Title};
use crate::drawing::{chart_frame, shift_anchor, Anchor, Obj};
use crate::formula::{self, Adjust};
use std::rc::Rc;

/// A chart as the dialogs see it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ChartSpec {
    /// col, col-stacked, col-percent, bar.., line.. (-markers), area.., pie, doughnut,
    /// scatter, scatter-lines, scatter-smooth, combo
    pub kind: String,
    /// "none", "auto" or "text"
    pub title_mode: String,
    pub title: String,
    /// none, b, r, t, l, tr
    pub legend: String,
    pub x_title: String,
    pub y_title: String,
    pub gridlines: bool,
    pub labels: bool,
    pub palette: u8,
    pub series: Vec<SeriesSpec>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SeriesSpec {
    /// references as typed (without "="); a name may be plain text in quotes
    pub name: String,
    pub cat: String,
    pub val: String,
    /// for combo charts: col, line, area
    pub kind: String,
    pub secondary: bool,
    pub color: Option<u32>,
}

pub fn kind_name(ch: &Chart) -> String {
    if ch.plots.len() > 1 {
        return "combo".into();
    }
    let p = match ch.plots.first() {
        Some(p) => p,
        None => return "col".into(),
    };
    let base = match p.kind {
        Kind::Bar => if p.horizontal { "bar" } else { "col" },
        Kind::Line => "line",
        Kind::Area => "area",
        Kind::Pie => "pie",
        Kind::Doughnut => "doughnut",
        Kind::Scatter => {
            return if !p.lines { "scatter".into() } else if p.smooth_any() { "scatter-smooth".into() } else { "scatter-lines".into() };
        }
    };
    let mut s = base.to_string();
    match p.group {
        Group::Stacked if !matches!(p.kind, Kind::Pie | Kind::Doughnut) => s.push_str("-stacked"),
        Group::Percent if !matches!(p.kind, Kind::Pie | Kind::Doughnut) => s.push_str("-percent"),
        _ => {}
    }
    if p.kind == Kind::Line && p.markers {
        s.push_str("-markers");
    }
    s
}

fn src_text(s: &Option<Src>) -> String {
    match s {
        Some(Src { f: Some(f), .. }) => f.clone(),
        Some(src) => {
            let t: Vec<String> = src.cache.iter().map(|c| match c {
                crate::chart::CVal::Str(s) => s.clone(),
                crate::chart::CVal::Num(n) => crate::xlsx::num_text(*n),
                crate::chart::CVal::Empty => String::new(),
            }).collect();
            if t.is_empty() { String::new() } else { format!("\"{}\"", t.join(" ")) }
        }
        None => String::new(),
    }
}

fn text_src(t: &str) -> Option<Src> {
    let t = t.trim();
    let t = t.strip_prefix('=').unwrap_or(t).trim();
    if t.is_empty() {
        return None;
    }
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return Some(Src { f: None, cache: vec![crate::chart::CVal::Str(t[1..t.len() - 1].replace("\"\"", "\""))], fmt: None });
    }
    if t.starts_with('{') {
        // a constant array
        let items: Vec<crate::chart::CVal> = t.trim_matches(|c| c == '{' || c == '}').split([',', ';']).map(|x| {
            let x = x.trim();
            match x.parse::<f64>() {
                Ok(n) => crate::chart::CVal::Num(n),
                Err(_) => crate::chart::CVal::Str(x.trim_matches('"').to_string()),
            }
        }).collect();
        return Some(Src { f: None, cache: items, fmt: None });
    }
    Some(Src { f: Some(t.to_string()), cache: vec![], fmt: None })
}

impl Book {
    pub fn chart(&self, s: usize, i: usize) -> Option<&Chart> {
        self.sheets.get(s)?.objects.get(i)?.chart.as_deref()
    }

    pub fn chart_spec(&self, s: usize, i: usize) -> Option<ChartSpec> {
        let ch = self.chart(s, i)?;
        Some(self.spec_of(ch))
    }

    /// The spec of a chart (the dialogs' view of it).
    pub fn spec_of(&self, ch: &Chart) -> ChartSpec {
        let (title_mode, title) = match &ch.title {
            Title::None => ("none", String::new()),
            Title::Auto => ("auto", String::new()),
            Title::Text(t) => ("text", t.clone()),
            Title::Ref(src) => ("text", format!("={}", src_text(&Some(src.clone())))),
        };
        let mut series = Vec::new();
        for p in &ch.plots {
            for se in &p.series {
                series.push(SeriesSpec {
                    name: src_text(&se.name),
                    cat: src_text(&se.cat),
                    val: src_text(&se.val),
                    kind: match p.kind {
                        Kind::Line => "line".into(),
                        Kind::Area => "area".into(),
                        _ => "col".into(),
                    },
                    secondary: p.secondary,
                    color: se.color,
                });
            }
        }
        ChartSpec {
            kind: kind_name(ch),
            title_mode: title_mode.into(),
            title,
            legend: ch.legend.clone().unwrap_or_else(|| "none".into()),
            x_title: ch.x_title.clone().unwrap_or_default(),
            y_title: ch.y_title.clone().unwrap_or_default(),
            gridlines: ch.gridlines,
            labels: ch.all_series().any(|s| s.labels),
            palette: ch.palette,
            series,
        }
    }

    /// A chart made from a spec (series with an empty value reference are left out).
    pub fn chart_of_spec(&self, spec: &ChartSpec) -> Result<Chart, String> {
        let mut list = Vec::new();
        for (k, ss) in spec.series.iter().enumerate() {
            let val = text_src(&ss.val);
            if val.is_none() {
                continue;
            }
            for (what, t) in [("values", &ss.val), ("categories", &ss.cat)] {
                if let Some(Src { f: Some(f), .. }) = text_src(t) {
                    if self.chart_ref_areas(&f).is_none() {
                        return Err(format!("The {} of series {} must be a reference to cells.", what, k + 1));
                    }
                }
            }
            let se = crate::chart::Series {
                name: text_src(&ss.name),
                cat: text_src(&ss.cat),
                val,
                color: ss.color,
                labels: spec.labels,
                smooth: spec.kind == "scatter-smooth",
            };
            list.push((se, ss.kind.clone(), ss.secondary));
        }
        if list.is_empty() {
            return Err("A chart needs at least one series of values.".into());
        }
        let mut ch = Chart::default();
        if spec.kind == "combo" {
            // one plot per (kind, axis), in the order they first appear
            let mut plots: Vec<crate::chart::Plot> = Vec::new();
            for (se, k, sec) in list {
                let kind = match k.as_str() {
                    "line" | "line-markers" => Kind::Line,
                    "area" => Kind::Area,
                    _ => Kind::Bar,
                };
                match plots.iter_mut().find(|p| p.kind == kind && p.secondary == sec) {
                    Some(p) => p.series.push(se),
                    None => {
                        let mut p = crate::chart::Plot::new(kind);
                        p.secondary = sec;
                        p.markers = k == "line-markers";
                        p.series.push(se);
                        plots.push(p);
                    }
                }
            }
            // the primary axis needs a plot
            if plots.iter().all(|p| p.secondary) {
                plots[0].secondary = false;
            }
            plots.sort_by_key(|p| p.secondary);
            ch.plots = plots;
        } else {
            ch.plots = plots_for(&spec.kind, list.into_iter().map(|x| x.0).collect());
        }
        ch.title = match spec.title_mode.as_str() {
            "none" => Title::None,
            "text" => {
                let t = spec.title.trim();
                match t.strip_prefix('=') {
                    Some(r) if self.chart_ref_areas(r).is_some() => Title::Ref(Src { f: Some(r.trim().to_string()), cache: vec![], fmt: None }),
                    _ => Title::Text(spec.title.clone()),
                }
            }
            _ => Title::Auto,
        };
        ch.legend = if spec.legend == "none" || spec.legend.is_empty() { None } else { Some(spec.legend.clone()) };
        ch.x_title = Some(spec.x_title.clone()).filter(|t| !t.is_empty());
        ch.y_title = Some(spec.y_title.clone()).filter(|t| !t.is_empty());
        ch.gridlines = spec.gridlines;
        ch.palette = spec.palette;
        Ok(ch)
    }

    fn next_object_name(&self, s: usize, base: &str) -> (u32, String) {
        let sh = &self.sheets[s];
        let mut n = 1;
        for o in &sh.objects {
            if let Some(k) = o.name.strip_prefix(base).and_then(|r| r.trim().parse::<u32>().ok()) {
                n = n.max(k + 1);
            }
        }
        (sh.objects.len() as u32 + 2, format!("{} {}", base, n))
    }

    /// Insert a picture (PNG, JPEG, GIF or BMP bytes) with its top left at sheet pixels (x, y),
    /// at its own size but no wider or taller than max_w by max_h. Returns its index.
    pub fn insert_picture(&mut self, s: usize, bytes: Vec<u8>, x: f64, y: f64, max_w: f64, max_h: f64) -> Result<usize, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("Pictures can be inserted on worksheets only.".into());
        }
        let pic = crate::picture::Picture::new(bytes).ok_or("This isn't a picture Sheets can read (PNG, JPEG, GIF or BMP).")?;
        let (pw, ph) = (pic.w as f64, pic.h as f64);
        let k = (max_w / pw).min(max_h / ph).min(1.0).max(1e-6);
        let (w, h) = ((pw * k).max(1.0), (ph * k).max(1.0));
        let (id, name) = self.next_object_name(s, "Picture");
        // pictures move with cells but keep their size, as in the desktop spreadsheet
        let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: Some("oneCell".into()) };
        let anchor = self.anchor_at(s, &like, x, y, w, h);
        let emu = crate::drawing::EMU;
        let obj = Obj {
            anchor,
            body: crate::drawing::picture_body(id, &name, (w * emu).round() as i64, (h * emu).round() as i64),
            chart: None,
            name,
            part: None,
            picture: Some(Rc::new(pic)),
            pic_part: None,
            ink: None,
            ink_part: None,
            raw: None,
        };
        let mut objs = self.sheets[s].objects.clone();
        objs.push(obj);
        let i = objs.len() - 1;
        self.begin("Insert Picture", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(i)
    }

    /// Ink drawn on the sheet (the Draw tab): strokes in sheet pixels (100 %), as one object
    /// that moves with the cells. Returns its index.
    pub fn add_ink(&mut self, s: usize, strokes: Vec<crate::ink::Stroke>) -> Result<usize, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("Ink can be drawn on worksheets only.".into());
        }
        let (x, y, w, h, ink) = crate::ink::Ink::from_sheet(strokes).ok_or("There is nothing to draw.")?;
        let (id, name) = self.next_object_name(s, "Ink");
        let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: Some("oneCell".into()) };
        let anchor = self.anchor_at(s, &like, x, y, w, h);
        let emu = crate::drawing::EMU;
        let obj = Obj {
            anchor,
            body: crate::drawing::ink_body(id, &name, (x * emu).round() as i64, (y * emu).round() as i64, (w * emu).round() as i64, (h * emu).round() as i64),
            chart: None,
            name,
            part: None,
            picture: None,
            pic_part: None,
            ink: Some(Rc::new(ink)),
            ink_part: None,
            raw: None,
        };
        let mut objs = self.sheets[s].objects.clone();
        objs.push(obj);
        let i = objs.len() - 1;
        self.begin("Ink", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(i)
    }

    /// An ink object's strokes in sheet pixels (100 %): (points, colour, width, kind).
    pub fn ink_strokes(&self, s: usize, i: usize) -> Option<Vec<crate::ink::Stroke>> {
        let o = self.sheets.get(s)?.objects.get(i)?;
        let ink = o.ink.as_ref()?;
        let (x, y, w, h) = self.obj_rect(s, o);
        let (sx, sy) = (w / ink.w.max(1e-9), h / ink.h.max(1e-9));
        Some(ink.strokes.iter().map(|st| crate::ink::Stroke {
            pts: st.pts.iter().map(|&(px, py)| (x + px * sx, y + py * sy)).collect(),
            ..st.clone()
        }).collect())
    }

    /// The ink objects a point (sheet pixels) is on or within `radius` of, topmost first.
    pub fn ink_at(&self, s: usize, x: f64, y: f64, radius: f64) -> Vec<usize> {
        let mut out = Vec::new();
        let n = self.sheets.get(s).map(|sh| sh.objects.len()).unwrap_or(0);
        for i in (0..n).rev() {
            if let Some(st) = self.ink_strokes(s, i) {
                if st.iter().any(|k| crate::ink::distance(k, x, y) <= radius + k.width / 2.0) {
                    out.push(i);
                }
            }
        }
        out
    }

    /// A picture object's file bytes and type ("png", "jpeg", ...).
    pub fn picture_bytes(&self, s: usize, i: usize) -> Option<(&[u8], &'static str)> {
        let p = self.sheets.get(s)?.objects.get(i)?.picture.as_ref()?;
        Some((&p.bytes, p.ext))
    }

    /// Insert a chart at sheet pixels (x, y), w by h. Returns its index.
    pub fn add_chart(&mut self, s: usize, ch: Chart, x: f64, y: f64, w: f64, h: f64) -> Result<usize, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("Charts can be inserted on worksheets only.".into());
        }
        let (id, name) = self.next_object_name(s, "Chart");
        let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: None };
        let anchor = self.anchor_at(s, &like, x, y, w, h);
        let obj = Obj { anchor, body: chart_frame(id, &name), chart: Some(Rc::new(ch)), name, part: None, picture: None, pic_part: None, ink: None, ink_part: None, raw: None };
        let mut objs = self.sheets[s].objects.clone();
        objs.push(obj);
        let i = objs.len() - 1;
        self.begin("Insert Chart", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(i)
    }

    /// A chart of the selection, placed at (x, y).
    pub fn insert_chart(&mut self, s: usize, rc: Rect, kind: &str, x: f64, y: f64) -> Result<usize, String> {
        let ch = self.chart_from_range(s, rc, kind)?;
        self.add_chart(s, ch, x, y, 480.0, 288.0)
    }

    pub fn set_chart(&mut self, s: usize, i: usize, ch: Chart, label: &str) -> Result<(), String> {
        let mut objs = self.sheets.get(s).ok_or("no such sheet")?.objects.clone();
        let o = objs.get_mut(i).ok_or("no such chart")?;
        if o.chart.is_none() {
            return Err("That object isn't a chart.".into());
        }
        o.chart = Some(Rc::new(ch));
        self.begin(label, s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }

    pub fn set_chart_spec(&mut self, s: usize, i: usize, spec: &ChartSpec, label: &str) -> Result<(), String> {
        let old = self.chart(s, i).ok_or("no such chart")?.clone();
        let mut ch = self.chart_of_spec(spec)?;
        // nothing the dialog shows changed: keep the chart as read
        if self.chart_spec(s, i).as_ref() == Some(spec) {
            return Ok(());
        }
        // the same series, only other references: keep the file's formatting
        if old.orig.is_some() && kind_name(&old) == spec.kind && old.title == ch.title && old.legend == ch.legend
            && old.x_title == ch.x_title && old.y_title == ch.y_title && old.gridlines == ch.gridlines && old.palette == ch.palette
            && old.all_series().count() == ch.all_series().count()
            && old.all_series().zip(ch.all_series()).all(|(a, b)| a.labels == b.labels && a.color == b.color) {
            let mut keep = old.clone();
            let new: Vec<_> = ch.all_series().cloned().collect();
            let mut k = 0;
            for p in keep.plots.iter_mut() {
                for se in p.series.iter_mut() {
                    se.name = new[k].name.clone();
                    se.cat = new[k].cat.clone();
                    se.val = new[k].val.clone();
                    k += 1;
                }
            }
            ch = keep;
        }
        self.set_chart(s, i, ch, label)
    }

    /// Change only the type (keeping the data and the other settings).
    pub fn set_chart_kind(&mut self, s: usize, i: usize, kind: &str) -> Result<(), String> {
        let mut spec = self.chart_spec(s, i).ok_or("no such chart")?;
        if kind == "combo" && spec.kind != "combo" {
            let n = spec.series.len();
            for (k, ss) in spec.series.iter_mut().enumerate() {
                ss.kind = if k + 1 == n && n > 1 { "line".into() } else { "col".into() };
                ss.secondary = false;
            }
        }
        spec.kind = kind.to_string();
        self.set_chart_spec(s, i, &spec, "Change Chart Type")
    }

    /// The block of cells a chart was made from, when its series form one:
    /// (sheet, whole range, data part, header row, header column, series in rows).
    pub fn chart_source(&self, ch: &Chart) -> Option<(usize, Rect, Rect, bool, bool, bool)> {
        let ser: Vec<&crate::chart::Series> = ch.all_series().collect();
        let first = |src: &Option<Src>| -> Option<(usize, Rect)> {
            let a = self.chart_ref_areas(src.as_ref()?.f.as_ref()?)?;
            if a.len() == 1 { Some(a[0]) } else { None }
        };
        let vals: Vec<(usize, Rect)> = ser.iter().map(|s| first(&s.val)).collect::<Option<Vec<_>>>()?;
        let s = vals[0].0;
        if vals.iter().any(|v| v.0 != s) {
            return None;
        }
        let mut d = vals[0].1;
        for v in &vals {
            d = d.union(&v.1);
        }
        let scatter = ch.plots.first().map(|p| p.kind == Kind::Scatter).unwrap_or(false);
        let by_rows = if vals.len() > 1 {
            vals[0].1.r1 == vals[0].1.r2 && vals[1].1.r1 != vals[0].1.r1
        } else {
            vals[0].1.r1 == vals[0].1.r2 && vals[0].1.c2 > vals[0].1.c1
        };
        let cats = ser.iter().find_map(|x| first(&x.cat));
        let names = ser.iter().find_map(|x| first(&x.name));
        let (mut hr, mut hc) = (false, false);
        if scatter {
            if let Some((_, c)) = cats {
                d = d.union(&c);
            }
            if let Some((_, n)) = names {
                if by_rows { hc = n.c1 + 1 == d.c1 } else { hr = n.r1 + 1 == d.r1 }
            }
        } else if !by_rows {
            hr = names.map(|(_, n)| n.r1 + 1 == d.r1).unwrap_or(false);
            hc = cats.map(|(_, c)| c.c1 + 1 == d.c1).unwrap_or(false);
        } else {
            hc = names.map(|(_, n)| n.c1 + 1 == d.c1).unwrap_or(false);
            hr = cats.map(|(_, c)| c.r1 + 1 == d.r1).unwrap_or(false);
        }
        let rc = Rect::new(d.r1 - hr as u32, d.c1 - hc as u32, d.r2, d.c2);
        Some((s, rc, d, hr, hc, by_rows))
    }

    /// The data range as text ("Sheet1!$A$1:$C$5"), when the chart has one block.
    pub fn chart_range_text(&self, s: usize, i: usize) -> Option<String> {
        let ch = self.chart(s, i)?;
        let (ds, rc, ..) = self.chart_source(ch)?;
        let q = crate::cellref::quote_sheet(&self.sheets[ds].name);
        let one = |r: u32, c: u32| format!("${}${}", crate::cellref::col_name(c), r + 1);
        Some(format!("{}!{}:{}", q, one(rc.r1, rc.c1), one(rc.r2, rc.c2)))
    }

    /// Select Data: new series from a block of cells (by_rows None: decide as Insert does).
    pub fn set_chart_range(&mut self, s: usize, i: usize, range: &str, by_rows: Option<bool>) -> Result<(), String> {
        let old = self.chart(s, i).ok_or("no such chart")?.clone();
        let t = range.trim();
        let t = t.strip_prefix('=').unwrap_or(t);
        let (sn, _, rest) = formula::split_sheet(t);
        let ds = match sn {
            Some(n) => self.sheet_index(&n).ok_or("The reference isn't valid.")?,
            None => s,
        };
        let rc = parse_range(&rest.replace('$', "")).ok_or("The reference isn't valid.")?;
        let kind = kind_name(&old);
        let kind_simple = if kind == "combo" { "col" } else { kind.as_str() };
        let mut fresh = self.chart_from_range(ds, rc, kind_simple)?;
        if let Some(br) = by_rows {
            if let Some((ss, rc2, d, hr, hc, cur)) = self.chart_source(&fresh) {
                if cur != br {
                    fresh = self.chart_from_parts(ss, rc2, d, hr, hc, br, kind_simple);
                }
            }
        }
        let mut spec = self.chart_spec(s, i).ok_or("no such chart")?;
        let mut fresh_spec = spec.clone();
        fresh_spec.series = {
            let mut tmp = fresh.clone();
            tmp.title = Title::None;
            let b = &*self;
            let mut out = Vec::new();
            for p in &tmp.plots {
                for se in &p.series {
                    out.push(SeriesSpec { name: src_text(&se.name), cat: src_text(&se.cat), val: src_text(&se.val), kind: "col".into(), secondary: false, color: None });
                }
            }
            let _ = b;
            out
        };
        if kind == "combo" {
            let n = fresh_spec.series.len();
            for (k, ss) in fresh_spec.series.iter_mut().enumerate() {
                ss.kind = if k + 1 == n && n > 1 { "line".into() } else { "col".into() };
            }
        }
        spec.series = fresh_spec.series;
        let mut ch = self.chart_of_spec(&spec)?;
        ch.palette = old.palette;
        self.set_chart(s, i, ch, "Select Data")
    }

    /// Switch Row/Column.
    pub fn switch_chart_rows(&mut self, s: usize, i: usize) -> Result<(), String> {
        let ch = self.chart(s, i).ok_or("no such chart")?.clone();
        let (ds, rc, _, _, _, by_rows) = self.chart_source(&ch).ok_or("This chart's data isn't one block of cells, so rows and columns can't be switched.")?;
        let q = crate::cellref::quote_sheet(&self.sheets[ds].name);
        let cn = crate::cellref::col_name;
        let range = format!("{}!{}{}:{}{}", q, cn(rc.c1), rc.r1 + 1, cn(rc.c2), rc.r2 + 1);
        self.set_chart_range(s, i, &range, Some(!by_rows))
    }

    pub fn set_object_rect(&mut self, s: usize, i: usize, x: f64, y: f64, w: f64, h: f64, label: &str) -> Result<(), String> {
        let mut objs = self.sheets.get(s).ok_or("no such sheet")?.objects.clone();
        let o = objs.get_mut(i).ok_or("no such object")?;
        let a = self.anchor_at(s, &o.anchor, x, y, w, h);
        if a == o.anchor {
            return Ok(());
        }
        o.anchor = a;
        self.begin(label, s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }

    pub fn delete_objects(&mut self, s: usize, which: &[usize]) -> Result<(), String> {
        let objs: Vec<Obj> = self.sheets.get(s).ok_or("no such sheet")?.objects.iter().enumerate()
            .filter(|(k, _)| !which.contains(k)).map(|(_, o)| o.clone()).collect();
        if objs.len() == self.sheets[s].objects.len() {
            return Ok(());
        }
        self.begin("Delete", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }

    /// The topmost object under sheet pixels (x, y).
    pub fn object_at(&self, s: usize, x: f64, y: f64) -> Option<usize> {
        let sh = self.sheets.get(s)?;
        (0..sh.objects.len()).rev().find(|&i| {
            let (ox, oy, w, h) = self.obj_rect(s, &sh.objects[i]);
            x >= ox && x <= ox + w && y >= oy && y <= oy + h
        })
    }

    /// Rewrite every chart reference (recorded), as formulas are.
    pub fn adjust_charts(&mut self, how: &Adjust) {
        for s in 0..self.sheets.len() {
            if !self.sheets[s].objects.iter().any(|o| o.chart.is_some()) {
                continue;
            }
            let here = self.sheets[s].name.clone();
            let how2 = rebase(how, &here);
            let mut objs = self.sheets[s].objects.clone();
            let mut changed = false;
            for o in objs.iter_mut() {
                if let Some(ch) = &o.chart {
                    let mut c = (**ch).clone();
                    if c.adjust(&how2) {
                        o.chart = Some(Rc::new(c));
                        changed = true;
                    }
                }
            }
            if changed {
                self.record(Delta::Objects(s, objs));
            }
        }
    }

    /// Objects move with inserted and deleted rows or columns (recorded).
    pub fn shift_objects(&mut self, s: usize, rows: bool, at: u32, n: i64) {
        if self.sheets[s].objects.is_empty() {
            return;
        }
        let objs: Vec<Obj> = self.sheets[s].objects.iter().map(|o| {
            let mut o = o.clone();
            o.anchor = shift_anchor(&o.anchor, rows, at, n);
            o
        }).collect();
        if objs != self.sheets[s].objects {
            self.record(Delta::Objects(s, objs));
        }
    }

    /// What is under a screen point: (object, "body" or a handle name nw..w).
    /// Handles of the selected objects come first.
    pub fn object_hit(&self, s: usize, x: f64, y: f64, p: &crate::view::ViewParams) -> Option<(usize, &'static str)> {
        let sh = self.sheets.get(s)?;
        let g = self.geometry(s, p);
        if x < g.hw || y < g.hh {
            return None;
        }
        for &i in p.sel_objects.iter().rev() {
            if let Some(o) = sh.objects.get(i) {
                let (ox, oy, w, h) = self.obj_screen_rect(s, p, &g, o);
                for (k, (hx, hy)) in crate::render::handles(ox, oy, w, h).iter().enumerate() {
                    if (x - hx).abs() <= 6.0 && (y - hy).abs() <= 6.0 {
                        return Some((i, crate::render::HANDLE_NAMES[k]));
                    }
                }
            }
        }
        for i in (0..sh.objects.len()).rev() {
            let (ox, oy, w, h) = self.obj_screen_rect(s, p, &g, &sh.objects[i]);
            if x >= ox && x <= ox + w && y >= oy && y <= oy + h {
                return Some((i, "body"));
            }
        }
        None
    }

    /// Screen rectangle of an object.
    pub fn object_screen_rect(&self, s: usize, i: usize, p: &crate::view::ViewParams) -> Option<(f64, f64, f64, f64)> {
        let g = self.geometry(s, p);
        let o = self.sheets.get(s)?.objects.get(i)?;
        Some(self.obj_screen_rect(s, p, &g, o))
    }

    /// Sheet pixels (100 %) of a screen point, measured as the scrolling band shows it.
    pub fn sheet_point(&self, s: usize, x: f64, y: f64, p: &crate::view::ViewParams) -> (f64, f64) {
        let g = self.geometry(s, p);
        let z = p.zoom;
        let lc = p.left_col.max(p.freeze_cols);
        let tr = p.top_row.max(p.freeze_rows);
        let ox = self.screen_x(s, p, &g, lc) - self.col_x(s, lc) * z;
        let oy = self.screen_y(s, p, &g, tr) - self.row_y(s, tr) * z;
        ((x - ox) / z, (y - oy) / z)
    }

    /// Move or size an object to a screen rectangle.
    pub fn place_object(&mut self, s: usize, i: usize, x: f64, y: f64, w: f64, h: f64, p: &crate::view::ViewParams, label: &str) -> Result<(), String> {
        let g = self.geometry(s, p);
        // screen -> sheet pixels at 100 %, measured from the top-left visible cell
        let z = p.zoom;
        let ox = self.screen_x(s, p, &g, p.left_col.max(p.freeze_cols)) - self.col_x(s, p.left_col.max(p.freeze_cols)) * z;
        let oy = self.screen_y(s, p, &g, p.top_row.max(p.freeze_rows)) - self.row_y(s, p.top_row.max(p.freeze_rows)) * z;
        self.set_object_rect(s, i, (x - ox) / z, (y - oy) / z, w / z, h / z, label)
    }
}
