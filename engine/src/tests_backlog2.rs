//! Backlog 2 engine tests: copied sheets, Advanced Filter, groups, chartEx, links, backgrounds.
use crate::book::Book;
use crate::cellref::Rect;
use crate::value::Value;

fn blank() -> Book {
    Book::open("testdata/Blank.xlsx").unwrap()
}

fn round_trip(b: &Book) -> Book {
    let bytes = b.to_bytes().unwrap();
    Book::from_bytes(&bytes).unwrap()
}

#[test]
fn copy_sheet_keeps_everything_and_renames() {
    let mut b = blank();
    b.set_input(0, 0, 0, "x").unwrap();
    b.set_input(0, 1, 0, "1").unwrap();
    b.set_input(0, 2, 0, "2").unwrap();
    b.set_input(0, 0, 1, "=SUM(Sheet1!A2:A3)").unwrap();
    let at = b.copy_sheet(0, 1).unwrap();
    assert_eq!(b.sheets[at].name, "Sheet1 (2)");
    assert_eq!(b.input(at, 0, 1), "=SUM('Sheet1 (2)'!A2:A3)");
    assert_eq!(crate::sheet_copy::copy_name(&b, "Sheet1 (2)"), "Sheet1 (3)");
    let r = round_trip(&b);
    assert_eq!(r.sheets.len(), 2);
    assert_eq!(r.input(1, 2, 0), "2");
}

#[test]
fn advanced_filter_rows_or_columns_and() {
    let mut b = blank();
    for (r, row) in [["Name", "Qty"], ["a", "5"], ["b", "15"], ["c", "25"]].iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            b.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    b.set_input(0, 10, 0, "Qty").unwrap();
    b.set_input(0, 11, 0, ">10").unwrap();
    b.set_input(0, 10, 1, "Name").unwrap();
    b.set_input(0, 11, 1, "c").unwrap();
    let m = b.advanced_matches(0, Rect::new(0, 0, 3, 1), Some((0, Rect::new(10, 0, 11, 1))), false).unwrap();
    assert_eq!(m, vec![3]);
    let n = b.advanced_filter(0, Rect::new(0, 0, 3, 1), Some((0, Rect::new(10, 0, 11, 0))), false).unwrap();
    assert_eq!(n, 2);
    assert!(b.sheets[0].row_hidden(1) && !b.sheets[0].row_hidden(2));
    assert!(b.advanced_show_all(0).unwrap());
    assert!(!b.sheets[0].row_hidden(1));
}

#[test]
fn groups_save_and_ungroup() {
    let mut b = blank();
    let a = b.insert_shape(0, "rect", 10.0, 10.0, 40.0, 20.0, false, false).unwrap();
    let c = b.insert_shape(0, "ellipse", 100.0, 60.0, 30.0, 30.0, false, false).unwrap();
    let g = b.group_objects(0, &[a, c]).unwrap();
    assert_eq!(b.sheets[0].objects[g].members.len(), 2);
    let mut r = round_trip(&b);
    assert_eq!(r.sheets[0].objects.len(), 1);
    assert_eq!(r.sheets[0].objects[0].kind(), "group");
    assert_eq!(r.sheets[0].objects[0].members.len(), 2);
    let out = r.ungroup_object(0, 0).unwrap();
    assert_eq!(out.len(), 2);
    let (x, y, w, h) = r.obj_rect(0, &r.sheets[0].objects[1]);
    assert!((x - 100.0).abs() < 1.5 && (y - 60.0).abs() < 1.5 && (w - 30.0).abs() < 1.5 && (h - 30.0).abs() < 1.5, "{:?}", (x, y, w, h));
}

#[test]
fn chartex_round_trip() {
    let mut b = blank();
    for (r, row) in [["Item", "Amount"], ["a", "10"], ["b", "-3"], ["c", "4"]].iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            b.set_input(0, r as u32, c as u32, v).unwrap();
        }
    }
    for kind in crate::chartex::KINDS {
        let i = b.insert_chartex(0, Rect::new(0, 0, 3, 1), kind, 200.0, 10.0).unwrap();
        assert_eq!(b.chartex(0, i).unwrap().kind, kind);
    }
    let r = round_trip(&b);
    let kinds: Vec<String> = r.sheets[0].objects.iter().filter_map(|o| o.chartex.as_ref().map(|c| c.kind.clone())).collect();
    assert_eq!(kinds, crate::chartex::KINDS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    // read back unchanged: written as read
    let r2 = round_trip(&r);
    assert_eq!(r2.sheets[0].objects.len(), 7);
    let (labels, counts) = crate::chartex::bins(&[1.0, 2.0, 2.5, 3.0, 9.0]);
    assert_eq!(counts.iter().sum::<f64>(), 5.0);
    assert_eq!(labels.len(), counts.len());
}

#[test]
fn background_kept() {
    let mut b = blank();
    // a 1x1 PNG
    let png: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0,
        0x90, 0x77, 0x53, 0xDE, 0, 0, 0, 12, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00,
        0x18, 0xDD, 0x8D, 0xB0, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
    b.set_background(0, Some(png)).unwrap();
    let r = round_trip(&b);
    assert!(r.sheets[0].background.is_some());
    let r2 = round_trip(&r);
    assert!(r2.sheets[0].background.is_some());
}

#[test]
fn sheet_elements_and_counts() {
    let mut b = blank();
    b.set_input(0, 0, 0, "1").unwrap();
    b.set_input(0, 1, 0, "=A1").unwrap();
    assert_eq!(b.sheet_counts(0), (2, 1));
    b.set_sheet_element(0, "protectedRanges", Some("<protectedRanges><protectedRange name=\"R\" sqref=\"A1:B2\"/></protectedRanges>"), Some("Allow")).unwrap();
    let r = round_trip(&b);
    assert!(r.sheet_element(0, "protectedRanges").unwrap().contains("A1:B2"));
    assert!(matches!(r.value(0, 1, 0), Value::Num(x) if x == 1.0));
}
