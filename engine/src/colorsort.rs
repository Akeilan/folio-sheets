//! Sort and filter by colour (S3-02): a cell's fill, font colour and conditional-format icon as
//! they are shown (conditional formats included), Sort On Cell Color / Font Color / Cell Icon
//! keys, and Filter by Color.
use crate::book::Book;
use crate::cellref::Rect;
use crate::value::Value;

/// What a sort key looks at.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum SortOn {
    #[default]
    Values,
    /// the cells with this fill (None = no fill) go on top (ascending) or at the bottom
    Cell(Option<u32>),
    Font(Option<u32>),
    /// (icon set, icon index)
    Icon(String, u32),
}

impl Book {
    /// The fill a cell shows: a conditional format's fill or colour scale first, then its own.
    pub fn shown_fill(&self, s: usize, r: u32, c: u32) -> Option<u32> {
        let blocks = self.cf_blocks(s);
        if let Some(o) = self.cf_at(s, r, c, &blocks) {
            if let Some(f) = o.dxf.fill {
                return Some(f);
            }
            if let Some(f) = o.scale {
                return Some(f);
            }
        }
        let xf = self.styles.xf(self.sheets[s].style_at(r, c));
        self.fill_color(&xf)
    }

    /// The font colour a cell shows (None = automatic).
    pub fn shown_font(&self, s: usize, r: u32, c: u32) -> Option<u32> {
        let blocks = self.cf_blocks(s);
        if let Some(o) = self.cf_at(s, r, c, &blocks) {
            if let Some(f) = o.dxf.color {
                return Some(f);
            }
        }
        let xf = self.styles.xf(self.sheets[s].style_at(r, c));
        match &self.styles.font(xf.font).color {
            crate::styles::Color::Auto => None,
            col => Some(self.styles.rgb(col, 0x000000)),
        }
    }

    /// The conditional-format icon a cell shows.
    pub fn shown_icon(&self, s: usize, r: u32, c: u32) -> Option<(String, u32)> {
        let blocks = self.cf_blocks(s);
        self.cf_at(s, r, c, &blocks).and_then(|o| o.icon)
    }

    /// Whether the cell matches a colour key.
    pub fn color_key_match(&self, s: usize, r: u32, c: u32, on: &SortOn) -> bool {
        match on {
            SortOn::Values => false,
            SortOn::Cell(want) => self.shown_fill(s, r, c) == *want,
            SortOn::Font(want) => self.shown_font(s, r, c).unwrap_or(0) == want.unwrap_or(0),
            SortOn::Icon(set, k) => self.shown_icon(s, r, c).map(|(a, b)| a == *set && b == *k).unwrap_or(false),
        }
    }

    /// The fills / font colours / icons used in a column of rc (for the Sort and filter menus),
    /// in the order they first appear. kind: "cell" | "font" | "icon".
    pub fn colors_in(&self, s: usize, rc: Rect, kind: &str) -> Vec<(Option<u32>, String, u32)> {
        let sh = &self.sheets[s];
        let last = match sh.data_extent() {
            Some((er, _)) => rc.r2.min(er.max(rc.r1)),
            None => return vec![],
        };
        let mut out: Vec<(Option<u32>, String, u32)> = Vec::new();
        for r in rc.r1..=last {
            if sh.rows.get(&r).map(|x| x.hidden).unwrap_or(false) && kind != "all" {
                // hidden rows still count, as in the original
            }
            for c in rc.c1..=rc.c2 {
                let item = match kind {
                    "font" => {
                        if matches!(sh.value(r, c), Value::Empty) {
                            continue;
                        }
                        (self.shown_font(s, r, c), String::new(), 0)
                    }
                    "icon" => match self.shown_icon(s, r, c) {
                        Some((set, k)) => (None, set, k),
                        None => continue,
                    },
                    _ => (self.shown_fill(s, r, c), String::new(), 0),
                };
                if !out.contains(&item) {
                    out.push(item);
                }
                if out.len() >= 64 {
                    return out;
                }
            }
        }
        out
    }
}
