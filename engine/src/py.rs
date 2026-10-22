//! The Python module: folio_sheets_core.Book and helper functions.

use crate::book::Book;
use crate::book_fmt::{spec, StyleOp};
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::styles::{AlignChange, Change, FontChange};
use crate::value::Value;
use crate::view::{HitPart, ViewParams};
use pyo3::exceptions::{PyIOError, PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};

type R4 = (u32, u32, u32, u32);

fn verr(e: String) -> PyErr {
    PyValueError::new_err(e)
}

fn rect(t: R4) -> Rect {
    Rect::new(t.0.min(t.2), t.1.min(t.3), t.0.max(t.2).min(MAX_ROWS - 1), t.1.max(t.3).min(MAX_COLS - 1))
}

fn rects(v: Vec<R4>) -> Vec<Rect> {
    v.into_iter().map(rect).collect()
}

fn tup(r: &Rect) -> R4 {
    (r.r1, r.c1, r.r2, r.c2)
}

pub fn py_value(py: Python<'_>, v: &Value) -> PyObject {
    match v {
        Value::Empty => py.None(),
        Value::Num(n) => n.into_py(py),
        Value::Str(s) => (&**s).into_py(py),
        Value::Bool(b) => b.into_py(py),
        Value::Err(e) => ("error", e.text()).into_py(py),
        Value::Arr(a) => py_value(py, a.data.first().unwrap_or(&Value::Empty)),
    }
}

fn get<'py, T: FromPyObject<'py>>(d: &Bound<'py, PyDict>, k: &str) -> Option<T> {
    d.get_item(k).ok().flatten().filter(|v| !v.is_none()).and_then(|v| v.extract::<T>().ok())
}

fn has(d: &Bound<'_, PyDict>, k: &str) -> bool {
    d.get_item(k).ok().flatten().is_some()
}

fn view_params(d: &Bound<'_, PyDict>) -> ViewParams {
    let mut p = ViewParams::default();
    if let Some(v) = get::<f64>(d, "width") {
        p.width = v;
    }
    if let Some(v) = get::<f64>(d, "height") {
        p.height = v;
    }
    if let Some(v) = get::<f64>(d, "zoom") {
        p.zoom = if v > 10.0 { v / 100.0 } else { v };
    }
    p.top_row = get(d, "top_row").unwrap_or(0);
    p.left_col = get(d, "left_col").unwrap_or(0);
    p.freeze_rows = get(d, "freeze_rows").unwrap_or(0);
    p.freeze_cols = get(d, "freeze_cols").unwrap_or(0);
    p.show_grid = get(d, "show_grid").unwrap_or(true);
    p.show_headers = get(d, "show_headers").unwrap_or(true);
    if let Some(v) = get::<Vec<R4>>(d, "sel") {
        p.sel = rects(v);
    }
    if let Some(v) = get::<(u32, u32)>(d, "active") {
        p.active = v;
    }
    p.editing = get(d, "editing");
    p.copy = get::<R4>(d, "copy").map(rect);
    p.ants_phase = get(d, "ants_phase").unwrap_or(0.0);
    if let Some(v) = get::<Vec<(R4, u32)>>(d, "refs") {
        p.refs = v.into_iter().map(|(r, c)| (rect(r), c)).collect();
    }
    p.show_formulas = get(d, "show_formulas").unwrap_or(false);
    if let Some(v) = get::<Vec<(R4, (u32, u32), u32)>>(d, "arrows") {
        p.arrows = v.into_iter().map(|(r, t, c)| (rect(r), t, c)).collect();
    }
    p.dark = get(d, "dark").unwrap_or(false);
    p
}

fn pen(v: &Bound<'_, PyAny>) -> PyResult<Option<(String, Option<u32>)>> {
    if v.is_none() {
        return Ok(None);
    }
    let (s, c): (String, Option<u32>) = v.extract()?;
    if s == "none" || s.is_empty() {
        return Ok(None);
    }
    Ok(Some((s, c)))
}

/// A derive() dict (font/fill/border/align/protection/numfmt/reset) as a Change.
fn change(d: &Bound<'_, PyDict>) -> PyResult<Change> {
    let mut ch = Change { reset: get(d, "reset").unwrap_or(false), ..Default::default() };
    if let Some(f) = d.get_item("font")? {
        let f = f.downcast::<PyDict>()?;
        let mut fc = FontChange { name: get(f, "name"), size: get(f, "size"), grow: get(f, "grow").unwrap_or(0), bold: get(f, "bold"), italic: get(f, "italic"), strike: get(f, "strike"), ..Default::default() };
        if has(f, "underline") {
            fc.underline = Some(get::<String>(f, "underline").filter(|s| s != "none"));
        }
        if has(f, "color") {
            fc.color = Some(get::<u32>(f, "color"));
        }
        if has(f, "vert") {
            fc.vert = Some(get::<String>(f, "vert").filter(|s| s != "baseline"));
        }
        ch.font = Some(fc);
    }
    if let Some(v) = d.get_item("fill")? {
        if v.is_none() {
            ch.fill = Some(None);
        } else {
            let v = v.downcast::<PyDict>()?;
            let pattern: String = get(v, "pattern").unwrap_or_else(|| "solid".into());
            let fg: Option<u32> = get(v, "fg");
            let bg: Option<u32> = get(v, "bg");
            ch.fill = if pattern == "none" || (fg.is_none() && pattern == "solid") { Some(None) } else { Some(Some((pattern, fg, bg))) };
        }
    }
    if let Some(b) = d.get_item("border")? {
        let b = b.downcast::<PyDict>()?;
        let mut up: Option<bool> = None;
        let mut down: Option<bool> = None;
        for (k, v) in b.iter() {
            let k: String = k.extract()?;
            match k.as_str() {
                "diagonalUp" => up = Some(v.is_truthy()?),
                "diagonalDown" => down = Some(v.is_truthy()?),
                side => ch.border.push((side.to_string(), pen(&v)?)),
            }
        }
        if up.is_some() || down.is_some() {
            ch.diag = Some((up.unwrap_or(false), down.unwrap_or(false)));
        }
    }
    if let Some(a) = d.get_item("align")? {
        let a = a.downcast::<PyDict>()?;
        ch.align = Some(AlignChange {
            horizontal: get(a, "horizontal"),
            vertical: get(a, "vertical"),
            wrap: get(a, "wrap"),
            shrink: get(a, "shrink"),
            indent: get(a, "indent"),
            indent_step: get(a, "indent_step").unwrap_or(0),
            rotation: get(a, "rotation"),
            reading: get(a, "reading"),
        });
    }
    if let Some(p) = d.get_item("protection")? {
        let p = p.downcast::<PyDict>()?;
        ch.locked = get(p, "locked");
        ch.hidden = get(p, "hidden");
    }
    if let Some(n) = get::<String>(d, "numfmt") {
        ch.numfmt = Some(n);
    }
    Ok(ch)
}

fn style_op(item: &Bound<'_, PyAny>) -> PyResult<StyleOp> {
    if let Ok(d) = item.downcast::<PyDict>() {
        return Ok(StyleOp::Change(change(d)?));
    }
    let t = item.downcast::<PyTuple>()?;
    let kind: String = t.get_item(0)?.extract()?;
    match kind.as_str() {
        "change" => Ok(StyleOp::Change(change(t.get_item(1)?.downcast::<PyDict>()?)?)),
        "preset" => {
            let preset: String = t.get_item(1)?.extract()?;
            let style: String = t.get_item(2).ok().and_then(|v| v.extract().ok()).unwrap_or_else(|| "thin".into());
            let color: Option<u32> = t.get_item(3).ok().and_then(|v| v.extract().ok());
            Ok(StyleOp::Preset(preset, style, color))
        }
        "layout" => {
            let d = t.get_item(1)?;
            let d = d.downcast::<PyDict>()?;
            let mut lay = Vec::new();
            for (k, v) in d.iter() {
                lay.push((k.extract::<String>()?, pen(&v)?));
            }
            Ok(StyleOp::Layout(lay))
        }
        "set" => Ok(StyleOp::Set(t.get_item(1)?.extract()?)),
        "grid" => Ok(StyleOp::Grid(t.get_item(1)?.extract()?)),
        k => Err(PyValueError::new_err(format!("unknown style operation {}", k))),
    }
}

fn paste_opts(d: Option<&Bound<'_, PyDict>>) -> PyResult<crate::book_clip::PasteOpts> {
    use crate::book_clip::{Op, PasteOpts, What};
    let mut o = PasteOpts::default();
    if let Some(d) = d {
        if let Some(w) = get::<String>(d, "what") {
            o.what = match w.as_str() {
                "all" => What::All,
                "formulas" => What::Formulas,
                "values" => What::Values,
                "formats" => What::Formats,
                "noborders" => What::NoBorders,
                "widths" => What::Widths,
                "formulas_numfmt" => What::FormulasNumfmt,
                "values_numfmt" => What::ValuesNumfmt,
                "all_widths" => What::AllWidths,
                x => return Err(PyValueError::new_err(format!("unknown paste {}", x))),
            };
        }
        if let Some(w) = get::<String>(d, "op") {
            o.op = match w.as_str() {
                "add" => Op::Add,
                "sub" => Op::Sub,
                "mul" => Op::Mul,
                "div" => Op::Div,
                _ => Op::None,
            };
        }
        o.skip_blanks = get(d, "skip_blanks").unwrap_or(false);
        o.transpose = get(d, "transpose").unwrap_or(false);
        o.link = get(d, "link").unwrap_or(false);
    }
    Ok(o)
}

#[pyclass(unsendable, name = "Book", module = "folio_sheets_core")]
pub struct PyBook {
    pub b: Book,
}

impl PyBook {
    fn sheet(&self, s: usize) -> PyResult<()> {
        if s >= self.b.sheets.len() {
            return Err(PyIndexError::new_err("no such sheet"));
        }
        Ok(())
    }
}

#[pymethods]
impl PyBook {
    #[staticmethod]
    fn open(path: &str) -> PyResult<PyBook> {
        Book::open(path).map(|b| PyBook { b }).map_err(PyIOError::new_err)
    }

    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<PyBook> {
        Book::from_bytes(data).map(|b| PyBook { b }).map_err(PyIOError::new_err)
    }

    #[staticmethod]
    fn new() -> PyBook {
        PyBook { b: Book::new_empty() }
    }

    fn save(&mut self, path: &str) -> PyResult<()> {
        self.b.save(path).map_err(PyIOError::new_err)
    }

    fn to_bytes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let d = self.b.to_bytes().map_err(PyIOError::new_err)?;
        Ok(PyBytes::new_bound(py, &d))
    }

    // ---- properties
    #[getter]
    fn sheet_count(&self) -> usize {
        self.b.sheets.len()
    }
    #[getter]
    fn active_sheet(&self) -> usize {
        self.b.active
    }
    #[setter]
    fn set_active_sheet(&mut self, i: usize) {
        if i < self.b.sheets.len() {
            self.b.active = i;
        }
    }
    #[getter]
    fn modified(&self) -> bool {
        self.b.modified()
    }
    #[getter]
    fn can_undo(&self) -> bool {
        !self.b.undo_stack.is_empty()
    }
    #[getter]
    fn can_redo(&self) -> bool {
        !self.b.redo_stack.is_empty()
    }
    #[getter]
    fn version(&self) -> u64 {
        self.b.edits
    }
    #[getter]
    fn date1904(&self) -> bool {
        self.b.date1904
    }
    #[getter]
    fn calc_manual(&self) -> bool {
        self.b.calc.manual
    }
    #[setter]
    fn set_calc_manual(&mut self, on: bool) {
        let mut c = self.b.calc.clone();
        c.manual = on;
        self.b.set_calc(c);
    }
    #[getter]
    fn calc_pending(&self) -> bool {
        self.b.pending
    }
    #[getter]
    fn iterate(&self) -> (bool, u32, f64) {
        (self.b.calc.iterate, self.b.calc.count, self.b.calc.delta)
    }
    fn set_iterate(&mut self, on: bool, count: u32, delta: f64) {
        let mut c = self.b.calc.clone();
        c.iterate = on;
        c.count = count.clamp(1, 32767);
        c.delta = delta.max(0.0);
        self.b.set_calc(c);
    }
    fn structure(&self) -> Option<bool> {
        None
    }

    // ---- history
    fn undo(&mut self) -> Option<String> {
        self.b.undo().map(|x| x.0)
    }
    fn redo(&mut self) -> Option<String> {
        self.b.redo().map(|x| x.0)
    }
    fn undo_labels(&self) -> Vec<String> {
        self.b.undo_labels()
    }
    fn redo_labels(&self) -> Vec<String> {
        self.b.redo_labels()
    }
    /// The sheet the next undo (or redo) step goes back to.
    fn undo_sheet(&self) -> Option<usize> {
        self.b.undo_stack.last().map(|s| s.sheet)
    }
    fn redo_sheet(&self) -> Option<usize> {
        self.b.redo_stack.last().map(|s| s.sheet)
    }
    fn mark_saved(&mut self) {
        self.b.mark_saved()
    }
    #[pyo3(signature = (label, sheet=None))]
    fn begin_group(&mut self, label: &str, sheet: Option<usize>) {
        let s = sheet.unwrap_or(self.b.active);
        self.b.begin_group(label, s);
    }
    fn end_group(&mut self) {
        self.b.end_group()
    }

    // ---- sheets
    fn sheet_info<'py>(&self, py: Python<'py>, i: usize) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(i)?;
        let sh = &self.b.sheets[i];
        let d = PyDict::new_bound(py);
        d.set_item("name", &sh.name)?;
        d.set_item("state", &sh.state)?;
        d.set_item("kind", &sh.kind)?;
        d.set_item("tab_color", sh.tab_color)?;
        let v = &sh.view;
        d.set_item("freeze_rows", v.freeze_rows)?;
        d.set_item("freeze_cols", v.freeze_cols)?;
        d.set_item("top_row", v.top_row)?;
        d.set_item("left_col", v.left_col)?;
        d.set_item("active", v.active)?;
        d.set_item("sel", v.sel.iter().map(tup).collect::<Vec<_>>())?;
        d.set_item("zoom", v.zoom)?;
        d.set_item("show_grid", v.show_grid)?;
        d.set_item("show_headers", v.show_headers)?;
        d.set_item("view", &v.mode)?;
        d.set_item("error", sh.error.clone())?;
        Ok(d)
    }

    #[pyo3(signature = (at, name=None))]
    fn insert_sheet(&mut self, at: usize, name: Option<&str>) -> PyResult<usize> {
        self.b.insert_sheet(at, name).map_err(verr)
    }
    fn delete_sheet(&mut self, i: usize) -> PyResult<()> {
        self.b.delete_sheet(i).map_err(verr)
    }
    fn rename_sheet(&mut self, i: usize, name: &str) -> PyResult<()> {
        self.b.rename_sheet(i, name).map_err(verr)
    }
    fn move_sheet(&mut self, from: usize, to: usize) -> PyResult<()> {
        self.b.move_sheet(from, to).map_err(verr)
    }
    fn set_sheet_state(&mut self, i: usize, state: &str) -> PyResult<()> {
        self.b.set_sheet_state(i, state).map_err(verr)
    }
    #[pyo3(signature = (i, rgb=None))]
    fn set_tab_color(&mut self, i: usize, rgb: Option<u32>) -> PyResult<()> {
        self.b.set_tab_color(i, rgb).map_err(verr)
    }
    #[pyo3(signature = (name, i=None))]
    fn check_sheet_name(&self, name: &str, i: Option<usize>) -> Option<String> {
        self.b.check_sheet_name(name, i)
    }
    fn has_data(&self, i: usize) -> bool {
        self.b.sheets.get(i).map(|s| s.data_extent().is_some() || s.cells.values().any(|c| !c.is_blank())).unwrap_or(false)
    }
    fn set_view(&mut self, s: usize, d: &Bound<'_, PyDict>) -> PyResult<()> {
        self.sheet(s)?;
        let v = &mut self.b.sheets[s].view;
        let old = v.clone();
        if let Some(x) = get(d, "top_row") {
            v.top_row = x;
        }
        if let Some(x) = get(d, "left_col") {
            v.left_col = x;
        }
        if let Some(x) = get::<f64>(d, "zoom") {
            v.zoom = if x <= 10.0 { (x * 100.0).round() as u32 } else { x.round() as u32 };
        }
        if let Some(x) = get(d, "freeze_rows") {
            v.freeze_rows = x;
        }
        if let Some(x) = get(d, "freeze_cols") {
            v.freeze_cols = x;
        }
        if let Some(x) = get(d, "show_grid") {
            v.show_grid = x;
        }
        if let Some(x) = get(d, "show_headers") {
            v.show_headers = x;
        }
        if let Some(x) = get(d, "active") {
            v.active = x;
        }
        if let Some(x) = get::<Vec<R4>>(d, "sel") {
            v.sel = rects(x);
        }
        if let Some(x) = get::<String>(d, "view") {
            v.mode = x;
        }
        let mut cmp = v.clone();
        cmp.changed = old.changed;
        if cmp != old {
            v.changed = true;
        }
        Ok(())
    }

    // ---- cells
    fn value(&self, py: Python<'_>, s: usize, row: u32, col: u32) -> PyObject {
        py_value(py, &self.b.value(s, row, col))
    }
    fn display(&self, s: usize, row: u32, col: u32) -> String {
        self.b.display(s, row, col)
    }
    fn input(&self, s: usize, row: u32, col: u32) -> String {
        self.b.input(s, row, col)
    }
    fn is_formula(&self, s: usize, row: u32, col: u32) -> bool {
        self.b.is_formula(s, row, col)
    }
    fn has_content(&self, s: usize, row: u32, col: u32) -> bool {
        self.b.sheets.get(s).map(|sh| sh.has_content(row, col)).unwrap_or(false)
    }
    fn number_format(&self, s: usize, row: u32, col: u32) -> String {
        self.b.number_format(s, row, col)
    }
    fn array_range(&self, s: usize, row: u32, col: u32) -> Option<R4> {
        self.b.array_range(s, row, col).map(|r| tup(&r))
    }
    /// The spilled range of the formula a cell belongs to.
    fn spill_range(&self, s: usize, row: u32, col: u32) -> Option<R4> {
        let sh = self.b.sheets.get(s)?;
        sh.spills.iter().find(|(_, rc)| rc.contains(row, col)).map(|(_, rc)| tup(rc))
    }
    fn set_input(&mut self, s: usize, row: u32, col: u32, text: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_input(s, row, col, text).map_err(verr)
    }
    fn set_array_formula(&mut self, s: usize, rg: R4, text: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_array_formula(s, rect(rg), text).map_err(verr)
    }
    #[pyo3(signature = (s, ranges, label="Clear"))]
    fn clear_contents(&mut self, s: usize, ranges: Vec<R4>, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.clear_contents(s, &rects(ranges), label).map_err(verr)
    }
    fn used_extent(&self, s: usize) -> Option<(u32, u32)> {
        self.b.sheets.get(s)?.used_extent()
    }
    fn cell_style<'py>(&self, py: Python<'py>, s: usize, row: u32, col: u32) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let sh = &self.b.sheets[s];
        let xi = match sh.merge_at(row, col) {
            Some(m) => sh.style_at(m.r1, m.c1),
            None => sh.style_at(row, col),
        };
        let st = &self.b.styles;
        let x = st.xf(xi);
        let f = crate::layout::font_of(st, xi);
        let d = PyDict::new_bound(py);
        d.set_item("font", &f.family)?;
        d.set_item("size", f.size)?;
        d.set_item("bold", f.bold)?;
        d.set_item("italic", f.italic)?;
        d.set_item("underline", f.underline)?;
        d.set_item("strike", f.strike)?;
        d.set_item("color", f.color)?;
        let fill = self.b.fill_color(x);
        d.set_item("fill", fill)?;
        d.set_item("halign", if x.halign == "general" { "" } else { &x.halign })?;
        d.set_item("valign", if x.valign == "bottom" { "" } else { &x.valign })?;
        d.set_item("wrap", x.wrap)?;
        d.set_item("numfmt", st.numfmt_code(x.numfmt))?;
        d.set_item("style_index", xi)?;
        Ok(d)
    }
    /// Everything Format Cells needs about a cell.
    fn style_spec<'py>(&self, py: Python<'py>, s: usize, row: u32, col: u32) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let sh = &self.b.sheets[s];
        let merged = sh.merge_at(row, col);
        let xi = match merged {
            Some(m) => sh.style_at(m.r1, m.c1),
            None => sh.style_at(row, col),
        };
        let sp = spec(&self.b.styles, xi);
        let d = PyDict::new_bound(py);
        let font = PyDict::new_bound(py);
        font.set_item("name", &sp.font_name)?;
        font.set_item("size", sp.size)?;
        font.set_item("bold", sp.bold)?;
        font.set_item("italic", sp.italic)?;
        font.set_item("strike", sp.strike)?;
        font.set_item("underline", sp.underline)?;
        font.set_item("color", sp.color)?;
        font.set_item("vert", sp.vert)?;
        d.set_item("font", font)?;
        let fill = PyDict::new_bound(py);
        fill.set_item("pattern", &sp.fill_pattern)?;
        fill.set_item("fg", sp.fill_fg)?;
        fill.set_item("bg", sp.fill_bg)?;
        d.set_item("fill", fill)?;
        let border = PyDict::new_bound(py);
        for (side, style, color) in &sp.border {
            border.set_item(side, (style, *color))?;
        }
        border.set_item("diagonalUp", sp.diag_up)?;
        border.set_item("diagonalDown", sp.diag_down)?;
        d.set_item("border", border)?;
        let al = PyDict::new_bound(py);
        al.set_item("horizontal", &sp.halign)?;
        al.set_item("vertical", &sp.valign)?;
        al.set_item("wrap", sp.wrap)?;
        al.set_item("shrink", sp.shrink)?;
        al.set_item("indent", sp.indent)?;
        al.set_item("rotation", sp.rotation)?;
        al.set_item("reading", sp.reading)?;
        d.set_item("align", al)?;
        let pr = PyDict::new_bound(py);
        pr.set_item("locked", sp.locked)?;
        pr.set_item("hidden", sp.hidden)?;
        d.set_item("protection", pr)?;
        d.set_item("numfmt", &sp.numfmt)?;
        d.set_item("merged", merged.map(|m| m.count() > 1).unwrap_or(false))?;
        d.set_item("style_index", xi)?;
        Ok(d)
    }
    /// Formatting operations over ranges as one undo step: derive() dicts, or tuples
    /// ("preset", name, style, color) / ("layout", {...}) / ("set", xf) / ("grid", [[xf]]).
    fn restyle(&mut self, s: usize, ranges: Vec<R4>, ops: &Bound<'_, PyList>, label: &str) -> PyResult<bool> {
        self.sheet(s)?;
        let mut v = Vec::new();
        for it in ops.iter() {
            v.push(style_op(&it)?);
        }
        self.b.restyle_ops(s, &rects(ranges), &v, label).map_err(verr)
    }
    fn xf_grid(&self, s: usize, rg: R4) -> PyResult<Vec<Vec<u32>>> {
        self.sheet(s)?;
        Ok(self.b.xf_grid(s, rect(rg)))
    }
    fn set_number_format(&mut self, s: usize, ranges: Vec<R4>, code: &str) -> PyResult<()> {
        self.b.set_number_format(s, &rects(ranges), code).map_err(verr)
    }
    fn adjust_decimals(&mut self, s: usize, ranges: Vec<R4>, row: u32, col: u32, delta: i32) -> PyResult<()> {
        self.b.adjust_decimals(s, &rects(ranges), row, col, delta).map_err(verr)
    }
    fn merges(&self, s: usize) -> Vec<R4> {
        self.b.sheets.get(s).map(|sh| sh.merges.iter().map(tup).collect()).unwrap_or_default()
    }
    fn merge_at(&self, s: usize, row: u32, col: u32) -> Option<R4> {
        self.b.sheets.get(s)?.merge_at(row, col).map(|r| tup(&r))
    }
    #[pyo3(signature = (s, merges, label="Merge Cells"))]
    fn set_merges(&mut self, s: usize, merges: Vec<R4>, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_merges_cmd(s, rects(merges), label);
        Ok(())
    }

    // ---- structure
    fn insert_rows(&mut self, s: usize, at: u32, n: u32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.insert_rows(s, at, n).map_err(verr)
    }
    fn delete_rows(&mut self, s: usize, at: u32, n: u32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.delete_rows(s, at, n).map_err(verr)
    }
    fn insert_cols(&mut self, s: usize, at: u32, n: u32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.insert_cols(s, at, n).map_err(verr)
    }
    fn delete_cols(&mut self, s: usize, at: u32, n: u32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.delete_cols(s, at, n).map_err(verr)
    }
    /// Insert Cells: shift the cells at rg down (down=True) or right.
    fn insert_cells(&mut self, s: usize, rg: R4, down: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.insert_cells(s, rect(rg), down).map_err(verr)
    }
    /// Delete Cells: shift the cells below up (up=True) or the cells to the right left.
    fn delete_cells(&mut self, s: usize, rg: R4, up: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.delete_cells(s, rect(rg), up).map_err(verr)
    }

    // ---- data
    #[pyo3(signature = (s, rg, keys, header, by_cols=false))]
    fn sort_range(&mut self, s: usize, rg: R4, keys: Vec<(u32, bool)>, header: bool, by_cols: bool) -> PyResult<()> {
        self.sheet(s)?;
        let keys: Vec<crate::book_data::SortKey> = keys.into_iter().map(|(line, ascending)| crate::book_data::SortKey { line, ascending }).collect();
        self.b.sort_range(s, rect(rg), &keys, header, by_cols).map_err(verr)
    }
    fn guess_header(&self, s: usize, rg: R4) -> PyResult<bool> {
        self.sheet(s)?;
        Ok(self.b.guess_header(s, rect(rg)))
    }
    fn remove_duplicates(&mut self, s: usize, rg: R4, cols: Vec<u32>, header: bool) -> PyResult<(usize, usize)> {
        self.sheet(s)?;
        self.b.remove_duplicates(s, rect(rg), &cols, header).map_err(verr)
    }
    fn autofilter<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let af = match &self.b.sheets[s].autofilter {
            Some(a) => a,
            None => return Ok(None),
        };
        let d = PyDict::new_bound(py);
        d.set_item("ref", tup(&af.rect))?;
        let cols = PyDict::new_bound(py);
        for fc in &af.cols {
            let f = PyDict::new_bound(py);
            match &fc.kind {
                crate::book_data::FilterKind::Values { values, blank } => {
                    f.set_item("values", values.clone())?;
                    f.set_item("blank", *blank)?;
                }
                crate::book_data::FilterKind::Custom { and, conds } => {
                    f.set_item("custom", conds.clone())?;
                    f.set_item("and", *and)?;
                }
                crate::book_data::FilterKind::Top10 { top, percent, val } => {
                    f.set_item("top10", (*top, *percent, *val))?;
                }
                crate::book_data::FilterKind::Raw(_) => {
                    f.set_item("other", true)?;
                }
            }
            cols.set_item(fc.col, f)?;
        }
        d.set_item("columns", cols)?;
        Ok(Some(d))
    }
    #[pyo3(signature = (s, rg=None))]
    fn set_autofilter(&mut self, s: usize, rg: Option<R4>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_autofilter(s, rg.map(rect)).map_err(verr)
    }
    /// Filter one column (offset in the range): {"values": [...], "blank": bool} |
    /// {"custom": [(op, val)...], "and": bool} | {"top10": (top, percent, n)} | None to clear.
    fn filter_column(&mut self, s: usize, col: u32, spec: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.sheet(s)?;
        let kind = match spec {
            None => None,
            Some(d) => {
                if let Some(v) = get::<Vec<String>>(d, "values") {
                    Some(crate::book_data::FilterKind::Values { values: v, blank: get(d, "blank").unwrap_or(false) })
                } else if let Some(c) = get::<Vec<(String, String)>>(d, "custom") {
                    Some(crate::book_data::FilterKind::Custom { and: get(d, "and").unwrap_or(true), conds: c })
                } else if let Some((top, percent, val)) = get::<(bool, bool, f64)>(d, "top10") {
                    Some(crate::book_data::FilterKind::Top10 { top, percent, val })
                } else {
                    return Err(PyValueError::new_err("unknown filter"));
                }
            }
        };
        self.b.filter_column(s, col, kind).map_err(verr)
    }
    fn clear_filters(&mut self, s: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.clear_filters(s).map_err(verr)
    }
    fn reapply_filter(&mut self, s: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.reapply_filter(s).map_err(verr)
    }
    fn filter_values(&self, s: usize, col: u32) -> PyResult<(Vec<String>, bool)> {
        self.sheet(s)?;
        Ok(self.b.filter_values(s, col))
    }

    // ---- find
    #[pyo3(signature = (sheets, pattern, case=false, whole=false, formulas=true, by_cols=false))]
    fn find_all(&self, sheets: Vec<usize>, pattern: &str, case: bool, whole: bool, formulas: bool, by_cols: bool) -> Vec<(usize, u32, u32)> {
        let o = crate::book_find::FindOpts { pattern: pattern.to_string(), case, whole, formulas, by_cols };
        self.b.find_all(&sheets, &o)
    }
    #[pyo3(signature = (sheets, pattern, with, case=false, whole=false, by_cols=false))]
    fn replace_all(&mut self, sheets: Vec<usize>, pattern: &str, with: &str, case: bool, whole: bool, by_cols: bool) -> PyResult<usize> {
        let o = crate::book_find::FindOpts { pattern: pattern.to_string(), case, whole, formulas: true, by_cols };
        self.b.replace_all(&sheets, &o, with).map_err(verr)
    }
    #[pyo3(signature = (s, row, col, pattern, with, case=false, whole=false))]
    fn replaced_text(&self, s: usize, row: u32, col: u32, pattern: &str, with: &str, case: bool, whole: bool) -> Option<String> {
        let o = crate::book_find::FindOpts { pattern: pattern.to_string(), case, whole, formulas: true, by_cols: false };
        self.b.replaced_text(s, row, col, &o, with)
    }
    #[pyo3(signature = (s, rg, kind, mask=23))]
    fn special_cells(&self, s: usize, rg: R4, kind: &str, mask: u32) -> PyResult<Vec<R4>> {
        self.sheet(s)?;
        Ok(self.b.special_cells(s, rect(rg), kind, mask).iter().map(tup).collect())
    }

    // ---- clipboard
    #[pyo3(signature = (src_s, src, dst_s, dst, opts=None, cut=false))]
    fn paste_cells(&mut self, src_s: usize, src: R4, dst_s: usize, dst: R4, opts: Option<&Bound<'_, PyDict>>, cut: bool) -> PyResult<R4> {
        self.sheet(src_s)?;
        self.sheet(dst_s)?;
        let o = paste_opts(opts)?;
        self.b.paste_cells(src_s, rect(src), dst_s, rect(dst), o, cut).map(|r| tup(&r)).map_err(verr)
    }
    /// A block as a small workbook, for the clipboard.
    fn copy_to_bytes<'py>(&self, py: Python<'py>, s: usize, rg: R4) -> PyResult<Bound<'py, PyBytes>> {
        self.sheet(s)?;
        let d = self.b.copy_to_bytes(s, rect(rg)).map_err(verr)?;
        Ok(PyBytes::new_bound(py, &d))
    }
    #[pyo3(signature = (data, dst_s, dst, opts=None))]
    fn paste_from_bytes(&mut self, data: &[u8], dst_s: usize, dst: R4, opts: Option<&Bound<'_, PyDict>>) -> PyResult<R4> {
        self.sheet(dst_s)?;
        let clip = Book::from_bytes(data).map_err(verr)?;
        let o = paste_opts(opts)?;
        let area = clip.clip_area();
        self.b.paste_from(&clip, 0, area, dst_s, rect(dst), o).map(|r| tup(&r)).map_err(verr)
    }

    // ---- calculation
    #[pyo3(signature = (full=false, sheet=None))]
    fn calculate(&mut self, full: bool, sheet: Option<usize>) -> usize {
        if let Some(s) = sheet {
            let keys: Vec<_> = self.b.deps.formulas().filter(|k| k.0 == s).collect();
            self.b.forced.extend(keys);
        }
        self.b.recalc(full)
    }
    fn circular(&self) -> Vec<(usize, u32, u32)> {
        self.b.circ.clone()
    }
    fn error_cells(&self, s: usize) -> Vec<(u32, u32, &'static str)> {
        self.b.error_cells(s)
    }
    fn precedents(&self, s: usize, row: u32, col: u32) -> Vec<(usize, u32, u32, u32, u32)> {
        self.b.deps.precedents((s, row, col)).into_iter().map(|(sh, r)| (sh, r.r1, r.c1, r.r2, r.c2)).collect()
    }
    fn dependents(&self, s: usize, row: u32, col: u32) -> Vec<(usize, u32, u32)> {
        let mut v = Vec::new();
        self.b.deps.dependents((s, row, col), &mut v);
        v.sort();
        v.dedup();
        v
    }
    #[pyo3(signature = (s, row, col, text, array=false))]
    fn evaluate(&mut self, s: usize, row: u32, col: u32, text: &str, array: bool) -> Option<String> {
        let t = text.strip_prefix('=').unwrap_or(text);
        let e = crate::formula::parse(t).ok()?;
        if s >= self.b.sheets.len() {
            return None;
        }
        let mut ev = crate::calc::Eval::new(&mut self.b, s, row, col, false);
        let v = ev.eval_top(&e);
        Some(match v {
            Value::Arr(a) => {
                if array || a.rows * a.cols > 1 {
                    crate::value::array_text(&a)
                } else {
                    a.data[0].to_string()
                }
            }
            Value::Num(n) => crate::numfmt::num_to_text(n),
            v => v.to_string(),
        })
    }
    fn names(&self) -> Vec<(String, String, Option<usize>, bool)> {
        self.b.names.iter().map(|n| (n.name.clone(), n.text.clone(), n.scope, n.hidden)).collect()
    }
    #[pyo3(signature = (name, refers_to, scope=None, comment=None, old=None))]
    fn define_name(&mut self, name: &str, refers_to: &str, scope: Option<usize>, comment: Option<&str>, old: Option<(String, Option<usize>)>) -> PyResult<()> {
        self.b.define_name(name, refers_to, scope, comment, old).map_err(verr)
    }
    #[pyo3(signature = (name, scope=None))]
    fn delete_name(&mut self, name: &str, scope: Option<usize>) -> PyResult<()> {
        self.b.delete_name(name, scope).map_err(verr)
    }
    #[pyo3(signature = (name, scope=None))]
    fn name_comment(&self, name: &str, scope: Option<usize>) -> Option<String> {
        self.b.names.iter().find(|n| n.name.eq_ignore_ascii_case(name) && n.scope == scope).and_then(|n| n.comment.clone())
    }
    fn shift_formula(&self, text: &str, dr: i64, dc: i64) -> String {
        crate::formula::adjust(text, &crate::formula::Adjust::Shift(dr, dc))
    }
    fn format_number(&self, v: f64, code: &str) -> String {
        crate::numfmt::format_number(v, code, self.b.date1904)
    }
    fn aggregates<'py>(&mut self, py: Python<'py>, s: usize, ranges: Vec<R4>) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let (count, nums, sum, min, max) = self.b.aggregates(s, &rects(ranges));
        let d = PyDict::new_bound(py);
        d.set_item("count", count)?;
        d.set_item("num_count", nums)?;
        d.set_item("sum", sum)?;
        d.set_item("min", if nums > 0 { Some(min) } else { None })?;
        d.set_item("max", if nums > 0 { Some(max) } else { None })?;
        d.set_item("average", if nums > 0 { Some(sum / nums as f64) } else { None })?;
        Ok(d)
    }

    // ---- navigation
    fn ctrl_move(&self, s: usize, row: u32, col: u32, dr: i32, dc: i32) -> (u32, u32) {
        self.b.ctrl_move(s, row, col, dr, dc)
    }
    fn current_region(&self, s: usize, row: u32, col: u32) -> R4 {
        tup(&self.b.current_region(s, row, col))
    }

    // ---- rows and columns
    fn mdw(&self, s: usize) -> f64 {
        self.b.mdw(s)
    }
    fn col_width(&self, s: usize, col: u32) -> f64 {
        self.b.col_width_chars(s, col)
    }
    fn col_px(&self, s: usize, col: u32) -> f64 {
        self.b.col_px(s, col)
    }
    fn row_px(&self, s: usize, row: u32) -> f64 {
        self.b.row_px(s, row)
    }
    fn row_height(&self, s: usize, row: u32) -> f64 {
        self.b.row_height_pt(s, row)
    }
    fn col_x(&self, s: usize, col: u32) -> f64 {
        self.b.col_x(s, col)
    }
    fn row_y(&self, s: usize, row: u32) -> f64 {
        self.b.row_y(s, row)
    }
    fn col_at(&self, s: usize, x: f64) -> u32 {
        self.b.col_at(s, x)
    }
    fn row_at(&self, s: usize, y: f64) -> u32 {
        self.b.row_at(s, y)
    }
    fn default_col_width(&self, s: usize) -> f64 {
        self.b.default_col_width_chars(s)
    }
    fn default_row_height(&self, s: usize) -> f64 {
        self.b.sheets[s].default_row_height
    }
    fn px_to_width(&self, s: usize, px: f64) -> f64 {
        self.b.px_to_width(s, px)
    }
    fn set_col_width(&mut self, s: usize, c1: u32, c2: u32, chars: f64) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_col_width(s, c1, c2, chars);
        Ok(())
    }
    fn set_col_px(&mut self, s: usize, c1: u32, c2: u32, px: f64) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_col_px(s, c1, c2, px);
        Ok(())
    }
    fn set_cols_hidden(&mut self, s: usize, c1: u32, c2: u32, hidden: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_cols_hidden(s, c1, c2, hidden);
        Ok(())
    }
    fn set_default_col_width(&mut self, s: usize, chars: f64) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_default_col_width(s, chars);
        Ok(())
    }
    fn set_row_height(&mut self, s: usize, r1: u32, r2: u32, pt: f64) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_row_height(s, r1, r2, pt);
        Ok(())
    }
    fn set_row_px(&mut self, s: usize, r1: u32, r2: u32, px: f64) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_row_px(s, r1, r2, px);
        Ok(())
    }
    fn set_rows_hidden(&mut self, s: usize, r1: u32, r2: u32, hidden: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_rows_hidden(s, r1, r2, hidden);
        Ok(())
    }
    #[pyo3(signature = (s, c1, c2, rows=None))]
    fn autofit_cols(&mut self, s: usize, c1: u32, c2: u32, rows: Option<(u32, u32)>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.autofit_cols(s, c1, c2, rows);
        Ok(())
    }
    fn autofit_rows(&mut self, s: usize, r1: u32, r2: u32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.autofit_rows(s, r1, r2);
        Ok(())
    }

    // ---- screen
    fn geometry<'py>(&self, py: Python<'py>, s: usize, p: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let g = self.b.geometry(s, &view_params(p));
        let d = PyDict::new_bound(py);
        d.set_item("hw", g.hw)?;
        d.set_item("hh", g.hh)?;
        d.set_item("rows", g.rows)?;
        d.set_item("cols", g.cols)?;
        d.set_item("row_spans", g.row_spans)?;
        d.set_item("col_spans", g.col_spans)?;
        Ok(d)
    }
    fn hit(&self, py: Python<'_>, s: usize, x: f64, y: f64, p: &Bound<'_, PyDict>) -> PyResult<PyObject> {
        self.sheet(s)?;
        let parts = self.b.hit(s, x, y, &view_params(p));
        let items: Vec<PyObject> = parts.into_iter().map(|h| match h {
            HitPart::S(t) => t.into_py(py),
            HitPart::N(n) => n.into_py(py),
        }).collect();
        Ok(PyTuple::new_bound(py, items).into_py(py))
    }
    fn cell_rect(&self, s: usize, row: u32, col: u32, p: &Bound<'_, PyDict>) -> PyResult<Option<(f64, f64, f64, f64)>> {
        self.sheet(s)?;
        Ok(self.b.cell_rect(s, row, col, &view_params(p)))
    }
    fn render(&self, s: usize, cr: &Bound<'_, PyAny>, p: &Bound<'_, PyDict>) -> PyResult<()> {
        self.sheet(s)?;
        let tn = cr.get_type().name()?.to_string();
        if tn != "Context" {
            return Err(PyValueError::new_err("render needs a cairo.Context"));
        }
        // pycairo's Context: PyObject_HEAD then the cairo_t pointer
        let ctx = unsafe { *((cr.as_ptr() as *const u8).add(std::mem::size_of::<pyo3::ffi::PyObject>()) as *const *mut crate::ffi::cairo_t) };
        if ctx.is_null() {
            return Err(PyValueError::new_err("no cairo context"));
        }
        let vp = view_params(p);
        unsafe { self.b.render(s, ctx, &vp) };
        Ok(())
    }
    fn render_png(&self, s: usize, path: &str, p: &Bound<'_, PyDict>) -> PyResult<()> {
        crate::render::render_png(&self.b, s, &view_params(p), path).map_err(PyIOError::new_err)
    }
}

#[pyfunction]
fn check_formula(text: &str) -> Option<String> {
    let t = text.strip_prefix('=').unwrap_or(text);
    crate::formula::check(t)
}

#[pyfunction]
fn col_name(c: u32) -> String {
    crate::cellref::col_name(c)
}

#[pyfunction]
#[pyo3(name = "format_number")]
fn format_number_fn(v: f64, code: &str) -> String {
    crate::numfmt::format_number(v, code, false)
}

#[pyfunction]
fn function_names() -> Vec<&'static str> {
    crate::fns::names()
}

#[pyfunction]
fn parse_ref(s: &str) -> Option<R4> {
    crate::cellref::parse_range(s.trim()).map(|r| tup(&r))
}

#[pyfunction]
fn ref_spans(text: &str) -> Vec<(usize, usize, Option<String>, u32, u32, u32, u32)> {
    crate::formula::ref_spans(text)
}

#[pymodule]
fn folio_sheets_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBook>()?;
    m.add_function(wrap_pyfunction!(check_formula, m)?)?;
    m.add_function(wrap_pyfunction!(col_name, m)?)?;
    m.add_function(wrap_pyfunction!(format_number_fn, m)?)?;
    m.add_function(wrap_pyfunction!(function_names, m)?)?;
    m.add_function(wrap_pyfunction!(parse_ref, m)?)?;
    m.add_function(wrap_pyfunction!(ref_spans, m)?)?;
    m.add("MAX_ROWS", MAX_ROWS)?;
    m.add("MAX_COLS", MAX_COLS)?;
    m.add("ENGINE", "folio-sheets-engine-2")?;
    Ok(())
}
