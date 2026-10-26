//! Workbook-level tests: typing, calculation, undo, files, drawing.
use crate::book::Book;
use crate::cellref::Rect;
use crate::value::{Value, XlErr};

fn blank() -> Book {
    Book::open("testdata/Blank.xlsx").unwrap()
}

fn n(b: &Book, r: u32, c: u32) -> f64 {
    match b.value(0, r, c) {
        Value::Num(x) => x,
        v => panic!("not a number at {},{}: {:?}", r, c, v),
    }
}

#[test]
fn typing_and_calc() {
    let mut b = blank();
    b.set_input(0, 0, 0, "5").unwrap();
    b.set_input(0, 0, 1, "=A1*2").unwrap();
    assert_eq!(n(&b, 0, 1), 10.0);
    b.set_input(0, 0, 0, "7").unwrap();
    assert_eq!(n(&b, 0, 1), 14.0);
    b.set_input(0, 1, 0, "=SUM(A1:B1)").unwrap();
    assert_eq!(n(&b, 1, 0), 21.0);
    b.set_input(0, 2, 0, "=1/0").unwrap();
    assert_eq!(b.value(0, 2, 0), Value::Err(XlErr::Div0));
    assert_eq!(b.input(0, 0, 1), "=A1*2");
    b.set_input(0, 3, 0, "1/2/2026").unwrap();
    assert_eq!(b.display(0, 3, 0), "1/2/2026");
    assert_eq!(b.input(0, 3, 0), "1/2/2026");
    b.set_input(0, 4, 0, "'=x").unwrap();
    assert_eq!(b.input(0, 4, 0), "'=x");
    assert_eq!(b.value(0, 4, 0), Value::str("=x"));
    b.set_input(0, 5, 0, "$1,234.5").unwrap();
    assert_eq!(b.display(0, 5, 0), "$1,234.50 ");
    b.set_input(0, 6, 0, "12345678901234").unwrap();
    assert_eq!(b.display(0, 6, 0), "1.23457E+13");
    // undo
    assert_eq!(b.undo().unwrap().0, "Typing");
    assert_eq!(b.value(0, 6, 0), Value::Empty);
    b.redo();
    assert_eq!(n(&b, 6, 0), 12345678901234.0);
}

#[test]
fn spills() {
    let mut b = blank();
    b.set_input(0, 0, 0, "=SEQUENCE(3)").unwrap();
    assert_eq!(n(&b, 2, 0), 3.0);
    b.set_input(0, 0, 1, "=SUM(A1#)").unwrap();
    assert_eq!(n(&b, 0, 1), 6.0);
    b.set_input(0, 0, 2, "=A3*10").unwrap();
    assert_eq!(n(&b, 0, 2), 30.0);
    b.set_input(0, 1, 0, "x").unwrap();
    assert_eq!(b.value(0, 0, 0), Value::Err(XlErr::Spill));
    assert_eq!(n(&b, 0, 2), 0.0);
    b.set_input(0, 1, 0, "").unwrap();
    assert_eq!(n(&b, 0, 0), 1.0);
    assert_eq!(n(&b, 0, 2), 30.0);
    b.set_input(0, 0, 0, "=SEQUENCE(4)").unwrap();
    assert_eq!(n(&b, 0, 1), 10.0);
}

#[test]
fn circular_and_sheets() {
    let mut b = blank();
    b.set_input(0, 0, 0, "=B1+1").unwrap();
    b.set_input(0, 0, 1, "=A1+1").unwrap();
    assert_eq!(b.circ.len(), 2);
    assert_eq!(n(&b, 0, 0), 0.0);
    b.rename_sheet(0, "Data").unwrap();
    b.insert_sheet(1, Some("Other")).unwrap();
    b.set_input(1, 0, 0, "=Data!A1+5").unwrap();
    b.set_input(0, 0, 0, "3").unwrap();
    assert_eq!(b.value(1, 0, 0), Value::num(8.0));
    b.rename_sheet(0, "New").unwrap();
    assert_eq!(b.input(1, 0, 0), "=New!A1+5");
    b.delete_sheet(0).unwrap();
    assert_eq!(b.input(0, 0, 0), "=#REF!+5");
    assert_eq!(b.value(0, 0, 0), Value::Err(XlErr::Ref));
    b.undo();
    assert_eq!(b.value(1, 0, 0), Value::num(8.0));
}

#[test]
fn iterate() {
    let mut b = blank();
    b.calc.iterate = true;
    b.calc.count = 50;
    b.set_input(0, 0, 0, "=A1+1").unwrap();
    assert_eq!(n(&b, 0, 0), 50.0);
}

#[test]
fn files_roundtrip() {
    let mut b = Book::open("testdata/ref.xlsx").unwrap();
    assert_eq!(b.value(0, 0, 0), Value::str("Item"));
    assert_eq!(n(&b, 3, 3), 32.5);
    assert_eq!(b.display(0, 1, 3), "15.00");
    assert_eq!(b.sheets[0].merges, vec![Rect::new(9, 0, 10, 2)]);
    b.set_input(0, 12, 5, "=SEQUENCE(2,2)").unwrap();
    b.define_name("rate", "=Sheet1!$C$2", None, None, None).unwrap();
    b.set_input(0, 20, 0, "=rate*2").unwrap();
    assert_eq!(n(&b, 20, 0), 2.5);
    let data = b.to_bytes().unwrap();
    let c = Book::from_bytes(&data).unwrap();
    assert_eq!(c.value(0, 13, 6), Value::num(4.0));
    assert_eq!(c.input(0, 12, 5), "=SEQUENCE(2,2)");
    assert_eq!(c.value(0, 20, 0), Value::num(2.5));
    assert_eq!(c.display(0, 1, 3), "15.00");
    assert_eq!(c.sheets[0].merges, vec![Rect::new(9, 0, 10, 2)]);
    std::fs::write("/tmp/rt.xlsx", &data).unwrap();
}

#[test]
fn geometry_and_render() {
    let b = Book::open("testdata/ref.xlsx").unwrap();
    assert_eq!(b.mdw(0), 7.0);
    assert_eq!(b.col_px(0, 0), 64.0);
    let p = crate::view::ViewParams { width: 300.0, height: 100.0, ..Default::default() };
    let g = b.geometry(0, &p);
    assert_eq!(g.hw, 27.0);
    assert_eq!(g.rows[1], (1, 40.0, 20.0));
    let p = crate::view::ViewParams { width: 640.0, height: 330.0, sel: vec![Rect::new(1, 1, 2, 3)], active: (1, 1), ..Default::default() };
    crate::render::render_png(&b, 0, &p, "/tmp/new_main.png").unwrap();
}

#[test]
fn insert_delete_rows_cols() {
    let mut b = blank();
    for r in 0..5 {
        b.set_input(0, r, 0, &format!("{}", r + 1)).unwrap();
    }
    b.set_input(0, 0, 1, "=SUM(A1:A5)").unwrap();
    b.set_input(0, 4, 1, "=A5*2").unwrap();
    b.insert_sheet(1, Some("S2")).unwrap();
    b.set_input(1, 0, 0, "=Sheet1!A3").unwrap();
    b.insert_rows(0, 2, 2).unwrap();
    assert_eq!(b.input(0, 0, 1), "=SUM(A1:A7)");
    assert_eq!(b.input(0, 6, 1), "=A7*2");
    assert_eq!(b.input(1, 0, 0), "=Sheet1!A5");
    assert_eq!(n(&b, 0, 1), 15.0);
    assert_eq!(b.value(0, 2, 0), Value::Empty);
    b.delete_rows(0, 4, 1).unwrap(); // the row that held 3
    assert_eq!(b.input(1, 0, 0), "=Sheet1!#REF!");
    assert_eq!(n(&b, 0, 1), 12.0);
    b.undo();
    assert_eq!(b.input(1, 0, 0), "=Sheet1!A5");
    b.undo();
    assert_eq!(b.input(0, 0, 1), "=SUM(A1:A5)");
    assert_eq!(n(&b, 2, 0), 3.0);
    b.insert_cols(0, 0, 1).unwrap();
    assert_eq!(b.input(0, 0, 2), "=SUM(B1:B5)");
    b.delete_cols(0, 0, 1).unwrap();
    // cells: shift down only in column A
    b.insert_cells(0, Rect::new(1, 0, 1, 0), true).unwrap();
    assert_eq!(b.input(0, 0, 1), "=SUM(A1:A6)");
    assert_eq!(n(&b, 2, 0), 2.0);
    assert_eq!(b.input(0, 4, 1), "=A6*2");
    b.delete_cells(0, Rect::new(1, 0, 1, 0), true).unwrap();
    assert_eq!(b.input(0, 0, 1), "=SUM(A1:A5)");
    // merges move; a merge split by a cell shift is refused
    b.set_merges_cmd(0, vec![Rect::new(10, 0, 10, 1)], "m");
    b.insert_rows(0, 5, 1).unwrap();
    assert_eq!(b.sheets[0].merges, vec![Rect::new(11, 0, 11, 1)]);
    assert!(b.insert_cells(0, Rect::new(11, 0, 11, 0), true).is_err());
}

#[test]
fn ref_error_with_sheet() {
    let mut b = blank();
    b.set_input(0, 0, 0, "=Sheet1!#REF!").unwrap();
    assert_eq!(b.value(0, 0, 0), Value::Err(XlErr::Ref));
}

#[test]
fn sort_filter_dedupe() {
    let mut b = blank();
    let data = [("Name", "Qty"), ("pear", "7"), ("apple", "12"), ("fig", "3"), ("apple", "12"), ("kiwi", "")];
    for (r, (a, q)) in data.iter().enumerate() {
        b.set_input(0, r as u32, 0, a).unwrap();
        if !q.is_empty() {
            b.set_input(0, r as u32, 1, q).unwrap();
        }
        b.set_input(0, r as u32, 2, &format!("=B{}*2", r + 1)).unwrap();
    }
    let rc = Rect::new(0, 0, 5, 2);
    assert!(b.guess_header(0, rc));
    b.sort_range(0, rc, &[crate::book_data::SortKey { line: 1, ascending: true }], true, false).unwrap();
    assert_eq!(b.value(0, 1, 0), Value::str("fig"));
    assert_eq!(b.value(0, 5, 0), Value::str("kiwi")); // blank last
    assert_eq!(b.input(0, 1, 2), "=B2*2");
    assert_eq!(n(&b, 1, 2), 6.0);
    b.sort_range(0, rc, &[crate::book_data::SortKey { line: 0, ascending: false }], true, false).unwrap();
    assert_eq!(b.value(0, 1, 0), Value::str("pear"));
    // filter
    b.set_autofilter(0, Some(rc)).unwrap();
    b.filter_column(0, 0, Some(crate::book_data::FilterKind::Values { values: vec!["apple".into()], blank: false })).unwrap();
    let hidden: Vec<u32> = (1..6).filter(|r| b.row_px(0, *r) == 0.0).collect();
    assert_eq!(hidden.len(), 3);
    let (vals, blank) = b.filter_values(0, 0);
    assert_eq!(vals, vec!["apple", "fig", "kiwi", "pear"]);
    assert!(!blank);
    b.filter_column(0, 1, Some(crate::book_data::FilterKind::Custom { and: true, conds: vec![("greaterThan".into(), "5".into())] })).unwrap();
    let data = b.to_bytes().unwrap();
    let c = Book::from_bytes(&data).unwrap();
    assert_eq!(c.sheets[0].autofilter, b.sheets[0].autofilter);
    b.clear_filters(0).unwrap();
    assert!((1..6).all(|r| b.row_px(0, r) > 0.0));
    b.set_autofilter(0, None).unwrap();
    // remove duplicates
    let (removed, left) = b.remove_duplicates(0, rc, &[0, 1], true).unwrap();
    assert_eq!((removed, left), (1, 4));
    assert_eq!(b.value(0, 5, 0), Value::Empty);
}

#[test]
fn clipboard() {
    use crate::book_clip::{Op, PasteOpts, What};
    let mut b = blank();
    b.set_input(0, 0, 0, "1").unwrap();
    b.set_input(0, 1, 0, "2").unwrap();
    b.set_input(0, 0, 1, "=A1*10").unwrap();
    b.set_input(0, 1, 1, "=A2*10").unwrap();
    b.set_input(0, 5, 5, "=B1+1").unwrap();
    // copy B1:B2 to D1: relative refs move
    let out = b.paste_cells(0, Rect::new(0, 1, 1, 1), 0, Rect::cell(0, 3), PasteOpts::default(), false).unwrap();
    assert_eq!(out, Rect::new(0, 3, 1, 3));
    assert_eq!(b.input(0, 0, 3), "=C1*10");
    // values
    b.paste_cells(0, Rect::new(0, 1, 1, 1), 0, Rect::cell(0, 4), PasteOpts { what: What::Values, ..Default::default() }, false).unwrap();
    assert_eq!(b.input(0, 1, 4), "20");
    // operation: add A1:A2 onto E1:E2
    b.paste_cells(0, Rect::new(0, 0, 1, 0), 0, Rect::cell(0, 4), PasteOpts { what: What::Values, op: Op::Add, ..Default::default() }, false).unwrap();
    assert_eq!(n(&b, 1, 4), 22.0);
    // transpose
    b.paste_cells(0, Rect::new(0, 0, 1, 1), 0, Rect::cell(10, 0), PasteOpts { transpose: true, ..Default::default() }, false).unwrap();
    assert_eq!(b.input(0, 10, 1), "2");
    assert_eq!(b.input(0, 11, 0), "=XFD12*10");
    // tiling: copy A1 into A20:A22
    b.paste_cells(0, Rect::cell(0, 0), 0, Rect::new(19, 0, 21, 0), PasteOpts::default(), false).unwrap();
    assert_eq!(b.input(0, 21, 0), "1");
    // cut moves and references follow
    b.paste_cells(0, Rect::new(0, 1, 0, 1), 0, Rect::cell(30, 1), PasteOpts::default(), true).unwrap();
    assert_eq!(b.input(0, 5, 5), "=B31+1");
    assert_eq!(b.input(0, 30, 1), "=A1*10");
    assert_eq!(b.value(0, 0, 1), Value::Empty);
    b.undo();
    assert_eq!(b.input(0, 5, 5), "=B1+1");
    // across workbooks
    let bytes = b.copy_to_bytes(0, Rect::new(0, 0, 1, 1)).unwrap();
    let clip = Book::from_bytes(&bytes).unwrap();
    let mut c = blank();
    c.paste_from(&clip, 0, Rect::new(0, 0, 1, 1), 0, Rect::cell(4, 4), PasteOpts::default()).unwrap();
    assert_eq!(c.input(0, 4, 5), "=E5*10");
    assert_eq!(c.value(0, 5, 5), Value::num(20.0));
    c.paste_from(&clip, 0, Rect::new(0, 0, 1, 1), 0, Rect::cell(10, 0), PasteOpts { what: What::Values, ..Default::default() }).unwrap();
    assert_eq!(c.input(0, 11, 1), "20");
}

#[test]
fn fills() {
    let mut b = blank();
    b.set_input(0, 0, 0, "1").unwrap();
    b.set_input(0, 1, 0, "3").unwrap();
    b.set_input(0, 0, 1, "Mon").unwrap();
    b.set_input(0, 0, 2, "Item 9").unwrap();
    b.set_input(0, 0, 3, "1/15/2026").unwrap();
    b.set_input(0, 1, 3, "2/15/2026").unwrap();
    b.autofill(0, Rect::new(0, 3, 1, 3), Rect::new(0, 3, 3, 3), true, false, false).unwrap();
    b.set_input(0, 0, 4, "=A1*2").unwrap();
    b.set_input(0, 0, 5, "x").unwrap();
    b.autofill(0, Rect::new(0, 0, 1, 0), Rect::new(0, 0, 4, 0), true, false, false).unwrap();
    assert_eq!(n(&b, 4, 0), 9.0);
    b.autofill(0, Rect::new(0, 1, 0, 2), Rect::new(0, 1, 3, 2), true, false, false).unwrap();
    b.autofill(0, Rect::new(0, 4, 0, 5), Rect::new(0, 4, 3, 5), true, false, false).unwrap();
    assert_eq!(b.value(0, 3, 1), Value::str("Thu"));
    assert_eq!(b.value(0, 2, 2), Value::str("Item 11"));
    assert_eq!(b.input(0, 3, 4), "=A4*2");
    assert_eq!(b.value(0, 3, 5), Value::str("x"));
    assert_eq!(b.display(0, 3, 3), "4/15/2026");
    // up
    b.set_input(0, 10, 7, "5").unwrap();
    b.set_input(0, 11, 7, "6").unwrap();
    b.autofill(0, Rect::new(10, 7, 11, 7), Rect::new(8, 7, 11, 7), true, false, false).unwrap();
    assert_eq!(n(&b, 8, 7), 3.0);
    // copy (Ctrl+drag)
    b.autofill(0, Rect::new(0, 0, 1, 0), Rect::new(0, 0, 5, 0), false, false, false).unwrap();
    assert_eq!(n(&b, 5, 0), 3.0);
    // series dialog
    b.set_input(0, 20, 0, "2").unwrap();
    let k = b.fill_series(0, Rect::new(20, 0, 30, 0), false, "growth", "day", 3.0, Some(500.0), false).unwrap();
    assert_eq!(k, 5);
    assert_eq!(n(&b, 25, 0), 486.0);
    // flash fill
    b.set_input(0, 40, 0, "John Smith").unwrap();
    b.set_input(0, 41, 0, "Ada Lovelace").unwrap();
    b.set_input(0, 42, 0, "Alan Turing").unwrap();
    b.set_input(0, 40, 1, "Smith").unwrap();
    assert_eq!(b.flash_fill(0, 40, 1).unwrap(), 2);
    assert_eq!(b.value(0, 42, 1), Value::str("Turing"));
}

#[test]
fn charts_read_and_write() {
    use crate::book_chart::kind_name;
    let b = Book::open("testdata/charts.xlsx").unwrap();
    let kinds: Vec<String> = b.sheets[0].objects.iter().filter_map(|o| o.chart.as_ref().map(|c| kind_name(c))).collect();
    assert_eq!(kinds, vec!["col", "line-markers", "pie", "scatter", "combo", "bar-stacked", "doughnut", "area"], "{:?}", kinds);
    assert_eq!(b.sheets[0].objects.iter().filter(|o| o.kind() == "picture").count(), 1);
    let ch = b.chart(0, 1).unwrap();
    assert_eq!(b.chart_title_text(ch).as_deref(), Some("Sales and costs"));
    let s0 = ch.plots[0].series[0].clone();
    assert_eq!(b.series_name(&s0, 0), "Sales");
    let v: Vec<Option<f64>> = b.chart_values(s0.val.as_ref().unwrap()).iter().map(|p| p.v).collect();
    assert_eq!(v, vec![Some(120.0), Some(150.0), Some(90.0), Some(180.0)]);
    assert!(b.chart(0, 5).unwrap().plots[1].secondary);
    // unchanged charts come back byte for byte
    let bytes = b.to_bytes().unwrap();
    let b2 = Book::from_bytes(&bytes).unwrap();
    assert_eq!(b2.sheets[0].objects.len(), 9);
    assert_eq!(b2.sheets[1].objects.len(), 1);
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    let ob = std::fs::read("testdata/charts.xlsx").unwrap();
    let orig = crate::zip::ZipReader::open(&ob).unwrap();
    assert!(z.read("xl/media/image1.png").is_ok());
    let a = String::from_utf8(z.read("xl/charts/chart1.xml").unwrap()).unwrap();
    let o = String::from_utf8(orig.read("xl/charts/chart1.xml").unwrap()).unwrap();
    assert_eq!(a.replace("\r\n", "\n").trim_start_matches(|c| c != '<').len() > 0, true);
    assert!(a.contains("<c:f>Data!$B$2:$B$5</c:f>") && o.contains("<c:f>Data!$B$2:$B$5</c:f>"));
}

#[test]
fn charts_follow_edits() {
    let mut b = Book::open("testdata/charts.xlsx").unwrap();
    let before = b.obj_rect(0, &b.sheets[0].objects[1].clone());
    b.insert_rows(0, 1, 2).unwrap();
    let ch = b.chart(0, 1).unwrap();
    assert_eq!(ch.plots[0].series[0].val.as_ref().unwrap().f.as_deref(), Some("Data!$B$4:$B$7"));
    let after = b.obj_rect(0, &b.sheets[0].objects[1].clone());
    assert!(after.1 > before.1, "the chart moves down with its rows");
    // the other sheet's chart follows too
    assert_eq!(b.chart(1, 0).unwrap().plots[0].series[0].val.as_ref().unwrap().f.as_deref(), Some("Data!$B$4:$B$7"));
    b.rename_sheet(0, "Figures").unwrap();
    assert_eq!(b.chart(1, 0).unwrap().plots[0].series[0].val.as_ref().unwrap().f.as_deref(), Some("Figures!$B$4:$B$7"));
    let bytes = b.to_bytes().unwrap();
    let _ = std::fs::write("/tmp/charts_edited.xlsx", &bytes);
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    let a = String::from_utf8(z.read("xl/charts/chart1.xml").unwrap()).unwrap();
    assert!(a.contains("<c:f>Figures!$B$4:$B$7</c:f>"), "{}", &a[..400.min(a.len())]);
    b.undo();
    b.undo();
    assert_eq!(b.chart(0, 1).unwrap().plots[0].series[0].val.as_ref().unwrap().f.as_deref(), Some("Data!$B$2:$B$5"));
}

#[test]
fn charts_insert_change_delete() {
    use crate::book_chart::kind_name;
    let mut b = blank();
    for (r, row) in [["", "North", "South"], ["Q1", "10", "20"], ["Q2", "15", "25"], ["Q3", "12", "30"]].iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            b.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    let i = b.insert_chart(0, Rect::new(0, 0, 3, 2), "col", 300.0, 20.0).unwrap();
    let ch = b.chart(0, i).unwrap().clone();
    assert_eq!(ch.plots[0].series.len(), 2);
    assert_eq!(b.series_name(&ch.plots[0].series[1], 1), "South");
    assert_eq!(ch.plots[0].series[0].cat.as_ref().unwrap().f.as_deref(), Some("Sheet1!$A$2:$A$4"));
    assert_eq!(b.chart_title_text(&ch).as_deref(), Some("Chart Title"));
    assert_eq!(b.chart_range_text(0, i).as_deref(), Some("Sheet1!$A$1:$C$4"));
    b.switch_chart_rows(0, i).unwrap();
    let ch = b.chart(0, i).unwrap().clone();
    assert_eq!(ch.plots[0].series.len(), 3);
    assert_eq!(b.series_name(&ch.plots[0].series[0], 0), "Q1");
    b.set_chart_kind(0, i, "line-markers").unwrap();
    assert_eq!(kind_name(b.chart(0, i).unwrap()), "line-markers");
    b.set_chart_kind(0, i, "combo").unwrap();
    assert_eq!(b.chart(0, i).unwrap().plots.len(), 2);
    // saved and read back
    let bytes = b.to_bytes().unwrap();
    let _ = std::fs::write("/tmp/charts_new.xlsx", &bytes);
    let b2 = Book::from_bytes(&bytes).unwrap();
    assert_eq!(kind_name(b2.chart(0, 0).unwrap()), "combo");
    assert_eq!(b2.chart(0, 0).unwrap().all_series().count(), 3);
    // moved, then deleted: the parts go
    b.set_object_rect(0, i, 10.0, 200.0, 400.0, 250.0, "Move Chart").unwrap();
    let r = b.obj_rect(0, &b.sheets[0].objects[i].clone());
    assert!((r.0 - 10.0).abs() < 1.0 && (r.1 - 200.0).abs() < 1.0 && (r.2 - 400.0).abs() < 1.0, "{:?}", r);
    b.delete_objects(0, &[i]).unwrap();
    let bytes = b.to_bytes().unwrap();
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    assert!(!z.names().iter().any(|n| n.contains("chart") || n.contains("drawing")), "{:?}", z.names());
    b.undo();
    assert_eq!(b.sheets[0].objects.len(), 1);
    // a chart reading the file back after the file had one
    let b3 = Book::from_bytes(&b.to_bytes().unwrap()).unwrap();
    assert_eq!(b3.sheets[0].objects.len(), 1);
}

#[test]
fn charts_render() {
    let b = Book::open("testdata/charts.xlsx").unwrap();
    let mut p = crate::view::ViewParams::default();
    p.width = 1600.0;
    p.height = 900.0;
    crate::render::render_png(&b, 0, &p, "/tmp/charts_test.png").unwrap();
}

#[test]
fn conditional_formats() {
    let b = Book::open("testdata/cf.xlsx").unwrap();
    let blocks = b.cf_blocks(0);
    assert_eq!(blocks.len(), 11);
    let at = |r: u32, c: u32| b.cf_at(0, r, c, &blocks);
    // A: > 5 (values 3,8,1,9,5,7,2,6,4,10)
    assert_eq!(at(1, 0).unwrap().dxf.fill, Some(0xFFC7CE));
    assert!(at(0, 0).is_none());
    assert!(at(4, 0).is_none(), "5 is not greater than 5");
    // B: top 3 = 10, 9, 8
    assert_eq!(at(9, 1).unwrap().dxf.bold, Some(true));
    assert!(at(1, 1).is_some() && at(5, 1).is_none());
    // C: above the average (5.5)
    assert!(at(5, 2).is_some() && at(4, 2).is_none());
    // D: colour scale: lowest red, highest green
    assert_eq!(at(2, 3).unwrap().scale, Some(0xF8696B));
    assert_eq!(at(9, 3).unwrap().scale, Some(0x63BE7B));
    // E: 2007 bar, 10 % to 90 %
    let (a, e, _, _, _) = at(9, 4).unwrap().bar.unwrap();
    assert!(a == 0.0 && (e - 0.9).abs() < 1e-9, "{} {}", a, e);
    // F: arrows by thirds
    assert_eq!(at(2, 5).unwrap().icon, Some(("3Arrows".to_string(), 0)));
    assert_eq!(at(9, 5).unwrap().icon, Some(("3Arrows".to_string(), 2)));
    // G: 2010 solid bar with negatives around an axis (values -2..5)
    let (a, e, bar, neg, axis) = at(2, 6).unwrap().bar.unwrap();
    assert!(!bar.gradient && neg && axis.is_some() && a < e, "{:?}", (a, e, neg, axis));
    // H: formula
    assert!(at(1, 7).is_some() && at(0, 7).is_none());
    // I: duplicates
    assert!(at(0, 8).is_some() && at(3, 8).is_none());
    // K: contains "an"
    assert!(at(0, 10).is_some() && at(2, 10).is_some() && at(1, 10).is_none());
    let mut p = crate::view::ViewParams::default();
    p.width = 900.0;
    p.height = 260.0;
    crate::render::render_png(&b, 0, &p, "/tmp/cf_test.png").unwrap();
}

#[test]
fn conditional_format_edits() {
    use crate::cf::RuleSpec;
    let mut b = blank();
    for r in 0..6 {
        b.set_input(0, r, 0, &format!("{}", r * 2)).unwrap();
    }
    let sp = RuleSpec { typ: "cellIs".into(), operator: "greaterThan".into(), values: vec!["5".into()], fill: Some(0xFFC7CE), font_color: Some(0x9C0006), ..Default::default() };
    b.add_cf_rule(0, &[Rect::new(0, 0, 5, 0)], &sp).unwrap();
    let blocks = b.cf_blocks(0);
    assert!(b.cf_at(0, 3, 0, &blocks).is_some() && b.cf_at(0, 2, 0, &blocks).is_none());
    // a formula rule, relative to the first cell
    let sp2 = RuleSpec { typ: "expression".into(), values: vec!["=A1=0".into()], bold: true, ..Default::default() };
    b.add_cf_rule(0, &[Rect::new(0, 0, 5, 0)], &sp2).unwrap();
    let blocks = b.cf_blocks(0);
    assert_eq!(blocks[1].rules[0].priority, 1, "the newest rule comes first");
    assert_eq!(b.cf_at(0, 0, 0, &blocks).unwrap().dxf.bold, Some(true));
    assert!(b.cf_at(0, 1, 0, &blocks).map(|o| o.dxf.bold.is_none()).unwrap_or(true));
    // rows inserted above move the rules
    b.insert_rows(0, 0, 1).unwrap();
    let blocks = b.cf_blocks(0);
    assert_eq!(blocks[0].ranges[0], Rect::new(1, 0, 6, 0));
    assert_eq!(b.cf_at(0, 1, 0, &blocks).unwrap().dxf.bold, Some(true));
    b.undo();
    // clearing part of a range
    b.clear_cf(0, Some(&[Rect::new(0, 0, 1, 0)])).unwrap();
    let blocks = b.cf_blocks(0);
    assert_eq!(blocks[0].ranges, vec![Rect::new(2, 0, 5, 0)]);
    // saved and read back
    let b2 = Book::from_bytes(&b.to_bytes().unwrap()).unwrap();
    let bl2 = b2.cf_blocks(0);
    assert_eq!(bl2.len(), 2);
    assert_eq!(b2.cf_at(0, 3, 0, &bl2).unwrap().dxf.fill, Some(0xFFC7CE));
    b.clear_cf(0, None).unwrap();
    assert!(b.cf_blocks(0).is_empty());
    b.undo();
    assert_eq!(b.cf_blocks(0).len(), 2);
}

#[test]
fn tables_from_file() {
    let b = Book::open("testdata/table.xlsx").unwrap();
    let t = &b.sheets[0].tables[0];
    assert_eq!(t.name, "Sales");
    assert!(t.totals && t.header);
    assert_eq!(t.style.as_deref(), Some("TableStyleMedium9"));
    // [@Qty]*[@Price], the totals and a reference from outside
    assert_eq!(b.value(0, 2, 4), Value::Num(25.0));
    assert_eq!(b.value(0, 5, 2), Value::Num(45.0));
    assert_eq!(b.value(0, 5, 4), Value::Num(25.0 + 60.0 + 60.0));
    assert_eq!(b.value(0, 1, 6), Value::Num(145.0));
    assert_eq!(b.input(0, 2, 4), "=[@Qty]*[@Price]");
    // saved: the references are qualified again
    let bytes = b.to_bytes().unwrap();
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    let sx = String::from_utf8(z.read("xl/worksheets/sheet1.xml").unwrap()).unwrap();
    assert!(sx.contains("Sales[[#This Row],[Qty]]*Sales[[#This Row],[Price]]"), "{}", &sx[..900]);
    assert!(sx.contains("<tableParts count=\"1\">"));
    let b2 = Book::from_bytes(&bytes).unwrap();
    assert_eq!(b2.value(0, 1, 6), Value::Num(145.0));
    assert_eq!(b2.sheets[0].tables[0].cols[3].calc.as_deref(), Some("Sales[[#This Row],[Qty]]*Sales[[#This Row],[Price]]"));
}

#[test]
fn tables_commands() {
    let mut b = blank();
    for (r, row) in [["Item", "Qty", "Price"], ["Pens", "3", "1.5"], ["Ink", "2", "4"], ["Pads", "5", "2"]].iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            b.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    let i = b.create_table(0, Rect::new(1, 1, 1, 1), true, "TableStyleMedium2").unwrap();
    assert_eq!(b.sheets[0].tables[i].rect, Rect::new(0, 0, 3, 2));
    assert_eq!(b.sheets[0].tables[i].name, "Table1");
    // a calculated column: typing a formula next to the table adds a column and fills it
    b.set_input(0, 0, 3, "Amount").unwrap();
    assert_eq!(b.sheets[0].tables[0].rect.c2, 3, "typing beside the table makes it wider");
    b.set_input(0, 1, 3, "=[@Qty]*[@Price]").unwrap();
    assert_eq!(b.value(0, 1, 3), Value::Num(4.5));
    assert_eq!(b.value(0, 3, 3), Value::Num(10.0), "the formula filled the column");
    // a new row below: the table grows and the formula comes along
    b.set_input(0, 4, 0, "Clips").unwrap();
    b.set_input(0, 4, 1, "10").unwrap();
    b.set_input(0, 4, 2, "0.1").unwrap();
    assert_eq!(b.sheets[0].tables[0].rect.r2, 4);
    assert_eq!(b.value(0, 4, 3), Value::Num(1.0));
    // a total row
    b.set_table_option(0, 0, "totals", true).unwrap();
    assert_eq!(b.value(0, 5, 3), Value::Num(4.5 + 8.0 + 10.0 + 1.0));
    assert_eq!(b.display(0, 5, 0), "Total");
    b.set_table_total(0, 0, 1, "average").unwrap();
    assert_eq!(b.value(0, 5, 1), Value::Num(5.0));
    // formulas outside
    b.set_input(0, 8, 0, "=SUM(Table1[Qty])").unwrap();
    assert_eq!(b.value(0, 8, 0), Value::Num(20.0));
    b.set_input(0, 9, 0, "=ROWS(Table1[#All])").unwrap();
    assert_eq!(b.value(0, 9, 0), Value::Num(6.0));
    // renaming the column header follows into the formulas
    b.set_input(0, 0, 1, "Count").unwrap();
    assert_eq!(b.input(0, 8, 0), "=SUM(Table1[Count])");
    assert_eq!(b.input(0, 1, 3), "=[@Count]*[@Price]");
    assert_eq!(b.value(0, 8, 0), Value::Num(20.0));
    // and renaming the table
    b.rename_table(0, 0, "Orders").unwrap();
    assert_eq!(b.input(0, 8, 0), "=SUM(Orders[Count])");
    // rows inserted inside make it taller
    b.insert_rows(0, 2, 1).unwrap();
    assert_eq!(b.sheets[0].tables[0].rect, Rect::new(0, 0, 6, 3));
    assert_eq!(b.value(0, 9, 0), Value::Num(20.0));
    b.undo();
    // saved and read back
    let b2 = Book::from_bytes(&b.to_bytes().unwrap()).unwrap();
    assert_eq!(b2.sheets[0].tables[0].name, "Orders");
    assert_eq!(b2.value(0, 8, 0), Value::Num(20.0));
    assert_eq!(b2.value(0, 3, 3), Value::Num(10.0));
    // convert to a range: plain references
    b.convert_table(0, 0).unwrap();
    assert!(b.sheets[0].tables.is_empty());
    assert_eq!(b.input(0, 8, 0), "=SUM($B$2:$B$5)");
    assert_eq!(b.value(0, 8, 0), Value::Num(20.0));
    b.undo();
    assert_eq!(b.sheets[0].tables.len(), 1);
    // the style draws
    let mut p = crate::view::ViewParams::default();
    p.width = 500.0;
    p.height = 200.0;
    crate::render::render_png(&b, 0, &p, "/tmp/table_test.png").unwrap();
}

#[test]
fn notes_and_comments() {
    let mut b = Book::open("testdata/notes.xlsx").unwrap();
    let n = b.note(0, 0, 0).unwrap().clone();
    assert_eq!(n.author, "Ron");
    assert_eq!(n.text, "Ron:\nCheck this value");
    assert!(!n.visible);
    let n2 = b.note(0, 2, 1).unwrap().clone();
    assert!(n2.visible && n2.w > crate::notes::NOTE_W, "{:?}", n2);
    // unchanged: the parts are written back as they were (the button too)
    let bytes = b.to_bytes().unwrap();
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    let ob = std::fs::read("testdata/notes.xlsx").unwrap();
    let zo = crate::zip::ZipReader::open(&ob).unwrap();
    assert_eq!(z.read("xl/drawings/vmlDrawing1.vml").unwrap(), zo.read("xl/drawings/vmlDrawing1.vml").unwrap());
    // edits
    b.set_note(0, 4, 4, "Ron", Some("Ron:\nNew one")).unwrap();
    b.set_note(0, 0, 0, "Ron", None).unwrap();
    b.add_comment(0, 6, 0, "Ron", "Is this right?").unwrap();
    b.add_comment(0, 6, 0, "Ana", "Yes").unwrap();
    assert!(b.add_comment(0, 4, 4, "Ron", "x").is_err(), "a note and a comment can't share a cell");
    assert_eq!(b.thread(0, 6, 0).unwrap().entries.len(), 2);
    let bytes = b.to_bytes().unwrap();
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    let vml = String::from_utf8(z.read("xl/drawings/vmlDrawing1.vml").unwrap()).unwrap();
    assert!(vml.contains("Press") || vml.contains("Button"), "the button stays");
    assert!(z.names().iter().any(|n| n.starts_with("xl/threadedComments/")), "{:?}", z.names());
    assert!(z.names().iter().any(|n| n == "xl/persons/person.xml"));
    let b2 = Book::from_bytes(&bytes).unwrap();
    assert!(b2.note(0, 0, 0).is_none());
    assert_eq!(b2.note(0, 4, 4).unwrap().text, "Ron:\nNew one");
    let t = b2.thread(0, 6, 0).unwrap();
    assert_eq!(t.entries.len(), 2);
    assert_eq!(t.entries[1].author, "Ana");
    assert_eq!(t.entries[0].text, "Is this right?");
    // rows inserted above move them
    b.insert_rows(0, 0, 2).unwrap();
    assert!(b.note(0, 6, 4).is_some() && b.thread(0, 8, 0).is_some());
    b.undo();
    // notes to comments
    assert_eq!(b.notes_to_comments(0).unwrap(), 2);
    assert!(b.sheets[0].notes.is_empty());
    assert_eq!(b.thread(0, 4, 4).unwrap().entries[0].text, "New one");
    // the picture
    let mut p = crate::view::ViewParams::default();
    p.width = 600.0;
    p.height = 260.0;
    b.undo();
    p.hover_note = Some((4, 4));
    crate::render::render_png(&b, 0, &p, "/tmp/notes_test.png").unwrap();
}

#[test]
fn page_setup_and_pages() {
    let mut b = blank();
    for r in 0..120 {
        for c in 0..14 {
            b.set_input(0, r, c, &format!("{}", r * 100 + c)).unwrap();
        }
    }
    let mut p = b.page_setup(0);
    assert_eq!(p.paper, 1);
    let pages = b.pages(0);
    // 14 columns of 64 px (48 pt) at 100 %: Letter is 7.1 in wide inside the margins
    assert!(pages.len() >= 4, "{}", pages.len());
    assert_eq!(pages[0].rows.0, 0);
    assert_eq!(pages[0].cols.0, 0);
    // fit to one page wide
    p.fit = Some((1, 0));
    p.landscape = true;
    p.title_rows = Some((0, 0));
    p.header = "&CQuarterly&R&P of &N".into();
    p.row_breaks = vec![60];
    b.set_page_setup(0, &p, "Page Setup").unwrap();
    let p2 = b.page_setup(0);
    assert_eq!(p2.fit, Some((1, 0)));
    assert!(p2.landscape);
    assert_eq!(p2.title_rows, Some((0, 0)));
    let pages = b.pages(0);
    assert!(pages.iter().all(|pg| pg.cols == (0, 13)), "one page wide: {:?}", pages);
    assert!(pages.iter().any(|pg| pg.rows.0 == 60), "the manual break: {:?}", pages);
    // saved and read back
    let b2 = Book::from_bytes(&b.to_bytes().unwrap()).unwrap();
    let p3 = b2.page_setup(0);
    assert_eq!(p3.fit, Some((1, 0)));
    assert_eq!(p3.header, "&CQuarterly&R&P of &N");
    assert_eq!(p3.row_breaks, vec![60]);
    assert_eq!(p3.title_rows, Some((0, 0)));
    // rows inserted above the break move it
    b.insert_rows(0, 10, 2).unwrap();
    assert_eq!(b.page_setup(0).row_breaks, vec![62]);
    b.undo();
    // a page drawn
    unsafe {
        use crate::ffi::*;
        let (w, h) = p2.paper_size();
        let surf = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, w as i32, h as i32);
        let cr = cairo_create(surf);
        b.render_page(0, 1, cr, "test.xlsx").unwrap();
        cairo_destroy(cr);
        let path = cstr("/tmp/page_test.png");
        cairo_surface_write_to_png(surf, path.as_ptr());
        cairo_surface_destroy(surf);
    }
    b.undo();
    assert_eq!(b.page_setup(0).fit, None);
}

#[test]
fn data_validation() {
    use crate::validation::DvSpec;
    let mut b = blank();
    let spec = |kind: &str, op: &str, f1: &str, f2: &str| DvSpec {
        kind: kind.into(), op: op.into(), f1: f1.into(), f2: f2.into(), allow_blank: true, dropdown: true,
        show_input: true, prompt_title: "Pick".into(), prompt: "One of the list".into(), show_error: true,
        style: "stop".into(), error_title: String::new(), error: String::new(),
    };
    // whole numbers 1..10 in A1:A5
    b.dv_set(0, &[Rect::new(0, 0, 4, 0)], &spec("whole", "between", "1", "10")).unwrap();
    assert!(b.dv_check(0, 0, 0, "5").is_none());
    assert!(b.dv_check(0, 2, 0, "11").is_some());
    assert!(b.dv_check(0, 2, 0, "2.5").is_some());
    assert!(b.dv_check(0, 2, 0, "abc").is_some());
    assert!(b.dv_check(0, 2, 0, "").is_none());
    assert!(b.dv_check(0, 6, 0, "11").is_none(), "outside the range");
    // a typed list in B1:B3, case-insensitive
    b.dv_set(0, &[Rect::new(0, 1, 2, 1)], &spec("list", "between", "Red,Green,Blue", "")).unwrap();
    assert_eq!(b.dv_list(0, 1, 1), vec!["Red", "Green", "Blue"]);
    assert!(b.dv_dropdown_at(0, 1, 1));
    assert!(b.dv_check(0, 1, 1, "green").is_none());
    assert!(b.dv_check(0, 1, 1, "Purple").is_some());
    // a list from cells, the source moving with each cell
    b.set_input(0, 0, 5, "x").unwrap();
    b.set_input(0, 1, 5, "y").unwrap();
    b.set_input(0, 2, 5, "7").unwrap();
    b.dv_set(0, &[Rect::new(0, 2, 0, 2)], &spec("list", "between", "=$F$1:$F$3", "")).unwrap();
    assert_eq!(b.dv_list(0, 0, 2), vec!["x", "y", "7"]);
    assert!(b.dv_check(0, 0, 2, "7").is_none());
    assert!(b.dv_check(0, 0, 2, "8").is_some());
    // text length, dates, custom formulas
    b.dv_set(0, &[Rect::new(0, 3, 0, 3)], &spec("textLength", "lessThanOrEqual", "3", "")).unwrap();
    assert!(b.dv_check(0, 0, 3, "abc").is_none());
    assert!(b.dv_check(0, 0, 3, "abcd").is_some());
    b.dv_set(0, &[Rect::new(0, 4, 0, 4)], &spec("date", "greaterThan", "=DATE(2026,1,1)", "")).unwrap();
    assert!(b.dv_check(0, 0, 4, "3/1/2026").is_none());
    assert!(b.dv_check(0, 0, 4, "3/1/2025").is_some());
    b.dv_set(0, &[Rect::new(1, 4, 1, 4)], &spec("date", "between", "1/1/2026", "12/31/2026")).unwrap();
    assert_eq!(b.dv_at(0, 1, 4).unwrap().f1, "46023", "a typed date is kept as its serial number");
    assert!(b.dv_check(0, 1, 4, "6/1/2026").is_none() && b.dv_check(0, 1, 4, "6/1/2027").is_some());
    assert!(b.dv_set(0, &[Rect::new(1, 4, 1, 4)], &spec("whole", "greaterThan", "ten", "")).is_err());
    b.dv_set(0, &[Rect::new(5, 0, 9, 0)], &spec("custom", "between", "=ISEVEN(A6)", "")).unwrap();
    assert!(b.dv_check(0, 7, 0, "4").is_none(), "the formula moves with the cell and sees the new value");
    assert!(b.dv_check(0, 7, 0, "5").is_some());
    // saved and read again
    let bytes = b.to_bytes().unwrap();
    let mut b2 = Book::from_bytes(&bytes).unwrap();
    let r = b2.dv_at(0, 1, 1).unwrap();
    assert_eq!((r.kind.as_str(), r.prompt.as_str(), r.dropdown), ("list", "One of the list", true));
    assert!(b2.dv_check(0, 2, 0, "11").is_some());
    // Clear All on part of a range leaves the rest
    b2.dv_clear(0, &[Rect::new(1, 0, 1, 0)]).unwrap();
    assert!(b2.dv_at(0, 1, 0).is_none());
    assert!(b2.dv_at(0, 0, 0).is_some() && b2.dv_at(0, 2, 0).is_some());
    b2.undo();
    assert!(b2.dv_at(0, 1, 0).is_some());
    // replacing a rule over part of another
    b2.dv_set(0, &[Rect::new(0, 0, 1, 0)], &spec("decimal", "greaterThan", "0", "")).unwrap();
    assert_eq!(b2.dv_at(0, 0, 0).unwrap().kind, "decimal");
    assert_eq!(b2.dv_at(0, 3, 0).unwrap().kind, "whole");
    // rows inserted above move the rules
    b2.insert_rows(0, 0, 2).unwrap();
    assert_eq!(b2.dv_at(0, 3, 1).unwrap().kind, "list");
    assert!(b2.dv_at(0, 0, 1).is_none());
    // circle invalid data
    b2.set_input(0, 5, 0, "50").unwrap();
    assert_eq!(b2.dv_invalid(0, 100), vec![(5, 0)]);
    // paste copies the rule along
    b2.begin("Paste", 0);
    b2.copy_validation(0, Rect::new(2, 1, 4, 1), 0, Rect::new(2, 8, 4, 8));
    b2.commit();
    assert_eq!(b2.dv_list(0, 3, 8), vec!["Red", "Green", "Blue"]);
    // so does Paste (all, or Paste Special > Validation), not Paste Values
    use crate::book_clip::{PasteOpts, What};
    b2.paste_cells(0, Rect::new(3, 1, 3, 1), 0, Rect::new(20, 2, 20, 2), PasteOpts::default(), false).unwrap();
    assert_eq!(b2.dv_at(0, 20, 2).unwrap().kind, "list");
    let vals = PasteOpts { what: What::Values, ..PasteOpts::default() };
    b2.paste_cells(0, Rect::new(3, 1, 3, 1), 0, Rect::new(21, 2, 21, 2), vals, false).unwrap();
    assert!(b2.dv_at(0, 21, 2).is_none());
    b2.set_input(0, 22, 2, "x").unwrap();
    let only = PasteOpts { what: What::Validation, ..PasteOpts::default() };
    b2.paste_cells(0, Rect::new(3, 1, 3, 1), 0, Rect::new(22, 2, 23, 2), only, false).unwrap();
    assert_eq!(b2.dv_at(0, 23, 2).unwrap().kind, "list");
    assert_eq!(b2.input(0, 22, 2), "x", "Paste Validation leaves the contents");
    b2.undo();
    assert!(b2.dv_at(0, 23, 2).is_none());
    // the selection: one rule, rules that differ, cells without the rule
    let sel = |b: &Book, r: Rect| b.dv_for_selection(0, (r.r1, r.c1), &[r]).1;
    assert_eq!(sel(&b2, Rect::new(3, 1, 4, 1)), "");
    assert_eq!(sel(&b2, Rect::new(3, 1, 3, 3)), "extend");
    assert_eq!(sel(&b2, Rect::new(3, 0, 3, 1)), "erase");
}

#[test]
fn hyperlinks() {
    use crate::links::Link;
    let mut b = blank();
    let web = |t: &str| Link { rect: Rect::new(0, 0, 0, 0), target: t.into(), location: String::new(), display: String::new(), tooltip: String::new() };
    b.set_input(0, 0, 0, "site").unwrap();
    b.set_link(0, Rect::new(0, 0, 0, 0), &Link { tooltip: "Go".into(), ..web("https://example.org/a?b=1&c=2") }).unwrap();
    b.set_link(0, Rect::new(2, 0, 3, 1), &Link { location: "Sheet1!D10".into(), ..web("") }).unwrap();
    assert_eq!(b.link_at(0, 0, 0).unwrap().target, "https://example.org/a?b=1&c=2");
    assert_eq!(b.link_at(0, 3, 1).unwrap().location, "Sheet1!D10");
    assert!(b.link_at(0, 1, 0).is_none());
    assert!(b.set_link(0, Rect::new(5, 5, 5, 5), &web("  ")).is_err());
    // saved: an outside address is a relationship, a place is an attribute
    let bytes = b.to_bytes().unwrap();
    let z = crate::zip::ZipReader::open(&bytes).unwrap();
    let get = |n: &str| z.read(n).map(|d| String::from_utf8_lossy(&d).to_string()).unwrap_or_default();
    let sheet = get("xl/worksheets/sheet1.xml");
    let rels = get("xl/worksheets/_rels/sheet1.xml.rels");
    assert!(sheet.contains("r:id=\"rId") && !sheet.contains("_target"), "{}", sheet);
    assert!(rels.contains("TargetMode=\"External\"") && rels.contains("example.org/a?b=1&amp;c=2"), "{}", rels);
    let mut b2 = Book::from_bytes(&bytes).unwrap();
    assert_eq!(b2.link_at(0, 0, 0).unwrap().target, "https://example.org/a?b=1&c=2");
    assert_eq!(b2.link_at(0, 0, 0).unwrap().tooltip, "Go");
    assert_eq!(b2.link_at(0, 2, 0).unwrap().location, "Sheet1!D10");
    // saving twice doesn't pile up relationships
    let bytes2 = b2.to_bytes().unwrap();
    let z2 = crate::zip::ZipReader::open(&bytes2).unwrap();
    let rels2 = String::from_utf8_lossy(&z2.read("xl/worksheets/_rels/sheet1.xml.rels").unwrap()).to_string();
    assert_eq!(rels2.matches("/hyperlink\"").count(), 1, "{}", rels2);
    // clearing part of a range link keeps the rest
    assert_eq!(b2.clear_links(0, &[Rect::new(2, 0, 2, 1)]), 1);
    assert!(b2.link_at(0, 2, 0).is_none() && b2.link_at(0, 3, 0).is_some());
    b2.undo();
    assert!(b2.link_at(0, 2, 0).is_some());
    // rows move links; deleting the row of a link deletes it
    b2.insert_rows(0, 0, 1).unwrap();
    assert!(b2.link_at(0, 1, 0).is_some() && b2.link_at(0, 0, 0).is_none());
    b2.delete_rows(0, 1, 1).unwrap();
    assert!(b2.link_at(0, 0, 0).is_none(), "the deleted cell's link goes, not onto the next row");
    assert_eq!(b2.links(0).len(), 1);
    // paste carries links
    use crate::book_clip::PasteOpts;
    b2.paste_cells(0, Rect::new(2, 0, 2, 0), 0, Rect::new(8, 8, 8, 8), PasteOpts::default(), false).unwrap();
    assert_eq!(b2.link_at(0, 8, 8).unwrap().location, "Sheet1!D10");
    // and a deleted row takes a validation rule with it too
    b2.dv_set(0, &[Rect::new(20, 0, 20, 0)], &crate::validation::DvSpec { kind: "whole".into(), op: "greaterThan".into(), f1: "1".into(), f2: String::new(), allow_blank: true, dropdown: true, show_input: true, prompt_title: String::new(), prompt: String::new(), show_error: true, style: "stop".into(), error_title: String::new(), error: String::new() }).unwrap();
    b2.delete_rows(0, 20, 1).unwrap();
    assert!(b2.dv_at(0, 20, 0).is_none());
    // =HYPERLINK() points where its first argument says
    b2.set_input(0, 30, 0, "x.org").unwrap();
    b2.set_input(0, 30, 1, "=HYPERLINK(\"https://\"&A31, \"go\")").unwrap();
    assert_eq!(b2.formula_link(0, 30, 1).as_deref(), Some("https://x.org"));
    assert!(b2.formula_link(0, 30, 0).is_none());
}
