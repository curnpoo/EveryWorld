//! Road marking rules by country: colours, widths, dash patterns, edge lines, crosswalks.
//!
//! Values come from the national manuals as summarised in BeamNG.Earth's
//! `docs/research/2026-10-28-road-markings-standards.md` (MUTCD 11th ed., UK Traffic Signs Manual ch. 5,
//! France IISR 7e partie, Germany RMS, Japan MLIT 1974 circular, Australia AS 1742.2, Canada TAC/OTM). Countries
//! the research did not cover (Spain, Italy, Korea ...) fall back to the neutral default or to a neighbour with
//! the same practice, and are marked UNVERIFIED in the table. Country detection is a set of bounding boxes: it is
//! right in the interior of a country and approximate within a few tens of km of a border.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Country {
    Us,
    Canada,
    LatinAmerica,
    Britain,
    Ireland,
    Germany,
    France,
    Netherlands,
    Japan,
    China,
    Australia,
    Other,
}

/// Which country a point is in (bounding boxes, most specific first).
pub fn locate(lat: f64, lon: f64) -> Country {
    let inb = |s: f64, n: f64, w: f64, e: f64| lat >= s && lat <= n && lon >= w && lon <= e;
    // Islands and small countries first.
    if inb(51.3, 55.45, -10.8, -5.4) && !inb(54.0, 55.45, -8.2, -5.4) {
        return Country::Ireland;
    }
    if inb(49.8, 60.9, -8.7, 1.8) {
        return Country::Britain;
    }
    if inb(50.7, 53.6, 3.3, 7.2) {
        return Country::Netherlands;
    }
    if inb(47.2, 55.1, 5.8, 15.1) {
        return Country::Germany;
    }
    if inb(41.3, 51.2, -5.2, 9.7) {
        return Country::France;
    }
    if inb(24.0, 46.0, 122.9, 146.0) {
        return Country::Japan;
    }
    if inb(18.0, 53.6, 73.5, 135.0) {
        return Country::China;
    }
    if inb(-47.5, -10.0, 112.0, 179.0) {
        return Country::Australia;
    }
    // The Americas: Alaska and Hawaii, then Canada north of the border line, then the lower 48, then the south.
    if inb(51.0, 71.5, -170.0, -141.0) || inb(18.5, 22.5, -161.0, -154.5) {
        return Country::Us;
    }
    if lat >= 41.7 && lon >= -141.0 && lon <= -52.0 && lat > canada_border_lat(lon) {
        return Country::Canada;
    }
    if inb(24.5, 49.4, -125.0, -66.9) {
        return Country::Us;
    }
    if inb(-56.0, 32.7, -118.4, -34.0) {
        return Country::LatinAmerica;
    }
    Country::Other
}

/// Latitude of the US-Canada border at a longitude, a coarse polyline (the lakes and the 45th parallel).
fn canada_border_lat(lon: f64) -> f64 {
    match lon {
        l if l < -95.0 => 49.0,
        l if l < -89.6 => 48.5,
        l if l < -84.6 => 47.0,
        l if l < -82.0 => 46.2,
        l if l < -79.2 => 42.4,
        l if l < -76.4 => 43.4,
        l if l < -75.0 => 44.3,
        l if l < -71.5 => 45.0,
        l if l < -67.7 => 45.5,
        _ => 0.0,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MarkStyle {
    pub yellow_centre: bool,
    /// Broken centre line (paint, gap) in metres: outside towns / in towns.
    pub centre: (f32, f32),
    pub centre_urban: (f32, f32),
    pub centre_w: f32,
    /// Broken lane line, same two settings, and widths (ordinary, motorway).
    pub lane: (f32, f32),
    pub lane_urban: (f32, f32),
    pub lane_w: f32,
    pub lane_w_motorway: f32,
    /// Edge line widths (ordinary, motorway) and an optional broken pattern on ordinary roads.
    pub edge_w: f32,
    pub edge_w_motorway: f32,
    pub edge_dash: Option<(f32, f32)>,
    /// Edge line colour is yellow (Ireland).
    pub edge_yellow: bool,
    /// The left edge of a one-way carriageway or ramp is yellow (the Americas).
    pub left_edge_yellow: bool,
    /// Double solid centre line on urban arterials and on wide undivided roads (US, Canada).
    pub double_centre: bool,
    /// No centre line on paved roads narrower than this (m).
    pub min_centre_m: f32,
    /// Edge lines from this road rank up (1 minor ... 5 motorway).
    pub edge_min_rank: u8,
    /// Crosswalk bars: width, gap (m); stop line width.
    pub zebra: (f32, f32),
    pub stop_w: f32,
}

const NEUTRAL: MarkStyle = MarkStyle {
    yellow_centre: false,
    centre: (3.0, 6.0),
    centre_urban: (3.0, 6.0),
    centre_w: 0.10,
    lane: (3.0, 9.0),
    lane_urban: (3.0, 9.0),
    lane_w: 0.10,
    lane_w_motorway: 0.15,
    edge_w: 0.10,
    edge_w_motorway: 0.15,
    edge_dash: None,
    edge_yellow: false,
    left_edge_yellow: false,
    double_centre: false,
    min_centre_m: 4.9,
    edge_min_rank: 2,
    zebra: (0.5, 0.5),
    stop_w: 0.30,
};

pub fn style_for(c: Country) -> MarkStyle {
    use Country::*;
    match c {
        // MUTCD 11th ed.: yellow centre, 10 ft / 30 ft broken, 4-6 in lines, 6 in on freeways, yellow left edge,
        // double solid on 4+ lane undivided roads, no centre line under 16 ft (4.9 m), 24 in continental bars.
        Us => MarkStyle {
            yellow_centre: true,
            centre: (3.05, 9.14),
            centre_urban: (3.05, 9.14),
            centre_w: 0.12,
            lane: (3.05, 9.14),
            lane_urban: (3.05, 9.14),
            lane_w: 0.12,
            edge_w: 0.12,
            left_edge_yellow: true,
            double_centre: true,
            zebra: (0.6, 0.6),
            stop_w: 0.45,
            ..NEUTRAL
        },
        // TAC / OTM: like the US with 3 m / 6 m broken lines.
        Canada => MarkStyle { yellow_centre: true, centre: (3.0, 6.0), centre_urban: (3.0, 6.0), centre_w: 0.12, lane_w: 0.12, edge_w: 0.12, left_edge_yellow: true, double_centre: true, zebra: (0.6, 0.6), stop_w: 0.45, ..NEUTRAL },
        // UNVERIFIED pattern lengths: the Americas' yellow centre and yellow left edge, US-like dashes.
        LatinAmerica => MarkStyle { yellow_centre: true, centre: (3.0, 9.0), centre_urban: (3.0, 9.0), centre_w: 0.12, left_edge_yellow: true, double_centre: true, stop_w: 0.40, ..NEUTRAL },
        // TSRGD / TSM ch.5: white; centre 2/4 m (<= 40 mph) or 3/6 m; lane 1/5 or 2/7; 100 mm, 150 on motorways,
        // 200 mm motorway edges; zebra stripes 0.5 m.
        Britain => MarkStyle {
            centre: (3.0, 6.0),
            centre_urban: (2.0, 4.0),
            lane: (2.0, 7.0),
            lane_urban: (1.0, 5.0),
            edge_w_motorway: 0.20,
            stop_w: 0.25,
            ..NEUTRAL
        },
        // As Britain, but edge lines are yellow.
        Ireland => MarkStyle { centre: (3.0, 6.0), centre_urban: (2.0, 4.0), lane: (2.0, 7.0), lane_urban: (1.0, 5.0), edge_w_motorway: 0.20, edge_yellow: true, stop_w: 0.25, ..NEUTRAL },
        // RMS: Schmalstrich 0.12 (0.15 Autobahn), Breitstrich 0.30 on Autobahn edges; Leitlinie 6/12 m outside
        // towns, 3/6 m in towns, only on roads >= 5.5 m wide; zebra 0.5/0.5; Haltlinie 0.5.
        Germany => MarkStyle {
            centre: (6.0, 12.0),
            centre_urban: (3.0, 6.0),
            centre_w: 0.12,
            lane: (6.0, 12.0),
            lane_urban: (3.0, 6.0),
            lane_w: 0.12,
            edge_w: 0.12,
            edge_w_motorway: 0.30,
            min_centre_m: 5.5,
            stop_w: 0.50,
            ..NEUTRAL
        },
        // IISR art.113: T1 3/10 m outside towns, T'1 1.5/5 m in towns; edge T2 3/3.5 m dashes on ordinary roads,
        // solid on motorways; bars 0.5 m with 0.5-0.8 m gaps; stop line 0.5 m.
        France => MarkStyle {
            centre: (3.0, 10.0),
            centre_urban: (1.5, 5.0),
            centre_w: 0.10,
            lane: (3.0, 10.0),
            lane_urban: (1.5, 5.0),
            lane_w: 0.10,
            lane_w_motorway: 0.15,
            edge_w: 0.15,
            edge_dash: Some((3.0, 3.5)),
            zebra: (0.5, 0.65),
            stop_w: 0.50,
            ..NEUTRAL
        },
        // CROW (secondary): 3-9 centre and lane lines, 3-3 broken edge lines on rural roads, 0.10 m (0.15 rural).
        Netherlands => MarkStyle { centre: (3.0, 9.0), centre_urban: (3.0, 9.0), centre_w: 0.10, lane: (3.0, 9.0), lane_urban: (3.0, 9.0), edge_w: 0.15, edge_dash: Some((3.0, 3.0)), ..NEUTRAL },
        // MLIT 1974: white broken centre 5/5 m 0.15; lane 6/9 m; edges 0.15; bars 0.45 m; stop line 0.45 m.
        Japan => MarkStyle {
            centre: (5.0, 5.0),
            centre_urban: (5.0, 5.0),
            centre_w: 0.15,
            lane: (6.0, 9.0),
            lane_urban: (6.0, 9.0),
            lane_w: 0.15,
            edge_w: 0.15,
            edge_w_motorway: 0.20,
            zebra: (0.45, 0.65),
            stop_w: 0.45,
            ..NEUTRAL
        },
        // UNVERIFIED from memory (GB 5768): yellow centre, white lanes, 4/6 m, 0.15 m lines.
        China => MarkStyle { yellow_centre: true, centre: (4.0, 6.0), centre_urban: (2.0, 4.0), centre_w: 0.15, lane: (4.0, 6.0), lane_urban: (2.0, 4.0), lane_w: 0.15, edge_w: 0.15, ..NEUTRAL },
        // AS 1742.2: white; S1 centre 3/9 m 0.10; lane 3/9 m; edge solid 0.12 (0.15 freeway).
        Australia => MarkStyle { centre: (3.0, 9.0), centre_urban: (3.0, 9.0), lane: (3.0, 9.0), lane_urban: (3.0, 9.0), edge_w: 0.12, edge_w_motorway: 0.15, stop_w: 0.35, ..NEUTRAL },
        Other => NEUTRAL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countries_by_city() {
        for (lat, lon, want) in [
            (40.71, -74.0, Country::Us),
            (34.05, -118.24, Country::Us),
            (61.2, -149.9, Country::Us),
            (43.65, -79.38, Country::Canada),
            (45.42, -75.7, Country::Canada),
            (49.28, -123.1, Country::Canada),
            (42.89, -78.88, Country::Us), // Buffalo
            (42.33, -83.05, Country::Us), // Detroit
            (47.6, -122.3, Country::Us),
            (19.43, -99.13, Country::LatinAmerica),
            (-23.55, -46.63, Country::LatinAmerica),
            (51.5, -0.12, Country::Britain),
            (53.35, -6.26, Country::Ireland),
            (54.6, -5.93, Country::Britain), // Belfast
            (52.37, 4.9, Country::Netherlands),
            (48.14, 11.58, Country::Germany),
            (48.85, 2.35, Country::France),
            (35.68, 139.7, Country::Japan),
            (31.23, 121.47, Country::China),
            (-33.87, 151.2, Country::Australia),
            (46.02, 7.75, Country::France), // Zermatt: inside the France box (Switzerland is not modelled)
        ] {
            assert_eq!(locate(lat, lon), want, "{lat},{lon}");
        }
    }

    #[test]
    fn rules_follow_the_manuals() {
        let us = style_for(Country::Us);
        assert!(us.yellow_centre && us.left_edge_yellow && us.double_centre);
        assert!((us.centre.0 - 3.05).abs() < 1e-3 && (us.centre.1 - 9.14).abs() < 1e-3);
        let gb = style_for(Country::Britain);
        assert!(!gb.yellow_centre && gb.centre_urban == (2.0, 4.0) && gb.centre == (3.0, 6.0));
        assert!(style_for(Country::Ireland).edge_yellow && !gb.edge_yellow);
        assert_eq!(style_for(Country::Germany).edge_w_motorway, 0.30);
        assert_eq!(style_for(Country::France).edge_dash, Some((3.0, 3.5)));
        assert_eq!(style_for(Country::Japan).centre, (5.0, 5.0));
        assert!(style_for(Country::Germany).min_centre_m > us.min_centre_m);
    }
}
