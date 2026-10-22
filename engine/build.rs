// Link against the cairo/pango runtime libraries even where their development
// symlinks (libcairo.so ...) are not installed.
use std::path::Path;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    let dir = Path::new(&out).join("links");
    std::fs::create_dir_all(&dir).unwrap();
    let libs = [
        ("cairo", "libcairo.so.2"),
        ("pango-1.0", "libpango-1.0.so.0"),
        ("pangocairo-1.0", "libpangocairo-1.0.so.0"),
        ("gobject-2.0", "libgobject-2.0.so.0"),
    ];
    for (name, so) in libs {
        for d in ["/usr/lib/x86_64-linux-gnu", "/lib/x86_64-linux-gnu", "/usr/lib64", "/usr/lib"] {
            let p = Path::new(d).join(so);
            if p.exists() {
                let l = dir.join(format!("lib{}.so", name));
                let _ = std::fs::remove_file(&l);
                std::os::unix::fs::symlink(&p, &l).unwrap();
                break;
            }
        }
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rerun-if-changed=build.rs");
}
