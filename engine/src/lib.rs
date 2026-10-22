//! Folio Sheets engine: workbook model, formulas, number formats, .xlsx files and drawing.
pub mod cellref;
pub mod dates;
pub mod deflate;
pub mod numfmt;
pub mod value;
pub mod xml;
pub mod zip;
pub mod formula;
pub mod styles;
pub mod sheet;
pub mod calc;
pub mod input;
pub mod fns;
pub mod ffi;
pub mod layout;
pub mod book;
pub mod recalc;
pub mod xlsx;
pub mod view;
pub mod render;
pub mod book_fmt;
#[cfg(feature = "python")]
pub mod py;
#[cfg(test)]
mod tests_book;
