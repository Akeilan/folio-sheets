//! PivotCharts: a chart of a PivotTable's body (its items down the rows, a series per value
//! column, the grand totals left out) that follows the PivotTable as it changes.

use crate::book::{Book, Delta};
use crate::book_chart::{ChartSpec, SeriesSpec};
use crate::cellref::col_name;
use crate::pivot::Role;
use std::rc::Rc;

impl Book {
    /// The series a PivotChart of pivot `pi` plots.
    pub fn pivot_chart_series(&self, pi: usize) -> Result<Vec<SeriesSpec>, String> {
        let p = self.pivots.get(pi).ok_or("No such PivotTable.")?;
        let t = p.table_rect().ok_or("The PivotTable is empty.")?;
        if p.values.is_empty() {
            return Err("Add a field to the Values area of the PivotTable first; a PivotChart plots its values.".into());
        }
        let role = |r: u32, c: u32| p.roles.get(&(r, c)).copied();
        let mut first_data: Option<(u32, u32)> = None;
        for r in t.r1..=t.r2 {
            for c in t.c1..=t.c2 {
                if role(r, c) == Some(Role::Data) {
                    first_data = Some(match first_data { Some((a, b)) => (a.min(r), b.min(c)), None => (r, c) });
                }
            }
        }
        let (r0, c0) = first_data.ok_or("The PivotTable has no values to plot.")?;
        let label_col = if c0 > t.c1 { c0 - 1 } else { t.c1 };
        let grand_row = |r: u32| (t.c1..=t.c2).any(|c| matches!(role(r, c), Some(Role::GrandLabel) | Some(Role::GrandRow)));
        let mut r1 = r0;
        for r in r0..=t.r2 {
            if grand_row(r) {
                break;
            }
            r1 = r;
        }
        let cols: Vec<u32> = (c0..=t.c2).filter(|&c| (r0..=r1).any(|r| role(r, c) == Some(Role::Data))
            && !(t.r1..=t.r2).any(|r| role(r, c) == Some(Role::GrandCol))).collect();
        let sheet = self.sheets[p.sheet].name.clone();
        let sq = if sheet.chars().all(|c| c.is_alphanumeric() || c == '_') { sheet } else { format!("'{}'", sheet.replace('\'', "''")) };
        let cell = |r: u32, c: u32| format!("{}!${}${}", sq, col_name(c), r + 1);
        let range = |ra: u32, rb: u32, c: u32| format!("{}!${}${}:${}${}", sq, col_name(c), ra + 1, col_name(c), rb + 1);
        let has_rows = !p.rows.is_empty() || (p.values.len() > 1 && p.values_in_rows);
        Ok(cols.iter().map(|&c| SeriesSpec {
            name: if r0 > t.r1 { cell(r0 - 1, c) } else { String::new() },
            cat: if has_rows { range(r0, r1, label_col) } else { String::new() },
            val: range(r0, r1, c),
            kind: "col".into(),
            secondary: false,
            color: None,
        }).collect())
    }

    /// Insert › PivotChart: a chart of the PivotTable, beside it.
    pub fn insert_pivot_chart(&mut self, pi: usize, kind: &str, x: f64, y: f64) -> Result<usize, String> {
        let series = self.pivot_chart_series(pi)?;
        let p = self.pivots[pi].clone();
        let spec = ChartSpec {
            kind: if kind.is_empty() { "col".into() } else { kind.to_string() },
            title_mode: if series.len() == 1 { "text".into() } else { "none".into() },
            title: if series.len() == 1 { "Total".into() } else { String::new() },
            legend: "r".into(),
            gridlines: true,
            series,
            ..Default::default()
        };
        let mut ch = self.chart_of_spec(&spec)?;
        ch.pivot = Some(p.name.clone());
        self.add_chart(p.sheet, ch, x, y, 480.0, 288.0)
    }

    /// The PivotCharts of pivot `pi` follow its new shape (inside the current edit).
    pub fn relink_pivot_charts(&mut self, pi: usize) {
        let Some(name) = self.pivots.get(pi).map(|p| p.name.clone()) else { return };
        for s in 0..self.sheets.len() {
            if !self.sheets[s].objects.iter().any(|o| o.chart.as_ref().map_or(false, |c| c.pivot.as_deref() == Some(&name))) {
                continue;
            }
            let Ok(series) = self.pivot_chart_series(pi) else { continue };
            let mut objs = self.sheets[s].objects.clone();
            let mut changed = false;
            for o in objs.iter_mut() {
                let Some(ch) = o.chart.clone() else { continue };
                if ch.pivot.as_deref() != Some(&name) {
                    continue;
                }
                let mut spec = self.spec_of(&ch);
                let kind0 = spec.series.first().map(|x| x.kind.clone()).unwrap_or_else(|| "col".into());
                spec.series = series.iter().map(|x| SeriesSpec { kind: kind0.clone(), ..x.clone() }).collect();
                if let Ok(mut fresh) = self.chart_of_spec(&spec) {
                    fresh.palette = ch.palette;
                    fresh.pivot = ch.pivot.clone();
                    o.chart = Some(Rc::new(fresh));
                    changed = true;
                }
            }
            if changed {
                self.record(Delta::Objects(s, objs));
            }
        }
    }
}
