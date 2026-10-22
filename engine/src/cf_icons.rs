//! The icons of conditional-format icon sets, drawn as vectors.

use crate::ffi::*;
use std::f64::consts::PI;

const RED: u32 = 0xE0484A;
const YELLOW: u32 = 0xF2B53B;
const GREEN: u32 = 0x3FA45B;
const GRAY: u32 = 0x8C8C8C;
const BLACK: u32 = 0x3A3A3A;
const PINK: u32 = 0xF4A6A6;

unsafe fn rgb(cr: *mut cairo_t, c: u32) {
    cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
}

/// An arrow in a square of size s at (x, y), pointing at angle a (0 = up, radians clockwise).
unsafe fn arrow(cr: *mut cairo_t, x: f64, y: f64, s: f64, a: f64, col: u32) {
    cairo_save(cr);
    cairo_translate(cr, x + s / 2.0, y + s / 2.0);
    cairo_rotate(cr, a);
    let h = s * 0.46;
    rgb(cr, col);
    cairo_new_path(cr);
    cairo_move_to(cr, 0.0, -h);
    cairo_line_to(cr, h * 0.95, 0.0);
    cairo_line_to(cr, h * 0.32, 0.0);
    cairo_line_to(cr, h * 0.32, h);
    cairo_line_to(cr, -h * 0.32, h);
    cairo_line_to(cr, -h * 0.32, 0.0);
    cairo_line_to(cr, -h * 0.95, 0.0);
    cairo_close_path(cr);
    cairo_fill(cr);
    cairo_restore(cr);
}

unsafe fn disc(cr: *mut cairo_t, x: f64, y: f64, s: f64, col: u32) {
    rgb(cr, col);
    cairo_new_path(cr);
    cairo_arc(cr, x + s / 2.0, y + s / 2.0, s * 0.42, 0.0, 2.0 * PI);
    cairo_fill(cr);
}

unsafe fn ring(cr: *mut cairo_t, x: f64, y: f64, s: f64, col: u32) {
    rgb(cr, col);
    cairo_set_line_width(cr, (s * 0.08).max(1.0));
    cairo_new_path(cr);
    cairo_arc(cr, x + s / 2.0, y + s / 2.0, s * 0.40, 0.0, 2.0 * PI);
    cairo_stroke(cr);
}

unsafe fn flag(cr: *mut cairo_t, x: f64, y: f64, s: f64, col: u32) {
    rgb(cr, BLACK);
    cairo_rectangle(cr, x + s * 0.18, y + s * 0.1, (s * 0.08).max(1.0), s * 0.8);
    cairo_fill(cr);
    rgb(cr, col);
    cairo_new_path(cr);
    cairo_move_to(cr, x + s * 0.26, y + s * 0.12);
    cairo_line_to(cr, x + s * 0.86, y + s * 0.3);
    cairo_line_to(cr, x + s * 0.26, y + s * 0.52);
    cairo_close_path(cr);
    cairo_fill(cr);
}

unsafe fn star(cr: *mut cairo_t, x: f64, y: f64, s: f64, fill: f64) {
    let cx = x + s / 2.0;
    let cy = y + s / 2.0 + s * 0.04;
    let ro = s * 0.47;
    let ri = ro * 0.45;
    cairo_new_path(cr);
    for k in 0..10 {
        let a = -PI / 2.0 + k as f64 * PI / 5.0;
        let r = if k % 2 == 0 { ro } else { ri };
        let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
        if k == 0 { cairo_move_to(cr, px, py) } else { cairo_line_to(cr, px, py) }
    }
    cairo_close_path(cr);
    if fill > 0.0 {
        cairo_save(cr);
        cairo_rectangle(cr, x, y, s * fill, s);
        cairo_clip(cr);
        rgb(cr, YELLOW);
        cairo_fill_preserve(cr);
        cairo_restore(cr);
    }
    rgb(cr, 0xC99A1E);
    cairo_set_line_width(cr, (s * 0.06).max(0.8));
    cairo_stroke(cr);
}

/// Bars of a signal meter, `on` of n lit.
unsafe fn bars(cr: *mut cairo_t, x: f64, y: f64, s: f64, n: u32, on: u32) {
    let w = s / (n as f64 * 1.4);
    for k in 0..n {
        let h = s * (0.3 + 0.7 * (k + 1) as f64 / n as f64) * 0.9;
        rgb(cr, if k < on { 0x3F6FB5 } else { 0xC8C8C8 });
        cairo_rectangle(cr, x + k as f64 * w * 1.4 + w * 0.2, y + s * 0.95 - h, w, h);
        cairo_fill(cr);
    }
}

unsafe fn quarters(cr: *mut cairo_t, x: f64, y: f64, s: f64, q: u32) {
    let (cx, cy, r) = (x + s / 2.0, y + s / 2.0, s * 0.42);
    rgb(cr, 0xFFFFFF);
    cairo_new_path(cr);
    cairo_arc(cr, cx, cy, r, 0.0, 2.0 * PI);
    cairo_fill(cr);
    if q > 0 {
        rgb(cr, BLACK);
        cairo_new_path(cr);
        cairo_move_to(cr, cx, cy);
        cairo_arc(cr, cx, cy, r, -PI / 2.0, -PI / 2.0 + q as f64 * PI / 2.0);
        cairo_close_path(cr);
        cairo_fill(cr);
    }
    ring(cr, x, y, s, BLACK);
}

unsafe fn boxes(cr: *mut cairo_t, x: f64, y: f64, s: f64, on: u32) {
    let b = s * 0.42;
    let pos = [(0.06, 0.06), (0.52, 0.06), (0.06, 0.52), (0.52, 0.52)];
    for (k, (px, py)) in pos.iter().enumerate() {
        rgb(cr, if (k as u32) < on { 0x3F6FB5 } else { 0xD0D0D0 });
        cairo_rectangle(cr, x + px * s, y + py * s, b, b);
        cairo_fill(cr);
    }
}

unsafe fn symbol(cr: *mut cairo_t, x: f64, y: f64, s: f64, kind: u32, circled: bool) {
    let col = [RED, YELLOW, GREEN][kind.min(2) as usize];
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    let mark = if circled { 0xFFFFFF } else { col };
    if circled {
        disc(cr, x, y, s, col);
    }
    rgb(cr, mark);
    cairo_set_line_width(cr, (s * 0.14).max(1.2));
    cairo_set_line_cap(cr, 1);
    let k = s * if circled { 0.18 } else { 0.3 };
    cairo_new_path(cr);
    match kind {
        0 => {
            cairo_move_to(cr, cx - k, cy - k);
            cairo_line_to(cr, cx + k, cy + k);
            cairo_move_to(cr, cx + k, cy - k);
            cairo_line_to(cr, cx - k, cy + k);
        }
        1 => {
            cairo_move_to(cr, cx, cy - k * 1.3);
            cairo_line_to(cr, cx, cy + k * 0.3);
            cairo_move_to(cr, cx, cy + k * 1.15);
            cairo_line_to(cr, cx, cy + k * 1.2);
        }
        _ => {
            cairo_move_to(cr, cx - k * 1.1, cy);
            cairo_line_to(cr, cx - k * 0.2, cy + k * 0.9);
            cairo_line_to(cr, cx + k * 1.2, cy - k);
        }
    }
    cairo_stroke(cr);
    cairo_set_line_cap(cr, 0);
}

unsafe fn sign(cr: *mut cairo_t, x: f64, y: f64, s: f64, kind: u32) {
    let (cx, cy) = (x + s / 2.0, y + s / 2.0);
    cairo_new_path(cr);
    match kind {
        0 => {
            rgb(cr, RED);
            cairo_move_to(cr, cx, y + s * 0.06);
            cairo_line_to(cr, x + s * 0.94, cy);
            cairo_line_to(cr, cx, y + s * 0.94);
            cairo_line_to(cr, x + s * 0.06, cy);
        }
        1 => {
            rgb(cr, YELLOW);
            cairo_move_to(cr, cx, y + s * 0.1);
            cairo_line_to(cr, x + s * 0.94, y + s * 0.88);
            cairo_line_to(cr, x + s * 0.06, y + s * 0.88);
        }
        _ => {
            disc(cr, x, y, s, GREEN);
            return;
        }
    }
    cairo_close_path(cr);
    cairo_fill(cr);
}

unsafe fn triangle(cr: *mut cairo_t, x: f64, y: f64, s: f64, kind: u32) {
    cairo_new_path(cr);
    match kind {
        0 => {
            rgb(cr, RED);
            cairo_move_to(cr, x + s * 0.12, y + s * 0.3);
            cairo_line_to(cr, x + s * 0.88, y + s * 0.3);
            cairo_line_to(cr, x + s * 0.5, y + s * 0.8);
            cairo_close_path(cr);
            cairo_fill(cr);
        }
        1 => {
            rgb(cr, YELLOW);
            cairo_rectangle(cr, x + s * 0.15, y + s * 0.42, s * 0.7, s * 0.16);
            cairo_fill(cr);
        }
        _ => {
            rgb(cr, GREEN);
            cairo_move_to(cr, x + s * 0.12, y + s * 0.7);
            cairo_line_to(cr, x + s * 0.88, y + s * 0.7);
            cairo_line_to(cr, x + s * 0.5, y + s * 0.2);
            cairo_close_path(cr);
            cairo_fill(cr);
        }
    }
}

/// Icon `i` (0 = the lowest values) of an icon set, in a square of size s at (x, y).
pub unsafe fn draw_icon(cr: *mut cairo_t, set: &str, i: u32, x: f64, y: f64, s: f64) {
    cairo_save(cr);
    let gray = set.ends_with("Gray");
    let up = 0.0;
    let down = PI;
    let right = PI / 2.0;
    let ur = PI / 4.0;
    let dr = 3.0 * PI / 4.0;
    match set {
        "3Arrows" | "3ArrowsGray" => {
            let (a, c) = [(down, RED), (right, YELLOW), (up, GREEN)][i.min(2) as usize];
            arrow(cr, x, y, s, a, if gray { GRAY } else { c });
        }
        "4Arrows" | "4ArrowsGray" => {
            let (a, c) = [(down, RED), (dr, YELLOW), (ur, YELLOW), (up, GREEN)][i.min(3) as usize];
            arrow(cr, x, y, s, a, if gray { GRAY } else { c });
        }
        "5Arrows" | "5ArrowsGray" => {
            let (a, c) = [(down, RED), (dr, YELLOW), (right, YELLOW), (ur, YELLOW), (up, GREEN)][i.min(4) as usize];
            arrow(cr, x, y, s, a, if gray { GRAY } else { c });
        }
        "3Flags" => flag(cr, x, y, s, [RED, YELLOW, GREEN][i.min(2) as usize]),
        "3TrafficLights1" => disc(cr, x, y, s, [RED, YELLOW, GREEN][i.min(2) as usize]),
        "3TrafficLights2" => {
            rgb(cr, BLACK);
            cairo_rectangle(cr, x + s * 0.04, y + s * 0.04, s * 0.92, s * 0.92);
            cairo_fill(cr);
            disc(cr, x + s * 0.12, y + s * 0.12, s * 0.76, [RED, YELLOW, GREEN][i.min(2) as usize]);
        }
        "4TrafficLights" => disc(cr, x, y, s, [BLACK, RED, YELLOW, GREEN][i.min(3) as usize]),
        "4RedToBlack" => disc(cr, x, y, s, [BLACK, GRAY, PINK, RED][i.min(3) as usize]),
        "3Signs" => sign(cr, x, y, s, i.min(2)),
        "3Symbols" => symbol(cr, x, y, s, i.min(2), true),
        "3Symbols2" => symbol(cr, x, y, s, i.min(2), false),
        "3Stars" => star(cr, x, y, s, [0.0, 0.5, 1.0][i.min(2) as usize]),
        "3Triangles" => triangle(cr, x, y, s, i.min(2)),
        "4Rating" => bars(cr, x, y, s, 4, i.min(3) + 1),
        "5Rating" => bars(cr, x, y, s, 4, i.min(4)),
        "5Quarters" => quarters(cr, x, y, s, i.min(4)),
        "5Boxes" => boxes(cr, x, y, s, i.min(4)),
        _ => disc(cr, x, y, s, [RED, YELLOW, GREEN][i.min(2) as usize]),
    }
    cairo_restore(cr);
}
