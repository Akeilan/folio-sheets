//! Map charts (S3-13): the countries and the states/provinces they colour, found by name or code
//! from the boundaries in data/maps.txt (Natural Earth, public domain; see tools/build_maps.py),
//! drawn in the chosen projection. Nothing is looked up online.
use std::collections::HashMap;
use std::sync::OnceLock;

const DATA: &str = include_str!("../data/maps.txt");

#[derive(Debug, Clone)]
pub struct Region {
    /// true: a state or province
    pub state: bool,
    /// a country's ISO 3166 alpha-3 code; for a state, its country's
    pub country: String,
    pub iso2: String,
    /// a state's ISO 3166-2 code, its postal code
    pub code: String,
    pub postal: String,
    pub name: String,
    pub alts: Vec<String>,
    pub continent: String,
    pub subregion: String,
    pub label: (f64, f64),
    pub rings: Vec<Vec<(f64, f64)>>,
}

pub struct Atlas {
    pub regions: Vec<Region>,
    countries: HashMap<String, usize>,
    states: HashMap<String, Vec<usize>>,
}

fn norm(s: &str) -> String {
    let mut out = String::new();
    for ch in s.trim().chars() {
        let c = match ch {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            'ñ' | 'Ñ' => 'n',
            '.' | '\'' | '’' => continue,
            '-' | '_' | ',' => ' ',
            c => c,
        };
        out.extend(c.to_lowercase());
    }
    let s = out.split_whitespace().collect::<Vec<_>>().join(" ");
    s.strip_prefix("the ").map(|x| x.to_string()).unwrap_or(s)
}

fn parse_rings(t: &str) -> Vec<Vec<(f64, f64)>> {
    t.split(';').filter(|r| !r.is_empty()).map(|r| {
        r.split(' ').filter_map(|p| {
            let (a, b) = p.split_once(',')?;
            Some((a.parse::<f64>().ok()? / 100.0, b.parse::<f64>().ok()? / 100.0))
        }).collect()
    }).collect()
}

fn parse_pt(t: &str) -> (f64, f64) {
    let (a, b) = t.split_once(',').unwrap_or(("0", "0"));
    (a.parse().unwrap_or(0.0), b.parse().unwrap_or(0.0))
}

/// Common names the data spells otherwise.
const EXTRA: &[(&str, &str)] = &[
    ("USA", "usa"), ("USA", "us"), ("USA", "united states"), ("USA", "america"), ("GBR", "uk"), ("GBR", "great britain"),
    ("GBR", "england"), ("GBR", "britain"), ("RUS", "russia"), ("KOR", "south korea"), ("PRK", "north korea"),
    ("CZE", "czech republic"), ("CIV", "ivory coast"), ("COD", "drc"), ("COD", "democratic republic of the congo"),
    ("COG", "republic of the congo"), ("MMR", "burma"), ("SWZ", "swaziland"), ("MKD", "macedonia"), ("TUR", "turkiye"),
    ("FRA", "fr"), ("NOR", "no"), ("CPV", "cape verde"), ("TLS", "east timor"), ("VAT", "vatican"), ("ARE", "uae"), ("NLD", "holland"),
];

impl Atlas {
    fn load() -> Atlas {
        let mut regions = Vec::new();
        for line in DATA.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('|').collect();
            let alts = |t: &str| t.split(';').filter(|x| !x.is_empty()).map(|x| x.to_string()).collect::<Vec<_>>();
            if f[0] == "C" && f.len() >= 9 {
                regions.push(Region { state: false, country: f[2].into(), iso2: f[1].into(), code: f[2].into(), postal: String::new(),
                    name: f[3].into(), alts: alts(f[4]), continent: f[5].into(), subregion: f[6].into(), label: parse_pt(f[7]), rings: parse_rings(f[8]) });
            } else if f[0] == "S" && f.len() >= 8 {
                regions.push(Region { state: true, country: f[1].into(), iso2: String::new(), code: f[2].into(), postal: f[3].into(),
                    name: f[4].into(), alts: alts(f[5]), continent: String::new(), subregion: String::new(), label: parse_pt(f[6]), rings: parse_rings(f[7]) });
            }
        }
        let mut countries = HashMap::new();
        let mut states: HashMap<String, Vec<usize>> = HashMap::new();
        // countries: names and codes first, the other names after (so "France" stays France,
        // not one of its territories that names it as its sovereign)
        for (i, r) in regions.iter().enumerate().filter(|(_, r)| !r.state) {
            for k in [norm(&r.name), norm(&r.iso2), norm(&r.code)] {
                if !k.is_empty() {
                    countries.entry(k).or_insert(i);
                }
            }
        }
        for (i, r) in regions.iter().enumerate().filter(|(_, r)| !r.state) {
            for a in &r.alts {
                countries.entry(norm(a)).or_insert(i);
            }
        }
        for (i, r) in regions.iter().enumerate().filter(|(_, r)| r.state) {
            let mut keys = vec![norm(&r.name), norm(&r.code)];
            if !r.postal.is_empty() {
                keys.push(norm(&r.postal));
            }
            keys.extend(r.alts.iter().map(|a| norm(a)));
            for k in keys {
                let e = states.entry(k).or_default();
                if !e.contains(&i) {
                    e.push(i);
                }
            }
        }
        for (code, name) in EXTRA {
            if let Some(i) = regions.iter().position(|r| !r.state && r.code == *code) {
                countries.insert(name.to_string(), i);
            }
        }
        Atlas { regions, countries, states }
    }

    pub fn country(&self, name: &str) -> Option<usize> {
        self.countries.get(&norm(name)).copied()
    }

    /// A state or province by name or code, in a country if one is named.
    pub fn state(&self, name: &str, country: Option<usize>) -> Option<usize> {
        let c = country.map(|i| self.regions[i].code.clone());
        let list = self.states.get(&norm(name))?;
        list.iter().copied().find(|&i| c.as_ref().map(|c| self.regions[i].country == *c).unwrap_or(true))
    }

    /// Which region each category names: countries or states, whichever more of them are
    /// (`parents`: a second category level, such as the country of each state).
    pub fn resolve(&self, names: &[String], parents: &[String]) -> (bool, Vec<Option<usize>>) {
        let cs: Vec<Option<usize>> = names.iter().map(|n| self.country(n)).collect();
        let ss: Vec<Option<usize>> = names.iter().enumerate().map(|(k, n)| {
            let p = parents.get(k).and_then(|p| self.country(p));
            self.state(n, p)
        }).collect();
        let nc = cs.iter().filter(|x| x.is_some()).count();
        let ns = ss.iter().filter(|x| x.is_some()).count();
        // two-letter codes are both: states win when the parents say so, or when more match
        if ns > nc || (ns == nc && ns > 0 && !parents.is_empty() && parents.iter().any(|p| self.country(p).is_some())) {
            (true, ss)
        } else {
            (false, cs)
        }
    }

    /// The states of a country.
    pub fn states_of(&self, iso3: &str) -> Vec<usize> {
        (0..self.regions.len()).filter(|&i| self.regions[i].state && self.regions[i].country == iso3).collect()
    }
}

pub fn atlas() -> &'static Atlas {
    static A: OnceLock<Atlas> = OnceLock::new();
    A.get_or_init(Atlas::load)
}

// ------------------------------------------------------------------ projections
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    Mercator,
    Miller,
    Robinson,
    /// Albers equal-area conic: (standard parallels, central meridian)
    Albers(f64, f64, f64),
}

const ROBINSON: [(f64, f64); 19] = [
    (1.0000, 0.0000), (0.9986, 0.0620), (0.9954, 0.1240), (0.9900, 0.1860), (0.9822, 0.2480), (0.9730, 0.3100),
    (0.9600, 0.3720), (0.9427, 0.4340), (0.9216, 0.4958), (0.8962, 0.5571), (0.8679, 0.6176), (0.8350, 0.6769),
    (0.7986, 0.7346), (0.7597, 0.7903), (0.7186, 0.8435), (0.6732, 0.8936), (0.6213, 0.9394), (0.5722, 0.9761), (0.5322, 1.0000),
];

impl Projection {
    /// (x, y) with y up, roughly in radians.
    pub fn project(&self, lon: f64, lat: f64) -> (f64, f64) {
        let lat = lat.clamp(-85.0, 85.0);
        let l = lon.to_radians();
        let p = lat.to_radians();
        match *self {
            Projection::Mercator => (l, (std::f64::consts::FRAC_PI_4 + p / 2.0).tan().ln()),
            Projection::Miller => (l, 1.25 * (std::f64::consts::FRAC_PI_4 + 0.4 * p).tan().ln()),
            Projection::Robinson => {
                let a = lat.abs() / 5.0;
                let i = (a.floor() as usize).min(17);
                let t = a - i as f64;
                let (x0, y0) = ROBINSON[i];
                let (x1, y1) = ROBINSON[i + 1];
                let px = x0 + (x1 - x0) * t;
                let py = y0 + (y1 - y0) * t;
                (0.8487 * px * l, 1.3523 * py * lat.signum())
            }
            Projection::Albers(p1, p2, lon0) => {
                let (p1, p2) = (p1.to_radians(), p2.to_radians());
                let n = (p1.sin() + p2.sin()) / 2.0;
                let c = p1.cos().powi(2) + 2.0 * n * p1.sin();
                let rho0 = c.sqrt() / n;
                let rho = (c - 2.0 * n * p.sin()).max(0.0).sqrt() / n;
                let mut d = lon - lon0;
                if d > 180.0 {
                    d -= 360.0;
                } else if d < -180.0 {
                    d += 360.0;
                }
                let th = n * d.to_radians();
                (rho * th.sin(), rho0 - rho * th.cos())
            }
        }
    }
}

/// The projection Excel's Automatic picks: Albers for the US and Canada, Miller for the world,
/// Mercator for one country or a few.
pub fn auto_projection(atlas: &Atlas, shown: &[usize], states: bool) -> Projection {
    if states {
        if let Some(&i) = shown.first() {
            let c = atlas.regions[i].country.as_str();
            if c == "USA" {
                return Projection::Albers(29.5, 45.5, -96.0);
            }
            if c == "CAN" {
                return Projection::Albers(50.0, 70.0, -96.0);
            }
        }
        return Projection::Mercator;
    }
    let conts: std::collections::HashSet<&str> = shown.iter().map(|&i| atlas.regions[i].continent.as_str()).collect();
    if conts.len() > 1 || shown.len() > 30 {
        Projection::Miller
    } else {
        Projection::Mercator
    }
}

/// The main part of a region (its largest ring and those near it in size): what a map that
/// shows "only regions with data" is sized to, so far-off islands don't shrink it.
pub fn main_rings(r: &Region) -> Vec<&Vec<(f64, f64)>> {
    let area = |ring: &Vec<(f64, f64)>| {
        let mut s = 0.0;
        for k in 0..ring.len() {
            let (x1, y1) = ring[k];
            let (x2, y2) = ring[(k + 1) % ring.len()];
            s += x1 * y2 - x2 * y1;
        }
        s.abs() / 2.0
    };
    let big = r.rings.iter().map(area).fold(0.0, f64::max);
    r.rings.iter().filter(|g| area(g) >= big * 0.25).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_countries_and_states() {
        let a = atlas();
        assert!(a.regions.len() > 300);
        let us = a.country("United States").unwrap();
        assert_eq!(a.regions[us].code, "USA");
        assert_eq!(a.country("USA"), Some(us));
        assert_eq!(a.country("U.S.A."), Some(us));
        assert_eq!(a.regions[a.country("Côte d'Ivoire").or(a.country("Ivory Coast")).unwrap()].code, "CIV");
        assert_eq!(a.regions[a.country("france").unwrap()].code, "FRA");
        assert_eq!(a.regions[a.country("Norway").unwrap()].code, "NOR");
        let wa = a.state("Washington", None).unwrap();
        assert_eq!(a.regions[wa].postal, "WA");
        let (st, ids) = a.resolve(&["California".into(), "Texas".into(), "NY".into()], &[]);
        assert!(st && ids.iter().all(|x| x.is_some()));
        let (st, ids) = a.resolve(&["France".into(), "Germany".into(), "Spain".into()], &[]);
        assert!(!st && ids.iter().all(|x| x.is_some()));
        assert!(a.states_of("CAN").len() >= 13);
        let p = Projection::Albers(29.5, 45.5, -96.0).project(-96.0, 40.0);
        assert!(p.0.abs() < 1e-9);
    }
}
