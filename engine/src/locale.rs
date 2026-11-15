//! The regional settings the program follows (S6-32): the decimal and thousands separators,
//! the order dates are typed in, the short date shown for the built-in date formats, and the
//! currency symbol typed amounts take. Files stay region-neutral: format codes and formulas
//! are stored as the original stores them, and only what is typed and shown follows the region.

use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateOrder {
    Mdy,
    Dmy,
    Ymd,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Locale {
    pub decimal: char,
    pub group: char,
    pub date_order: DateOrder,
    /// the short date the built-in date format (m/d/yyyy, number format 14) shows as
    pub short_date: String,
    /// "$", "€", "£"...
    pub currency: String,
    pub currency_before: bool,
    pub currency_space: bool,
}

impl Default for Locale {
    fn default() -> Self {
        Locale { decimal: '.', group: ',', date_order: DateOrder::Mdy, short_date: "m/d/yyyy".into(), currency: "$".into(), currency_before: true, currency_space: false }
    }
}

thread_local! {
    static CURRENT: RefCell<Locale> = RefCell::new(Locale::default());
}

pub fn get() -> Locale {
    CURRENT.with(|c| c.borrow().clone())
}

pub fn with<R>(f: impl FnOnce(&Locale) -> R) -> R {
    CURRENT.with(|c| f(&c.borrow()))
}

pub fn set(l: Locale) {
    CURRENT.with(|c| *c.borrow_mut() = l);
    crate::numfmt::clear_cache();
}

pub fn is_default() -> bool {
    with(|l| *l == Locale::default())
}

/// Marks number formatting writes for the decimal point and the thousands separator; they
/// become the region's characters at the end, so literal text in a format is left alone.
pub const DEC: char = '\u{E000}';
pub const GRP: char = '\u{E001}';

/// The currency formats typed amounts take ("$1,234.50" or "1.234,50 €").
pub fn currency_code(decimals: bool) -> String {
    with(|l| {
        let sym = format!("\"{}\"", l.currency.replace('"', ""));
        let num = if decimals { "#,##0.00" } else { "#,##0" };
        let sp = if l.currency_space { " " } else { "" };
        if l.currency == "$" && l.currency_before && !l.currency_space {
            return if decimals { crate::input::FMT_CURRENCY2.to_string() } else { crate::input::FMT_CURRENCY0.to_string() };
        }
        let pos = if l.currency_before { format!("{}{}{}", sym, sp, num) } else { format!("{}{}{}", num, sp, sym) };
        let neg = if l.currency_before { format!("[Red]-{}{}{}", sym, sp, num) } else { format!("[Red]-{}{}{}", num, sp, sym) };
        format!("{};{}", pos, neg)
    })
}

/// A number written with the region's decimal separator (=""&1.5 is "1,5" in the Netherlands).
pub fn num_text(v: f64) -> String {
    let t = crate::numfmt::num_to_text(v);
    with(|l| if l.decimal == '.' { t } else { t.replace('.', &l.decimal.to_string()) })
}
