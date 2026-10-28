//! Elements of the worksheet and workbook parts the app edits as XML (protected ranges,
//! custom views), and counts for Workbook Statistics.
use crate::book::{Book, Delta};
use crate::sheet::Piece;
use crate::xml;

const WB_ORDER: [&str; 19] = ["fileVersion", "fileSharing", "workbookPr", "workbookProtection", "bookViews", "sheets",
    "functionGroups", "externalReferences", "definedNames", "calcPr", "oleSize", "customWorkbookViews", "pivotCaches",
    "smartTagPr", "smartTagTypes", "webPublishing", "fileRecoveryPr", "webPublishObjects", "extLst"];

pub const DEFAULT_WB: &str = "<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><bookViews><workbookView/></bookViews><sheets/><calcPr calcId=\"191029\"/></workbook>";

impl Book {
    /// A worksheet element kept as XML (by local name), e.g. "protectedRanges".
    pub fn sheet_element(&self, s: usize, tag: &str) -> Option<String> {
        self.sheets.get(s)?.pieces.iter().find_map(|p| match p {
            Piece::Raw(t, x) if t == tag => Some(x.clone()),
            _ => None,
        })
    }

    /// Put (Some) or take away (None) a worksheet element; an undo step when `label` is given.
    pub fn set_sheet_element(&mut self, s: usize, tag: &str, x: Option<&str>, label: Option<&str>) -> Result<(), String> {
        if s >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        if let Some(t) = x {
            xml::parse(t).map_err(|e| format!("bad XML: {}", e))?;
        }
        let mut pieces = self.sheets[s].pieces.clone();
        let at = pieces.iter().position(|p| matches!(p, Piece::Raw(t, _) | Piece::Known(t) if t == tag));
        match (at, x) {
            (Some(i), Some(t)) => pieces[i] = Piece::Raw(tag.to_string(), t.to_string()),
            (Some(i), None) => {
                pieces.remove(i);
            }
            (None, Some(t)) => pieces.push(Piece::Raw(tag.to_string(), t.to_string())),
            (None, None) => return Ok(()),
        }
        match label {
            Some(l) => {
                self.begin(l, s);
                self.record(Delta::Pieces(s, pieces));
                self.commit();
            }
            None => {
                self.sheets[s].pieces = pieces;
                self.edits += 1;
            }
        }
        Ok(())
    }

    /// A workbook element (by local name), e.g. "customWorkbookViews".
    pub fn workbook_element(&self, tag: &str) -> Option<String> {
        self.file.wb.as_ref()?.1.child(tag).map(|e| e.to_string())
    }

    /// Put or take away a workbook element (not an undo step, as in the original).
    pub fn set_workbook_element(&mut self, tag: &str, x: Option<&str>) -> Result<(), String> {
        let e = match x {
            Some(t) => Some(xml::parse(t).map_err(|e| format!("bad XML: {}", e))?.1),
            None => None,
        };
        if self.file.wb.is_none() {
            self.file.wb = Some(xml::parse(DEFAULT_WB).map_err(|e| e.to_string())?);
        }
        let wb = &mut self.file.wb.as_mut().unwrap().1;
        wb.remove_children(tag);
        if let Some(e) = e {
            wb.insert_ordered(e, &WB_ORDER);
        }
        self.edits += 1;
        Ok(())
    }

    /// (cells with data, formulas) on a sheet.
    pub fn sheet_counts(&self, s: usize) -> (usize, usize) {
        let sh = &self.sheets[s];
        let mut data = 0;
        let mut f = 0;
        for c in sh.cells.values() {
            if !c.is_blank() {
                data += 1;
            }
            if c.f.is_some() {
                f += 1;
            }
        }
        (data, f)
    }

    /// Hidden rows and columns as spans (first, last), for Custom Views.
    pub fn hidden_spans(&self, s: usize) -> (Vec<(u32, u32)>, Vec<(u32, u32)>) {
        let sh = &self.sheets[s];
        let mut rows: Vec<(u32, u32)> = Vec::new();
        for (r, ri) in sh.rows.iter() {
            if ri.hidden {
                match rows.last_mut() {
                    Some(l) if l.1 + 1 == *r => l.1 = *r,
                    _ => rows.push((*r, *r)),
                }
            }
        }
        let mut cols: Vec<(u32, u32)> = Vec::new();
        for c in sh.cols.iter().filter(|c| c.hidden) {
            match cols.last_mut() {
                Some(l) if l.1 + 1 == c.min => l.1 = c.max,
                _ => cols.push((c.min, c.max)),
            }
        }
        (rows, cols)
    }

    /// Page Layout > Background (bytes of a PNG, JPEG, GIF or BMP) or Delete Background (None).
    pub fn set_background(&mut self, s: usize, bytes: Option<Vec<u8>>) -> Result<(), String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("A background can be put on worksheets only.".into());
        }
        let pic = match bytes {
            Some(b) => Some(std::rc::Rc::new(crate::picture::Picture::new(b).ok_or("This isn't a picture Sheets can read (PNG, JPEG, GIF or BMP).")?)),
            None => None,
        };
        let sh = &mut self.sheets[s];
        if pic.is_none() {
            sh.pieces.retain(|p| !matches!(p, crate::sheet::Piece::Raw(t, _) | crate::sheet::Piece::Known(t) if t == "picture"));
        }
        sh.background = pic;
        sh.bg_part = None;
        self.edits += 1;
        Ok(())
    }
}
