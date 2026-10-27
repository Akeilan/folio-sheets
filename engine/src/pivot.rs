//! PivotTables (SH-20): a summary of a range or table — fields in rows, columns, filters and
//! values — laid out on a sheet as the desktop spreadsheet lays it out (compact, outline or
//! tabular form, subtotals, grand totals), its cells written as values and drawn in a pivot style.

use crate::book::{Book, Delta};
use crate::cellref::Rect;
use crate::sheet::Cell;
use crate::value::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

pub const PIVOT_CHANGE: &str = "We can't make this change for the selected cells because it will affect a PivotTable. Use the field list to change the report. If you are trying to insert or delete cells, move the PivotTable and try again.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Agg {
    Sum,
    Count,
    Average,
    Max,
    Min,
    Product,
    CountNums,
    StdDev,
    StdDevP,
    Var,
    VarP,
}

impl Agg {
    pub fn name(&self) -> &'static str {
        match self {
            Agg::Sum => "Sum",
            Agg::Count => "Count",
            Agg::Average => "Average",
            Agg::Max => "Max",
            Agg::Min => "Min",
            Agg::Product => "Product",
            Agg::CountNums => "Count",
            Agg::StdDev => "StdDev",
            Agg::StdDevP => "StdDevp",
            Agg::Var => "Var",
            Agg::VarP => "Varp",
        }
    }
    /// The file's subtotal attribute.
    pub fn code(&self) -> &'static str {
        match self {
            Agg::Sum => "sum",
            Agg::Count => "count",
            Agg::Average => "average",
            Agg::Max => "max",
            Agg::Min => "min",
            Agg::Product => "product",
            Agg::CountNums => "countNums",
            Agg::StdDev => "stdDev",
            Agg::StdDevP => "stdDevp",
            Agg::Var => "var",
            Agg::VarP => "varp",
        }
    }
    pub fn from_code(s: &str) -> Agg {
        match s {
            "count" => Agg::Count,
            "average" => Agg::Average,
            "max" => Agg::Max,
            "min" => Agg::Min,
            "product" => Agg::Product,
            "countNums" => Agg::CountNums,
            "stdDev" => Agg::StdDev,
            "stdDevp" => Agg::StdDevP,
            "var" => Agg::Var,
            "varp" => Agg::VarP,
            _ => Agg::Sum,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowAs {
    Normal,
    PercentOfTotal,
    PercentOfCol,
    PercentOfRow,
    RunningTotal,
}

impl ShowAs {
    pub fn code(&self) -> &'static str {
        match self {
            ShowAs::Normal => "normal",
            ShowAs::PercentOfTotal => "percentOfTotal",
            ShowAs::PercentOfCol => "percentOfCol",
            ShowAs::PercentOfRow => "percentOfRow",
            ShowAs::RunningTotal => "runTotal",
        }
    }
    pub fn from_code(s: &str) -> ShowAs {
        match s {
            "percentOfTotal" => ShowAs::PercentOfTotal,
            "percentOfCol" => ShowAs::PercentOfCol,
            "percentOfRow" => ShowAs::PercentOfRow,
            "runTotal" => ShowAs::RunningTotal,
            _ => ShowAs::Normal,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataField {
    pub field: usize,
    pub agg: Agg,
    pub name: String,
    pub show: ShowAs,
    pub numfmt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PField {
    pub name: String,
    /// items filtered out (their texts)
    pub hidden: HashSet<String>,
    /// -1 descending, else ascending
    pub sort: i8,
    pub subtotal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Compact,
    Outline,
    Tabular,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subtotals {
    None,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    PageLabel,
    PageValue,
    /// "Row Labels", "Sum of X", "Column Labels", field names in outline/tabular form
    Header,
    ColItem,
    /// a row item at a level
    RowItem(u8),
    /// a row that holds a subtotal (its label and values)
    Subtotal(u8),
    Data,
    GrandLabel,
    GrandRow,
    GrandCol,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pivot {
    pub name: String,
    /// the source: a sheet's range (or a table, by name)
    pub source_sheet: String,
    pub source: Rect,
    pub source_table: Option<String>,
    pub sheet: usize,
    /// the top-left of the table (filters go above it)
    pub at: (u32, u32),
    pub fields: Vec<PField>,
    pub rows: Vec<usize>,
    pub cols: Vec<usize>,
    /// filter fields, with the item shown (None: all)
    pub pages: Vec<(usize, Option<String>)>,
    pub values: Vec<DataField>,
    /// Σ Values in the rows (else the columns)
    pub values_in_rows: bool,
    pub layout: Layout,
    pub subtotals: Subtotals,
    pub grand_rows: bool,
    pub grand_cols: bool,
    pub repeat_labels: bool,
    pub blank_rows: bool,
    pub style: String,
    pub row_headers: bool,
    pub col_headers: bool,
    pub banded_rows: bool,
    pub banded_cols: bool,
    pub field_headers: bool,
    pub cache_id: u32,
    /// what it covers on the sheet now (filters included)
    pub out: Option<Rect>,
    pub roles: Rc<HashMap<(u32, u32), Role>>,
    /// output cell -> the record filter it sums (row path, col path), for Show Details
    pub paths: Rc<HashMap<(u32, u32), (Vec<String>, Vec<String>)>>,
    /// the file's own parts (kept when this version can't remake them)
    pub table_part: Option<String>,
    pub cache_part: Option<String>,
    /// options this version doesn't model are kept as read
    pub raw: Option<String>,
    pub refreshable: bool,
    /// changed since it was read: its parts are written anew
    pub dirty: bool,
    /// fields a slicer or timeline filters (their hidden items count even outside the areas)
    pub slicer_fields: Vec<usize>,
}

impl Pivot {
    pub fn new(name: &str, source_sheet: &str, source: Rect, sheet: usize, at: (u32, u32), headers: Vec<String>) -> Pivot {
        Pivot {
            name: name.to_string(),
            source_sheet: source_sheet.to_string(),
            source,
            source_table: None,
            sheet,
            at,
            fields: headers.into_iter().map(|n| PField { name: n, subtotal: true, ..Default::default() }).collect(),
            rows: vec![],
            cols: vec![],
            pages: vec![],
            values: vec![],
            values_in_rows: false,
            layout: Layout::Compact,
            subtotals: Subtotals::Top,
            grand_rows: true,
            grand_cols: true,
            repeat_labels: false,
            blank_rows: false,
            style: "PivotStyleLight16".into(),
            row_headers: true,
            col_headers: true,
            banded_rows: false,
            banded_cols: false,
            field_headers: true,
            cache_id: 0,
            out: None,
            roles: Rc::new(HashMap::new()),
            paths: Rc::new(HashMap::new()),
            table_part: None,
            cache_part: None,
            raw: None,
            refreshable: true,
            dirty: true,
            slicer_fields: Vec::new(),
        }
    }

    /// The range the table itself takes (filters not included).
    pub fn table_rect(&self) -> Option<Rect> {
        let o = self.out?;
        let top = if self.pages.is_empty() { o.r1 } else { o.r1 + self.pages.len() as u32 + 1 };
        Some(Rect::new(top, o.c1, o.r2, o.c2))
    }
}

// ---------------------------------------------------------------------- aggregation

#[derive(Clone, Debug, Default)]
struct Acc {
    count: f64,
    nums: f64,
    sum: f64,
    sumsq: f64,
    min: f64,
    max: f64,
    prod: f64,
}

impl Acc {
    fn add(&mut self, v: &Value) {
        if v.is_empty() {
            return;
        }
        self.count += 1.0;
        if let Value::Num(x) = v {
            if self.nums == 0.0 {
                self.min = *x;
                self.max = *x;
                self.prod = 1.0;
            }
            self.nums += 1.0;
            self.sum += x;
            self.sumsq += x * x;
            self.min = self.min.min(*x);
            self.max = self.max.max(*x);
            self.prod *= x;
        }
    }

    fn get(&self, a: Agg) -> Value {
        let n = self.nums;
        let var = |samp: bool| -> Option<f64> {
            let d = if samp { n - 1.0 } else { n };
            if d <= 0.0 {
                return None;
            }
            Some(((self.sumsq - self.sum * self.sum / n) / d).max(0.0))
        };
        let num = |x: Option<f64>| x.map(Value::num).unwrap_or(Value::Err(crate::value::XlErr::Div0));
        match a {
            Agg::Count => Value::num(self.count),
            Agg::CountNums => Value::num(n),
            Agg::Sum => Value::num(self.sum),
            Agg::Average => num(if n > 0.0 { Some(self.sum / n) } else { None }),
            Agg::Max => Value::num(if n > 0.0 { self.max } else { 0.0 }),
            Agg::Min => Value::num(if n > 0.0 { self.min } else { 0.0 }),
            Agg::Product => Value::num(if n > 0.0 { self.prod } else { 0.0 }),
            Agg::StdDev => num(var(true).map(|v| v.sqrt())),
            Agg::StdDevP => num(var(false).map(|v| v.sqrt())),
            Agg::Var => num(var(true)),
            Agg::VarP => num(var(false)),
        }
    }
}

/// What each cell of a laid-out pivot is, and the rectangle it takes.
fn layout_roles(out: &Output, r0: u32, c0: u32) -> (Rect, HashMap<(u32, u32), Role>) {
    let rect = Rect::new(r0, c0, r0 + out.height.max(1) - 1, c0 + out.width.max(1) - 1);
    let mut roles: HashMap<(u32, u32), Role> = HashMap::new();
    for (r, c, _, role, _) in &out.cells {
        roles.insert((r0 + r, c0 + c), *role);
    }
    for r in rect.r1..=rect.r2 {
        for c in rect.c1..=rect.c2 {
            roles.entry((r, c)).or_insert(if r < r0 + out.first_data_row { Role::Header } else { Role::Data });
        }
    }
    if out.page_rows > 0 {
        for c in rect.c1..=rect.c2 {
            roles.remove(&(r0 + out.page_rows - 1, c));
            for k in 0..out.page_rows - 1 {
                if c > c0 + 1 {
                    roles.remove(&(r0 + k, c));
                }
            }
        }
    }
    (rect, roles)
}

/// An item's text as the pivot shows it, and a key that sorts it.
pub(crate) fn item_text(b: &Book, v: &Value) -> String {
    match v {
        Value::Empty => "(blank)".into(),
        Value::Str(s) => s.to_string(),
        Value::Bool(t) => if *t { "TRUE".into() } else { "FALSE".into() },
        Value::Num(x) => crate::numfmt::num_to_text(*x),
        Value::Err(e) => e.text().to_string(),
        other => {
            let _ = b;
            format!("{:?}", other)
        }
    }
}

#[derive(Clone, PartialEq, PartialOrd)]
enum SortKey {
    Num(f64),
    Text(String),
    Blank,
}

fn sort_key(v: &Value) -> SortKey {
    match v {
        Value::Num(x) => SortKey::Num(*x),
        Value::Empty => SortKey::Blank,
        Value::Bool(t) => SortKey::Text(if *t { "TRUE".into() } else { "FALSE".into() }),
        v => SortKey::Text(match v { Value::Str(s) => s.to_lowercase(), _ => String::new() }),
    }
}

/// The source records: the header names and the rows below them.
pub struct Source {
    pub headers: Vec<String>,
    pub records: Vec<Vec<Value>>,
}

struct Node {
    text: String,
    key: SortKey,
    children: Vec<Node>,
}

fn build_tree(paths: &[Vec<(String, SortKey)>], depth: usize, sort: &[i8]) -> Vec<Node> {
    let mut map: BTreeMap<String, (SortKey, Vec<Vec<(String, SortKey)>>)> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in paths {
        if depth >= p.len() {
            continue;
        }
        let (t, k) = &p[depth];
        let e = map.entry(t.clone()).or_insert_with(|| {
            order.push(t.clone());
            (k.clone(), Vec::new())
        });
        e.1.push(p.clone());
    }
    let mut nodes: Vec<Node> = order.into_iter().map(|t| {
        let (k, sub) = map.remove(&t).unwrap();
        Node { children: build_tree(&sub, depth + 1, sort), text: t, key: k }
    }).collect();
    let desc = sort.get(depth).copied().unwrap_or(1) < 0;
    nodes.sort_by(|a, b| {
        // numbers before text, blanks last, as the desktop spreadsheet sorts items
        let rank = |k: &SortKey| match k { SortKey::Num(_) => 0, SortKey::Text(_) => 1, SortKey::Blank => 2 };
        let o = rank(&a.key).cmp(&rank(&b.key)).then(a.key.partial_cmp(&b.key).unwrap_or(std::cmp::Ordering::Equal));
        if desc && rank(&a.key) == rank(&b.key) { o.reverse() } else { o }
    });
    nodes
}

/// One line of the rows (or one column of the columns).
#[derive(Clone, Debug)]
struct Line {
    path: Vec<String>,
    kind: LineKind,
}

#[derive(Clone, Debug, PartialEq)]
enum LineKind {
    /// an item; with subtotals at the top it carries its group's values
    Item { carries: bool },
    /// "X Total" (subtotals at the bottom)
    Total,
    Grand,
    Blank,
}

fn flatten(nodes: &[Node], prefix: &mut Vec<String>, depth: usize, levels: usize, sub: Subtotals, sub_on: &[bool], blank: bool, out: &mut Vec<Line>) {
    for n in nodes {
        prefix.push(n.text.clone());
        let leaf = depth + 1 >= levels;
        let carries = !leaf && sub == Subtotals::Top && sub_on.get(depth).copied().unwrap_or(true);
        out.push(Line { path: prefix.clone(), kind: LineKind::Item { carries: leaf || carries } });
        if !leaf {
            flatten(&n.children, prefix, depth + 1, levels, sub, sub_on, blank, out);
            if sub == Subtotals::Bottom && sub_on.get(depth).copied().unwrap_or(true) {
                out.push(Line { path: prefix.clone(), kind: LineKind::Total });
            }
            if blank && depth == 0 {
                out.push(Line { path: prefix.clone(), kind: LineKind::Blank });
            }
        }
        prefix.pop();
    }
}

/// The computed table: cells (relative to the top-left of the whole output) with their roles.
pub struct Output {
    pub cells: Vec<(u32, u32, Value, Role, Option<String>)>,
    pub height: u32,
    pub width: u32,
    pub paths: HashMap<(u32, u32), (Vec<String>, Vec<String>)>,
    pub first_data_row: u32,
    pub first_data_col: u32,
    pub page_rows: u32,
    /// the items of each field in shown order (for the file's pivot definition)
    /// (path, kind, the data field of a line when Σ Values is in the rows)
    pub row_lines: Vec<(Vec<String>, String, Option<usize>)>,
    pub col_lines: Vec<(Vec<String>, String, usize)>,
    pub items: Vec<Vec<String>>,
}

pub fn compute(b: &Book, p: &Pivot, src: &Source) -> Output {
    let nf = src.headers.len();
    // the items of every field, for the filters and the file
    let mut items: Vec<Vec<String>> = vec![Vec::new(); nf];
    let mut seen: Vec<HashSet<String>> = vec![HashSet::new(); nf];
    for rec in &src.records {
        for (i, v) in rec.iter().enumerate().take(nf) {
            let t = item_text(b, v);
            if seen[i].insert(t.clone()) {
                items[i].push(t);
            }
        }
    }
    // records that pass the filters (hidden items, filter fields)
    let keep: Vec<&Vec<Value>> = src.records.iter().filter(|rec| {
        for (i, f) in p.fields.iter().enumerate() {
            if !f.hidden.is_empty() {
                if let Some(v) = rec.get(i) {
                    if f.hidden.contains(&item_text(b, v)) && (p.rows.contains(&i) || p.cols.contains(&i) || p.pages.iter().any(|x| x.0 == i) || p.slicer_fields.contains(&i)) {
                        return false;
                    }
                }
            }
        }
        for (i, sel) in &p.pages {
            if let Some(sel) = sel {
                if rec.get(*i).map(|v| &item_text(b, v) != sel).unwrap_or(true) {
                    return false;
                }
            }
        }
        true
    }).collect();
    let rp = |rec: &Vec<Value>| -> Vec<(String, SortKey)> { p.rows.iter().map(|i| { let v = rec.get(*i).cloned().unwrap_or(Value::Empty); (item_text(b, &v), sort_key(&v)) }).collect() };
    let cp = |rec: &Vec<Value>| -> Vec<(String, SortKey)> { p.cols.iter().map(|i| { let v = rec.get(*i).cloned().unwrap_or(Value::Empty); (item_text(b, &v), sort_key(&v)) }).collect() };
    let row_paths: Vec<Vec<(String, SortKey)>> = keep.iter().map(|r| rp(r)).collect();
    let col_paths: Vec<Vec<(String, SortKey)>> = keep.iter().map(|r| cp(r)).collect();
    let rsort: Vec<i8> = p.rows.iter().map(|i| p.fields.get(*i).map(|f| f.sort).unwrap_or(1)).collect();
    let csort: Vec<i8> = p.cols.iter().map(|i| p.fields.get(*i).map(|f| f.sort).unwrap_or(1)).collect();
    let rtree = build_tree(&row_paths, 0, &rsort);
    let ctree = build_tree(&col_paths, 0, &csort);
    // aggregates by (row prefix, col prefix)
    let nd = p.values.len();
    let mut acc: HashMap<(Vec<String>, Vec<String>), Vec<Acc>> = HashMap::new();
    for (k, rec) in keep.iter().enumerate() {
        let rpth: Vec<String> = row_paths[k].iter().map(|x| x.0.clone()).collect();
        let cpth: Vec<String> = col_paths[k].iter().map(|x| x.0.clone()).collect();
        for d in 0..=rpth.len() {
            for e in 0..=cpth.len() {
                let a = acc.entry((rpth[..d].to_vec(), cpth[..e].to_vec())).or_insert_with(|| vec![Acc::default(); nd]);
                for (j, df) in p.values.iter().enumerate() {
                    if let Some(v) = rec.get(df.field) {
                        a[j].add(v);
                    }
                }
            }
        }
    }
    let value_at = |rpath: &[String], cpath: &[String], j: usize| -> Value {
        let df = &p.values[j];
        let get = |r: &[String], c: &[String]| -> Value {
            acc.get(&(r.to_vec(), c.to_vec())).map(|a| a[j].get(df.agg)).unwrap_or(Value::Empty)
        };
        let v = get(rpath, cpath);
        let ratio = |num: &Value, den: &Value| -> Value {
            match (num, den) {
                (Value::Num(a), Value::Num(b)) if *b != 0.0 => Value::num(a / b),
                (Value::Empty, _) => Value::Empty,
                _ => Value::Err(crate::value::XlErr::Div0),
            }
        };
        match df.show {
            ShowAs::Normal | ShowAs::RunningTotal => v,
            ShowAs::PercentOfTotal => ratio(&v, &get(&[], &[])),
            ShowAs::PercentOfCol => ratio(&v, &get(&[], cpath)),
            ShowAs::PercentOfRow => ratio(&v, &get(rpath, &[])),
        }
    };
    // rows and columns
    let sub_on_r: Vec<bool> = p.rows.iter().map(|i| p.fields.get(*i).map(|f| f.subtotal).unwrap_or(true)).collect();
    let sub_on_c: Vec<bool> = p.cols.iter().map(|i| p.fields.get(*i).map(|f| f.subtotal).unwrap_or(true)).collect();
    let nd = p.values.len();
    // Σ Values in the rows: each line splits into a line per data field
    let vir = nd > 1 && p.values_in_rows;
    let rsub = if (vir || p.layout == Layout::Tabular) && p.subtotals == Subtotals::Top { Subtotals::Bottom } else { p.subtotals };
    let mut rlines = Vec::new();
    flatten(&rtree, &mut vec![], 0, p.rows.len(), rsub, &sub_on_r, p.blank_rows, &mut rlines);
    if p.layout == Layout::Tabular {
        // tabular form: a group's first item goes on its parent's row
        rlines.retain(|l| !(matches!(l.kind, LineKind::Item { .. }) && l.path.len() < p.rows.len()));
    }
    if p.rows.is_empty() && nd > 0 {
        // values only: one line with the totals
        rlines.push(Line { path: vec![], kind: LineKind::Grand });
    } else if p.grand_cols_effective() {
        rlines.push(Line { path: vec![], kind: LineKind::Grand });
    }
    let rlines: Vec<(Line, Option<usize>)> = if !vir {
        rlines.into_iter().map(|l| (l, None)).collect()
    } else {
        let mut v = Vec::new();
        for l in rlines {
            match l.kind {
                LineKind::Item { .. } if l.path.len() == p.rows.len() && !p.rows.is_empty() => {
                    v.push((Line { path: l.path.clone(), kind: LineKind::Item { carries: false } }, None));
                    for j in 0..nd {
                        v.push((Line { path: l.path.clone(), kind: LineKind::Item { carries: true } }, Some(j)));
                    }
                }
                LineKind::Item { .. } => v.push((Line { path: l.path, kind: LineKind::Item { carries: false } }, None)),
                LineKind::Total | LineKind::Grand => {
                    for j in 0..nd {
                        v.push((l.clone(), Some(j)));
                    }
                }
                LineKind::Blank => v.push((l, None)),
            }
        }
        v
    };
    let mut clines = Vec::new();
    // column subtotals come after their group
    flatten(&ctree, &mut vec![], 0, p.cols.len(), if p.subtotals == Subtotals::None { Subtotals::None } else { Subtotals::Bottom }, &sub_on_c, false, &mut clines);
    clines.retain(|l| !matches!(l.kind, LineKind::Item { .. }) || l.path.len() == p.cols.len());
    if !p.cols.is_empty() && p.grand_rows_effective() {
        clines.push(Line { path: vec![], kind: LineKind::Grand });
    }
    if p.cols.is_empty() {
        clines.push(Line { path: vec![], kind: LineKind::Grand });
    }
    // each column line, for each data field
    let mut cols: Vec<(Line, usize)> = Vec::new();
    for l in &clines {
        for j in 0..if vir { 1 } else { nd.max(1) } {
            cols.push((l.clone(), j));
        }
    }
    // several data fields across the columns
    let multi = nd > 1 && !vir;
    // shape
    let label_cols: u32 = match p.layout {
        Layout::Compact => 1,
        _ => (p.rows.len() as u32 + if vir { 1 } else { 0 }).max(1),
    };
    let col_levels = p.cols.len() as u32 + if multi { 1 } else { 0 };
    let page_rows = if p.pages.is_empty() { 0 } else { p.pages.len() as u32 + 1 };
    let header_rows = if p.cols.is_empty() { 1 } else { col_levels + 1 };
    let first_data_row = page_rows + header_rows;
    let mut out = Output { cells: vec![], height: 0, width: 0, paths: HashMap::new(), first_data_row, first_data_col: label_cols, page_rows,
                           row_lines: vec![], col_lines: vec![], items };
    let mut put = |out: &mut Output, r: u32, c: u32, v: Value, role: Role, fmt: Option<String>| {
        out.cells.push((r, c, v, role, fmt));
    };
    // filters
    for (k, (i, sel)) in p.pages.iter().enumerate() {
        let name = p.fields.get(*i).map(|f| f.name.clone()).unwrap_or_default();
        put(&mut out, k as u32, 0, Value::Str(Rc::from(name.as_str())), Role::PageLabel, None);
        let shown = match sel {
            Some(s) => s.clone(),
            None => {
                let f = &p.fields[*i];
                if f.hidden.is_empty() { "(All)".into() } else { "(Multiple Items)".into() }
            }
        };
        put(&mut out, k as u32, 1, Value::Str(Rc::from(shown.as_str())), Role::PageValue, None);
    }
    if nd == 0 && p.rows.is_empty() && p.cols.is_empty() {
        // the empty report's box (drawn, not written)
        out.height = page_rows + 18;
        out.width = 3;
        out.first_data_row = out.height;
        return out;
    }
    let s = |t: &str| Value::Str(Rc::from(t));
    let fname = |i: usize| p.fields.get(i).map(|f| f.name.clone()).unwrap_or_default();
    // headers
    let hr = page_rows + header_rows - 1;
    if p.cols.is_empty() {
        // Row Labels | Sum of X ...
        match p.layout {
            Layout::Compact => put(&mut out, hr, 0, s(if p.field_headers { if p.rows.is_empty() && !vir { "" } else { "Row Labels" } } else { "" }), Role::Header, None),
            _ => {
                for (k, i) in p.rows.iter().enumerate() {
                    put(&mut out, hr, k as u32, s(&fname(*i)), Role::Header, None);
                }
                if vir {
                    put(&mut out, hr, p.rows.len() as u32, s("Values"), Role::Header, None);
                }
            }
        }
        if vir {
            put(&mut out, hr, label_cols, s(""), Role::Header, None);
        } else {
            for (k, df) in p.values.iter().enumerate() {
                put(&mut out, hr, label_cols + k as u32, s(&df.name), Role::Header, None);
            }
        }
    } else {
        let top = page_rows;
        if !multi && !vir {
            put(&mut out, top, 0, s(&p.values.first().map(|d| d.name.clone()).unwrap_or_default()), Role::Header, None);
        }
        put(&mut out, top, label_cols, s(if p.field_headers { "Column Labels" } else { "" }), Role::Header, None);
        // column items, a row per level
        for (ci, (line, j)) in cols.iter().enumerate() {
            let c = label_cols + ci as u32;
            for lvl in 0..p.cols.len() {
                let r = top + 1 + lvl as u32;
                let text = match &line.kind {
                    LineKind::Grand => if lvl == 0 && (*j == 0 || !multi) { Some(if multi { format!("Total {}", p.values[*j].name) } else { "Grand Total".into() }) } else { None },
                    LineKind::Total => if lvl + 1 == line.path.len() && (*j == 0 || !multi) { Some(format!("{} Total", line.path[lvl])) } else { None },
                    _ => {
                        // an item shows where it starts
                        let first = ci == 0 || {
                            let prev = &cols[ci - 1].0;
                            prev.path.get(..=lvl) != line.path.get(..=lvl) || !matches!(prev.kind, LineKind::Item { .. })
                        };
                        if first && *j == 0 { line.path.get(lvl).cloned() } else { None }
                    }
                };
                if let Some(t) = text {
                    put(&mut out, r, c, s(&t), Role::ColItem, None);
                }
            }
            if multi {
                let name = match line.kind {
                    LineKind::Grand => format!("Total {}", p.values[*j].name),
                    LineKind::Total => format!("{} {}", line.path.last().cloned().unwrap_or_default(), p.values[*j].name),
                    _ => p.values[*j].name.clone(),
                };
                put(&mut out, top + p.cols.len() as u32 + 1, c, s(&name), Role::ColItem, None);
            }
        }
        match p.layout {
            Layout::Compact => put(&mut out, hr, 0, s(if p.field_headers { if p.rows.is_empty() && !vir { "" } else { "Row Labels" } } else { "" }), Role::Header, None),
            _ => {
                for (k, i) in p.rows.iter().enumerate() {
                    put(&mut out, hr, k as u32, s(&fname(*i)), Role::Header, None);
                }
                if vir {
                    put(&mut out, hr, p.rows.len() as u32, s("Values"), Role::Header, None);
                }
            }
        }
    }
    // the data rows
    let mut r = first_data_row;
    let mut last_leaf: Option<Vec<String>> = None;
    for (line, vj) in &rlines {
        let depth = line.path.len().saturating_sub(1);
        if let Some(j) = vj {
            // a data field's line
            let name = &p.values[*j].name;
            let (text, role, col) = match line.kind {
                LineKind::Grand => (if p.rows.is_empty() { name.clone() } else { format!("Total {}", name) }, Role::GrandLabel, 0),
                LineKind::Total => (format!("{} {}", line.path.last().cloned().unwrap_or_default(), name), Role::Subtotal(depth as u8),
                                    if p.layout == Layout::Compact { 0 } else { depth as u32 }),
                _ => (name.clone(), Role::RowItem(line.path.len() as u8), if p.layout == Layout::Compact { 0 } else { p.rows.len() as u32 }),
            };
            let (text, role) = if p.rows.is_empty() { (name.clone(), Role::RowItem(0)) } else { (text, role) };
            put(&mut out, r, col, s(&text), role, None);
        } else {
        match &line.kind {
            LineKind::Blank => {
                r += 1;
                continue;
            }
            LineKind::Grand => {
                put(&mut out, r, 0, s(if p.rows.is_empty() && nd > 0 { "" } else { "Grand Total" }), Role::GrandLabel, None);
            }
            LineKind::Total => {
                let col = if p.layout == Layout::Compact { 0 } else { depth as u32 };
                put(&mut out, r, col, s(&format!("{} Total", line.path.last().cloned().unwrap_or_default())), Role::Subtotal(depth as u8), None);
            }
            LineKind::Item { .. } => {
                let leaf = line.path.len() == p.rows.len();
                let role = if leaf { Role::RowItem(depth as u8) } else { Role::Subtotal(depth as u8) };
                let col = if p.layout == Layout::Compact { 0 } else { depth as u32 };
                put(&mut out, r, col, s(line.path.last().map(|x| x.as_str()).unwrap_or("")), role, None);
                if p.layout == Layout::Tabular && !p.repeat_labels {
                    // the parents' labels where their group starts
                    let prev = last_leaf.clone().unwrap_or_default();
                    let start = (0..depth).find(|k| prev.get(*k) != line.path.get(*k)).unwrap_or(depth);
                    for k in start..depth {
                        put(&mut out, r, k as u32, s(&line.path[k]), Role::RowItem(k as u8), None);
                    }
                }
                if leaf {
                    last_leaf = Some(line.path.clone());
                }
                if p.repeat_labels && p.layout != Layout::Compact {
                    for (k, t) in line.path.iter().enumerate().take(depth) {
                        put(&mut out, r, k as u32, s(t), Role::RowItem(k as u8), None);
                    }
                }
            }
        }
        }
        let carries = match &line.kind {
            LineKind::Item { carries } => *carries,
            _ => true,
        };
        if carries {
            let role = match line.kind {
                _ if vj.is_some() && p.rows.is_empty() => Role::Data,
                LineKind::Grand => Role::GrandRow,
                LineKind::Total => Role::Subtotal(depth as u8),
                LineKind::Item { .. } if line.path.len() < p.rows.len() => Role::Subtotal(depth as u8),
                _ => Role::Data,
            };
            let mut running: Vec<f64> = vec![0.0; cols.len()];
            let _ = &mut running;
            for (ci, (cl, cj)) in cols.iter().enumerate() {
                if nd == 0 {
                    break;
                }
                let j = &vj.unwrap_or(*cj);
                let v = value_at(&line.path, &cl.path, *j);
                let v = if p.values[*j].show == ShowAs::RunningTotal && line.kind != LineKind::Grand && line.path.len() == 1 {
                    // running total over the first row field's items in order
                    let mut sum = 0.0;
                    for (l2, _) in rlines.iter().filter(|(l, k)| l.path.len() == 1 && matches!(l.kind, LineKind::Item { .. }) && (k.is_none() || k == vj)) {
                        if let Value::Num(x) = value_at(&l2.path, &cl.path, *j) {
                            sum += x;
                        }
                        if l2.path == line.path {
                            break;
                        }
                    }
                    Value::num(sum)
                } else {
                    v
                };
                let c = label_cols + ci as u32;
                let role = if cl.kind == LineKind::Grand && !p.cols.is_empty() { if role == Role::GrandRow { Role::GrandRow } else { Role::GrandCol } } else { role };
                let fmt = p.values[*j].numfmt.clone().or_else(|| match p.values[*j].show {
                    ShowAs::PercentOfTotal | ShowAs::PercentOfCol | ShowAs::PercentOfRow => Some("0.00%".into()),
                    _ => None,
                });
                out.paths.insert((r, c), (line.path.clone(), cl.path.clone()));
                put(&mut out, r, c, v, role, fmt);
            }
        }
        out.row_lines.push((line.path.clone(), format!("{:?}", line.kind).split(' ').next().unwrap_or("").to_string(), *vj));
        r += 1;
    }
    for (l, j) in &cols {
        out.col_lines.push((l.path.clone(), format!("{:?}", l.kind).split(' ').next().unwrap_or("").to_string(), *j));
    }
    out.height = r;
    out.width = label_cols + cols.len().max(1) as u32;
    out
}

impl Pivot {
    /// Grand totals for rows (a column at the right) and for columns (a row at the bottom), as the
    /// desktop spreadsheet names them.
    fn grand_rows_effective(&self) -> bool {
        self.grand_rows
    }
    fn grand_cols_effective(&self) -> bool {
        self.grand_cols
    }
}

// ---------------------------------------------------------------------- in the book

impl Book {
    /// The records of a pivot's source (None when the source is gone).
    pub fn pivot_source(&self, p: &Pivot) -> Option<Source> {
        let (s, rect) = match &p.source_table {
            Some(t) => {
                let (s, i) = self.sheets.iter().enumerate().find_map(|(s, sh)| sh.tables.iter().position(|x| x.name.eq_ignore_ascii_case(t)).map(|i| (s, i)))?;
                let tb = &self.sheets[s].tables[i];
                (s, tb.rect)
            }
            None => (self.sheet_index(&p.source_sheet)?, p.source),
        };
        let headers: Vec<String> = (rect.c1..=rect.c2).map(|c| {
            let v = self.value(s, rect.r1, c);
            match v {
                Value::Empty => format!("Column{}", c - rect.c1 + 1),
                v => crate::calc::to_text(&v).unwrap_or_default(),
            }
        }).collect();
        let mut records = Vec::new();
        let last = if p.source_table.is_some() {
            let tb = self.sheets[s].tables.iter().find(|x| x.rect == rect);
            if tb.and_then(|t| t.totals_row()).is_some() { rect.r2 - 1 } else { rect.r2 }
        } else {
            rect.r2
        };
        for r in rect.r1 + 1..=last {
            let rec: Vec<Value> = (rect.c1..=rect.c2).map(|c| self.value(s, r, c)).collect();
            if rec.iter().all(|v| v.is_empty()) && p.source_table.is_none() {
                continue;
            }
            records.push(rec);
        }
        Some(Source { headers, records })
    }

    pub fn pivot_at(&self, s: usize, r: u32, c: u32) -> Option<usize> {
        self.pivots.iter().position(|p| p.sheet == s && p.out.map(|o| o.contains(r, c)).unwrap_or(false))
    }

    /// Lay a pivot out: its old cells cleared, its new ones written (part of the open undo step).
    fn place_pivot(&mut self, mut p: Pivot, me: Option<usize>) -> Result<Pivot, String> {
        let src = self.pivot_source(&p).ok_or("The PivotTable's source data is gone.")?;
        if src.headers.len() != p.fields.len() {
            // the source grew or shrank: fields by name
            let mut nf: Vec<PField> = src.headers.iter().map(|h| p.fields.iter().find(|f| &f.name == h).cloned()
                .unwrap_or(PField { name: h.clone(), subtotal: true, ..Default::default() })).collect();
            let remap = |i: usize| -> Option<usize> { src.headers.iter().position(|h| Some(h) == p.fields.get(i).map(|f| &f.name)) };
            p.rows = p.rows.iter().filter_map(|i| remap(*i)).collect();
            p.cols = p.cols.iter().filter_map(|i| remap(*i)).collect();
            p.pages = p.pages.iter().filter_map(|(i, sel)| remap(*i).map(|j| (j, sel.clone()))).collect();
            p.values = p.values.iter().filter_map(|d| remap(d.field).map(|j| DataField { field: j, ..d.clone() })).collect();
            for f in nf.iter_mut() {
                f.name = f.name.clone();
            }
            p.fields = nf;
        }
        let out = compute(self, &p, &src);
        let s = p.sheet;
        let (r0, c0) = p.at;
        let r0 = r0.saturating_sub(out.page_rows);
        let rect = Rect::new(r0, c0, r0 + out.height.max(1) - 1, c0 + out.width.max(1) - 1);
        // it mustn't run over another pivot or a table
        for (k, q) in self.pivots.iter().enumerate() {
            if q.sheet == s && Some(k) != me && q.out.map(|o| o.intersects(&rect)).unwrap_or(false) {
                return Err("A PivotTable report cannot overlap another PivotTable report.".into());
            }
        }
        if self.sheets[s].tables.iter().any(|t| t.rect.intersects(&rect)) {
            return Err("A PivotTable report cannot overlap a table.".into());
        }
        // clear what it showed before
        if let Some(old) = p.out {
            for r in old.r1..=old.r2 {
                for c in old.c1..=old.c2 {
                    if !rect.contains(r, c) && self.sheets[s].get(r, c).is_some() {
                        let st = self.sheets[s].get(r, c).map(|x| x.s).unwrap_or(0);
                        self.record(Delta::Cell(s, r, c, if st != 0 { Some(Cell { s: st, ..Default::default() }) } else { None }));
                    }
                }
            }
        }
        let mut roles: HashMap<(u32, u32), Role> = HashMap::new();
        let mut filled: HashSet<(u32, u32)> = HashSet::new();
        for (r, c, v, role, fmt) in &out.cells {
            let (rr, cc) = (r0 + r, c0 + c);
            roles.insert((rr, cc), *role);
            filled.insert((rr, cc));
            let mut st = self.sheets[s].get(rr, cc).map(|x| x.s).unwrap_or_else(|| self.sheets[s].style_at(rr, cc));
            if let Some(f) = fmt {
                let ch = crate::styles::Change { numfmt: Some(f.clone()), ..Default::default() };
                st = self.styles.derive(st, &ch);
            }
            let cell = Cell { v: v.clone(), s: st, ..Default::default() };
            let cur = self.sheets[s].get(rr, cc);
            if cur.map(|x| x.v != cell.v || x.s != cell.s || x.f.is_some()).unwrap_or(true) {
                self.record(Delta::Cell(s, rr, cc, Some(cell)));
            }
        }
        // the rest of its rectangle is empty
        for r in rect.r1..=rect.r2 {
            for c in rect.c1..=rect.c2 {
                if !filled.contains(&(r, c)) {
                    if let Some(cur) = self.sheets[s].get(r, c) {
                        if !cur.v.is_empty() || cur.f.is_some() {
                            let st = cur.s;
                            self.record(Delta::Cell(s, r, c, Some(Cell { s: st, ..Default::default() })));
                        }
                    }
                    roles.insert((r, c), if r < r0 + out.first_data_row { Role::Header } else { Role::Data });
                }
            }
        }
        // the page-field area's blank row isn't part of the table
        if out.page_rows > 0 {
            for c in rect.c1..=rect.c2 {
                roles.remove(&(r0 + out.page_rows - 1, c));
                for k in 0..out.page_rows - 1 {
                    if c > c0 + 1 {
                        roles.remove(&(r0 + k, c));
                    }
                }
            }
        }
        p.out = Some(rect);
        p.dirty = true;
        if p.rows.is_empty() && p.cols.is_empty() && p.values.is_empty() {
            roles.retain(|_, r| matches!(r, Role::PageLabel | Role::PageValue));
        }
        p.roles = Rc::new(roles);
        p.paths = Rc::new(out.paths.iter().map(|(k, v)| ((r0 + k.0, c0 + k.1), v.clone())).collect());
        Ok(p)
    }

    /// A pivot read from the file: its cells stay as the file has them; what each cell is
    /// (for its style, Show Details and the field list) comes from laying it out here, when that
    /// gives what the file shows — else it's shown as read and not refreshed here.
    pub fn adopt_pivot(&mut self, mut p: Pivot) {
        p.dirty = false;
        let file_rect = p.out;
        if p.refreshable {
            if let Some(src) = self.pivot_source(&p) {
                if src.headers.len() == p.fields.len() && src.headers.iter().zip(p.fields.iter()).all(|(h, f)| *h == f.name) {
                    let out = compute(self, &p, &src);
                    let (r0, c0) = (p.at.0.saturating_sub(out.page_rows), p.at.1);
                    let rect = Rect::new(r0, c0, r0 + out.height.max(1) - 1, c0 + out.width.max(1) - 1);
                    let s = p.sheet;
                    let same = Some(rect) == file_rect && out.cells.iter().all(|(r, c, v, _, _)| {
                        let cur = self.sheets.get(s).and_then(|sh| sh.get(r0 + r, c0 + c)).map(|x| x.v.clone()).unwrap_or(Value::Empty);
                        match (&cur, v) {
                            (Value::Num(a), Value::Num(b)) => (a - b).abs() <= 1e-9 * a.abs().max(1.0),
                            _ => item_text(self, &cur) == item_text(self, v) || (cur.is_empty() && v.is_empty()),
                        }
                    });
                    if same {
                        let (_, mut roles) = layout_roles(&out, r0, c0);
                        if p.rows.is_empty() && p.cols.is_empty() && p.values.is_empty() {
                            roles.retain(|_, r| matches!(r, Role::PageLabel | Role::PageValue));
                        }
                        p.roles = Rc::new(roles);
                        p.paths = Rc::new(out.paths.iter().map(|(k, v)| ((r0 + k.0, c0 + k.1), v.clone())).collect());
                        self.pivots.push(p);
                        return;
                    }
                }
            }
        }
        // shown as read: headers above the data, labels to its left, the grand total last
        p.refreshable = false;
        if let (Some(o), Some(t)) = (file_rect, p.table_rect()) {
            let mut roles = HashMap::new();
            let hdr = 1 + p.cols.len() as u32 + if p.values.len() > 1 && !p.values_in_rows { 1 } else { 0 };
            for r in o.r1..=o.r2 {
                for c in o.c1..=o.c2 {
                    let role = if r < t.r1 {
                        if c == o.c1 { Role::PageLabel } else if c == o.c1 + 1 { Role::PageValue } else { continue }
                    } else if r < t.r1 + hdr.min(t.rows()) {
                        Role::Header
                    } else if p.grand_rows && r == t.r2 {
                        if c == t.c1 { Role::GrandLabel } else { Role::GrandRow }
                    } else if c == t.c1 {
                        Role::RowItem(0)
                    } else {
                        Role::Data
                    };
                    if r == t.r1.saturating_sub(1) && !p.pages.is_empty() {
                        continue;
                    }
                    roles.insert((r, c), role);
                }
            }
            p.roles = Rc::new(roles);
        }
        self.pivots.push(p);
    }

    /// Insert › PivotTable: an empty PivotTable over a range, at a cell.
    pub fn create_pivot(&mut self, src_sheet: usize, src: Rect, dest_sheet: usize, at: (u32, u32)) -> Result<usize, String> {
        if src.rows() < 2 {
            return Err("The PivotTable field name is not valid. To create a PivotTable report, you must use data that is organized as a list with labeled columns.".into());
        }
        for c in src.c1..=src.c2 {
            if self.value(src_sheet, src.r1, c).is_empty() {
                return Err("The PivotTable field name is not valid. To create a PivotTable report, you must use data that is organized as a list with labeled columns. If you are changing the name of a PivotTable field, you must type a new name for the field.".into());
            }
        }
        let headers: Vec<String> = (src.c1..=src.c2).map(|c| crate::calc::to_text(&self.value(src_sheet, src.r1, c)).unwrap_or_default()).collect();
        let mut k = 1;
        let mut name;
        loop {
            name = format!("PivotTable{}", k);
            if !self.pivots.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
                break;
            }
            k += 1;
        }
        let sname = self.sheets[src_sheet].name.clone();
        let mut p = Pivot::new(&name, &sname, src, dest_sheet, at, headers);
        // a table as the source: by its name
        if let Some(t) = self.sheets[src_sheet].tables.iter().find(|t| t.rect == src) {
            p.source_table = Some(t.name.clone());
        }
        p.cache_id = self.pivots.iter().map(|q| q.cache_id + 1).max().unwrap_or(1).max(1);
        self.begin("PivotTable", dest_sheet);
        let p = match self.place_pivot(p, None) {
            Ok(p) => p,
            Err(e) => {
                self.abort();
                return Err(e);
            }
        };
        let mut all = self.pivots.clone();
        all.push(p);
        self.record(Delta::Pivots(all));
        self.commit();
        Ok(self.pivots.len() - 1)
    }

    /// Change a pivot (fields, layout, options) and lay it out again, as one undo step.
    pub fn edit_pivot(&mut self, i: usize, label: &str, f: impl FnOnce(&mut Pivot)) -> Result<(), String> {
        let mut p = self.pivots.get(i).cloned().ok_or("No such PivotTable.")?;
        if !p.refreshable {
            return Err("This PivotTable can't be changed here: it uses features this version doesn't have.".into());
        }
        f(&mut p);
        // a field is in one area at a time
        let s = p.sheet;
        self.begin(label, s);
        match self.place_pivot(p, Some(i)) {
            Ok(p) => {
                let mut all = self.pivots.clone();
                all[i] = p;
                self.record(Delta::Pivots(all));
                self.relink_pivot_charts(i);
                self.commit();
                Ok(())
            }
            Err(e) => {
                self.abort();
                Err(e)
            }
        }
    }

    /// The drop-down buttons of the pivots on a sheet: (row, col, filtered, what, index) —
    /// what is "rows" / "cols" (index: the row field, or the first), or "page" (index: the filter).
    pub fn pivot_buttons(&self, s: usize) -> Vec<(u32, u32, bool, &'static str, usize)> {
        let mut out = Vec::new();
        for p in self.pivots.iter().filter(|p| p.sheet == s && p.field_headers) {
            let (o, t) = match (p.out, p.table_rect()) {
                (Some(o), Some(t)) => (o, t),
                _ => continue,
            };
            let hidden = |f: usize| p.fields.get(f).map(|x| !x.hidden.is_empty()).unwrap_or(false);
            for (k, (f, sel)) in p.pages.iter().enumerate() {
                out.push((o.r1 + k as u32, o.c1 + 1, sel.is_some() || hidden(*f), "page", k));
            }
            let vir = p.values.len() > 1 && p.values_in_rows;
            let multi = p.values.len() > 1 && !vir;
            let header_rows = if p.cols.is_empty() { 1 } else { p.cols.len() as u32 + if multi { 1 } else { 0 } + 1 };
            let hr = t.r1 + header_rows - 1;
            let label_cols = match p.layout { Layout::Compact => 1, _ => (p.rows.len() as u32 + vir as u32).max(1) };
            if !p.rows.is_empty() {
                if p.layout == Layout::Compact {
                    out.push((hr, t.c1, p.rows.iter().any(|f| hidden(*f)), "rows", p.rows[0]));
                } else {
                    for (k, f) in p.rows.iter().enumerate() {
                        out.push((hr, t.c1 + k as u32, hidden(*f), "rows", *f));
                    }
                }
            }
            if !p.cols.is_empty() {
                out.push((t.r1, t.c1 + label_cols, p.cols.iter().any(|f| hidden(*f)), "cols", p.cols[0]));
            }
        }
        out
    }

    /// A field's items in the order the pivot lists them (for the filter lists).
    pub fn pivot_field_items(&self, i: usize, f: usize) -> Vec<String> {
        let p = match self.pivots.get(i) {
            Some(p) => p,
            None => return vec![],
        };
        let src = match self.pivot_source(p) {
            Some(s) => s,
            None => return vec![],
        };
        let mut seen = HashSet::new();
        let mut v: Vec<(String, SortKey)> = Vec::new();
        for rec in &src.records {
            let x = rec.get(f).cloned().unwrap_or(Value::Empty);
            let t = item_text(self, &x);
            if seen.insert(t.clone()) {
                v.push((t, sort_key(&x)));
            }
        }
        let rank = |k: &SortKey| match k { SortKey::Num(_) => 0, SortKey::Text(_) => 1, SortKey::Blank => 2 };
        v.sort_by(|a, b| rank(&a.1).cmp(&rank(&b.1)).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
        v.into_iter().map(|x| x.0).collect()
    }

    /// Typing in a PivotTable: a data field's caption renames it; the rest can't be changed.
    pub fn pivot_input(&mut self, i: usize, r: u32, c: u32, text: &str) -> Result<(), String> {
        let p = &self.pivots[i];
        let cur = crate::calc::to_text(&self.value(p.sheet, r, c)).unwrap_or_default();
        if p.roles.get(&(r, c)) == Some(&Role::Header) || p.roles.get(&(r, c)) == Some(&Role::ColItem) || matches!(p.roles.get(&(r, c)), Some(Role::RowItem(_))) {
            if let Some(j) = p.values.iter().position(|d| d.name == cur) {
                let t = text.trim().to_string();
                if t.is_empty() {
                    return Err("Cannot change this part of a PivotTable report.".into());
                }
                if p.values.iter().enumerate().any(|(k, d)| k != j && d.name.eq_ignore_ascii_case(&t)) || p.fields.iter().any(|f| f.name.eq_ignore_ascii_case(&t)) {
                    return Err("PivotTable field name already exists.".into());
                }
                return self.edit_pivot(i, "Rename", |p| p.values[j].name = t);
            }
        }
        Err("Cannot change this part of a PivotTable report.".into())
    }

    /// Move PivotTable: to another place (and sheet).
    pub fn move_pivot(&mut self, i: usize, sheet: usize, at: (u32, u32)) -> Result<(), String> {
        let p = self.pivots.get(i).cloned().ok_or("No such PivotTable.")?;
        if !p.refreshable {
            return Err("This PivotTable can't be changed here: it uses features this version doesn't have.".into());
        }
        self.begin("Move PivotTable", sheet);
        if let Some(o) = p.out {
            for (r, c) in self.sheets[p.sheet].positions_in(o) {
                let st = self.sheets[p.sheet].get(r, c).map(|x| x.s).unwrap_or(0);
                self.record(Delta::Cell(p.sheet, r, c, if st != 0 { Some(Cell { s: st, ..Default::default() }) } else { None }));
            }
        }
        let mut q = p.clone();
        q.sheet = sheet;
        q.out = None;
        // the filters go above the new place, as they were above the old
        q.at = (at.0 + if q.pages.is_empty() { 0 } else { q.pages.len() as u32 + 1 }, at.1);
        // the old place is free now
        let mut all = self.pivots.clone();
        all[i].out = None;
        let keep = std::mem::replace(&mut self.pivots, all);
        match self.place_pivot(q, Some(i)) {
            Ok(q) => {
                self.pivots = keep.clone();
                let mut all = keep;
                all[i] = q;
                self.record(Delta::Pivots(all));
                self.relink_pivot_charts(i);
                self.commit();
                Ok(())
            }
            Err(e) => {
                self.pivots = keep;
                self.abort();
                Err(e)
            }
        }
    }

    /// Refresh: read the source again.
    pub fn refresh_pivot(&mut self, i: usize) -> Result<(), String> {
        self.edit_pivot(i, "Refresh", |_| {})
    }

    pub fn refresh_all_pivots(&mut self) -> Result<(), String> {
        let n = self.pivots.len();
        let s = self.active;
        self.begin_group("Refresh All", s);
        let mut res = Ok(());
        for i in 0..n {
            if self.pivots[i].refreshable {
                if let Err(e) = self.refresh_pivot(i) {
                    res = Err(e);
                }
            }
        }
        self.end_group();
        res
    }

    /// Clear All / delete the PivotTable: its cells emptied.
    pub fn delete_pivot(&mut self, i: usize) -> Result<(), String> {
        let p = self.pivots.get(i).cloned().ok_or("No such PivotTable.")?;
        self.begin("Delete PivotTable", p.sheet);
        if let Some(o) = p.out {
            for r in o.r1..=o.r2 {
                for c in o.c1..=o.c2 {
                    if let Some(cur) = self.sheets[p.sheet].get(r, c) {
                        let st = cur.s;
                        self.record(Delta::Cell(p.sheet, r, c, if st != 0 { Some(Cell { s: st, ..Default::default() }) } else { None }));
                    }
                }
            }
        }
        let mut all = self.pivots.clone();
        all.remove(i);
        self.record(Delta::Pivots(all));
        self.commit();
        Ok(())
    }

    /// Show Details: the records behind a value, on a new sheet in front of the pivot's.
    pub fn pivot_details(&mut self, i: usize, r: u32, c: u32) -> Result<usize, String> {
        let p = self.pivots.get(i).cloned().ok_or("No such PivotTable.")?;
        let (rp, cp) = p.paths.get(&(r, c)).cloned().ok_or("Cannot change this part of a PivotTable report.")?;
        let src = self.pivot_source(&p).ok_or("The PivotTable's source data is gone.")?;
        let keep: Vec<&Vec<Value>> = src.records.iter().filter(|rec| {
            p.rows.iter().zip(rp.iter()).all(|(f, t)| rec.get(*f).map(|v| &item_text(self, v) == t).unwrap_or(false))
                && p.cols.iter().zip(cp.iter()).all(|(f, t)| rec.get(*f).map(|v| &item_text(self, v) == t).unwrap_or(false))
                && p.pages.iter().all(|(f, sel)| sel.as_ref().map(|s| rec.get(*f).map(|v| &item_text(self, v) == s).unwrap_or(false)).unwrap_or(true))
                && p.fields.iter().enumerate().all(|(k, fd)| fd.hidden.is_empty() || !(p.rows.contains(&k) || p.cols.contains(&k) || p.pages.iter().any(|x| x.0 == k) || p.slicer_fields.contains(&k))
                    || rec.get(k).map(|v| !fd.hidden.contains(&item_text(self, v))).unwrap_or(true))
        }).collect();
        let rows: Vec<Vec<Value>> = keep.into_iter().cloned().collect();
        let s = p.sheet;
        self.begin_group("Show Details", s);
        let ns = match self.insert_sheet(s, None) {
            Ok(n) => n,
            Err(e) => {
                self.end_group();
                return Err(e);
            }
        };
        self.begin("Show Details", ns);
        for (c, h) in src.headers.iter().enumerate() {
            self.record(Delta::Cell(ns, 0, c as u32, Some(Cell { v: Value::Str(Rc::from(h.as_str())), ..Default::default() })));
        }
        for (k, rec) in rows.iter().enumerate() {
            for (c, v) in rec.iter().enumerate() {
                if !v.is_empty() {
                    self.record(Delta::Cell(ns, 1 + k as u32, c as u32, Some(Cell { v: v.clone(), ..Default::default() })));
                }
            }
        }
        self.commit();
        let n = rows.len() as u32;
        let _ = self.create_table(ns, Rect::new(0, 0, n.max(1), src.headers.len().max(1) as u32 - 1), true, "TableStyleMedium2");
        self.end_group();
        Ok(ns)
    }

    /// The pivot style's look of a cell in a pivot's area.
    pub fn pivot_cell(&self, p: &Pivot, r: u32, c: u32) -> Option<crate::tables::TCell> {
        let role = *p.roles.get(&(r, c))?;
        let mut out = crate::tables::TCell::default();
        let (fam, k) = pivot_style(&p.style)?;
        let th = &self.styles.theme;
        let idx = (k - 1) % 7;
        let base = if idx == 0 { 0x000000 } else { th[3 + idx as usize] };
        let tint = |t: f64| crate::styles::apply_tint(if idx == 0 { 0x808080 } else { base }, t);
        let thin = "thin";
        let tr = p.table_rect().unwrap_or(Rect::cell(r, c));
        let header_row = |r: u32| r < tr.r1 + if p.cols.is_empty() { 1 } else { p.cols.len() as u32 + 1 + if p.values.len() > 1 { 1 } else { 0 } };
        match role {
            Role::PageLabel | Role::PageValue => {
                out.borders = [Some((thin, tint(0.4))), Some((thin, tint(0.4))), Some((thin, tint(0.4))), Some((thin, tint(0.4)))];
                if role == Role::PageLabel {
                    out.bold = true;
                }
                if fam == "Medium" || fam == "Dark" {
                    out.fill = Some(tint(0.8));
                }
            }
            Role::Header | Role::ColItem => {
                if p.col_headers || role == Role::Header {
                    out.bold = true;
                    match fam {
                        "Light" => {
                            out.fill = Some(tint(0.8));
                            out.borders[3] = Some((thin, tint(0.4)));
                        }
                        "Medium" => {
                            out.fill = Some(base);
                            out.color = Some(0xFFFFFF);
                        }
                        _ => {
                            out.fill = Some(crate::styles::apply_tint(base, -0.5));
                            out.color = Some(0xFFFFFF);
                        }
                    }
                }
            }
            Role::Subtotal(lvl) => {
                out.bold = true;
                if fam != "Light" || lvl == 0 {
                    if lvl == 0 && p.rows.len() > 1 {
                        out.borders[2] = Some((thin, tint(0.4)));
                    }
                }
                if fam == "Medium" && lvl == 0 {
                    out.fill = Some(tint(0.8));
                }
            }
            Role::RowItem(_) => {
                if p.row_headers && p.layout != Layout::Compact {
                    out.bold = false;
                }
            }
            Role::GrandLabel | Role::GrandRow => {
                out.bold = true;
                match fam {
                    "Light" => {
                        out.fill = Some(tint(0.8));
                        out.borders[2] = Some((thin, tint(0.4)));
                    }
                    "Medium" => {
                        out.fill = Some(tint(0.6));
                        out.borders[2] = Some(("double", base));
                    }
                    _ => {
                        out.fill = Some(crate::styles::apply_tint(base, -0.5));
                        out.color = Some(0xFFFFFF);
                    }
                }
            }
            Role::GrandCol => {
                out.bold = true;
            }
            Role::Data => {}
        }
        if p.banded_rows && matches!(role, Role::Data | Role::RowItem(_)) && !header_row(r) && (r - tr.r1) % 2 == 1 {
            out.fill = out.fill.or(Some(tint(0.8)));
        }
        if p.banded_cols && matches!(role, Role::Data) && (c - tr.c1) % 2 == 1 {
            out.fill = out.fill.or(Some(tint(0.8)));
        }
        Some(out)
    }

    /// The indent of a row label (compact form) in pixels at 100%.
    /// A small picture of a PivotTable style (the style gallery).
    pub unsafe fn paint_pivot_style(&self, cr: *mut crate::ffi::cairo_t, style: &str, w: f64, h: f64, banded_rows: bool, banded_cols: bool) {
        use crate::ffi::*;
        let mut p = Pivot::new("", "", Rect::cell(0, 0), 0, (0, 0), vec!["a".into(), "b".into()]);
        p.style = style.to_string();
        p.rows = vec![0];
        p.values = vec![DataField { field: 1, agg: Agg::Sum, name: String::new(), show: ShowAs::Normal, numfmt: None }];
        p.banded_rows = banded_rows;
        p.banded_cols = banded_cols;
        p.out = Some(Rect::new(0, 0, 5, 3));
        let mut roles = HashMap::new();
        for c in 0..4u32 {
            roles.insert((0, c), Role::Header);
            roles.insert((5, c), if c == 0 { Role::GrandLabel } else { Role::GrandRow });
            for r in 1..5u32 {
                let sub = r == 1 || r == 3;
                roles.insert((r, c), match (c, sub) {
                    (0, true) => Role::Subtotal(0),
                    (0, false) => Role::RowItem(1),
                    (_, true) => Role::Subtotal(0),
                    _ => Role::Data,
                });
            }
        }
        p.roles = Rc::new(roles);
        let (nr, nc) = (6u32, 4u32);
        let cw = (w - 2.0) / nc as f64;
        let ch = (h - 2.0) / nr as f64;
        let set = |c: u32| cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
        set(0xFFFFFF);
        cairo_rectangle(cr, 0.0, 0.0, w, h);
        cairo_fill(cr);
        for r in 0..nr {
            for c in 0..nc {
                let tc = self.pivot_cell(&p, r, c).unwrap_or_default();
                let (x, y) = (1.0 + c as f64 * cw, 1.0 + r as f64 * ch);
                if let Some(f) = tc.fill {
                    set(f);
                    cairo_rectangle(cr, x, y, cw, ch);
                    cairo_fill(cr);
                }
                set(tc.color.unwrap_or(0x7F7F7F));
                let ind = if c == 0 && matches!(p.roles.get(&(r, c)), Some(Role::RowItem(_))) { cw * 0.15 } else { 0.0 };
                cairo_rectangle(cr, x + cw * 0.2 + ind, y + ch * 0.4, cw * 0.5, (ch * if tc.bold { 0.3 } else { 0.2 }).max(1.0));
                cairo_fill(cr);
                cairo_set_line_width(cr, 1.0);
                for (k, b) in tc.borders.iter().enumerate() {
                    if let Some((_, col)) = b {
                        set(*col);
                        let (x0, y0, x1, y1) = match k {
                            0 => (x, y, x, y + ch),
                            1 => (x + cw, y, x + cw, y + ch),
                            2 => (x, y, x + cw, y),
                            _ => (x, y + ch, x + cw, y + ch),
                        };
                        cairo_move_to(cr, x0.round() + 0.5, y0.round() + 0.5);
                        cairo_line_to(cr, x1.round() + 0.5, y1.round() + 0.5);
                        cairo_stroke(cr);
                    }
                }
            }
        }
    }

    pub fn pivot_indent(&self, p: &Pivot, r: u32, c: u32) -> f64 {
        if p.layout != Layout::Compact || c != p.at.1 {
            return 0.0;
        }
        match p.roles.get(&(r, c)) {
            Some(Role::RowItem(l)) | Some(Role::Subtotal(l)) => *l as f64 * 12.0,
            _ => 0.0,
        }
    }
}

/// PivotStyleLight1..28, Medium1..28, Dark1..28.
pub fn pivot_style(name: &str) -> Option<(&'static str, u32)> {
    let rest = name.strip_prefix("PivotStyle")?;
    for fam in ["Light", "Medium", "Dark"] {
        if let Some(k) = rest.strip_prefix(fam) {
            let k: u32 = k.parse().ok()?;
            if (1..=28).contains(&k) {
                return Some((match fam { "Light" => "Light", "Medium" => "Medium", _ => "Dark" }, k));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tx(b: &Book, r: u32, c: u32) -> String {
        crate::calc::to_text(&b.value(1, r, c)).unwrap_or_default()
    }

    fn book() -> Book {
        let mut b = Book::new_empty();
        let rows = [["Region", "Product", "Sales", "Units"], ["East", "Apples", "10", "1"], ["West", "Apples", "20", "2"],
                    ["East", "Pears", "30", "3"], ["West", "Pears", "40", "4"], ["East", "Apples", "5", "5"]];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                b.set_input(0, r as u32, c as u32, v).unwrap();
            }
        }
        b
    }

    #[test]
    fn layouts() {
        let mut b = book();
        b.insert_sheet(1, None).unwrap();
        let i = b.create_pivot(0, Rect::new(0, 0, 5, 3), 1, (2, 0)).unwrap();
        b.edit_pivot(i, "Add", |p| {
            p.rows = vec![0];
            p.values = vec![DataField { field: 2, agg: Agg::Sum, name: "Sum of Sales".into(), show: ShowAs::Normal, numfmt: None }];
        }).unwrap();
        assert_eq!((tx(&b, 2, 0), tx(&b, 2, 1)), ("Row Labels".into(), "Sum of Sales".into()));
        assert_eq!((tx(&b, 3, 0), tx(&b, 3, 1)), ("East".into(), "45".into()));
        assert_eq!((tx(&b, 4, 0), tx(&b, 4, 1)), ("West".into(), "60".into()));
        assert_eq!((tx(&b, 5, 0), tx(&b, 5, 1)), ("Grand Total".into(), "105".into()));
        // columns, two rows levels, a filter
        b.edit_pivot(i, "Layout", |p| {
            p.rows = vec![0];
            p.cols = vec![1];
            p.pages = vec![(3, None)];
        }).unwrap();
        // filter at A1:B1, table from A3
        assert_eq!((tx(&b, 0, 0), tx(&b, 0, 1)), ("Units".into(), "(All)".into()));
        assert_eq!((tx(&b, 2, 0), tx(&b, 2, 1)), ("Sum of Sales".into(), "Column Labels".into()));
        assert_eq!((tx(&b, 3, 0), tx(&b, 3, 1), tx(&b, 3, 2), tx(&b, 3, 3)), ("Row Labels".into(), "Apples".into(), "Pears".into(), "Grand Total".into()));
        assert_eq!((tx(&b, 4, 0), tx(&b, 4, 1), tx(&b, 4, 2), tx(&b, 4, 3)), ("East".into(), "15".into(), "30".into(), "45".into()));
        assert_eq!((tx(&b, 6, 0), tx(&b, 6, 3)), ("Grand Total".into(), "105".into()));
        b.edit_pivot(i, "Filter", |p| p.pages = vec![(3, Some("2".into()))]).unwrap();
        assert_eq!((tx(&b, 4, 0), tx(&b, 4, 1)), ("West".into(), "20".into()));
        // average, percent of total
        b.edit_pivot(i, "Values", |p| {
            p.pages.clear();
            p.cols.clear();
            p.rows = vec![0, 1];
            p.values = vec![DataField { field: 2, agg: Agg::Sum, name: "Sum of Sales".into(), show: ShowAs::PercentOfTotal, numfmt: None },
                            DataField { field: 3, agg: Agg::Average, name: "Average of Units".into(), show: ShowAs::Normal, numfmt: None }];
        }).unwrap();
        assert_eq!(tx(&b, 2, 0), "Row Labels");
        assert_eq!((tx(&b, 3, 0), tx(&b, 4, 0), tx(&b, 5, 0)), ("East".into(), "Apples".into(), "Pears".into()));
        match b.value(1, 3, 1) { Value::Num(x) => assert!((x - 45.0 / 105.0).abs() < 1e-12), v => panic!("{:?}", v) }
        match b.value(1, 4, 2) { Value::Num(x) => assert!((x - 3.0).abs() < 1e-12), v => panic!("{:?}", v) }
        // tabular form: subtotals at the bottom
        b.edit_pivot(i, "Layout", |p| p.layout = Layout::Tabular).unwrap();
        assert_eq!((tx(&b, 2, 0), tx(&b, 2, 1)), ("Region".into(), "Product".into()));
        assert_eq!((tx(&b, 3, 0), tx(&b, 3, 1), tx(&b, 4, 1)), ("East".into(), "Apples".into(), "Pears".into()));
        assert_eq!(tx(&b, 5, 0), "East Total");
        // the source changes: Refresh
        b.set_input(0, 1, 2, "100").unwrap();
        assert_eq!(tx(&b, 3, 3), "3");
        b.refresh_pivot(i).unwrap();
        match b.value(1, 3, 2) { Value::Num(x) => assert!((x - 105.0 / 195.0).abs() < 1e-12), v => panic!("{:?}", v) }
        // undo the refresh
        b.undo();
        match b.value(1, 3, 2) { Value::Num(x) => assert!((x - 15.0 / 105.0).abs() < 1e-12), v => panic!("{:?}", v) }
        // details
        b.edit_pivot(i, "Simple", |p| {
            p.layout = Layout::Compact;
            p.rows = vec![0];
            p.values.truncate(1);
            p.values[0].show = ShowAs::Normal;
        }).unwrap();
        let d = b.pivot_details(b.pivot_at(2, 3, 1).unwrap_or(i), 3, 1).unwrap();
        assert_eq!(b.value(d, 0, 0), Value::Str(Rc::from("Region")));
        assert_eq!(b.value(d, 3, 0), Value::Str(Rc::from("East")));
        assert!(b.value(d, 4, 0).is_empty());
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    fn book() -> Book {
        let mut b = Book::new_empty();
        let rows = [["Region", "Product", "Sales", "Units"], ["East", "Apples", "10", "1"], ["West", "Apples", "20", "2"],
                    ["East", "Pears", "30", "3"], ["West", "Pears", "40", "4"], ["East", "Apples", "5", "5"]];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                b.set_input(0, r as u32, c as u32, v).unwrap();
            }
        }
        b.insert_sheet(1, None).unwrap();
        let i = b.create_pivot(0, Rect::new(0, 0, 5, 3), 1, (2, 1)).unwrap();
        b.edit_pivot(i, "x", |p| {
            p.rows = vec![0];
            p.values = vec![DataField { field: 2, agg: Agg::Sum, name: "Sum of Sales".into(), show: ShowAs::Normal, numfmt: None },
                            DataField { field: 3, agg: Agg::Sum, name: "Sum of Units".into(), show: ShowAs::Normal, numfmt: None }];
        }).unwrap();
        b
    }
    fn tx(b: &Book, s: usize, r: u32, c: u32) -> String {
        crate::calc::to_text(&b.value(s, r, c)).unwrap_or_default()
    }

    #[test]
    fn guards_and_moves() {
        let mut b = book();
        // B3:D6
        assert_eq!(b.pivots[0].out, Some(Rect::new(2, 1, 5, 3)));
        assert!(b.set_input(1, 3, 2, "1").is_err());
        b.set_input(1, 2, 2, "Sales total").unwrap();
        assert_eq!(b.pivots[0].values[0].name, "Sales total");
        assert!(b.set_input(1, 2, 3, "Sales total").is_err());
        // rows through it: refused; above it: it moves
        assert!(b.insert_rows(1, 4, 1).is_err());
        assert!(b.delete_rows(1, 3, 1).is_err());
        assert!(b.insert_cells(1, Rect::new(0, 2, 0, 2), true).is_err());
        b.insert_rows(1, 0, 2).unwrap();
        assert_eq!(b.pivots[0].out, Some(Rect::new(4, 1, 7, 3)));
        assert_eq!(tx(&b, 1, 5, 1), "East");
        assert!(b.pivots[0].roles.contains_key(&(5, 1)));
        b.undo();
        assert_eq!(b.pivots[0].out, Some(Rect::new(2, 1, 5, 3)));
        // a row inserted in the source: it grows
        b.insert_rows(0, 3, 1).unwrap();
        assert_eq!(b.pivots[0].source, Rect::new(0, 0, 6, 3));
        b.undo();
        // moved
        b.move_pivot(0, 1, (10, 5)).unwrap();
        assert_eq!(b.pivots[0].out, Some(Rect::new(10, 5, 13, 7)));
        assert_eq!(tx(&b, 1, 11, 5), "East");
        assert_eq!(tx(&b, 1, 3, 1), "");
        b.undo();
        assert_eq!(b.pivots[0].out, Some(Rect::new(2, 1, 5, 3)));
        assert_eq!(tx(&b, 1, 3, 1), "East");
        // cleared in part: refused; all of it: gone
        assert!(b.clear_contents(1, &[Rect::new(3, 1, 3, 3)], "Clear").is_err());
        b.clear_contents(1, &[Rect::new(0, 0, 20, 10)], "Clear").unwrap();
        assert!(b.pivots.is_empty());
        assert_eq!(tx(&b, 1, 3, 1), "");
        b.undo();
        assert_eq!(b.pivots.len(), 1);
        // the pivot's sheet deleted: the pivot goes with it
        b.delete_sheet(1).unwrap();
        assert!(b.pivots.is_empty());
        b.undo();
        assert_eq!(b.pivots.len(), 1);
        // values in the rows
        b.edit_pivot(0, "x", |p| p.values_in_rows = true).unwrap();
        assert_eq!((tx(&b, 1, 3, 1), tx(&b, 1, 4, 1), tx(&b, 1, 5, 1)), ("East".into(), "Sales total".into(), "Sum of Units".into()));
        assert_eq!(tx(&b, 1, 4, 2), "45");
        assert_eq!(tx(&b, 1, 9, 1), "Total Sales total");
        assert_eq!(tx(&b, 1, 10, 2), "15");
    }
}
