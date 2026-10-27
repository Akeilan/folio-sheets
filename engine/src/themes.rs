//! Themes (SH-22): the workbook's theme part — its colours, fonts and effects — and the
//! ready-made themes, colour sets and font sets of the Page Layout tab. Changing them changes
//! everything drawn in theme colours and theme fonts: cells, the palette, charts, shapes,
//! table styles, cell styles and sparklines.

use crate::book::{Book, Delta};
use crate::xml::{self, Element, Node};

/// Colours in the theme part's order: dk1 lt1 dk2 lt2 accent1..6 hlink folHlink.
pub type Scheme = [u32; 12];

pub const SLOTS: [&str; 12] = ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"];

/// The colour sets of Page Layout › Colors.
pub const COLOR_SETS: &[(&str, Scheme)] = &[
    ("Office", [0x000000, 0xFFFFFF, 0x44546A, 0xE7E6E6, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47, 0x0563C1, 0x954F72]),
    ("Office 2023", [0x000000, 0xFFFFFF, 0x0E2841, 0xE8E8E8, 0x156082, 0xE97132, 0x196B24, 0x0F9ED5, 0xA02B93, 0x4EA72E, 0x467886, 0x96607D]),
    ("Office 2007 - 2010", [0x000000, 0xFFFFFF, 0x1F497D, 0xEEECE1, 0x4F81BD, 0xC0504D, 0x9BBB59, 0x8064A2, 0x4BACC6, 0xF79646, 0x0000FF, 0x800080]),
    ("Grayscale", [0x000000, 0xFFFFFF, 0x000000, 0xF8F8F8, 0xDDDDDD, 0xB2B2B2, 0x969696, 0x808080, 0x5F5F5F, 0x4D4D4D, 0x5F5F5F, 0x919191]),
    ("Blue Warm", [0x000000, 0xFFFFFF, 0x242852, 0xACCBF9, 0x4A66AC, 0x629DD1, 0x297FD5, 0x7F8FA9, 0x5AA2AE, 0x9D90A0, 0x9454C3, 0x3EBBF0]),
    ("Blue", [0x000000, 0xFFFFFF, 0x17406D, 0xDBEFF9, 0x0F6FC6, 0x009DD9, 0x0BD0D9, 0x10CF9B, 0x7CCA62, 0xA5C249, 0xF49100, 0x85DFD0]),
    ("Blue II", [0x000000, 0xFFFFFF, 0x335B74, 0xDFE3E5, 0x1CADE4, 0x2683C6, 0x27CED7, 0x42BA97, 0x3E8853, 0x62A39F, 0x6B9F25, 0xB26B02]),
    ("Blue Green", [0x000000, 0xFFFFFF, 0x373545, 0xCEDBE6, 0x3494BA, 0x58B6C0, 0x75BDA7, 0x7A8C8E, 0x84ACB6, 0x2683C6, 0x6B9F25, 0x9F6715]),
    ("Green", [0x000000, 0xFFFFFF, 0x455F51, 0xE3DED1, 0x549E39, 0x8AB833, 0xC0CF3A, 0x029676, 0x4AB5C4, 0x0989B1, 0x6B9F25, 0xBA6906]),
    ("Green Yellow", [0x000000, 0xFFFFFF, 0x455F51, 0xE3DED1, 0x99CB38, 0x63A537, 0x37A76F, 0x44C1A3, 0x4EB3CF, 0x51C3F9, 0xEE7B08, 0x977B2D]),
    ("Yellow", [0x000000, 0xFFFFFF, 0x39302A, 0xE5DEDB, 0xFFCA08, 0xF8931D, 0xCE8D3E, 0xEC7016, 0xE64823, 0x9C6A6A, 0x2998E3, 0x7F723D]),
    ("Yellow Orange", [0x000000, 0xFFFFFF, 0x4E3B30, 0xFBEEC9, 0xF0A22E, 0xA5644E, 0xB58B80, 0xC3986D, 0xA19574, 0xC17529, 0xAD1F1F, 0xFFC42F]),
    ("Orange", [0x000000, 0xFFFFFF, 0x637052, 0xCCDDEA, 0xE48312, 0xBD582C, 0x865640, 0x9B8357, 0xC2BC80, 0x94A088, 0x2998E3, 0x8C8C8C]),
    ("Orange Red", [0x000000, 0xFFFFFF, 0x696464, 0xE9E5DC, 0xD34817, 0x9B2D1F, 0xA28E6A, 0x956251, 0x918485, 0x855D5D, 0xCC9900, 0x96A9A9]),
    ("Red Orange", [0x000000, 0xFFFFFF, 0x505046, 0xEEECE1, 0xE84C22, 0xFFBD47, 0xB64926, 0xFF8427, 0xCC9900, 0xB22600, 0xCC9900, 0x666699]),
    ("Red", [0x000000, 0xFFFFFF, 0x323232, 0xE5C243, 0xA5300F, 0xD55816, 0xE19825, 0xB19C7D, 0x7F5F52, 0xB27D49, 0x6B9F25, 0xB26B02]),
    ("Red Violet", [0x000000, 0xFFFFFF, 0x454551, 0xD8D9DC, 0xE32D91, 0xC830CC, 0x4EA6DC, 0x4775E7, 0x8971E1, 0xD54773, 0x6B9F25, 0x8C8C8C]),
    ("Violet", [0x000000, 0xFFFFFF, 0x373545, 0xDCD8DC, 0xAD84C6, 0x8784C7, 0x5D739A, 0x6997AF, 0x84ACB6, 0x6F8183, 0x69A020, 0x8C8C8C]),
    ("Violet II", [0x000000, 0xFFFFFF, 0x632E62, 0xEAE5EB, 0x92278F, 0x9B57D3, 0x755DD9, 0x665EB8, 0x45A5ED, 0x5982DB, 0x0066FF, 0x666699]),
    ("Median", [0x000000, 0xFFFFFF, 0x775F55, 0xEBDDC3, 0x94B6D2, 0xDD8047, 0xA5AB81, 0xD8B25C, 0x7BA79D, 0x968C8C, 0xF7B615, 0x704404]),
    ("Paper", [0x000000, 0xFFFFFF, 0x444D26, 0xFEFAC9, 0xA5B592, 0xF3A447, 0xE7BC29, 0xD092A7, 0x9C85C0, 0x809EC2, 0x8E58B6, 0x7F6F6F]),
    ("Marquee", [0x000000, 0xFFFFFF, 0x5E5E5E, 0xDDDDDD, 0x418AB3, 0xA6B727, 0xF69200, 0x838383, 0xFEC306, 0xDF5327, 0xF59E00, 0xB2B2B2]),
    ("Slipstream", [0x000000, 0xFFFFFF, 0x212745, 0xB4DCFA, 0x4E67C8, 0x5ECCF3, 0xA7EA52, 0x5DCEAF, 0xFF8021, 0xF14124, 0x56C7AA, 0x59A8D1]),
    ("Aspect", [0x000000, 0xFFFFFF, 0x323232, 0xE3DED1, 0xF07F09, 0x9F2936, 0x1B587C, 0x4E8542, 0x604878, 0xC19859, 0x6B9F25, 0xB26B02]),
];

/// The font sets of Page Layout › Fonts: (name, headings, body).
pub const FONT_SETS: &[(&str, &str, &str)] = &[
    ("Office", "Calibri Light", "Calibri"),
    ("Office 2023", "Aptos Display", "Aptos"),
    ("Office 2007 - 2010", "Cambria", "Calibri"),
    ("Calibri", "Calibri", "Calibri"),
    ("Arial", "Arial", "Arial"),
    ("Corbel", "Corbel", "Corbel"),
    ("Candara", "Candara", "Candara"),
    ("Franklin Gothic", "Franklin Gothic Medium", "Franklin Gothic Book"),
    ("Century Gothic", "Century Gothic", "Century Gothic"),
    ("Tw Cen MT", "Tw Cen MT", "Tw Cen MT"),
    ("Cambria", "Cambria", "Cambria"),
    ("Garamond", "Garamond", "Garamond"),
    ("Times New Roman", "Times New Roman", "Times New Roman"),
    ("Trebuchet MS", "Trebuchet MS", "Trebuchet MS"),
    ("Georgia", "Georgia", "Georgia"),
    ("Segoe UI", "Segoe UI", "Segoe UI"),
    ("Calibri-Cambria", "Calibri", "Cambria"),
    ("Arial Black-Arial", "Arial Black", "Arial"),
];

/// The ready-made themes of Page Layout › Themes: (name, colours name, colours, fonts name, headings, body).
pub const THEMES: &[(&str, &str, Scheme, &str, &str, &str)] = &[
    ("Office Theme", "Office", COLOR_SETS[0].1, "Office", "Calibri Light", "Calibri"),
    ("Office 2023 Theme", "Office 2023", COLOR_SETS[1].1, "Office 2023", "Aptos Display", "Aptos"),
    ("Office 2007 - 2010 Theme", "Office 2007 - 2010", COLOR_SETS[2].1, "Office 2007 - 2010", "Cambria", "Calibri"),
    ("Facet", "Facet", [0x000000, 0xFFFFFF, 0x2C3C43, 0xEBEBEB, 0x90C226, 0x54A021, 0xE6B91E, 0xE76618, 0xC42F1A, 0x918655, 0x99CA3C, 0xB9D181],
     "Facet", "Trebuchet MS", "Trebuchet MS"),
    ("Integral", "Integral", COLOR_SETS[6].1, "Integral", "Tw Cen MT Condensed", "Tw Cen MT"),
    ("Ion", "Ion", [0x000000, 0xFFFFFF, 0x1E5155, 0xEBEBEB, 0xB01513, 0xEA6312, 0xE6B729, 0x6AAC90, 0x5F9C9D, 0x9E5E9B, 0x58C1BA, 0x9DFFCB],
     "Ion", "Century Gothic", "Century Gothic"),
    ("Retrospect", "Retrospect", COLOR_SETS[12].1, "Retrospect", "Calibri Light", "Calibri"),
    ("Wisp", "Wisp", [0x000000, 0xFFFFFF, 0x4E3B30, 0xFBEEC9, 0xA53010, 0xDE7E18, 0x9F8351, 0x728653, 0x92AA4C, 0x6AAC91, 0xFB4A18, 0xFB9318],
     "Wisp", "Century Gothic", "Century Gothic"),
];

/// The effect scheme of the default theme (subtle fills, thin lines, one soft shadow).
const FMT_SCHEME: &str = concat!(
    "<a:fmtScheme name=\"Office\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>",
    "<a:gradFill rotWithShape=\"1\"><a:gsLst><a:gs pos=\"0\"><a:schemeClr val=\"phClr\"><a:lumMod val=\"110000\"/><a:satMod val=\"105000\"/><a:tint val=\"67000\"/></a:schemeClr></a:gs>",
    "<a:gs pos=\"50000\"><a:schemeClr val=\"phClr\"><a:lumMod val=\"105000\"/><a:satMod val=\"103000\"/><a:tint val=\"73000\"/></a:schemeClr></a:gs>",
    "<a:gs pos=\"100000\"><a:schemeClr val=\"phClr\"><a:lumMod val=\"105000\"/><a:satMod val=\"109000\"/><a:tint val=\"81000\"/></a:schemeClr></a:gs></a:gsLst><a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill>",
    "<a:gradFill rotWithShape=\"1\"><a:gsLst><a:gs pos=\"0\"><a:schemeClr val=\"phClr\"><a:satMod val=\"103000\"/><a:lumMod val=\"102000\"/><a:tint val=\"94000\"/></a:schemeClr></a:gs>",
    "<a:gs pos=\"50000\"><a:schemeClr val=\"phClr\"><a:satMod val=\"110000\"/><a:lumMod val=\"100000\"/><a:shade val=\"100000\"/></a:schemeClr></a:gs>",
    "<a:gs pos=\"100000\"><a:schemeClr val=\"phClr\"><a:lumMod val=\"99000\"/><a:satMod val=\"120000\"/><a:shade val=\"78000\"/></a:schemeClr></a:gs></a:gsLst><a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill></a:fillStyleLst>",
    "<a:lnStyleLst><a:ln w=\"6350\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln>",
    "<a:ln w=\"12700\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln>",
    "<a:ln w=\"19050\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/><a:miter lim=\"800000\"/></a:ln></a:lnStyleLst>",
    "<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst>",
    "<a:outerShdw blurRad=\"57150\" dist=\"19050\" dir=\"5400000\" algn=\"ctr\" rotWithShape=\"0\"><a:srgbClr val=\"000000\"><a:alpha val=\"63000\"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle></a:effectStyleLst>",
    "<a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"><a:tint val=\"95000\"/><a:satMod val=\"170000\"/></a:schemeClr></a:solidFill>",
    "<a:gradFill rotWithShape=\"1\"><a:gsLst><a:gs pos=\"0\"><a:schemeClr val=\"phClr\"><a:tint val=\"93000\"/><a:satMod val=\"150000\"/><a:shade val=\"98000\"/><a:lumMod val=\"102000\"/></a:schemeClr></a:gs>",
    "<a:gs pos=\"50000\"><a:schemeClr val=\"phClr\"><a:tint val=\"98000\"/><a:satMod val=\"130000\"/><a:shade val=\"90000\"/><a:lumMod val=\"103000\"/></a:schemeClr></a:gs>",
    "<a:gs pos=\"100000\"><a:schemeClr val=\"phClr\"><a:shade val=\"63000\"/><a:satMod val=\"120000\"/></a:schemeClr></a:gs></a:gsLst><a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill></a:bgFillStyleLst></a:fmtScheme>"
);

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

fn clr_scheme(name: &str, c: &Scheme) -> String {
    let mut s = format!("<a:clrScheme name=\"{}\">", xml::escape_attr(name));
    for (k, slot) in SLOTS.iter().enumerate() {
        if k == 0 {
            s.push_str(&format!("<a:dk1><a:sysClr val=\"windowText\" lastClr=\"{:06X}\"/></a:dk1>", c[0]));
        } else if k == 1 {
            s.push_str(&format!("<a:lt1><a:sysClr val=\"window\" lastClr=\"{:06X}\"/></a:lt1>", c[1]));
        } else {
            s.push_str(&format!("<a:{0}><a:srgbClr val=\"{1:06X}\"/></a:{0}>", slot, c[k]));
        }
    }
    s.push_str("</a:clrScheme>");
    s
}

fn font_scheme(name: &str, major: &str, minor: &str) -> String {
    format!("<a:fontScheme name=\"{}\"><a:majorFont><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont><a:minorFont><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme>",
            xml::escape_attr(name), xml::escape_attr(major), xml::escape_attr(minor))
}

/// A whole theme part.
pub fn theme_xml(name: &str, colors_name: &str, colors: &Scheme, fonts_name: &str, major: &str, minor: &str) -> String {
    format!("{}<a:theme xmlns:a=\"{}\" name=\"{}\"><a:themeElements>{}{}{}</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
            xml::DECL, NS_A, xml::escape_attr(name), clr_scheme(colors_name, colors), font_scheme(fonts_name, major, minor), FMT_SCHEME)
}

pub fn default_theme_xml() -> String {
    let t = &THEMES[0];
    theme_xml(t.0, t.1, &t.2, t.3, t.4, t.5)
}

/// What a theme part says: names, colours (part order), fonts.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeInfo {
    pub name: String,
    pub colors_name: String,
    pub colors: Scheme,
    pub fonts_name: String,
    pub major: String,
    pub minor: String,
    pub effects_name: String,
}

pub fn info(x: &str) -> Option<ThemeInfo> {
    let (_, root) = xml::parse(x).ok()?;
    if root.local() != "theme" {
        return None;
    }
    let te = root.child("themeElements")?;
    let cs = te.child("clrScheme")?;
    let mut colors = COLOR_SETS[0].1;
    for (k, slot) in SLOTS.iter().enumerate() {
        if let Some(c) = cs.child(slot) {
            let v = c.child("srgbClr").and_then(|x| x.attr("val")).or_else(|| c.child("sysClr").and_then(|x| x.attr("lastClr")));
            if let Some(v) = v.and_then(|h| u32::from_str_radix(h, 16).ok()) {
                colors[k] = v;
            }
        }
    }
    let fs = te.child("fontScheme");
    let face = |which: &str| fs.and_then(|f| f.child(which)).and_then(|m| m.child("latin")).and_then(|l| l.attr("typeface")).unwrap_or("").to_string();
    Some(ThemeInfo {
        name: root.attr("name").unwrap_or("").to_string(),
        colors_name: cs.attr("name").unwrap_or("").to_string(),
        colors,
        fonts_name: fs.and_then(|f| f.attr("name")).unwrap_or("").to_string(),
        major: face("majorFont"),
        minor: face("minorFont"),
        effects_name: te.child("fmtScheme").and_then(|f| f.attr("name")).unwrap_or("").to_string(),
    })
}

fn replace_child(x: &str, local: &str, new_xml: &str) -> Result<String, String> {
    let (prolog, mut root) = xml::parse(x)?;
    let (_, new_el) = xml::parse(&format!("<x xmlns:a=\"{}\">{}</x>", NS_A, new_xml))?;
    let new_el = new_el.elems().next().cloned().ok_or("bad theme element")?;
    let te = root.child_mut("themeElements").ok_or("The theme has no theme elements.")?;
    let pfx = te.name.split(':').next().filter(|_| te.name.contains(':')).map(|p| format!("{}:", p)).unwrap_or_default();
    let mut new_el = new_el;
    if pfx != "a:" {
        rename_prefix(&mut new_el, "a:", &pfx);
    }
    match te.children.iter().position(|n| matches!(n, Node::Elem(e) if e.local() == local)) {
        Some(i) => te.children[i] = Node::Elem(new_el),
        None => te.children.push(Node::Elem(new_el)),
    }
    Ok(xml::document(&prolog, &root))
}

fn rename_prefix(e: &mut Element, from: &str, to: &str) {
    if let Some(rest) = e.name.strip_prefix(from) {
        e.name = format!("{}{}", to, rest);
    }
    for n in e.children.iter_mut() {
        if let Node::Elem(c) = n {
            rename_prefix(c, from, to);
        }
    }
}

pub fn with_colors(x: &str, name: &str, c: &Scheme) -> Result<String, String> {
    replace_child(x, "clrScheme", &clr_scheme(name, c))
}

pub fn with_fonts(x: &str, name: &str, major: &str, minor: &str) -> Result<String, String> {
    replace_child(x, "fontScheme", &font_scheme(name, major, minor))
}

pub fn with_effects_of(x: &str, from: &str) -> Result<String, String> {
    let (_, src) = xml::parse(from)?;
    let fmt = src.child("themeElements").and_then(|t| t.child("fmtScheme")).ok_or("That theme has no effects.")?;
    let mut f = fmt.clone();
    let p = f.name.split(':').next().filter(|_| f.name.contains(':')).map(|p| format!("{}:", p)).unwrap_or_default();
    if p != "a:" {
        rename_prefix(&mut f, &p, "a:");
    }
    replace_child(x, "fmtScheme", &f.to_string())
}

pub fn renamed(x: &str, name: &str) -> Result<String, String> {
    let (prolog, mut root) = xml::parse(x)?;
    root.set_attr("name", name);
    Ok(xml::document(&prolog, &root))
}

// ---------------------------------------------------------------------- theme files (.thmx)

const THMX_CT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/theme/theme/themeManager.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.themeManager+xml\"/><Override PartName=\"/theme/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/></Types>";
const THMX_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"theme/theme/themeManager.xml\"/></Relationships>";
const THMX_MGR: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<a:themeManager xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"/>";
const THMX_MGR_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"theme1.xml\"/></Relationships>";

/// A theme file (.thmx) holding a theme part.
pub fn thmx_bytes(theme: &str) -> Vec<u8> {
    let mut z = crate::zip::ZipWriter::new();
    z.add("[Content_Types].xml", THMX_CT.as_bytes(), true);
    z.add("_rels/.rels", THMX_RELS.as_bytes(), true);
    z.add("theme/theme/themeManager.xml", THMX_MGR.as_bytes(), true);
    z.add("theme/theme/theme1.xml", theme.as_bytes(), true);
    z.add("theme/theme/_rels/themeManager.xml.rels", THMX_MGR_RELS.as_bytes(), true);
    z.finish()
}

/// The theme part in a theme file (.thmx), a workbook, or a theme part's own XML.
pub fn theme_from_bytes(data: &[u8]) -> Result<String, String> {
    if data.starts_with(b"PK") {
        let z = crate::zip::ZipReader::open(data)?;
        let mut names = z.names();
        names.sort_by_key(|n| (!n.contains("theme1.xml"), n.clone()));
        for n in names {
            if n.ends_with(".xml") && n.contains("theme/") && !n.contains("themeManager") && !n.contains("_rels") {
                let x = xml::decode(&z.read(&n)?)?;
                if info(&x).is_some() {
                    return Ok(x);
                }
            }
        }
        return Err("This file doesn't hold a theme.".into());
    }
    let x = xml::decode(data)?;
    if info(&x).is_some() {
        Ok(x)
    } else {
        Err("This file doesn't hold a theme.".into())
    }
}

impl Book {
    /// The theme part in use (the default theme's for a workbook without one).
    pub fn theme_xml(&self) -> String {
        self.styles.theme_xml.clone().unwrap_or_else(default_theme_xml)
    }

    pub fn theme_info(&self) -> ThemeInfo {
        info(&self.theme_xml()).unwrap_or_else(|| info(&default_theme_xml()).unwrap())
    }

    /// Use a theme part, as one undo step.
    pub fn set_theme_xml(&mut self, x: &str, label: &str) -> Result<(), String> {
        info(x).ok_or("This isn't a theme.")?;
        let s = self.active;
        self.begin(label, s);
        self.record(Delta::Theme(Some(x.to_string())));
        self.commit();
        Ok(())
    }

    /// Page Layout › Themes: a ready-made theme.
    pub fn apply_theme(&mut self, name: &str) -> Result<(), String> {
        let t = THEMES.iter().find(|t| t.0 == name).ok_or("No such theme.")?;
        // the effects stay the default theme's (the ready-made themes' own aren't kept here)
        let x = theme_xml(t.0, t.1, &t.2, t.3, t.4, t.5);
        self.set_theme_xml(&x, "Theme")
    }

    /// Page Layout › Colors: a colour set (part order: dk1 lt1 dk2 lt2 accents hlink folHlink).
    pub fn set_theme_colors(&mut self, name: &str, c: &Scheme) -> Result<(), String> {
        let x = with_colors(&self.theme_xml(), name, c)?;
        self.set_theme_xml(&x, "Theme Colors")
    }

    /// Page Layout › Fonts: headings and body fonts.
    pub fn set_theme_fonts(&mut self, name: &str, major: &str, minor: &str) -> Result<(), String> {
        let x = with_fonts(&self.theme_xml(), name, major, minor)?;
        self.set_theme_xml(&x, "Theme Fonts")
    }

    /// Page Layout › Effects: the effects of another theme part.
    pub fn set_theme_effects(&mut self, from: &str) -> Result<(), String> {
        let x = with_effects_of(&self.theme_xml(), from)?;
        self.set_theme_xml(&x, "Theme Effects")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_build_read_and_change() {
        let d = default_theme_xml();
        let i = info(&d).unwrap();
        assert_eq!(i.name, "Office Theme");
        assert_eq!(i.colors[4], 0x4472C4);
        assert_eq!((i.major.as_str(), i.minor.as_str()), ("Calibri Light", "Calibri"));
        let x = with_colors(&d, "Green", &COLOR_SETS[8].1).unwrap();
        let j = info(&x).unwrap();
        assert_eq!(j.colors_name, "Green");
        assert_eq!(j.colors[4], 0x549E39);
        assert_eq!(j.major, "Calibri Light");
        let y = with_fonts(&x, "Georgia", "Georgia", "Georgia").unwrap();
        assert_eq!(info(&y).unwrap().minor, "Georgia");
        let t = thmx_bytes(&y);
        assert_eq!(theme_from_bytes(&t).unwrap(), y);
        for th in THEMES {
            assert!(info(&theme_xml(th.0, th.1, &th.2, th.3, th.4, th.5)).is_some());
        }
    }
}
