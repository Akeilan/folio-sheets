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
    p.pane_top = get(d, "pane_top").unwrap_or(0);
    p.pane_left = get(d, "pane_left").unwrap_or(0);
    p.split = get(d, "split").unwrap_or(false);
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
    p.fill_target = get::<R4>(d, "fill_target").map(rect);
    p.sel_objects = get(d, "sel_objects").unwrap_or_default();
    p.obj_preview = get(d, "obj_preview");
    p.hover_note = get(d, "hover_note");
    p.edit_note = get(d, "edit_note");
    p.print = get(d, "print").unwrap_or(false);
    p.page_breaks = get(d, "page_breaks").unwrap_or(false);
    p.dv_circles = get(d, "dv_circles").unwrap_or_default();
    p
}

fn dv_dict<'py>(py: Python<'py>, x: &crate::validation::DvRule) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new_bound(py);
    d.set_item("kind", &x.kind)?;
    d.set_item("op", &x.op)?;
    d.set_item("allow_blank", x.allow_blank)?;
    d.set_item("dropdown", x.dropdown)?;
    d.set_item("show_input", x.show_input)?;
    d.set_item("show_error", x.show_error)?;
    d.set_item("style", &x.style)?;
    d.set_item("error_title", &x.error_title)?;
    d.set_item("error", &x.error)?;
    d.set_item("prompt_title", &x.prompt_title)?;
    d.set_item("prompt", &x.prompt)?;
    d.set_item("f1", &x.f1)?;
    d.set_item("f2", &x.f2)?;
    d.set_item("ranges", x.ranges.iter().map(|r| (r.r1, r.c1, r.r2, r.c2)).collect::<Vec<_>>())?;
    Ok(d)
}

fn cairo_ptr(cr: &Bound<'_, PyAny>) -> PyResult<*mut crate::ffi::cairo_t> {
    let tn = cr.get_type().name()?.to_string();
    if tn != "Context" {
        return Err(PyValueError::new_err("needs a cairo.Context"));
    }
    let ctx = unsafe { *((cr.as_ptr() as *const u8).add(std::mem::size_of::<pyo3::ffi::PyObject>()) as *const *mut crate::ffi::cairo_t) };
    if ctx.is_null() {
        return Err(PyValueError::new_err("no cairo context"));
    }
    Ok(ctx)
}

fn spec_dict<'py>(py: Python<'py>, sp: &crate::book_chart::ChartSpec) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new_bound(py);
    d.set_item("kind", &sp.kind)?;
    d.set_item("title_mode", &sp.title_mode)?;
    d.set_item("title", &sp.title)?;
    d.set_item("legend", &sp.legend)?;
    d.set_item("x_title", &sp.x_title)?;
    d.set_item("y_title", &sp.y_title)?;
    d.set_item("gridlines", sp.gridlines)?;
    d.set_item("labels", sp.labels)?;
    d.set_item("palette", sp.palette)?;
    let mut list = Vec::new();
    for s in &sp.series {
        let e = PyDict::new_bound(py);
        e.set_item("name", &s.name)?;
        e.set_item("cat", &s.cat)?;
        e.set_item("val", &s.val)?;
        e.set_item("kind", &s.kind)?;
        e.set_item("secondary", s.secondary)?;
        e.set_item("color", s.color)?;
        list.push(e);
    }
    d.set_item("series", list)?;
    Ok(d)
}

fn pivot_dict<'py>(py: Python<'py>, p: &crate::pivot::Pivot) -> PyResult<Bound<'py, PyDict>> {
    use crate::pivot::{Layout, Subtotals};
    let d = PyDict::new_bound(py);
    d.set_item("name", &p.name)?;
    d.set_item("source_sheet", &p.source_sheet)?;
    d.set_item("source", tup(&p.source))?;
    d.set_item("source_table", p.source_table.clone())?;
    d.set_item("sheet", p.sheet)?;
    d.set_item("at", p.at)?;
    let fields = pyo3::types::PyList::empty_bound(py);
    for f in &p.fields {
        let fd = PyDict::new_bound(py);
        fd.set_item("name", &f.name)?;
        let mut h: Vec<String> = f.hidden.iter().cloned().collect();
        h.sort();
        fd.set_item("hidden", h)?;
        fd.set_item("sort", f.sort)?;
        fd.set_item("subtotal", f.subtotal)?;
        fields.append(fd)?;
    }
    d.set_item("fields", fields)?;
    d.set_item("rows", p.rows.clone())?;
    d.set_item("cols", p.cols.clone())?;
    d.set_item("pages", p.pages.clone())?;
    let vals = pyo3::types::PyList::empty_bound(py);
    for v in &p.values {
        let vd = PyDict::new_bound(py);
        vd.set_item("field", v.field)?;
        vd.set_item("agg", v.agg.code())?;
        vd.set_item("name", &v.name)?;
        vd.set_item("show", v.show.code())?;
        vd.set_item("numfmt", v.numfmt.clone())?;
        vals.append(vd)?;
    }
    d.set_item("values", vals)?;
    d.set_item("values_in_rows", p.values_in_rows)?;
    d.set_item("layout", match p.layout { Layout::Compact => "compact", Layout::Outline => "outline", Layout::Tabular => "tabular" })?;
    d.set_item("subtotals", match p.subtotals { Subtotals::Top => "top", Subtotals::Bottom => "bottom", Subtotals::None => "none" })?;
    for (k, v) in [("grand_rows", p.grand_rows), ("grand_cols", p.grand_cols), ("repeat_labels", p.repeat_labels), ("blank_rows", p.blank_rows),
                   ("row_headers", p.row_headers), ("col_headers", p.col_headers), ("banded_rows", p.banded_rows), ("banded_cols", p.banded_cols),
                   ("field_headers", p.field_headers), ("refreshable", p.refreshable)] {
        d.set_item(k, v)?;
    }
    d.set_item("style", &p.style)?;
    d.set_item("out", p.out.map(|o| tup(&o)))?;
    d.set_item("table", p.table_rect().map(|o| tup(&o)))?;
    Ok(d)
}

/// The settings in `d` (any subset of pivot_dict's keys) onto a pivot.
fn dict_into_pivot(d: &Bound<'_, PyDict>, p: &mut crate::pivot::Pivot) -> PyResult<()> {
    use crate::pivot::{Agg, DataField, Layout, ShowAs, Subtotals};
    if let Some(x) = get::<String>(d, "name") { p.name = x; }
    if let Some(fl) = d.get_item("fields")? {
        let fl = fl.downcast::<pyo3::types::PyList>()?;
        for (k, f) in fl.iter().enumerate() {
            if k >= p.fields.len() { break; }
            let f = f.downcast::<PyDict>()?;
            if let Some(h) = get::<Vec<String>>(f, "hidden") { p.fields[k].hidden = h.into_iter().collect(); }
            if let Some(x) = get::<i8>(f, "sort") { p.fields[k].sort = x; }
            if let Some(x) = get::<bool>(f, "subtotal") { p.fields[k].subtotal = x; }
        }
    }
    let n = p.fields.len();
    if let Some(x) = get::<Vec<usize>>(d, "rows") { p.rows = x.into_iter().filter(|i| *i < n).collect(); }
    if let Some(x) = get::<Vec<usize>>(d, "cols") { p.cols = x.into_iter().filter(|i| *i < n).collect(); }
    if let Some(x) = get::<Vec<(usize, Option<String>)>>(d, "pages") { p.pages = x.into_iter().filter(|i| i.0 < n).collect(); }
    if let Some(vl) = d.get_item("values")? {
        let vl = vl.downcast::<pyo3::types::PyList>()?;
        let mut vals = Vec::new();
        for v in vl.iter() {
            let v = v.downcast::<PyDict>()?;
            let field: usize = get(v, "field").unwrap_or(0);
            if field >= n { continue; }
            let agg = Agg::from_code(&get::<String>(v, "agg").unwrap_or_else(|| "sum".into()));
            let name = get::<String>(v, "name").unwrap_or_else(|| format!("{} of {}", agg.name(), p.fields[field].name));
            vals.push(DataField { field, agg, name, show: ShowAs::from_code(&get::<String>(v, "show").unwrap_or_default()), numfmt: get(v, "numfmt") });
        }
        p.values = vals;
    }
    // a field is in one area at a time (values aside)
    let rows = p.rows.clone();
    p.cols.retain(|i| !rows.contains(i));
    let cols = p.cols.clone();
    p.pages.retain(|(i, _)| !rows.contains(i) && !cols.contains(i));
    if let Some(x) = get::<bool>(d, "values_in_rows") { p.values_in_rows = x; }
    if let Some(x) = get::<String>(d, "layout") {
        p.layout = match x.as_str() { "outline" => Layout::Outline, "tabular" => Layout::Tabular, _ => Layout::Compact };
    }
    if let Some(x) = get::<String>(d, "subtotals") {
        p.subtotals = match x.as_str() { "bottom" => Subtotals::Bottom, "none" => Subtotals::None, _ => Subtotals::Top };
    }
    if let Some(x) = get(d, "grand_rows") { p.grand_rows = x; }
    if let Some(x) = get(d, "grand_cols") { p.grand_cols = x; }
    if let Some(x) = get(d, "repeat_labels") { p.repeat_labels = x; }
    if let Some(x) = get(d, "blank_rows") { p.blank_rows = x; }
    if let Some(x) = get(d, "row_headers") { p.row_headers = x; }
    if let Some(x) = get(d, "col_headers") { p.col_headers = x; }
    if let Some(x) = get(d, "banded_rows") { p.banded_rows = x; }
    if let Some(x) = get(d, "banded_cols") { p.banded_cols = x; }
    if let Some(x) = get(d, "field_headers") { p.field_headers = x; }
    if let Some(x) = get::<String>(d, "style") { if crate::pivot::pivot_style(&x).is_some() || x.is_empty() { p.style = x; } }
    if let Some(x) = get::<R4>(d, "source") { p.source = Rect::new(x.0, x.1, x.2, x.3); p.source_table = None; }
    if let Some(x) = get::<String>(d, "source_sheet") { p.source_sheet = x; }
    Ok(())
}

fn solver_dict<'py>(py: Python<'py>, m: &crate::solver::Model) -> PyResult<Bound<'py, PyDict>> {
    use crate::solver::{Goal, Method};
    let d = PyDict::new_bound(py);
    d.set_item("objective", m.objective)?;
    let (g, v) = match m.goal {
        Goal::Max => ("max", 0.0),
        Goal::Min => ("min", 0.0),
        Goal::Value(v) => ("value", v),
    };
    d.set_item("goal", g)?;
    d.set_item("value", v)?;
    d.set_item("vars", m.vars.iter().map(tup).collect::<Vec<_>>())?;
    d.set_item("constraints", m.constraints.iter().map(|c| (tup(&c.lhs), c.rel.text().to_string(), c.rhs.clone())).collect::<Vec<_>>())?;
    d.set_item("nonneg", m.nonneg)?;
    d.set_item("method", match m.method { Method::Grg => "grg", Method::Simplex => "simplex", Method::Evolutionary => "evolutionary" })?;
    d.set_item("max_iter", m.max_iter)?;
    d.set_item("precision", m.precision)?;
    d.set_item("int_gap", m.int_gap)?;
    Ok(d)
}

fn dict_solver(d: &Bound<'_, PyDict>) -> PyResult<crate::solver::Model> {
    use crate::solver::{Constraint, Goal, Method, Model, Rel};
    let mut m = Model::default();
    m.objective = get(d, "objective");
    let v: f64 = get(d, "value").unwrap_or(0.0);
    m.goal = match get::<String>(d, "goal").as_deref() {
        Some("min") => Goal::Min,
        Some("value") => Goal::Value(v),
        _ => Goal::Max,
    };
    let vars: Vec<R4> = get(d, "vars").unwrap_or_default();
    m.vars = rects(vars);
    let cons: Vec<(R4, String, String)> = get(d, "constraints").unwrap_or_default();
    m.constraints = cons.into_iter().map(|(r, rel, rhs)| Constraint { lhs: rects(vec![r]).remove(0), rel: Rel::parse(&rel), rhs }).collect();
    m.nonneg = get(d, "nonneg").unwrap_or(true);
    m.method = match get::<String>(d, "method").as_deref() {
        Some("simplex") => Method::Simplex,
        Some("evolutionary") => Method::Evolutionary,
        _ => Method::Grg,
    };
    if let Some(x) = get::<u32>(d, "max_iter") { m.max_iter = x; }
    if let Some(x) = get::<f64>(d, "precision") { m.precision = x; }
    if let Some(x) = get::<f64>(d, "int_gap") { m.int_gap = x; }
    Ok(m)
}

fn dict_spec(d: &Bound<'_, PyDict>) -> PyResult<crate::book_chart::ChartSpec> {
    let mut sp = crate::book_chart::ChartSpec {
        kind: get(d, "kind").unwrap_or_else(|| "col".into()),
        title_mode: get(d, "title_mode").unwrap_or_else(|| "auto".into()),
        title: get(d, "title").unwrap_or_default(),
        legend: get(d, "legend").unwrap_or_else(|| "b".into()),
        x_title: get(d, "x_title").unwrap_or_default(),
        y_title: get(d, "y_title").unwrap_or_default(),
        gridlines: get(d, "gridlines").unwrap_or(true),
        labels: get(d, "labels").unwrap_or(false),
        palette: get(d, "palette").unwrap_or(0),
        series: vec![],
    };
    if let Some(list) = d.get_item("series")?.filter(|v| !v.is_none()) {
        for item in list.iter()? {
            let item = item?;
            let e = item.downcast::<PyDict>()?;
            sp.series.push(crate::book_chart::SeriesSpec {
                name: get(e, "name").unwrap_or_default(),
                cat: get(e, "cat").unwrap_or_default(),
                val: get(e, "val").unwrap_or_default(),
                kind: get(e, "kind").unwrap_or_else(|| "col".into()),
                secondary: get(e, "secondary").unwrap_or(false),
                color: get(e, "color"),
            });
        }
    }
    Ok(sp)
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
                "validation" => What::Validation,
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
    /// Changes with every step taken (and with undo and redo).
    #[getter]
    fn history_state(&self) -> u64 {
        self.b.state as u64
    }
    /// After a step was taken back because it wasn't allowed: it can't be redone.
    fn forget_redo(&mut self) {
        self.b.redo_stack.clear();
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
        d.set_item("tab_color", sh.tab_color.map(|c| self.b.styles.resolve(c)))?;
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
    /// The filter a header cell belongs to (at), or the sheet's AutoFilter.
    #[pyo3(signature = (s, at=None))]
    fn autofilter<'py>(&self, py: Python<'py>, s: usize, at: Option<(u32, u32)>) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let o = match self.b.filter_owner(s, at) {
            Some(o) => o,
            None => return Ok(None),
        };
        let af = match self.b.owner_filter(s, o) {
            Some(a) => a,
            None => return Ok(None),
        };
        let af = &af;
        let d = PyDict::new_bound(py);
        d.set_item("ref", tup(&af.rect))?;
        d.set_item("table", match o { crate::book_data::FOwner::Table(i) => Some(i), _ => None })?;
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
    #[pyo3(signature = (s, col, spec=None, at=None))]
    fn filter_column(&mut self, s: usize, col: u32, spec: Option<&Bound<'_, PyDict>>, at: Option<(u32, u32)>) -> PyResult<()> {
        self.sheet(s)?;
        let o = self.b.filter_owner(s, at).ok_or_else(|| PyValueError::new_err("There's no filter on this sheet."))?;
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
        self.b.filter_column_at(s, o, col, kind).map_err(verr)
    }
    #[pyo3(signature = (s, at=None))]
    fn clear_filters(&mut self, s: usize, at: Option<(u32, u32)>) -> PyResult<()> {
        self.sheet(s)?;
        let o = self.b.filter_owner(s, at).ok_or_else(|| PyValueError::new_err("There's no filter on this sheet."))?;
        self.b.clear_filters_at(s, o).map_err(verr)
    }
    fn reapply_filter(&mut self, s: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.reapply_filter(s).map_err(verr)
    }
    #[pyo3(signature = (s, col, at=None))]
    fn filter_values(&self, s: usize, col: u32, at: Option<(u32, u32)>) -> PyResult<(Vec<String>, bool)> {
        self.sheet(s)?;
        Ok(match self.b.filter_owner(s, at) {
            Some(o) => self.b.filter_values_at(s, o, col),
            None => (vec![], false),
        })
    }

    // ---- page layout and printing
    fn page_setup<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let p = self.b.page_setup(s);
        let d = PyDict::new_bound(py);
        d.set_item("paper", p.paper)?;
        d.set_item("landscape", p.landscape)?;
        d.set_item("scale", p.scale)?;
        d.set_item("fit", p.fit)?;
        d.set_item("margins", p.margins.to_vec())?;
        d.set_item("center_h", p.center_h)?;
        d.set_item("center_v", p.center_v)?;
        d.set_item("gridlines", p.gridlines)?;
        d.set_item("headings", p.headings)?;
        d.set_item("first_page", p.first_page)?;
        d.set_item("over_then_down", p.over_then_down)?;
        d.set_item("header", &p.header)?;
        d.set_item("footer", &p.footer)?;
        d.set_item("row_breaks", p.row_breaks.clone())?;
        d.set_item("col_breaks", p.col_breaks.clone())?;
        d.set_item("print_area", p.print_area.iter().map(tup).collect::<Vec<_>>())?;
        d.set_item("title_rows", p.title_rows)?;
        d.set_item("title_cols", p.title_cols)?;
        d.set_item("paper_size", p.paper_size())?;
        d.set_item("print_scale", self.b.print_scale(s, &p))?;
        Ok(d)
    }
    #[pyo3(signature = (s, spec, label="Page Setup"))]
    fn set_page_setup(&mut self, s: usize, spec: &Bound<'_, PyDict>, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        let mut p = self.b.page_setup(s);
        if let Some(v) = get(spec, "paper") { p.paper = v }
        if let Some(v) = get(spec, "landscape") { p.landscape = v }
        if let Some(v) = get(spec, "scale") { p.scale = v }
        if has(spec, "fit") { p.fit = get(spec, "fit") }
        if let Some(v) = get::<Vec<f64>>(spec, "margins") {
            if v.len() == 6 {
                p.margins.copy_from_slice(&v);
            }
        }
        if let Some(v) = get(spec, "center_h") { p.center_h = v }
        if let Some(v) = get(spec, "center_v") { p.center_v = v }
        if let Some(v) = get(spec, "gridlines") { p.gridlines = v }
        if let Some(v) = get(spec, "headings") { p.headings = v }
        if has(spec, "first_page") { p.first_page = get(spec, "first_page") }
        if let Some(v) = get(spec, "over_then_down") { p.over_then_down = v }
        if let Some(v) = get(spec, "header") { p.header = v }
        if let Some(v) = get(spec, "footer") { p.footer = v }
        if let Some(v) = get(spec, "row_breaks") { p.row_breaks = v }
        if let Some(v) = get(spec, "col_breaks") { p.col_breaks = v }
        if let Some(v) = get::<Vec<R4>>(spec, "print_area") { p.print_area = rects(v) }
        if has(spec, "title_rows") { p.title_rows = get(spec, "title_rows") }
        if has(spec, "title_cols") { p.title_cols = get(spec, "title_cols") }
        self.b.set_page_setup(s, &p, label).map_err(verr)
    }
    fn pages(&self, s: usize) -> PyResult<Vec<((u32, u32), (u32, u32), u32)>> {
        self.sheet(s)?;
        Ok(self.b.pages(s).into_iter().map(|p| (p.rows, p.cols, p.number)).collect())
    }
    /// Draw page k into a cairo context in points.
    #[pyo3(signature = (s, k, cr, file_name=""))]
    fn render_page(&self, s: usize, k: usize, cr: &Bound<'_, PyAny>, file_name: &str) -> PyResult<()> {
        self.sheet(s)?;
        let ctx = cairo_ptr(cr)?;
        unsafe { self.b.render_page(s, k, ctx, file_name) }.map_err(verr)
    }

    // ---- notes and comments
    fn notes<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let mut out = Vec::new();
        for ((r, c), n) in &self.b.sheets[s].notes {
            let d = PyDict::new_bound(py);
            d.set_item("cell", (*r, *c))?;
            d.set_item("author", &n.author)?;
            d.set_item("text", &n.text)?;
            d.set_item("visible", n.visible)?;
            out.push(d);
        }
        Ok(out)
    }
    fn note<'py>(&self, py: Python<'py>, s: usize, r: u32, c: u32) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        match self.b.note(s, r, c) {
            Some(n) => {
                let d = PyDict::new_bound(py);
                d.set_item("author", &n.author)?;
                d.set_item("text", &n.text)?;
                d.set_item("visible", n.visible)?;
                d.set_item("size", (n.w, n.h))?;
                Ok(Some(d))
            }
            None => Ok(None),
        }
    }
    #[pyo3(signature = (s, r, c, author, text=None))]
    fn set_note(&mut self, s: usize, r: u32, c: u32, author: &str, text: Option<&str>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_note(s, r, c, author, text).map_err(verr)
    }
    #[pyo3(signature = (s, cells=None, on=None))]
    fn show_notes(&mut self, s: usize, cells: Option<Vec<(u32, u32)>>, on: Option<bool>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.show_notes(s, cells.as_deref(), on).map_err(verr)
    }
    fn set_note_size(&mut self, s: usize, r: u32, c: u32, w: f64, h: f64) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_note_size(s, r, c, w, h).map_err(verr)
    }
    fn note_box(&self, s: usize, r: u32, c: u32, p: &Bound<'_, PyDict>) -> PyResult<(f64, f64, f64, f64)> {
        self.sheet(s)?;
        let vp = view_params(p);
        let g = self.b.geometry(s, &vp);
        Ok(self.b.note_box(s, &vp, &g, r, c))
    }
    fn thread<'py>(&self, py: Python<'py>, s: usize, r: u32, c: u32) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        match self.b.thread(s, r, c) {
            Some(t) => {
                let d = PyDict::new_bound(py);
                d.set_item("resolved", t.resolved)?;
                d.set_item("entries", t.entries.iter().map(|e| (e.author.clone(), e.date.clone(), e.text.clone())).collect::<Vec<_>>())?;
                Ok(Some(d))
            }
            None => Ok(None),
        }
    }
    fn threads(&self, s: usize) -> PyResult<Vec<(u32, u32)>> {
        self.sheet(s)?;
        Ok(self.b.sheets[s].threads.keys().copied().collect())
    }
    fn add_comment(&mut self, s: usize, r: u32, c: u32, author: &str, text: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.add_comment(s, r, c, author, text).map_err(verr)
    }
    #[pyo3(signature = (s, r, c, k, text=None))]
    fn edit_comment(&mut self, s: usize, r: u32, c: u32, k: usize, text: Option<&str>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.edit_comment(s, r, c, k, text).map_err(verr)
    }
    fn resolve_comment(&mut self, s: usize, r: u32, c: u32, resolved: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.resolve_comment(s, r, c, resolved).map_err(verr)
    }
    fn clear_comments(&mut self, s: usize, ranges: Vec<R4>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.clear_comments(s, &rects(ranges)).map_err(verr)
    }
    fn notes_to_comments(&mut self, s: usize) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.notes_to_comments(s).map_err(verr)
    }
    fn next_comment(&self, s: usize, r: u32, c: u32, back: bool, notes: bool) -> PyResult<Option<(u32, u32)>> {
        self.sheet(s)?;
        Ok(self.b.next_comment(s, r, c, back, notes))
    }

    // ---- tables
    fn tables<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let mut out = Vec::new();
        for t in &self.b.sheets[s].tables {
            let d = PyDict::new_bound(py);
            d.set_item("name", &t.name)?;
            d.set_item("ref", tup(&t.rect))?;
            d.set_item("data", tup(&t.data()))?;
            d.set_item("header", t.header)?;
            d.set_item("totals", t.totals)?;
            d.set_item("style", t.style.clone())?;
            d.set_item("first_col", t.first_col)?;
            d.set_item("last_col", t.last_col)?;
            d.set_item("row_stripes", t.row_stripes)?;
            d.set_item("col_stripes", t.col_stripes)?;
            d.set_item("filter_button", t.filter_button)?;
            d.set_item("columns", t.cols.iter().map(|c| c.name.clone()).collect::<Vec<_>>())?;
            d.set_item("totals_fn", t.cols.iter().map(|c| c.totals_fn.clone()).collect::<Vec<_>>())?;
            out.push(d);
        }
        Ok(out)
    }
    fn table_at(&self, s: usize, r: u32, c: u32) -> PyResult<Option<usize>> {
        self.sheet(s)?;
        Ok(self.b.table_at(s, r, c))
    }
    fn create_table(&mut self, s: usize, rg: R4, has_headers: bool, style: &str) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.create_table(s, rect(rg), has_headers, style).map_err(verr)
    }
    fn convert_table(&mut self, s: usize, i: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.convert_table(s, i).map_err(verr)
    }
    fn set_table_option(&mut self, s: usize, i: usize, what: &str, on: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_table_option(s, i, what, on).map_err(verr)
    }
    #[pyo3(signature = (s, i, style=None))]
    fn set_table_style(&mut self, s: usize, i: usize, style: Option<&str>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_table_style(s, i, style).map_err(verr)
    }
    fn set_table_total(&mut self, s: usize, i: usize, col: usize, func: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_table_total(s, i, col, func).map_err(verr)
    }
    fn rename_table(&mut self, s: usize, i: usize, name: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.rename_table(s, i, name).map_err(verr)
    }
    fn resize_table(&mut self, s: usize, i: usize, rg: R4) -> PyResult<()> {
        self.sheet(s)?;
        self.b.resize_table(s, i, rect(rg)).map_err(verr)
    }
    /// A thumbnail of a table style (the gallery).
    #[pyo3(signature = (cr, style, w, h, banded_rows=false, banded_cols=false))]
    fn render_pivot_style(&self, cr: &Bound<'_, PyAny>, style: &str, w: f64, h: f64, banded_rows: bool, banded_cols: bool) -> PyResult<()> {
        let ctx = cairo_ptr(cr)?;
        unsafe { self.b.paint_pivot_style(ctx, style, w, h, banded_rows, banded_cols) };
        Ok(())
    }
    fn render_table_style(&self, cr: &Bound<'_, PyAny>, style: &str, w: f64, h: f64) -> PyResult<()> {
        let ctx = cairo_ptr(cr)?;
        unsafe { self.b.paint_table_style(ctx, style, w, h) };
        Ok(())
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

    // ---- fill
    #[pyo3(signature = (s, src, target, series=true, formats_only=false, values_only=false))]
    fn autofill(&mut self, s: usize, src: R4, target: R4, series: bool, formats_only: bool, values_only: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.autofill(s, rect(src), rect(target), series, formats_only, values_only).map_err(verr)
    }
    #[pyo3(signature = (s, rg, by_rows, kind, unit="day", step=1.0, stop=None, trend=false))]
    #[allow(clippy::too_many_arguments)]
    fn fill_series(&mut self, s: usize, rg: R4, by_rows: bool, kind: &str, unit: &str, step: f64, stop: Option<f64>, trend: bool) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.fill_series(s, rect(rg), by_rows, kind, unit, step, stop, trend).map_err(verr)
    }
    fn flash_fill(&mut self, s: usize, row: u32, col: u32) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.flash_fill(s, row, col).map_err(verr)
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
    #[pyo3(signature = (name, hidden, scope=None))]
    fn set_name_hidden(&mut self, name: &str, hidden: bool, scope: Option<usize>) -> PyResult<()> {
        self.b.set_name_hidden(name, scope, hidden).map_err(verr)
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
    fn row_hidden(&self, s: usize, r: u32) -> PyResult<bool> {
        self.sheet(s)?;
        Ok(self.b.sheets[s].row_hidden(r))
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
    // ---- data validation
    fn dv_at<'py>(&self, py: Python<'py>, s: usize, r: u32, c: u32) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        match self.b.dv_at(s, r, c) {
            Some(x) => Ok(Some(dv_dict(py, &x)?)),
            None => Ok(None),
        }
    }
    /// The rule a typed entry breaks (None when it passes).
    fn dv_check<'py>(&self, py: Python<'py>, s: usize, r: u32, c: u32, text: &str) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        match self.b.dv_check(s, r, c, text) {
            Some(x) => Ok(Some(dv_dict(py, &x)?)),
            None => Ok(None),
        }
    }
    fn dv_list(&self, s: usize, r: u32, c: u32) -> PyResult<Vec<String>> {
        self.sheet(s)?;
        Ok(self.b.dv_list(s, r, c))
    }
    fn dv_dropdown_at(&self, s: usize, r: u32, c: u32) -> PyResult<bool> {
        self.sheet(s)?;
        Ok(self.b.dv_dropdown_at(s, r, c))
    }
    fn dv_set(&mut self, s: usize, ranges: Vec<R4>, spec: &Bound<'_, PyDict>) -> PyResult<()> {
        self.sheet(s)?;
        let sp = crate::validation::DvSpec {
            kind: get(spec, "kind").unwrap_or_else(|| "none".into()),
            op: get(spec, "op").unwrap_or_else(|| "between".into()),
            f1: get(spec, "f1").unwrap_or_default(),
            f2: get(spec, "f2").unwrap_or_default(),
            allow_blank: get(spec, "allow_blank").unwrap_or(true),
            dropdown: get(spec, "dropdown").unwrap_or(true),
            show_input: get(spec, "show_input").unwrap_or(true),
            prompt_title: get(spec, "prompt_title").unwrap_or_default(),
            prompt: get(spec, "prompt").unwrap_or_default(),
            show_error: get(spec, "show_error").unwrap_or(true),
            style: get(spec, "style").unwrap_or_else(|| "stop".into()),
            error_title: get(spec, "error_title").unwrap_or_default(),
            error: get(spec, "error").unwrap_or_default(),
        };
        self.b.dv_set(s, &rects(ranges), &sp).map_err(PyValueError::new_err)
    }
    fn dv_clear(&mut self, s: usize, ranges: Vec<R4>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.dv_clear(s, &rects(ranges)).map_err(PyValueError::new_err)
    }
    /// (the active cell's rule or None, "" / "erase" / "extend", ranges with the same settings)
    fn dv_for_selection<'py>(&self, py: Python<'py>, s: usize, active: (u32, u32), ranges: Vec<R4>) -> PyResult<(Option<Bound<'py, PyDict>>, String, Vec<R4>)> {
        self.sheet(s)?;
        let (rule, mixed, same) = self.b.dv_for_selection(s, active, &rects(ranges));
        let d = match rule {
            Some(x) => Some(dv_dict(py, &x)?),
            None => None,
        };
        Ok((d, mixed.to_string(), same.iter().map(|r| (r.r1, r.c1, r.r2, r.c2)).collect()))
    }
    /// The link on a cell: {target, location, display, tooltip, range} or None.
    fn link_at<'py>(&self, py: Python<'py>, s: usize, r: u32, c: u32) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        match self.b.link_at(s, r, c) {
            Some(l) => {
                let d = PyDict::new_bound(py);
                d.set_item("target", &l.target)?;
                d.set_item("location", &l.location)?;
                d.set_item("display", &l.display)?;
                d.set_item("tooltip", &l.tooltip)?;
                d.set_item("range", (l.rect.r1, l.rect.c1, l.rect.r2, l.rect.c2))?;
                Ok(Some(d))
            }
            None => Ok(None),
        }
    }
    // ---- Group and Outline (rows: True for rows, False for columns)
    fn outline_lines(&mut self, s: usize, rows: bool, first: u32, last: u32, delta: i32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.outline_lines(s, rows, first, last, delta).map_err(PyValueError::new_err)
    }
    fn set_detail(&mut self, s: usize, rows: bool, i: u32, show: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_detail(s, rows, i, show).map_err(PyValueError::new_err)
    }
    fn set_group_detail(&mut self, s: usize, rows: bool, a: u32, b: u32, level: u8, show: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_group_detail(s, rows, a, b, level, show);
        Ok(())
    }
    fn show_levels(&mut self, s: usize, rows: bool, n: u8) -> PyResult<()> {
        self.sheet(s)?;
        self.b.show_levels(s, rows, n);
        Ok(())
    }
    fn clear_outline(&mut self, s: usize, rows: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.clear_outline(s, rows);
        Ok(())
    }
    fn outline_level(&self, s: usize, rows: bool, i: u32) -> PyResult<u8> {
        self.sheet(s)?;
        Ok(self.b.line(s, rows, i).level)
    }
    fn max_outline_level(&self, s: usize, rows: bool) -> PyResult<u8> {
        self.sheet(s)?;
        Ok(self.b.max_level(s, rows))
    }
    /// Every group: (first, last, level, collapsed, summary line or None).
    fn outline_groups(&self, s: usize, rows: bool) -> PyResult<Vec<(u32, u32, u8, bool, Option<u32>)>> {
        self.sheet(s)?;
        Ok(self.b.groups(s, rows).into_iter().map(|(a, b, lv)| {
            let x = self.b.summary_of(s, rows, (a, b));
            // closed when every line of it is hidden (a shared summary line's flag belongs to the outer group)
            let collapsed = (a..=b).all(|k| self.b.line(s, rows, k).hidden);
            (a, b, lv, collapsed, x)
        }).collect())
    }
    fn summary_after(&self, s: usize, rows: bool) -> PyResult<bool> {
        self.sheet(s)?;
        Ok(if rows { self.b.summary_below(s) } else { self.b.summary_right(s) })
    }
    fn sheet_protection(&self, s: usize) -> PyResult<Option<Vec<(String, String)>>> {
        self.sheet(s)?;
        Ok(self.b.sheet_protection(s))
    }
    fn set_sheet_protection(&mut self, s: usize, attrs: Option<Vec<(String, String)>>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_sheet_protection(s, attrs);
        Ok(())
    }
    fn workbook_protection(&self) -> Option<Vec<(String, String)>> {
        self.b.workbook_protection()
    }
    fn set_workbook_protection(&mut self, attrs: Option<Vec<(String, String)>>) {
        self.b.set_workbook_protection(attrs)
    }
    fn locked_in(&self, s: usize, ranges: Vec<R4>) -> PyResult<bool> {
        self.sheet(s)?;
        Ok(self.b.locked_in(s, &rects(ranges)))
    }
    fn formula_link(&self, s: usize, r: u32, c: u32) -> PyResult<Option<String>> {
        self.sheet(s)?;
        Ok(self.b.formula_link(s, r, c))
    }
    fn link_count(&self, s: usize) -> PyResult<usize> {
        self.sheet(s)?;
        Ok(self.b.links(s).len())
    }
    #[pyo3(signature = (s, rg, target, location="", display="", tooltip=""))]
    fn set_link(&mut self, s: usize, rg: R4, target: &str, location: &str, display: &str, tooltip: &str) -> PyResult<()> {
        self.sheet(s)?;
        let l = crate::links::Link { rect: rect(rg), target: target.to_string(), location: location.to_string(), display: display.to_string(), tooltip: tooltip.to_string() };
        self.b.set_link(s, rect(rg), &l).map_err(PyValueError::new_err)
    }
    fn clear_links(&mut self, s: usize, ranges: Vec<R4>) -> PyResult<usize> {
        self.sheet(s)?;
        Ok(self.b.clear_links(s, &rects(ranges)))
    }
    fn dv_invalid(&self, s: usize, limit: usize) -> PyResult<Vec<(u32, u32)>> {
        self.sheet(s)?;
        Ok(self.b.dv_invalid(s, limit))
    }
    fn copy_validation(&mut self, s: usize, src: R4, dst_s: usize, dst: R4) -> PyResult<()> {
        self.sheet(s)?;
        self.sheet(dst_s)?;
        let label = "Paste Validation";
        self.b.begin(label, dst_s);
        self.b.copy_validation(s, rect(src), dst_s, rect(dst));
        self.b.commit();
        Ok(())
    }
    // ---- conditional formatting
    fn add_cf_rule(&mut self, s: usize, ranges: Vec<R4>, spec: &Bound<'_, PyDict>) -> PyResult<()> {
        self.sheet(s)?;
        let mut sp = crate::cf::RuleSpec {
            typ: get(spec, "type").unwrap_or_default(),
            operator: get(spec, "operator").unwrap_or_default(),
            values: get(spec, "values").unwrap_or_default(),
            text: get(spec, "text").unwrap_or_default(),
            time_period: get(spec, "time_period").unwrap_or_default(),
            rank: get(spec, "rank").unwrap_or(10),
            percent: get(spec, "percent").unwrap_or(false),
            bottom: get(spec, "bottom").unwrap_or(false),
            above: get(spec, "above").unwrap_or(true),
            equal: get(spec, "equal").unwrap_or(false),
            std_dev: get(spec, "std_dev").unwrap_or(0),
            points: vec![],
            bar_color: get(spec, "bar_color").unwrap_or(0x638EC6),
            gradient: get(spec, "gradient").unwrap_or(true),
            icon_set: get(spec, "icon_set").unwrap_or_else(|| "3TrafficLights1".into()),
            reverse: get(spec, "reverse").unwrap_or(false),
            show_value: get(spec, "show_value").unwrap_or(true),
            stop: get(spec, "stop").unwrap_or(false),
            font_color: get(spec, "font_color"),
            fill: get(spec, "fill"),
            bold: get(spec, "bold").unwrap_or(false),
            italic: get(spec, "italic").unwrap_or(false),
            underline: get(spec, "underline").unwrap_or(false),
            strike: get(spec, "strike").unwrap_or(false),
            border: get(spec, "border"),
            numfmt: get(spec, "numfmt"),
        };
        if let Some(pts) = get::<Vec<(String, String, u32)>>(spec, "points") {
            sp.points = pts;
        }
        self.b.add_cf_rule(s, &rects(ranges), &sp).map_err(verr)
    }
    #[pyo3(signature = (s, ranges=None))]
    fn clear_cf(&mut self, s: usize, ranges: Option<Vec<R4>>) -> PyResult<()> {
        self.sheet(s)?;
        let r = ranges.map(rects);
        self.b.clear_cf(s, r.as_deref()).map_err(verr)
    }
    #[pyo3(signature = (s, rg=None))]
    fn cf_rules<'py>(&self, py: Python<'py>, s: usize, rg: Option<R4>) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let mut out = Vec::new();
        for r in self.b.cf_rules(s, rg.map(rect)) {
            let d = PyDict::new_bound(py);
            d.set_item("piece", r.piece)?;
            d.set_item("index", r.index)?;
            d.set_item("priority", r.priority)?;
            d.set_item("type", &r.typ)?;
            d.set_item("desc", &r.desc)?;
            d.set_item("stop", r.stop)?;
            d.set_item("applies", &r.applies)?;
            d.set_item("fill", r.dxf.fill)?;
            d.set_item("color", r.dxf.color)?;
            d.set_item("bold", r.dxf.bold.unwrap_or(false))?;
            d.set_item("italic", r.dxf.italic.unwrap_or(false))?;
            out.push(d);
        }
        Ok(out)
    }
    fn delete_cf_rule(&mut self, s: usize, piece: usize, index: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.delete_cf_rule(s, piece, index).map_err(verr)
    }
    #[pyo3(signature = (s, piece, index, stop=None, applies=None, step=0))]
    fn edit_cf_rule(&mut self, s: usize, piece: usize, index: usize, stop: Option<bool>, applies: Option<&str>, step: i32) -> PyResult<()> {
        self.sheet(s)?;
        self.b.edit_cf_rule(s, piece, index, stop, applies, step).map_err(verr)
    }
    /// What conditional formatting does to a cell (for tests and the status bar).
    fn cf_at<'py>(&self, py: Python<'py>, s: usize, r: u32, c: u32) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let blocks = self.b.cf_blocks(s);
        let o = match self.b.cf_at(s, r, c, &blocks) {
            Some(o) => o,
            None => return Ok(None),
        };
        let d = PyDict::new_bound(py);
        d.set_item("fill", o.scale.or(o.dxf.fill))?;
        d.set_item("color", o.dxf.color)?;
        d.set_item("bold", o.dxf.bold)?;
        d.set_item("icon", o.icon.clone())?;
        d.set_item("bar", o.bar.as_ref().map(|b| (b.0, b.1)))?;
        Ok(Some(d))
    }

    // ---- charts and objects
    fn objects<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let mut out = Vec::new();
        for o in &self.b.sheets[s].objects {
            let d = PyDict::new_bound(py);
            d.set_item("kind", o.kind())?;
            d.set_item("name", &o.name)?;
            d.set_item("rect", self.b.obj_rect(s, o))?;
            out.push(d);
        }
        Ok(out)
    }
    fn object_hit(&self, s: usize, x: f64, y: f64, p: &Bound<'_, PyDict>) -> PyResult<Option<(usize, String)>> {
        self.sheet(s)?;
        Ok(self.b.object_hit(s, x, y, &view_params(p)).map(|(i, k)| (i, k.to_string())))
    }
    fn object_screen_rect(&self, s: usize, i: usize, p: &Bound<'_, PyDict>) -> PyResult<Option<(f64, f64, f64, f64)>> {
        self.sheet(s)?;
        Ok(self.b.object_screen_rect(s, i, &view_params(p)))
    }
    #[pyo3(signature = (s, i, x, y, w, h, p, label="Move Chart"))]
    fn place_object(&mut self, s: usize, i: usize, x: f64, y: f64, w: f64, h: f64, p: &Bound<'_, PyDict>, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        let vp = view_params(p);
        self.b.place_object(s, i, x, y, w, h, &vp, label).map_err(verr)
    }
    #[pyo3(signature = (s, i, x, y, w, h, label="Move Chart"))]
    fn set_object_rect(&mut self, s: usize, i: usize, x: f64, y: f64, w: f64, h: f64, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_object_rect(s, i, x, y, w, h, label).map_err(verr)
    }
    fn sheet_point(&self, s: usize, x: f64, y: f64, p: &Bound<'_, PyDict>) -> PyResult<(f64, f64)> {
        self.sheet(s)?;
        Ok(self.b.sheet_point(s, x, y, &view_params(p)))
    }
    fn delete_objects(&mut self, s: usize, which: Vec<usize>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.delete_objects(s, &which).map_err(verr)
    }
    #[pyo3(signature = (s, data, x, y, max_w=1e9, max_h=1e9))]
    fn insert_picture(&mut self, s: usize, data: Vec<u8>, x: f64, y: f64, max_w: f64, max_h: f64) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.insert_picture(s, data, x, y, max_w, max_h).map_err(verr)
    }
    fn picture_bytes<'py>(&self, py: Python<'py>, s: usize, i: usize) -> PyResult<Option<(Bound<'py, pyo3::types::PyBytes>, String)>> {
        self.sheet(s)?;
        Ok(self.b.picture_bytes(s, i).map(|(b, e)| (pyo3::types::PyBytes::new_bound(py, b), e.to_string())))
    }
    /// Ink from the Draw tab: strokes [(points, colour, width px, kind 0 pen 1 pencil 2 highlighter)]
    /// in sheet pixels at 100 %. Returns the new object's index.
    fn add_ink(&mut self, s: usize, strokes: Vec<(Vec<(f64, f64)>, u32, f64, u8)>) -> PyResult<usize> {
        self.sheet(s)?;
        let st = strokes.into_iter().map(|(pts, color, width, kind)| crate::ink::Stroke { pts, color, width, kind }).collect();
        self.b.add_ink(s, st).map_err(verr)
    }
    fn ink_strokes(&self, s: usize, i: usize) -> PyResult<Option<Vec<(Vec<(f64, f64)>, u32, f64, u8)>>> {
        self.sheet(s)?;
        Ok(self.b.ink_strokes(s, i).map(|v| v.into_iter().map(|k| (k.pts, k.color, k.width, k.kind)).collect()))
    }
    fn ink_at(&self, s: usize, x: f64, y: f64, radius: f64) -> PyResult<Vec<usize>> {
        self.sheet(s)?;
        Ok(self.b.ink_at(s, x, y, radius))
    }
    // ---- shapes, text boxes, WordArt
    #[pyo3(signature = (s, prst, x, y, w, h, flip_h=false, flip_v=false))]
    fn insert_shape(&mut self, s: usize, prst: &str, x: f64, y: f64, w: f64, h: f64, flip_h: bool, flip_v: bool) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.insert_shape(s, prst, x, y, w, h, flip_h, flip_v).map_err(verr)
    }
    fn insert_text_box(&mut self, s: usize, x: f64, y: f64, w: f64, h: f64, text: &str) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.insert_text_box(s, x, y, w, h, text).map_err(verr)
    }
    fn insert_wordart(&mut self, s: usize, style: usize, text: &str, x: f64, y: f64) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.insert_wordart(s, style, text, x, y).map_err(verr)
    }
    /// What the Shape Format tab shows for a shape, or None for other objects.
    fn shape_info<'py>(&self, py: Python<'py>, s: usize, i: usize) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let v = match self.b.shape_view(s, i) {
            Some(v) => v,
            None => return Ok(None),
        };
        let d = PyDict::new_bound(py);
        d.set_item("geom", &v.geom)?;
        d.set_item("fill", v.fill.map(|f| f.0))?;
        d.set_item("line", v.line.map(|l| (l.0, l.1, l.2)))?;
        d.set_item("text", v.text())?;
        d.set_item("textbox", v.textbox)?;
        d.set_item("connector", v.connector)?;
        d.set_item("equation", v.math.as_ref().map(|(m, _)| crate::math::to_linear(m)))?;
        d.set_item("anchor", v.anchor.to_string())?;
        let r = v.paras.iter().flat_map(|p| p.runs.first()).next();
        d.set_item("bold", r.map(|r| r.bold).unwrap_or(false))?;
        d.set_item("italic", r.map(|r| r.italic).unwrap_or(false))?;
        d.set_item("size", r.map(|r| r.size).unwrap_or(11.0))?;
        d.set_item("color", r.map(|r| r.color))?;
        d.set_item("outline", r.and_then(|r| r.outline).map(|o| o.0))?;
        d.set_item("align", v.paras.first().map(|p| p.align).unwrap_or(0))?;
        Ok(Some(d))
    }
    /// Shape Fill: a colour, or None for No Fill.
    fn shape_set_fill(&mut self, s: usize, which: Vec<usize>, color: Option<u32>) -> PyResult<()> {
        self.b.edit_shape(s, &which, "Shape Fill", |b| crate::shape::set_fill(b, color)).map_err(verr)
    }
    /// Shape Outline: colour ("none" for No Outline), weight in points, dashes.
    #[pyo3(signature = (s, which, color=None, no_line=false, weight=None, dashed=None))]
    fn shape_set_line(&mut self, s: usize, which: Vec<usize>, color: Option<u32>, no_line: bool, weight: Option<f64>, dashed: Option<bool>) -> PyResult<()> {
        let c = if no_line { Some(None) } else { color.map(Some) };
        self.b.edit_shape(s, &which, "Shape Outline", |b| crate::shape::set_line(b, c, weight, dashed)).map_err(verr)
    }
    fn shape_set_text(&mut self, s: usize, i: usize, text: &str) -> PyResult<()> {
        self.b.edit_shape(s, &[i], "Typing", |b| crate::shape::set_text(b, text)).map_err(verr)
    }
    #[pyo3(signature = (s, which, bold=None, italic=None, size=None, color=None, align=None))]
    fn shape_set_text_format(&mut self, s: usize, which: Vec<usize>, bold: Option<bool>, italic: Option<bool>, size: Option<f64>, color: Option<u32>, align: Option<i32>) -> PyResult<()> {
        self.b.edit_shape(s, &which, "Font", |b| crate::shape::set_text_format(b, bold, italic, size, color, align)).map_err(verr)
    }
    fn shape_set_anchor(&mut self, s: usize, which: Vec<usize>, anchor: &str) -> PyResult<()> {
        let a = anchor.chars().next().unwrap_or('t');
        self.b.edit_shape(s, &which, "Align Text", |b| crate::shape::set_text_anchor(b, a)).map_err(verr)
    }
    fn shape_set_geometry(&mut self, s: usize, which: Vec<usize>, prst: &str) -> PyResult<()> {
        self.b.edit_shape(s, &which, "Change Shape", |b| crate::shape::set_geometry(b, prst)).map_err(verr)
    }
    /// Insert › Sparklines: kind "line", "column" or "winloss"; location like "F2:F10".
    fn add_sparklines(&mut self, s: usize, kind: &str, data: &str, location: &str) -> PyResult<usize> {
        self.sheet(s)?;
        let loc = location.rsplit('!').next().unwrap_or(location).replace('$', "");
        let r = crate::cellref::parse_range(loc.trim()).ok_or_else(|| verr("The location reference isn't valid.".to_string()))?;
        self.b.add_sparklines(s, kind, data.trim().trim_start_matches('='), r).map_err(verr)
    }
    fn sparklines<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let mut out = Vec::new();
        for g in self.b.sparkline_groups(s) {
            let d = PyDict::new_bound(py);
            d.set_item("kind", match g.kind.as_str() { "stacked" => "winloss", k => k })?;
            for (k, v) in [("color", g.color), ("negative_color", g.negative), ("markers_color", g.markers_color),
                           ("high_color", g.high_color), ("low_color", g.low_color), ("first_color", g.first_color),
                           ("last_color", g.last_color)] {
                d.set_item(k, v)?;
            }
            for (k, v) in [("markers", g.markers), ("high", g.high), ("low", g.low), ("first", g.first),
                           ("last", g.last), ("negative", g.show_negative)] {
                d.set_item(k, v)?;
            }
            d.set_item("weight", g.weight)?;
            d.set_item("items", g.items.clone())?;
            out.push(d);
        }
        Ok(out)
    }
    fn sparkline_at(&self, s: usize, r: u32, c: u32) -> PyResult<Option<(usize, usize)>> {
        self.sheet(s)?;
        Ok(self.b.sparkline_at(s, r, c))
    }
    /// Change a sparkline group: keys as sparklines() gives them.
    fn sparkline_set(&mut self, s: usize, group: usize, opts: &Bound<'_, PyDict>) -> PyResult<()> {
        self.sheet(s)?;
        let mut kind: Option<String> = None;
        let mut cols: Vec<(String, u32)> = vec![];
        let mut flags: Vec<(String, bool)> = vec![];
        let mut weight: Option<f64> = None;
        for (k, v) in opts.iter() {
            let k: String = k.extract()?;
            match k.as_str() {
                "kind" => kind = Some(v.extract()?),
                "weight" => weight = Some(v.extract()?),
                x if x.ends_with("color") => cols.push((k.clone(), v.extract()?)),
                _ => flags.push((k.clone(), v.extract()?)),
            }
        }
        self.b.edit_sparkline_group(s, group, |g| {
            if let Some(k) = kind {
                g.kind = match k.as_str() { "column" => "column", "winloss" | "stacked" => "stacked", _ => "line" }.into();
            }
            if let Some(w) = weight { g.weight = w; }
            for (k, v) in cols {
                match k.as_str() {
                    "color" => g.color = v, "negative_color" => g.negative = v, "markers_color" => g.markers_color = v,
                    "high_color" => g.high_color = v, "low_color" => g.low_color = v, "first_color" => g.first_color = v,
                    "last_color" => g.last_color = v, _ => {}
                }
            }
            for (k, v) in flags {
                match k.as_str() {
                    "markers" => g.markers = v, "high" => g.high = v, "low" => g.low = v, "first" => g.first = v,
                    "last" => g.last = v, "negative" => g.show_negative = v, _ => {}
                }
            }
        }).map_err(verr)
    }
    fn regroup_sparklines(&mut self, s: usize, r1: u32, c1: u32, r2: u32, c2: u32, group: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.regroup_sparklines(s, crate::cellref::Rect::new(r1, c1, r2, c2), group).map_err(verr)
    }
    #[pyo3(signature = (s, r1, c1, r2, c2, group=false))]
    fn clear_sparklines(&mut self, s: usize, r1: u32, c1: u32, r2: u32, c2: u32, group: bool) -> PyResult<()> {
        self.sheet(s)?;
        self.b.clear_sparklines(s, crate::cellref::Rect::new(r1, c1, r2, c2), group).map_err(verr)
    }
    fn reorder_object(&mut self, s: usize, i: usize, by: i64) -> PyResult<usize> {
        self.b.reorder_object(s, i, by).map_err(verr)
    }
    // ---------------------------------------------------------------- What-If Analysis, Solver (SH-20)
    /// Goal Seek: (the value found, the target's value with it, whether it got there).
    fn goal_seek(&mut self, s: usize, tr: u32, tc: u32, goal: f64, cr: u32, cc: u32) -> PyResult<(f64, f64, bool)> {
        self.sheet(s)?;
        self.b.goal_seek(s, (tr, tc), goal, (cr, cc)).map_err(verr)
    }
    #[pyo3(signature = (s, r1, c1, r2, c2, row_input=None, col_input=None))]
    fn create_data_table(&mut self, s: usize, r1: u32, c1: u32, r2: u32, c2: u32, row_input: Option<(u32, u32)>, col_input: Option<(u32, u32)>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.create_data_table(s, crate::cellref::Rect::new(r1, c1, r2, c2), row_input, col_input).map_err(verr)
    }
    fn data_table_at(&self, s: usize, r: u32, c: u32) -> Option<R4> {
        self.b.data_table_at(s, r, c).map(|t| tup(&t.rect))
    }
    fn scenarios<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Vec<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        let mut out = Vec::new();
        for sc in self.b.scenarios(s) {
            let d = PyDict::new_bound(py);
            d.set_item("name", &sc.name)?;
            d.set_item("comment", &sc.comment)?;
            d.set_item("user", &sc.user)?;
            d.set_item("locked", sc.locked)?;
            d.set_item("hidden", sc.hidden)?;
            d.set_item("cells", sc.cells.iter().map(|((r, c), v)| (*r, *c, v.clone())).collect::<Vec<_>>())?;
            out.push(d);
        }
        Ok(out)
    }
    #[pyo3(signature = (s, list, label="Scenario"))]
    fn set_scenarios(&mut self, s: usize, list: Vec<Bound<'_, PyDict>>, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        let mut v = Vec::new();
        for d in list {
            let cells: Vec<(u32, u32, String)> = get(&d, "cells").unwrap_or_default();
            v.push(crate::whatif::Scenario {
                name: get(&d, "name").unwrap_or_default(),
                comment: get(&d, "comment").unwrap_or_default(),
                user: get(&d, "user").unwrap_or_default(),
                locked: get(&d, "locked").unwrap_or(true),
                hidden: get(&d, "hidden").unwrap_or(false),
                cells: cells.into_iter().map(|(r, c, x)| ((r, c), x)).collect(),
            });
        }
        self.b.set_scenarios(s, &v, label).map_err(verr)
    }
    fn show_scenario(&mut self, s: usize, i: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.show_scenario(s, i).map_err(verr)
    }
    fn scenario_results(&mut self, py: Python<'_>, s: usize, cells: Vec<(u32, u32)>) -> PyResult<Vec<Vec<PyObject>>> {
        self.sheet(s)?;
        Ok(self.b.scenario_results(s, &cells).iter().map(|r| r.iter().map(|v| py_value(py, v)).collect()).collect())
    }
    /// The Solver model saved with the sheet.
    fn solver_model<'py>(&self, py: Python<'py>, s: usize) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let m = self.b.solver_model(s);
        solver_dict(py, &m)
    }
    fn set_solver_model(&mut self, s: usize, d: &Bound<'_, PyDict>) -> PyResult<()> {
        self.sheet(s)?;
        let m = dict_solver(d)?;
        self.b.set_solver_model(s, &m).map_err(verr)
    }
    /// Solve: {status, message, found, x, start, objective, objective_start, cells} (the sheet isn't changed).
    fn solve<'py>(&mut self, py: Python<'py>, s: usize, d: &Bound<'_, PyDict>) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let m = dict_solver(d)?;
        let o = self.b.solve(s, &m);
        let r = PyDict::new_bound(py);
        r.set_item("status", format!("{:?}", o.status).split('(').next().unwrap_or("").to_string())?;
        r.set_item("message", o.status.message())?;
        r.set_item("found", o.status.found())?;
        r.set_item("x", o.x)?;
        r.set_item("start", o.start)?;
        r.set_item("objective", o.objective)?;
        r.set_item("objective_start", o.objective_start)?;
        r.set_item("cells", o.cells)?;
        Ok(r)
    }
    // ---------------------------------------------------------------- PivotTables (SH-20)
    fn pivot_count(&self) -> usize {
        self.b.pivots.len()
    }
    fn pivot<'py>(&self, py: Python<'py>, i: usize) -> PyResult<Bound<'py, PyDict>> {
        let p = self.b.pivots.get(i).ok_or_else(|| PyIndexError::new_err("no such PivotTable"))?;
        pivot_dict(py, p)
    }
    fn pivot_at(&self, s: usize, r: u32, c: u32) -> Option<usize> {
        self.b.pivot_at(s, r, c)
    }
    /// What a cell of a pivot is: page_label, page_value, header, col_item, row_item, subtotal, data, grand_label, grand_row, grand_col.
    fn pivot_role(&self, s: usize, r: u32, c: u32) -> Option<(String, u8)> {
        use crate::pivot::Role;
        let i = self.b.pivot_at(s, r, c)?;
        let role = *self.b.pivots[i].roles.get(&(r, c))?;
        Some(match role {
            Role::PageLabel => ("page_label".into(), 0),
            Role::PageValue => ("page_value".into(), 0),
            Role::Header => ("header".into(), 0),
            Role::ColItem => ("col_item".into(), 0),
            Role::RowItem(l) => ("row_item".into(), l),
            Role::Subtotal(l) => ("subtotal".into(), l),
            Role::Data => ("data".into(), 0),
            Role::GrandLabel => ("grand_label".into(), 0),
            Role::GrandRow => ("grand_row".into(), 0),
            Role::GrandCol => ("grand_col".into(), 0),
        })
    }
    /// The (row items, column items) a value cell sums.
    fn pivot_path(&self, s: usize, r: u32, c: u32) -> Option<(Vec<String>, Vec<String>)> {
        let i = self.b.pivot_at(s, r, c)?;
        self.b.pivots[i].paths.get(&(r, c)).cloned()
    }
    /// The pivot drop-down at a cell: (pivot, what, index) — what: rows / cols / page.
    fn pivot_button_at(&self, s: usize, r: u32, c: u32) -> Option<(usize, String, usize)> {
        let i = self.b.pivot_at(s, r, c)?;
        self.b.pivot_buttons(s).into_iter().find(|b| b.0 == r && b.1 == c).map(|b| (i, b.3.to_string(), b.4))
    }
    fn pivot_field_items(&self, i: usize, f: usize) -> Vec<String> {
        self.b.pivot_field_items(i, f)
    }
    fn create_pivot(&mut self, src_sheet: usize, src: R4, dest_sheet: usize, at: (u32, u32)) -> PyResult<usize> {
        self.sheet(src_sheet)?;
        self.sheet(dest_sheet)?;
        self.b.create_pivot(src_sheet, Rect::new(src.0, src.1, src.2, src.3), dest_sheet, at).map_err(verr)
    }
    #[pyo3(signature = (i, d, label="PivotTable"))]
    fn set_pivot(&mut self, i: usize, d: &Bound<'_, PyDict>, label: &str) -> PyResult<()> {
        let mut p = self.b.pivots.get(i).cloned().ok_or_else(|| PyIndexError::new_err("no such PivotTable"))?;
        dict_into_pivot(d, &mut p)?;
        if p.name != self.b.pivots[i].name && self.b.pivots.iter().enumerate().any(|(k, q)| k != i && q.name.eq_ignore_ascii_case(&p.name)) {
            return Err(verr("A PivotTable with that name already exists.".into()));
        }
        self.b.edit_pivot(i, label, move |q| *q = p).map_err(verr)
    }
    fn refresh_pivot(&mut self, i: usize) -> PyResult<()> {
        self.b.refresh_pivot(i).map_err(verr)
    }
    fn refresh_all_pivots(&mut self) -> PyResult<()> {
        self.b.refresh_all_pivots().map_err(verr)
    }
    fn delete_pivot(&mut self, i: usize) -> PyResult<()> {
        self.b.delete_pivot(i).map_err(verr)
    }
    fn move_pivot(&mut self, i: usize, sheet: usize, at: (u32, u32)) -> PyResult<()> {
        self.sheet(sheet)?;
        self.b.move_pivot(i, sheet, at).map_err(verr)
    }
    /// Show Details: the new sheet's index.
    fn pivot_details(&mut self, i: usize, r: u32, c: u32) -> PyResult<usize> {
        self.b.pivot_details(i, r, c).map_err(verr)
    }
    // ---------------------------------------------------------------- PivotCharts (SH-20)
    #[pyo3(signature = (pivot, kind="col", x=0.0, y=0.0))]
    fn insert_pivot_chart(&mut self, pivot: usize, kind: &str, x: f64, y: f64) -> PyResult<usize> {
        self.b.insert_pivot_chart(pivot, kind, x, y).map_err(verr)
    }
    /// The PivotTable a chart plots (None: an ordinary chart).
    fn chart_pivot(&self, s: usize, i: usize) -> Option<String> {
        self.b.chart(s, i).and_then(|c| c.pivot.clone())
    }
    fn pivot_chart_series(&self, pivot: usize) -> PyResult<Vec<(String, String, String)>> {
        self.b.pivot_chart_series(pivot).map(|v| v.into_iter().map(|x| (x.name, x.cat, x.val)).collect()).map_err(verr)
    }
    // ---------------------------------------------------------------- slicers and timelines (SH-20)
    #[pyo3(signature = (pivot, field, timeline=false))]
    fn insert_slicer(&mut self, pivot: usize, field: usize, timeline: bool) -> PyResult<usize> {
        self.b.insert_slicer(pivot, field, timeline).map_err(verr)
    }
    /// A slicer's settings: name, caption, cache, field, pivots, timeline, level, span, columns, style, header.
    fn slicer_info<'py>(&self, py: Python<'py>, s: usize, i: usize) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(x) = self.b.slicer(s, i) else { return Ok(None) };
        let d = PyDict::new_bound(py);
        d.set_item("name", &x.name)?;
        d.set_item("caption", &x.caption)?;
        d.set_item("cache", &x.cache)?;
        d.set_item("field", &x.field)?;
        d.set_item("pivots", x.pivots.clone())?;
        d.set_item("timeline", x.timeline)?;
        d.set_item("level", x.level)?;
        d.set_item("span", x.span)?;
        d.set_item("columns", x.columns)?;
        d.set_item("style", &x.style)?;
        d.set_item("header", x.show_header)?;
        d.set_item("target", self.b.slicer_target(x))?;
        Ok(Some(d))
    }
    fn slicer_items(&self, s: usize, i: usize) -> Vec<(String, bool)> {
        self.b.slicer_items(s, i)
    }
    #[pyo3(signature = (s, i, item, add=false))]
    fn slicer_choose(&mut self, s: usize, i: usize, item: usize, add: bool) -> PyResult<()> {
        self.b.slicer_choose(s, i, item, add).map_err(verr)
    }
    fn slicer_clear(&mut self, s: usize, i: usize) -> PyResult<()> {
        self.b.slicer_clear(s, i).map_err(verr)
    }
    /// What's under (x, y), an offset from the slicer's top left on screen.
    fn slicer_hit(&self, s: usize, i: usize, x: f64, y: f64, w: f64, h: f64, z: f64) -> Option<(String, usize)> {
        self.b.slicer_hit(s, i, x, y, w, h, z).map(|(k, n)| (k.to_string(), n))
    }
    /// Change a slicer: any of caption, columns, style, level, header, pivots.
    #[pyo3(signature = (s, i, caption=None, columns=None, style=None, level=None, header=None, pivots=None, label="Slicer Settings"))]
    #[allow(clippy::too_many_arguments)]
    fn set_slicer(&mut self, s: usize, i: usize, caption: Option<String>, columns: Option<u32>, style: Option<String>, level: Option<u8>,
                  header: Option<bool>, pivots: Option<Vec<String>>, label: &str) -> PyResult<()> {
        self.b.set_slicer_object(s, i, label, |x| {
            if let Some(c) = caption { x.caption = c; }
            if let Some(c) = columns { x.columns = c; }
            if let Some(c) = style { x.style = c; }
            if let Some(c) = level { x.level = c.min(3); }
            if let Some(c) = header { x.show_header = c; }
            if let Some(c) = pivots { x.pivots = c; }
        }).map_err(verr)
    }
    fn timeline_periods(&self, s: usize, i: usize) -> Vec<(String, f64, f64, String)> {
        self.b.timeline_periods(s, i)
    }
    fn timeline_select(&mut self, s: usize, i: usize, first: usize, last: usize) -> PyResult<()> {
        self.b.timeline_select(s, i, first, last).map_err(verr)
    }
    fn timeline_dates(&self, pivot: usize, field: usize) -> Vec<f64> {
        self.b.timeline_dates(pivot, field)
    }
    // ---------------------------------------------------------------- themes (SH-22)
    /// The theme: name, colours (part order dk1 lt1 dk2 lt2 accent1..6 hlink folHlink), fonts, effects.
    fn theme_info<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let t = self.b.theme_info();
        let d = PyDict::new_bound(py);
        d.set_item("name", t.name)?;
        d.set_item("colors_name", t.colors_name)?;
        d.set_item("colors", t.colors.to_vec())?;
        d.set_item("fonts_name", t.fonts_name)?;
        d.set_item("major", t.major)?;
        d.set_item("minor", t.minor)?;
        d.set_item("effects_name", t.effects_name)?;
        Ok(d)
    }
    /// The palette's theme columns: light 1, dark 1, light 2, dark 2, accents 1-6, hyperlinks.
    fn theme_colors(&self) -> Vec<u32> {
        self.b.styles.theme.to_vec()
    }
    /// A colour as it shows (theme references worked out).
    fn resolve_color(&self, c: u32) -> u32 {
        self.b.styles.resolve(c)
    }
    fn theme_xml(&self) -> String {
        self.b.theme_xml()
    }
    fn set_theme_xml(&mut self, x: &str) -> PyResult<()> {
        self.b.set_theme_xml(x, "Theme").map_err(verr)
    }
    fn apply_theme(&mut self, name: &str) -> PyResult<()> {
        self.b.apply_theme(name).map_err(verr)
    }
    fn set_theme_colors(&mut self, name: &str, colors: Vec<u32>) -> PyResult<()> {
        let c: [u32; 12] = colors.try_into().map_err(|_| verr("Twelve colours are needed.".to_string()))?;
        self.b.set_theme_colors(name, &c).map_err(verr)
    }
    fn set_theme_fonts(&mut self, name: &str, major: &str, minor: &str) -> PyResult<()> {
        self.b.set_theme_fonts(name, major, minor).map_err(verr)
    }
    fn set_theme_effects(&mut self, from_xml: &str) -> PyResult<()> {
        self.b.set_theme_effects(from_xml).map_err(verr)
    }
    /// Save Current Theme: a theme file (.thmx).
    fn save_theme(&self, path: &str, name: Option<&str>) -> PyResult<()> {
        let mut x = self.b.theme_xml();
        if let Some(n) = name {
            x = crate::themes::renamed(&x, n).map_err(verr)?;
        }
        std::fs::write(path, crate::themes::thmx_bytes(&x)).map_err(|e| verr(e.to_string()))
    }
    /// Browse for Themes: the theme in a .thmx file, a workbook, or a theme part.
    #[staticmethod]
    fn read_theme_file(path: &str) -> PyResult<String> {
        let d = std::fs::read(path).map_err(|e| verr(e.to_string()))?;
        crate::themes::theme_from_bytes(&d).map_err(verr)
    }
    #[staticmethod]
    fn theme_xml_info<'py>(py: Python<'py>, x: &str) -> PyResult<Option<Bound<'py, PyDict>>> {
        let t = match crate::themes::info(x) {
            Some(t) => t,
            None => return Ok(None),
        };
        let d = PyDict::new_bound(py);
        d.set_item("name", t.name)?;
        d.set_item("colors_name", t.colors_name)?;
        d.set_item("colors", t.colors.to_vec())?;
        d.set_item("fonts_name", t.fonts_name)?;
        d.set_item("major", t.major)?;
        d.set_item("minor", t.minor)?;
        d.set_item("effects_name", t.effects_name)?;
        Ok(Some(d))
    }
    #[staticmethod]
    fn default_theme_xml() -> String {
        crate::themes::default_theme_xml()
    }
    #[staticmethod]
    fn color_sets() -> Vec<(String, Vec<u32>)> {
        crate::themes::COLOR_SETS.iter().map(|(n, c)| (n.to_string(), c.to_vec())).collect()
    }
    #[staticmethod]
    fn font_sets() -> Vec<(String, String, String)> {
        crate::themes::FONT_SETS.iter().map(|(n, a, b)| (n.to_string(), a.to_string(), b.to_string())).collect()
    }
    /// The ready-made themes: (name, colours, headings font, body font).
    #[staticmethod]
    fn builtin_themes() -> Vec<(String, Vec<u32>, String, String)> {
        crate::themes::THEMES.iter().map(|t| (t.0.to_string(), t.2.to_vec(), t.4.to_string(), t.5.to_string())).collect()
    }
    /// A theme reference for a palette pick: slot (palette column) and tint.
    #[staticmethod]
    fn theme_ref(slot: u32, tint: f64) -> u32 {
        crate::styles::theme_ref(slot, tint)
    }
    /// Insert › Equation: a text box with an equation typed in the linear format.
    fn insert_equation(&mut self, s: usize, linear: &str, x: f64, y: f64) -> PyResult<usize> {
        self.b.insert_equation(s, linear, x, y).map_err(verr)
    }
    fn equation_linear(&self, s: usize, i: usize) -> Option<String> {
        self.b.equation_linear(s, i)
    }
    fn set_equation(&mut self, s: usize, i: usize, linear: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_equation(s, i, linear).map_err(verr)
    }
    /// Draw an equation (linear format) with its baseline's left end at (x, y); returns (w, asc, desc).
    #[staticmethod]
    #[pyo3(signature = (cr, linear, x, y, size_px, color=0))]
    fn paint_equation(cr: &Bound<'_, PyAny>, linear: &str, x: f64, y: f64, size_px: f64, color: u32) -> PyResult<(f64, f64, f64)> {
        let p = cairo_ptr(cr)?;
        let m = crate::math::parse_linear(linear);
        unsafe {
            let mut ms = crate::math::PangoMeasure::new(p);
            let b = crate::math::layout(&m, &mut ms, size_px);
            crate::math::paint(p, &mut ms, &b, x, y, color);
            Ok((b.w, b.asc, b.desc))
        }
    }
    #[staticmethod]
    fn equation_extent(linear: &str, size_px: f64) -> (f64, f64, f64) {
        crate::book_shape::equation_extent(&crate::math::parse_linear(linear), size_px)
    }
    /// The equation as Office Math (a14:m), and back as linear: for tests and the clipboard.
    #[staticmethod]
    fn equation_omml(linear: &str) -> String {
        crate::math::write_omml(&crate::math::parse_linear(linear), None)
    }
    #[staticmethod]
    fn equation_normalize(linear: &str) -> String {
        crate::math::to_linear(&crate::math::parse_linear(linear))
    }
    #[staticmethod]
    fn equation_built_ins() -> Vec<(String, String)> {
        crate::math::BUILT_IN.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }
    /// The Shapes gallery: [(group, [preset names])].
    #[staticmethod]
    fn shape_gallery() -> Vec<(String, Vec<String>)> {
        crate::shape::GALLERY.iter().map(|(g, l)| (g.to_string(), l.iter().map(|s| s.to_string()).collect())).collect()
    }
    #[staticmethod]
    fn wordart_styles() -> Vec<String> {
        crate::shape::WORDART.iter().map(|(n, _)| n.to_string()).collect()
    }
    /// Draw a preset's outline (the gallery's icons), in a w × h box at (x, y).
    #[staticmethod]
    fn paint_shape_icon(cr: &Bound<'_, PyAny>, prst: &str, x: f64, y: f64, w: f64, h: f64) -> PyResult<()> {
        let p = cairo_ptr(cr)?;
        let v = crate::shape::ShapeView::parse(&crate::shape::new_body(1, "x", if prst == "textBox" { "rect" } else { prst }, 100, 100, None, None), &[0xFFFFFF, 0, 0, 0, 0x4472C4, 0, 0, 0, 0, 0, 0, 0]);
        if let Some(mut v) = v {
            v.geom = prst.to_string();
            unsafe {
                use crate::ffi::*;
                use crate::shape::Seg;
                for sub in crate::shape::preset(&v, w, h) {
                    cairo_new_path(p);
                    for sg in &sub.segs {
                        match *sg {
                            Seg::Move(a, b) => cairo_move_to(p, x + a, y + b),
                            Seg::Line(a, b) => cairo_line_to(p, x + a, y + b),
                            Seg::Curve(a, b, c, d, e, f) => cairo_curve_to(p, x + a, y + b, x + c, y + d, x + e, y + f),
                            Seg::Close => cairo_close_path(p),
                        }
                    }
                    cairo_stroke(p);
                }
            }
        }
        Ok(())
    }
    /// True when the workbook holds a VBA project (kept, never run).
    fn has_macros(&self) -> bool {
        self.b.has_macros()
    }
    /// The macros a user could run: (shown name, module, standard module).
    fn macros(&self) -> Vec<(String, String, bool)> {
        self.b.macros()
    }
    fn drop_macros(&mut self) {
        self.b.drop_macros()
    }
        fn insert_chart(&mut self, s: usize, rg: R4, kind: &str, x: f64, y: f64) -> PyResult<usize> {
        self.sheet(s)?;
        self.b.insert_chart(s, rect(rg), kind, x, y).map_err(verr)
    }
    fn chart_spec<'py>(&self, py: Python<'py>, s: usize, i: usize) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sheet(s)?;
        match self.b.chart_spec(s, i) {
            Some(sp) => Ok(Some(spec_dict(py, &sp)?)),
            None => Ok(None),
        }
    }
    #[pyo3(signature = (s, i, spec, label="Format Chart"))]
    fn set_chart_spec(&mut self, s: usize, i: usize, spec: &Bound<'_, PyDict>, label: &str) -> PyResult<()> {
        self.sheet(s)?;
        let sp = dict_spec(spec)?;
        self.b.set_chart_spec(s, i, &sp, label).map_err(verr)
    }
    fn set_chart_kind(&mut self, s: usize, i: usize, kind: &str) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_chart_kind(s, i, kind).map_err(verr)
    }
    fn switch_chart_rows(&mut self, s: usize, i: usize) -> PyResult<()> {
        self.sheet(s)?;
        self.b.switch_chart_rows(s, i).map_err(verr)
    }
    fn chart_range_text(&self, s: usize, i: usize) -> PyResult<Option<String>> {
        self.sheet(s)?;
        Ok(self.b.chart_range_text(s, i))
    }
    #[pyo3(signature = (s, i, range, by_rows=None))]
    fn set_chart_range(&mut self, s: usize, i: usize, range: &str, by_rows: Option<bool>) -> PyResult<()> {
        self.sheet(s)?;
        self.b.set_chart_range(s, i, range, by_rows).map_err(verr)
    }
    fn chart_title(&self, s: usize, i: usize) -> PyResult<Option<String>> {
        self.sheet(s)?;
        Ok(self.b.chart(s, i).and_then(|c| self.b.chart_title_text(c)))
    }
    fn chart_exotic(&self, s: usize, i: usize) -> PyResult<bool> {
        self.sheet(s)?;
        Ok(self.b.chart(s, i).map(|c| c.exotic()).unwrap_or(false))
    }
    /// Draw a chart into a cairo context: the sheet's chart i, or (rg, kind) a preview
    /// of a new chart of that range.
    #[pyo3(signature = (s, cr, w, h, i=None, rg=None, kind=None, zoom=1.0))]
    fn render_chart(&self, s: usize, cr: &Bound<'_, PyAny>, w: f64, h: f64, i: Option<usize>, rg: Option<R4>, kind: Option<&str>, zoom: f64) -> PyResult<()> {
        self.sheet(s)?;
        let ctx = cairo_ptr(cr)?;
        let ch = match (i, rg) {
            (Some(i), _) => self.b.chart(s, i).cloned().ok_or_else(|| PyValueError::new_err("no such chart"))?,
            (None, Some(rg)) => self.b.chart_from_range(s, rect(rg), kind.unwrap_or("col")).map_err(verr)?,
            _ => return Err(PyValueError::new_err("give a chart or a range")),
        };
        unsafe { self.b.paint_chart_at(&ch, ctx, 0.0, 0.0, w, h, zoom) };
        Ok(())
    }
    /// The text of the cells a chart reference names.
    fn chart_ref_text(&self, f: &str) -> String {
        let src = crate::chart::Src { f: Some(f.trim_start_matches('=').to_string()), cache: vec![], fmt: None };
        self.b.chart_values(&src).into_iter().map(|p| p.text).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ")
    }
    /// The spec a new chart of a range would have.
    fn range_spec<'py>(&self, py: Python<'py>, s: usize, rg: R4, kind: &str) -> PyResult<Bound<'py, PyDict>> {
        self.sheet(s)?;
        let ch = self.b.chart_from_range(s, rect(rg), kind).map_err(verr)?;
        spec_dict(py, &self.b.spec_of(&ch))
    }
    #[pyo3(signature = (s, cr, w, h, spec, zoom=1.0))]
    fn render_spec(&self, s: usize, cr: &Bound<'_, PyAny>, w: f64, h: f64, spec: &Bound<'_, PyDict>, zoom: f64) -> PyResult<()> {
        self.sheet(s)?;
        let ctx = cairo_ptr(cr)?;
        let ch = self.b.chart_of_spec(&dict_spec(spec)?).map_err(verr)?;
        unsafe { self.b.paint_chart_at(&ch, ctx, 0.0, 0.0, w, h, zoom) };
        Ok(())
    }
    #[pyo3(signature = (s, spec, x, y, w=480.0, h=288.0))]
    fn add_chart_spec(&mut self, s: usize, spec: &Bound<'_, PyDict>, x: f64, y: f64, w: f64, h: f64) -> PyResult<usize> {
        self.sheet(s)?;
        let ch = self.b.chart_of_spec(&dict_spec(spec)?).map_err(verr)?;
        self.b.add_chart(s, ch, x, y, w, h).map_err(verr)
    }
    /// Whether a range can be charted (None) or why not.
    fn chart_check(&self, s: usize, rg: R4, kind: &str) -> PyResult<Option<String>> {
        self.sheet(s)?;
        Ok(self.b.chart_from_range(s, rect(rg), kind).err())
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

/// Draw one icon of a conditional-format icon set (menus and dialogs).
#[pyfunction]
fn draw_cf_icon(cr: &Bound<'_, PyAny>, set: &str, i: u32, x: f64, y: f64, size: f64) -> PyResult<()> {
    let ctx = cairo_ptr(cr)?;
    unsafe { crate::cf_icons::draw_icon(ctx, set, i, x, y, size) };
    Ok(())
}

/// The paper sizes: (code, name, width, height in points).
#[pyfunction]
fn papers() -> Vec<(u32, &'static str, f64, f64)> {
    crate::pagelayout::PAPERS.to_vec()
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
    m.add_function(wrap_pyfunction!(draw_cf_icon, m)?)?;
    m.add_function(wrap_pyfunction!(papers, m)?)?;
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
