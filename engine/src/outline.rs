//! Group and Outline: outline levels on rows and columns (up to 7), hiding and showing a
//! group's detail, showing levels, clearing the outline. A group's summary line is the
//! one below it (right of it for columns), unless the sheet says otherwise.

use crate::book::{Book, Delta};
use crate::cellref::{MAX_COLS, MAX_ROWS};
use crate::sheet::Sheet;

pub const MAX_LEVEL: u8 = 7;

/// One line (a row or a column) as the outline sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Line {
    pub level: u8,
    pub hidden: bool,
    pub collapsed: bool,
}

impl Book {
    pub fn summary_below(&self, s: usize) -> bool {
        self.sheets[s].raw_sheetpr.as_ref().and_then(|e| e.child("outlinePr")).and_then(|p| p.attr("summaryBelow")).map(|v| v != "0" && v != "false").unwrap_or(true)
    }

    pub fn summary_right(&self, s: usize) -> bool {
        self.sheets[s].raw_sheetpr.as_ref().and_then(|e| e.child("outlinePr")).and_then(|p| p.attr("summaryRight")).map(|v| v != "0" && v != "false").unwrap_or(true)
    }

    pub fn line(&self, s: usize, rows: bool, i: u32) -> Line {
        let sh = &self.sheets[s];
        if rows {
            sh.rows.get(&i).map(|ri| Line { level: ri.outline, hidden: ri.hidden, collapsed: ri.collapsed }).unwrap_or_default()
        } else {
            sh.col_span(i).map(|c| Line { level: c.outline, hidden: c.hidden, collapsed: c.collapsed }).unwrap_or_default()
        }
    }

    /// The deepest level used (0: no outline).
    pub fn max_level(&self, s: usize, rows: bool) -> u8 {
        let sh = &self.sheets[s];
        if rows {
            sh.rows.values().map(|r| r.outline).max().unwrap_or(0)
        } else {
            sh.cols.iter().map(|c| c.outline).max().unwrap_or(0)
        }
    }

    /// The lines that carry outline settings, in order (rows that have them; columns of spans).
    fn outlined(&self, s: usize, rows: bool) -> Vec<(u32, Line)> {
        let sh = &self.sheets[s];
        if rows {
            sh.rows.iter().filter(|(_, ri)| ri.outline > 0 || ri.collapsed).map(|(r, ri)| (*r, Line { level: ri.outline, hidden: ri.hidden, collapsed: ri.collapsed })).collect()
        } else {
            let mut out = Vec::new();
            for c in &sh.cols {
                if c.outline > 0 || c.collapsed {
                    for i in c.min..=c.max.min(MAX_COLS - 1) {
                        out.push((i, Line { level: c.outline, hidden: c.hidden, collapsed: c.collapsed }));
                    }
                }
            }
            out
        }
    }

    /// The group at `level` that holds line i: (first, last), or None.
    pub fn group_at(&self, s: usize, rows: bool, i: u32, level: u8) -> Option<(u32, u32)> {
        if level == 0 || self.line(s, rows, i).level < level {
            return None;
        }
        let mut a = i;
        while a > 0 && self.line(s, rows, a - 1).level >= level {
            a -= 1;
        }
        let max = if rows { MAX_ROWS } else { MAX_COLS } - 1;
        let mut b = i;
        while b < max && self.line(s, rows, b + 1).level >= level {
            b += 1;
        }
        Some((a, b))
    }

    /// The summary line of a group (below / right of it, or above / left when the sheet says so).
    pub fn summary_of(&self, s: usize, rows: bool, g: (u32, u32)) -> Option<u32> {
        let after = if rows { self.summary_below(s) } else { self.summary_right(s) };
        let max = if rows { MAX_ROWS } else { MAX_COLS } - 1;
        if after {
            if g.1 < max {
                Some(g.1 + 1)
            } else {
                None
            }
        } else if g.0 > 0 {
            Some(g.0 - 1)
        } else {
            None
        }
    }

    /// Write changed lines as one step.
    fn put_lines(&mut self, s: usize, rows: bool, changes: Vec<(u32, Line)>, label: &str) {
        self.begin(label, s);
        if rows {
            for (r, l) in changes {
                let old = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
                let mut ri = old.clone();
                ri.outline = l.level;
                ri.hidden = l.hidden;
                ri.collapsed = l.collapsed;
                if !l.hidden && ri.ht == Some(0.0) {
                    ri.ht = None;
                }
                if ri != old {
                    let v = if ri.is_default() { None } else { Some(ri) };
                    self.record(Delta::Row(s, r, v));
                }
            }
        } else {
            let mut sh = Sheet::new(0, "");
            sh.cols = self.sheets[s].cols.clone();
            let mdw = self.mdw(s);
            let dw = self.sheets[s].default_width_units(mdw);
            for (c, l) in changes {
                sh.map_cols(c, c, |span| {
                    span.outline = l.level;
                    span.hidden = l.hidden;
                    span.collapsed = l.collapsed;
                    if !l.hidden && span.width == Some(0.0) {
                        span.width = Some(dw);
                    }
                });
            }
            merge_spans(&mut sh.cols);
            if sh.cols != self.sheets[s].cols {
                self.record(Delta::Cols(s, sh.cols));
            }
        }
        self.commit();
    }

    /// Group (delta 1) or Ungroup (delta -1) lines first..=last.
    pub fn outline_lines(&mut self, s: usize, rows: bool, first: u32, last: u32, delta: i32) -> Result<(), String> {
        let max = if rows { MAX_ROWS } else { MAX_COLS } - 1;
        let last = last.min(max);
        if last - first > 200_000 {
            return Err("Select fewer rows or columns to group.".into());
        }
        let mut changes = Vec::new();
        for i in first..=last {
            let mut l = self.line(s, rows, i);
            let nl = (l.level as i32 + delta).clamp(0, MAX_LEVEL as i32) as u8;
            if nl != l.level {
                if nl == 0 {
                    // out of every group: shown again
                    l.hidden = false;
                }
                l.level = nl;
                changes.push((i, l));
            }
        }
        if changes.is_empty() {
            return Err(if delta < 0 { "Cannot ungroup that selection.".into() } else { "Cannot group that selection: it is at the deepest level already.".into() });
        }
        if delta < 0 {
            // a summary line left collapsed with nothing hidden in front of it opens again
            let sum = if rows { self.summary_below(s) } else { self.summary_right(s) };
            let k = if sum { last + 1 } else { first.saturating_sub(1) };
            if k <= max {
                let mut l = self.line(s, rows, k);
                if l.collapsed {
                    l.collapsed = false;
                    changes.push((k, l));
                }
            }
        }
        self.put_lines(s, rows, changes, if delta > 0 { "Group" } else { "Ungroup" });
        Ok(())
    }

    /// Hide Detail (collapse) or Show Detail (expand) the innermost group at line i, or the
    /// group whose summary line i is.
    pub fn set_detail(&mut self, s: usize, rows: bool, i: u32, show: bool) -> Result<(), String> {
        let g = match self.detail_group(s, rows, i) {
            Some(g) => g,
            None => return Err("Cannot show or hide detail for this selection.".into()),
        };
        self.set_group_detail(s, rows, g.0, g.1, g.2, show);
        Ok(())
    }

    /// Show or hide one group's detail (the outline bar's + and - buttons).
    pub fn set_group_detail(&mut self, s: usize, rows: bool, a: u32, b: u32, level: u8, show: bool) {
        let mut changes = Vec::new();
        let own = self.summary_of(s, rows, (a, b));
        if show {
            // shown again, except what sits in a deeper group that is still collapsed (a
            // deeper group ending with this one shares its summary line: that flag is ours)
            for k in a..=b {
                let l = self.line(s, rows, k);
                let mut visible = true;
                for lv in (level + 1)..=l.level {
                    if let Some(sub) = self.group_at(s, rows, k, lv) {
                        let sx = self.summary_of(s, rows, sub);
                        let collapsed = sx != own && sx.map(|x| self.line(s, rows, x).collapsed).unwrap_or(false);
                        if collapsed {
                            visible = false;
                        }
                    }
                }
                if l.hidden == visible {
                    changes.push((k, Line { hidden: !visible, ..l }));
                }
            }
        } else {
            for k in a..=b {
                let l = self.line(s, rows, k);
                if !l.hidden {
                    changes.push((k, Line { hidden: true, ..l }));
                }
            }
        }
        if let Some(x) = self.summary_of(s, rows, (a, b)) {
            let l = self.line(s, rows, x);
            if l.collapsed == show {
                changes.push((x, Line { collapsed: !show, ..l }));
            }
        }
        self.put_lines(s, rows, changes, if show { "Show Detail" } else { "Hide Detail" });
    }

    /// The group Show/Hide Detail works on from line i: (first, last, level).
    pub fn detail_group(&self, s: usize, rows: bool, i: u32) -> Option<(u32, u32, u8)> {
        let l = self.line(s, rows, i);
        let after = if rows { self.summary_below(s) } else { self.summary_right(s) };
        // a summary line: the group in front of it (behind it when summaries come first)
        let nb = if after { i.checked_sub(1) } else { Some(i + 1) };
        if let Some(n) = nb {
            let nl = self.line(s, rows, n);
            if nl.level > l.level {
                let g = self.group_at(s, rows, n, l.level + 1)?;
                return Some((g.0, g.1, l.level + 1));
            }
        }
        if l.level > 0 {
            let g = self.group_at(s, rows, i, l.level)?;
            return Some((g.0, g.1, l.level));
        }
        None
    }

    /// Every group: (first, last, level), found in one pass per level.
    pub fn groups(&self, s: usize, rows: bool) -> Vec<(u32, u32, u8)> {
        let lines = self.outlined(s, rows);
        let top = lines.iter().map(|(_, l)| l.level).max().unwrap_or(0);
        let mut out = Vec::new();
        for lv in 1..=top {
            let mut run: Option<(u32, u32)> = None;
            for (i, l) in &lines {
                if l.level >= lv {
                    run = match run {
                        Some((a, b)) if b + 1 == *i => Some((a, *i)),
                        Some((a, b)) => {
                            out.push((a, b, lv));
                            Some((*i, *i))
                        }
                        None => Some((*i, *i)),
                    };
                } else if let Some((a, b)) = run.take() {
                    out.push((a, b, lv));
                }
            }
            if let Some((a, b)) = run {
                out.push((a, b, lv));
            }
        }
        out
    }

    /// The outline's level buttons: level n shows the lines of levels below n.
    pub fn show_levels(&mut self, s: usize, rows: bool, n: u8) {
        let mut changes: std::collections::BTreeMap<u32, Line> = std::collections::BTreeMap::new();
        for (i, l) in self.outlined(s, rows) {
            let hidden = l.level >= n;
            if hidden != l.hidden {
                changes.insert(i, Line { hidden, ..l });
            }
        }
        // a group's summary line says whether the group is open
        for (a, b, lv) in self.groups(s, rows) {
            if let Some(x) = self.summary_of(s, rows, (a, b)) {
                let cur = changes.get(&x).copied().unwrap_or_else(|| self.line(s, rows, x));
                let collapsed = lv >= n;
                if cur.collapsed != collapsed {
                    changes.insert(x, Line { collapsed, ..cur });
                }
            }
        }
        self.put_lines(s, rows, changes.into_iter().collect(), "Show Levels");
    }

    /// Clear Outline: no levels left (hidden lines stay hidden).
    pub fn clear_outline(&mut self, s: usize, rows: bool) {
        let changes: Vec<(u32, Line)> = self.outlined(s, rows).into_iter().map(|(i, l)| (i, Line { level: 0, collapsed: false, ..l })).collect();
        self.put_lines(s, rows, changes, "Clear Outline");
    }
}

/// Neighbouring column spans with the same settings become one.
fn merge_spans(cols: &mut Vec<crate::sheet::ColSpan>) {
    let mut out: Vec<crate::sheet::ColSpan> = Vec::new();
    for c in cols.drain(..) {
        if let Some(last) = out.last_mut() {
            if last.max + 1 == c.min && last.same_settings(&c) {
                last.max = c.max;
                continue;
            }
        }
        out.push(c);
    }
    *cols = out;
}
