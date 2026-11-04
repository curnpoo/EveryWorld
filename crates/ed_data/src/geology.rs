//! Bedrock lithology from Macrostrat (CC BY 4.0, <https://macrostrat.org>):
//! the dominant rock type under a point, reduced to a few classes that decide
//! the colour of exposed rock, scree and sand. Looked up once per tile and
//! cached forever; offline just means "unknown" (biome colours apply).

use crate::DiskCache;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lithology {
    Granitic,
    Basalt,
    Limestone,
    Sandstone,
    Shale,
    Metamorphic,
    Volcanic,
    Unconsolidated,
}

impl Lithology {
    /// Classify a Macrostrat `lith` description by keyword (most specific first).
    pub fn from_text(text: &str) -> Option<Lithology> {
        let t = text.to_ascii_lowercase();
        let has = |ks: &[&str]| ks.iter().any(|k| t.contains(k));
        Some(if has(&["basalt", "mafic", "ophiolite", "gabbro", "peridotite"]) {
            Lithology::Basalt
        } else if has(&["granit", "granod", "diorit", "plutonic", "intrusive", "syenit", "gneiss"]) {
            Lithology::Granitic
        } else if has(&["volcan", "rhyol", "andesit", "tuff", "ignimbrite", "lava"]) {
            Lithology::Volcanic
        } else if has(&["limestone", "carbonate", "dolo", "chalk", "marl"]) {
            Lithology::Limestone
        } else if has(&["alluvi", "gravel", "moraine", "loess", "unconsolidated", "lockergestein", "quaternary", "glacial"]) {
            Lithology::Unconsolidated
        } else if has(&["sandstone", "arkose", "conglomerate", "quartzite", "sand "]) || t == "sand" {
            Lithology::Sandstone
        } else if has(&["shale", "mud", "clay", "silt", "argillite", "slate", "claystone"]) {
            Lithology::Shale
        } else if has(&["schist", "phyllite", "metamorph", "marble", "amphibolite", "meta-"]) {
            Lithology::Metamorphic
        } else {
            return None;
        })
    }

    /// sRGB hex of freshly exposed rock.
    pub fn rock_rgb(self) -> u32 {
        match self {
            Lithology::Granitic => 0xa59f98,
            Lithology::Basalt => 0x4f4c4a,
            Lithology::Limestone => 0xcfc7b4,
            Lithology::Sandstone => 0xb98a5a,
            Lithology::Shale => 0x6f6a63,
            Lithology::Metamorphic => 0x7d786f,
            Lithology::Volcanic => 0x8a6a58,
            Lithology::Unconsolidated => 0x9b8f7c,
        }
    }

    /// sRGB hex of the sand/soil weathered from it.
    pub fn sand_rgb(self) -> u32 {
        match self {
            Lithology::Granitic => 0xd3b98f,
            Lithology::Basalt => 0x4a4440,
            Lithology::Limestone => 0xe6dcc0,
            Lithology::Sandstone => 0xd8a066,
            Lithology::Shale => 0x9a8a72,
            Lithology::Metamorphic => 0xb09b78,
            Lithology::Volcanic => 0x9a6e52,
            Lithology::Unconsolidated => 0xcdb98f,
        }
    }
}

/// Dominant lithology under a lat/lon, preferring the largest-scale
/// (most detailed) map unit that classifies. `Ok(None)` means no usable data.
pub fn lookup(lat: f64, lon: f64, cache: &DiskCache) -> anyhow::Result<Option<Lithology>> {
    // Quantise so neighbouring tiles share the cached answer (~1 km).
    let (qla, qlo) = ((lat * 100.0).round() / 100.0, (lon * 100.0).round() / 100.0);
    let key = format!("macrostrat/{qla:.2}_{qlo:.2}.json");
    let bytes = cache.get_or_fetch(&key, None, || {
        crate::http::get_bytes(&format!("https://macrostrat.org/api/v2/geologic_units/map?lat={qla}&lng={qlo}&response=short"))
    })?;
    Ok(parse(&bytes))
}

fn parse(bytes: &[u8]) -> Option<Lithology> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let units = v.pointer("/success/data")?.as_array()?;
    let rank = |s: &str| match s {
        "large" => 0,
        "medium" => 1,
        "small" => 2,
        _ => 3,
    };
    let mut best: Option<(i32, Lithology)> = None;
    for u in units {
        let text = ["lith", "name", "descrip"].iter().filter_map(|k| u.get(*k)?.as_str()).collect::<Vec<_>>().join(" ");
        if let Some(l) = Lithology::from_text(&text) {
            let r = rank(u.get("scale").and_then(|s| s.as_str()).unwrap_or(""));
            if best.is_none_or(|(br, _)| r < br) {
                best = Some((r, l));
            }
        }
    }
    best.map(|(_, l)| l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords() {
        assert_eq!(Lithology::from_text("Loamy gravel, sand, silt, blocks (moraine)"), Some(Lithology::Unconsolidated));
        assert_eq!(Lithology::from_text("ophiolite complex; mafic volcanic rocks"), Some(Lithology::Basalt));
        assert_eq!(Lithology::from_text("continental sedimentary"), None);
        assert_eq!(Lithology::from_text("Limestone and dolostone"), Some(Lithology::Limestone));
    }

    #[test]
    fn parses_response() {
        let j = br##"{"success":{"data":[{"lith":"granite","scale":"small"},{"lith":"alluvium","scale":"large"}]}}"##;
        assert_eq!(parse(j), Some(Lithology::Unconsolidated));
        assert_eq!(parse(b"junk"), None);
    }

    #[test]
    #[ignore = "network"]
    fn real_lookup() {
        let c = DiskCache::default_location();
        assert!(lookup(46.0, 7.7, &c).unwrap().is_some());
    }
}
