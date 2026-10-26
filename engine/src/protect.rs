//! Protect Sheet and Protect Workbook: their settings, and which cells are locked.
//! Passwords are hashed and checked by the app; the engine keeps the attributes.

use crate::book::Book;
use crate::cellref::Rect;

impl Book {
    pub fn sheet_protection(&self, s: usize) -> Option<Vec<(String, String)>> {
        self.sheets.get(s)?.protection.clone()
    }

    /// Turn protection on (the attributes) or off (None). Not an undo step: undo must not
    /// unprotect; the book is marked changed.
    pub fn set_sheet_protection(&mut self, s: usize, attrs: Option<Vec<(String, String)>>) {
        if self.sheets[s].protection != attrs {
            self.sheets[s].protection = attrs;
            self.edits += 1;
        }
    }

    pub fn workbook_protection(&self) -> Option<Vec<(String, String)>> {
        match &self.wb_protection {
            Some(x) => x.clone(),
            None => self.file.wb.as_ref().and_then(|(_, e)| e.child("workbookProtection")).map(|e| e.attrs.clone()),
        }
    }

    pub fn set_workbook_protection(&mut self, attrs: Option<Vec<(String, String)>>) {
        if self.workbook_protection() != attrs {
            self.wb_protection = Some(attrs);
            self.edits += 1;
        }
    }

    fn xf_locked(&self, xf: u32) -> bool {
        self.styles.xf(xf).locked
    }

    /// Whether any cell of the ranges is locked (the Lock Cell setting; on by default).
    pub fn locked_in(&self, s: usize, ranges: &[Rect]) -> bool {
        let sh = &self.sheets[s];
        for rc in ranges {
            let rc = sh.clamp_rect(*rc);
            if rc.count() <= 250_000 {
                for r in rc.r1..=rc.r2 {
                    for c in rc.c1..=rc.c2 {
                        if self.xf_locked(sh.style_at(r, c)) {
                            return true;
                        }
                    }
                }
                continue;
            }
            // a big range: the cells there, then what the rest takes from its row or column
            for ((r, c), cell) in sh.cells.range((rc.r1, 0)..=(rc.r2, u32::MAX)) {
                if *c >= rc.c1 && *c <= rc.c2 && *r >= rc.r1 && self.xf_locked(cell.s) {
                    return true;
                }
            }
            for (_, ri) in sh.rows.range(rc.r1..=rc.r2) {
                if let Some(st) = ri.style {
                    if self.xf_locked(st) {
                        return true;
                    }
                }
            }
            for c in rc.c1..=rc.c2 {
                if self.xf_locked(sh.col_style(c)) {
                    return true;
                }
            }
        }
        false
    }
}
