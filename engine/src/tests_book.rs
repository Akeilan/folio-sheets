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
