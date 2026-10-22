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
