//! Filled Map charts (S3-13): a chartEx regionMap, its regions coloured by value (a two- or
//! three-colour scale) or by category, drawn from the boundaries in geo.rs.
use crate::book::Book;
use crate::cellref::{col_name, quote_sheet, Rect};
use crate::chart::{auto_color, Src};
use crate::chartex::{ChartEx, ExSeries};
use crate::ffi::*;
use crate::geo::{atlas, auto_projection, main_rings, Projection};
use crate::render::{mix_pub as mix, Pen};

const TEXT: u32 = 0x595959;
const NO_DATA: u32 = 0xE6E6E6;

unsafe fn rgb(cr: *mut cairo_t, c: u32) {
    cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
}

fn absref(sheet: &str, rc: Rect) -> String {
    format!("{}!${}${}:${}${}", quote_sheet(sheet), col_name(rc.c1), rc.r1 + 1, col_name(rc.c2), rc.r2 + 1)
}

/// What a map shows, worked out from its data.
pub struct MapPlan {
    pub states: bool,
    /// (region, value index) of each category that was found
    pub found: Vec<(usize, usize)>,
    /// the categories no region was found for
    pub unknown: Vec<String>,
    /// the regions drawn, and those the map is sized to
    pub drawn: Vec<usize>,
    pub fit: Vec<usize>,
    pub projection: Projection,
}

impl Book {
    /// A new Filled Map from the selected cells: the text columns name the places (the broadest
    /// first, as Country, State), the number column — or a last text column — is what colours them.
    pub(crate) fn map_from_range(&self, s: usize, rc: Rect, header: bool, r1: u32, text_cols: &[u32], num_cols: &[u32]) -> Result<ChartEx, String> {
        let sh = &self.sheets[s];
        let name = sh.name.clone();
        let mut header = header;
        let mut r1 = r1;
        // a map of categories: a header row of text over text values
        if num_cols.is_empty() && text_cols.len() >= 2 && rc.r2 > rc.r1 {
            let first = sh.value(rc.r1, text_cols[0]).to_string();
            if atlas().country(&first).is_none() && atlas().state(&first, None).is_none() {
                header = true;
                r1 = rc.r1 + 1;
            }
        }
        let col = |c: u32| Src { f: Some(absref(&name, Rect::new(r1, c, rc.r2, c))), cache: vec![], fmt: None };
        let mut cx = ChartEx { kind: "regionmap".into(), ..Default::default() };
        let (place_cols, value): (Vec<u32>, (u32, bool)) = if let Some(&v) = num_cols.first() {
            (text_cols.to_vec(), (v, true))
        } else if text_cols.len() >= 2 {
            (text_cols[..text_cols.len() - 1].to_vec(), (text_cols[text_cols.len() - 1], false))
        } else {
            return Err("Select the places (countries, states or provinces) and the values to show for them.".into());
        };
        if place_cols.is_empty() {
            return Err("Select a column of places: countries, or states and provinces.".into());
        }
        for &c in place_cols.iter().rev() {
            cx.cats.push(col(c));
        }
        let nm = if header { Some(Src { f: Some(format!("{}!${}${}", quote_sheet(&name), col_name(value.0), rc.r1 + 1)), cache: vec![], fmt: None }) } else { None };
        cx.series.push(ExSeries { name: nm, val: col(value.0) });
        if !value.1 {
            cx.color_str = Some(col(value.0));
        }
        let plan = self.map_plan(&cx);
        if plan.found.is_empty() {
            return Err("None of the places in the selection were found. Name countries, or states and provinces of the United States, Canada, Brazil or Australia.".into());
        }
        Ok(cx)
    }

    pub fn map_plan(&self, cx: &ChartEx) -> MapPlan {
        let a = atlas();
        let names: Vec<String> = cx.cats.first().map(|c| self.chart_values(c).into_iter().map(|p| p.text).collect()).unwrap_or_default();
        let parents: Vec<String> = cx.cats.get(1).map(|c| self.chart_values(c).into_iter().map(|p| p.text).collect()).unwrap_or_default();
        let (states, ids) = a.resolve(&names, &parents);
        let mut found = Vec::new();
        let mut unknown = Vec::new();
        for (k, id) in ids.iter().enumerate() {
            match id {
                Some(i) => found.push((*i, k)),
                None if !names[k].trim().is_empty() => unknown.push(names[k].clone()),
                None => {}
            }
        }
        let with: Vec<usize> = found.iter().map(|f| f.0).collect();
        let all_countries: Vec<usize> = (0..a.regions.len()).filter(|&i| !a.regions[i].state && (a.regions[i].code != "ATA" || with.contains(&i))).collect();
        let (drawn, fit) = if states {
            let mut countries: Vec<String> = with.iter().map(|&i| a.regions[i].country.clone()).collect();
            countries.dedup();
            let mut st: Vec<usize> = Vec::new();
            for c in &countries {
                st.extend(a.states_of(c));
            }
            match cx.map.area.as_str() {
                "world" => {
                    let mut d: Vec<usize> = all_countries.iter().copied().filter(|&i| !countries.contains(&a.regions[i].code)).collect();
                    d.extend(st);
                    (d, all_countries.clone())
                }
                "dataOnly" => (with.clone(), with.clone()),
                _ => {
                    // the whole country, its main part: without far-off territories
                    let fit: Vec<usize> = st.iter().copied().filter(|&i| !matches!(a.regions[i].code.as_str(), "US-AK" | "US-HI") || with.contains(&i)).collect();
                    (st, fit)
                }
            }
        } else {
            match cx.map.area.as_str() {
                "world" => (all_countries.clone(), all_countries.clone()),
                "dataOnly" => (with.clone(), with.clone()),
                _ => (all_countries.clone(), with.clone()),
            }
        };
        let fit_set: Vec<usize> = if fit.is_empty() { all_countries } else { fit };
        let projection = match cx.map.projection.as_str() {
            "mercator" => Projection::Mercator,
            "miller" => Projection::Miller,
            "robinson" => Projection::Robinson,
            "albers" => {
                let (mut lo, mut hi, mut w, mut e) = (90.0f64, -90.0f64, 180.0f64, -180.0f64);
                for &i in &fit_set {
                    for ring in main_rings(&a.regions[i]) {
                        for &(x, y) in ring.iter() {
                            lo = lo.min(y);
                            hi = hi.max(y);
                            w = w.min(x);
                            e = e.max(x);
                        }
                    }
                }
                let span = (hi - lo).max(1.0);
                Projection::Albers(lo + span / 6.0, hi - span / 6.0, (w + e) / 2.0)
            }
            _ => auto_projection(a, &if with.is_empty() { fit_set.clone() } else { with.clone() }, states),
        };
        MapPlan { states, found, unknown, drawn, fit: fit_set, projection }
    }

    pub(crate) unsafe fn paint_region_map(&self, cx: &ChartEx, cr: *mut cairo_t, l: f64, t: f64, r: f64, b: f64, z: f64, pen: &mut Pen) {
        let a = atlas();
        let plan = self.map_plan(cx);
        let theme = self.styles.theme;
        let small = crate::layout::FontSpec { family: self.styles.theme_fonts.1.clone(), size: 9.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: TEXT };
        // the values: numbers, or categories
        let texts: Option<Vec<String>> = cx.color_str.as_ref().map(|c| self.chart_values(c).into_iter().map(|p| p.text).collect());
        let pts = cx.series.first().map(|s| self.chart_values(&s.val)).unwrap_or_default();
        let mut colour: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
        let mut legend: Vec<(u32, String)> = Vec::new();
        let mut scale: Option<(f64, f64, String, String, u32, Option<u32>, u32)> = None;
        if let Some(texts) = &texts {
            let mut cats: Vec<String> = Vec::new();
            for &(_i, k) in &plan.found {
                if let Some(tx) = texts.get(k) {
                    if !tx.is_empty() && !cats.contains(tx) {
                        cats.push(tx.clone());
                    }
                }
            }
            for (n, c) in cats.iter().enumerate() {
                legend.push((auto_color(&theme, 0, n), c.clone()));
            }
            for &(i, k) in &plan.found {
                if let Some(n) = texts.get(k).and_then(|tx| cats.iter().position(|c| c == tx)) {
                    colour.insert(i, legend[n].0);
                }
            }
        } else {
            let vals: Vec<(usize, f64)> = plan.found.iter().filter_map(|&(i, k)| pts.get(k).and_then(|p| p.v).map(|v| (i, v))).collect();
            if !vals.is_empty() {
                let lo = vals.iter().map(|v| v.1).fold(f64::MAX, f64::min);
                let hi = vals.iter().map(|v| v.1).fold(f64::MIN, f64::max);
                let accent = theme[4];
                let (c0, cm, c1) = cx.map.colors.unwrap_or((mix(accent, 0xFFFFFF, 0.82), None, mix(accent, 0x000000, 0.3)));
                for &(i, v) in &vals {
                    let tpos = if hi > lo { (v - lo) / (hi - lo) } else { 1.0 };
                    let c = match cm {
                        Some(m) if tpos < 0.5 => mix(c0, m, tpos * 2.0),
                        Some(m) => mix(m, c1, (tpos - 0.5) * 2.0),
                        None => mix(c0, c1, tpos),
                    };
                    colour.insert(i, c);
                }
                let text_of = |want: f64| plan.found.iter().filter_map(|&(_i, k)| pts.get(k)).find(|p| p.v == Some(want)).map(|p| p.text.clone()).unwrap_or_default();
                scale = Some((lo, hi, text_of(lo), text_of(hi), c0, cm, c1));
            }
        }
        // room for the legend
        let lw = if scale.is_some() {
            let (w1, _) = pen.set(&scale.as_ref().unwrap().3, &small, z, None);
            let (w2, _) = pen.set(&scale.as_ref().unwrap().2, &small, z, None);
            14.0 * z + w1.max(w2) + 10.0 * z
        } else if !legend.is_empty() {
            let w = legend.iter().map(|(_, n)| pen.set(n, &small, z, None).0).fold(0.0, f64::max);
            w + 22.0 * z
        } else {
            0.0
        };
        let (ml, mt, mr, mb) = (l, t, (r - lw - 6.0 * z).max(l + 10.0), b);
        // fit the shown regions into the plot
        let proj = plan.projection;
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &i in &plan.fit {
            for ring in main_rings(&a.regions[i]) {
                for &(lon, lat) in ring.iter() {
                    let (px, py) = proj.project(lon, lat);
                    x0 = x0.min(px);
                    x1 = x1.max(px);
                    y0 = y0.min(py);
                    y1 = y1.max(py);
                }
            }
        }
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let pad = 0.03 * (x1 - x0).max(y1 - y0);
        let (x0, x1, y0, y1) = (x0 - pad, x1 + pad, y0 - pad, y1 + pad);
        let k = ((mr - ml) / (x1 - x0)).min((mb - mt) / (y1 - y0));
        let ox = ml + ((mr - ml) - (x1 - x0) * k) / 2.0;
        let oy = mt + ((mb - mt) - (y1 - y0) * k) / 2.0;
        let to = |lon: f64, lat: f64| {
            let (px, py) = proj.project(lon, lat);
            (ox + (px - x0) * k, oy + (y1 - py) * k)
        };
        cairo_save(cr);
        cairo_rectangle(cr, ml, mt, mr - ml, mb - mt);
        cairo_clip(cr);
        cairo_set_line_join(cr, 1);
        // regions without data first, those with data over them
        let mut order: Vec<usize> = plan.drawn.clone();
        order.sort_by_key(|i| colour.contains_key(i));
        for &i in &order {
            let reg = &a.regions[i];
            cairo_new_path(cr);
            for ring in &reg.rings {
                let mut first = true;
                for &(lon, lat) in ring.iter() {
                    let (px, py) = to(lon, lat);
                    if first {
                        cairo_move_to(cr, px, py);
                        first = false;
                    } else {
                        cairo_line_to(cr, px, py);
                    }
                }
                cairo_close_path(cr);
            }
            cairo_set_fill_rule(cr, 1);
            rgb(cr, colour.get(&i).copied().unwrap_or(NO_DATA));
            cairo_fill_preserve(cr);
            rgb(cr, 0xFFFFFF);
            cairo_set_line_width(cr, (0.6 * z).max(0.4));
            cairo_stroke(cr);
        }
        // labels
        if cx.map.labels != "none" {
            for &(i, _k) in &plan.found {
                let reg = &a.regions[i];
                let (lx, ly) = to(reg.label.0, reg.label.1);
                let mut bx = (f64::MAX, f64::MIN);
                for ring in main_rings(reg) {
                    for &(lon, lat) in ring.iter() {
                        let (px, _) = to(lon, lat);
                        bx = (bx.0.min(px), bx.1.max(px));
                    }
                }
                let name = if plan.states && !reg.postal.is_empty() && cx.map.labels != "showAll" && (bx.1 - bx.0) < 60.0 * z { reg.postal.clone() } else { reg.name.clone() };
                let (tw, th) = pen.set(&name, &small, z, None);
                if cx.map.labels == "showAll" || tw <= (bx.1 - bx.0) * 0.9 {
                    let c = colour.get(&i).copied().unwrap_or(NO_DATA);
                    let lum = 0.299 * ((c >> 16) & 255) as f64 + 0.587 * ((c >> 8) & 255) as f64 + 0.114 * (c & 255) as f64;
                    rgb(cr, if lum < 140.0 { 0xFFFFFF } else { 0x404040 });
                    pen.show(cr, lx - tw / 2.0, ly - th / 2.0);
                }
            }
        }
        cairo_restore(cr);
        // the legend
        let gx = r - lw + 4.0 * z;
        if let Some((_lo, _hi, tlo, thi, c0, cm, c1)) = scale {
            let gh = ((b - t) * 0.6).max(30.0 * z);
            let gy = t + ((b - t) - gh) / 2.0;
            let steps = 40;
            for n in 0..steps {
                let tpos = 1.0 - n as f64 / steps as f64;
                let c = match cm {
                    Some(m) if tpos < 0.5 => mix(c0, m, tpos * 2.0),
                    Some(m) => mix(m, c1, (tpos - 0.5) * 2.0),
                    None => mix(c0, c1, tpos),
                };
                rgb(cr, c);
                cairo_rectangle(cr, gx, gy + gh * n as f64 / steps as f64, 10.0 * z, gh / steps as f64 + 0.5);
                cairo_fill(cr);
            }
            let (_, th) = pen.set(&thi, &small, z, None);
            rgb(cr, TEXT);
            pen.show(cr, gx + 14.0 * z, gy - th / 2.0);
            pen.set(&tlo, &small, z, None);
            rgb(cr, TEXT);
            pen.show(cr, gx + 14.0 * z, gy + gh - th / 2.0);
        } else if !legend.is_empty() {
            let (_, th) = pen.set("Ag", &small, z, None);
            let total = legend.len() as f64 * (th + 4.0 * z);
            let mut y = t + ((b - t) - total).max(0.0) / 2.0;
            for (c, name) in &legend {
                rgb(cr, *c);
                cairo_rectangle(cr, gx, y + (th - 8.0 * z) / 2.0, 8.0 * z, 8.0 * z);
                cairo_fill(cr);
                pen.set(name, &small, z, None);
                rgb(cr, TEXT);
                pen.show(cr, gx + 12.0 * z, y);
                y += th + 4.0 * z;
            }
        }
    }
}
