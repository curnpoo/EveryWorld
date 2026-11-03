//! Delivery missions: "get this parcel from here to there", planned as a
//! chain of legs across real airports and real launch sites.
//!
//! Example: Brooklyn -> Kyoto plans as
//!   drive to JFK -> fly JFK to MCO -> drive to Cape Canaveral ->
//!   rocket to Tanegashima -> fly to KIX -> drive to the door.

use ed_geo::Geodetic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubKind {
    Airport,
    Spaceport,
}

#[derive(Debug, Clone)]
pub struct Hub {
    pub code: &'static str,
    pub name: &'static str,
    pub kind: HubKind,
    pub pos: Geodetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Drive,
    Fly,
    Rocket,
}

#[derive(Debug, Clone)]
pub struct Leg {
    pub mode: Mode,
    pub from: Geodetic,
    pub to: Geodetic,
    pub to_name: String,
    pub distance_m: f64,
}

impl Leg {
    /// Rough real-time minutes at game speeds. Vehicles have "superpower"
    /// boost and cruise legs use time compression, so these are effective
    /// speeds, not top speeds: supercar w/ boost ~250 km/h, jet cruise warp
    /// ~3 km/s, rocket ~60 km/s. Tune these to hit the ~10-20 min trip target.
    pub fn est_minutes(&self) -> f64 {
        let v = match self.mode {
            Mode::Drive => 70.0,
            Mode::Fly => 3_000.0,
            Mode::Rocket => 60_000.0,
        };
        self.distance_m / v / 60.0 + 0.5
    }
}

const fn hub(code: &'static str, name: &'static str, kind: HubKind, lat: f64, lon: f64) -> Hub {
    Hub { code, name, kind, pos: Geodetic::new(lat, lon, 0.0) }
}

/// Real orbital launch sites.
pub const SPACEPORTS: &[Hub] = &[
    hub("KSC", "Kennedy Space Center", HubKind::Spaceport, 28.5729, -80.6490),
    hub("VSFB", "Vandenberg SFB", HubKind::Spaceport, 34.7420, -120.5724),
    hub("STB", "Starbase", HubKind::Spaceport, 25.9971, -97.1554),
    hub("WAL", "Wallops", HubKind::Spaceport, 37.8402, -75.4882),
    hub("CSG", "Guiana Space Centre", HubKind::Spaceport, 5.2390, -52.7686),
    hub("BAI", "Baikonur", HubKind::Spaceport, 45.9650, 63.3050),
    hub("PLE", "Plesetsk", HubKind::Spaceport, 62.9271, 40.5777),
    hub("VOS", "Vostochny", HubKind::Spaceport, 51.8844, 128.3339),
    hub("JSLC", "Jiuquan", HubKind::Spaceport, 40.9606, 100.2983),
    hub("WSLC", "Wenchang", HubKind::Spaceport, 19.6145, 110.9510),
    hub("XSLC", "Xichang", HubKind::Spaceport, 28.2463, 102.0265),
    hub("SDSC", "Satish Dhawan", HubKind::Spaceport, 13.7199, 80.2304),
    hub("TNSC", "Tanegashima", HubKind::Spaceport, 30.4009, 130.9750),
    hub("RLM", "Rocket Lab Mahia", HubKind::Spaceport, -39.2615, 177.8649),
    hub("SAX", "SaxaVord", HubKind::Spaceport, 60.8160, -0.7680),
    hub("AND", "Andøya", HubKind::Spaceport, 69.2943, 16.0207),
    hub("ALC", "Alcântara", HubKind::Spaceport, -2.3730, -44.3960),
    hub("WRR", "Woomera", HubKind::Spaceport, -30.9553, 136.5322),
];

/// Seed list of major airports. At runtime the game also learns airports
/// from the OpenMapTiles `aerodrome_label` layer as tiles stream in.
pub const AIRPORTS: &[Hub] = &[
    hub("JFK", "New York JFK", HubKind::Airport, 40.6413, -73.7781),
    hub("LAX", "Los Angeles", HubKind::Airport, 33.9416, -118.4085),
    hub("SFO", "San Francisco", HubKind::Airport, 37.6213, -122.3790),
    hub("ORD", "Chicago O'Hare", HubKind::Airport, 41.9742, -87.9073),
    hub("MCO", "Orlando", HubKind::Airport, 28.4312, -81.3081),
    hub("MIA", "Miami", HubKind::Airport, 25.7959, -80.2870),
    hub("DFW", "Dallas/Fort Worth", HubKind::Airport, 32.8998, -97.0403),
    hub("BRO", "Brownsville", HubKind::Airport, 25.9068, -97.4259),
    hub("SEA", "Seattle", HubKind::Airport, 47.4502, -122.3088),
    hub("YYZ", "Toronto", HubKind::Airport, 43.6777, -79.6248),
    hub("MEX", "Mexico City", HubKind::Airport, 19.4361, -99.0719),
    hub("GRU", "São Paulo", HubKind::Airport, -23.4356, -46.4731),
    hub("SLZ", "São Luís", HubKind::Airport, -2.5853, -44.2341),
    hub("EZE", "Buenos Aires", HubKind::Airport, -34.8222, -58.5358),
    hub("CAY", "Cayenne", HubKind::Airport, 4.8198, -52.3604),
    hub("LHR", "London Heathrow", HubKind::Airport, 51.4700, -0.4543),
    hub("LSI", "Sumburgh", HubKind::Airport, 59.8789, -1.2956),
    hub("CDG", "Paris CDG", HubKind::Airport, 49.0097, 2.5479),
    hub("NCE", "Nice", HubKind::Airport, 43.6584, 7.2159),
    hub("GVA", "Geneva", HubKind::Airport, 46.2381, 6.1090),
    hub("ZRH", "Zurich", HubKind::Airport, 47.4582, 8.5555),
    hub("FRA", "Frankfurt", HubKind::Airport, 50.0379, 8.5622),
    hub("AMS", "Amsterdam", HubKind::Airport, 52.3105, 4.7683),
    hub("MAD", "Madrid", HubKind::Airport, 40.4983, -3.5676),
    hub("FCO", "Rome", HubKind::Airport, 41.8003, 12.2389),
    hub("ANX", "Andøya", HubKind::Airport, 69.2925, 16.1442),
    hub("ARH", "Arkhangelsk", HubKind::Airport, 64.6003, 40.7167),
    hub("SVO", "Moscow", HubKind::Airport, 55.9726, 37.4146),
    hub("IST", "Istanbul", HubKind::Airport, 41.2753, 28.7519),
    hub("CAI", "Cairo", HubKind::Airport, 30.1219, 31.4056),
    hub("RAK", "Marrakech", HubKind::Airport, 31.6069, -8.0363),
    hub("DXB", "Dubai", HubKind::Airport, 25.2532, 55.3657),
    hub("NBO", "Nairobi", HubKind::Airport, -1.3192, 36.9278),
    hub("LOS", "Lagos", HubKind::Airport, 6.5774, 3.3212),
    hub("JNB", "Johannesburg", HubKind::Airport, -26.1367, 28.2411),
    hub("CPT", "Cape Town", HubKind::Airport, -33.9715, 18.6021),
    hub("DEL", "Delhi", HubKind::Airport, 28.5562, 77.1000),
    hub("BOM", "Mumbai", HubKind::Airport, 19.0896, 72.8656),
    hub("MAA", "Chennai", HubKind::Airport, 12.9941, 80.1709),
    hub("KZO", "Kyzylorda", HubKind::Airport, 44.7069, 65.5925),
    hub("PEK", "Beijing", HubKind::Airport, 40.0799, 116.6031),
    hub("PVG", "Shanghai", HubKind::Airport, 31.1443, 121.8083),
    hub("HAK", "Haikou", HubKind::Airport, 19.9349, 110.4590),
    hub("XIC", "Xichang", HubKind::Airport, 27.9891, 102.1844),
    hub("JGN", "Jiayuguan", HubKind::Airport, 39.8569, 98.3414),
    hub("BQS", "Blagoveshchensk", HubKind::Airport, 50.4254, 127.4120),
    hub("HKG", "Hong Kong", HubKind::Airport, 22.3080, 113.9185),
    hub("HND", "Tokyo Haneda", HubKind::Airport, 35.5494, 139.7798),
    hub("ITM", "Osaka Itami", HubKind::Airport, 34.7855, 135.4382),
    hub("KIX", "Osaka Kansai", HubKind::Airport, 34.4320, 135.2304),
    hub("TNE", "Tanegashima", HubKind::Airport, 30.6051, 130.9911),
    hub("ICN", "Seoul Incheon", HubKind::Airport, 37.4602, 126.4407),
    hub("SIN", "Singapore", HubKind::Airport, 1.3644, 103.9915),
    hub("BKK", "Bangkok", HubKind::Airport, 13.6900, 100.7501),
    hub("SYD", "Sydney", HubKind::Airport, -33.9399, 151.1753),
    hub("ADL", "Adelaide", HubKind::Airport, -34.9450, 138.5306),
    hub("AKL", "Auckland", HubKind::Airport, -37.0082, 174.7850),
    hub("NPE", "Napier", HubKind::Airport, -39.4658, 176.8700),
    hub("SMX", "Santa Maria", HubKind::Airport, 34.8989, -120.4575),
    hub("PHF", "Newport News", HubKind::Airport, 37.1319, -76.4930),
    hub("KEF", "Reykjavík", HubKind::Airport, 63.9850, -22.6056),
    hub("ANC", "Anchorage", HubKind::Airport, 61.1743, -149.9963),
    hub("HNL", "Honolulu", HubKind::Airport, 21.3245, -157.9251),
];

fn nearest<'a>(hubs: &'a [Hub], p: Geodetic) -> &'a Hub {
    hubs.iter().min_by(|a, b| a.pos.distance_to(p).total_cmp(&b.pos.distance_to(p))).expect("non-empty hub list")
}

fn leg(mode: Mode, from: Geodetic, to: Geodetic, name: impl Into<String>) -> Leg {
    Leg { mode, from, to, to_name: name.into(), distance_m: from.distance_to(to) }
}

/// Plan a multi-modal delivery. Short hops drive; medium hops fly; anything
/// past ~4,000 km takes a rocket between the launch sites nearest each end.
pub fn plan(from: Geodetic, to: Geodetic, dest_name: &str, airports: &[Hub]) -> Vec<Leg> {
    let d = from.distance_to(to);
    let mut legs = Vec::new();
    if d < 60_000.0 {
        legs.push(leg(Mode::Drive, from, to, dest_name));
        return legs;
    }
    // Launch sites have runways (KSC's Shuttle Landing Facility etc.), so
    // they count as airfields too.
    let fields: Vec<Hub> = airports.iter().chain(SPACEPORTS).cloned().collect();
    // Fly from `a` to `b` via nearest airfields, driving at both ends.
    let fly = |legs: &mut Vec<Leg>, a: Geodetic, b: Geodetic, b_name: &str| {
        let (ap_a, ap_b) = (nearest(&fields, a), nearest(&fields, b));
        if ap_a.code == ap_b.code {
            legs.push(leg(Mode::Drive, a, b, b_name));
            return;
        }
        legs.push(leg(Mode::Drive, a, ap_a.pos, ap_a.name));
        legs.push(leg(Mode::Fly, ap_a.pos, ap_b.pos, ap_b.name));
        legs.push(leg(Mode::Drive, ap_b.pos, b, b_name));
    };
    if d < 4_000_000.0 {
        fly(&mut legs, from, to, dest_name);
        return legs;
    }
    let (sp_a, sp_b) = (nearest(SPACEPORTS, from), nearest(SPACEPORTS, to));
    if sp_a.code == sp_b.code {
        fly(&mut legs, from, to, dest_name);
        return legs;
    }
    fly(&mut legs, from, sp_a.pos, sp_a.name);
    legs.push(leg(Mode::Rocket, sp_a.pos, sp_b.pos, sp_b.name));
    fly(&mut legs, sp_b.pos, to, dest_name);
    // Drop zero-length drives.
    legs.retain(|l| l.distance_m > 50.0);
    legs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brooklyn_to_kyoto_uses_a_rocket() {
        let brooklyn = Geodetic::new(40.6782, -73.9442, 0.0);
        let kyoto = Geodetic::new(35.0116, 135.7681, 0.0);
        let legs = plan(brooklyn, kyoto, "Kyoto", AIRPORTS);
        let modes: Vec<_> = legs.iter().map(|l| l.mode).collect();
        assert!(modes.contains(&Mode::Rocket), "{legs:#?}");
        assert_eq!(legs.first().unwrap().mode, Mode::Drive);
        assert_eq!(legs.last().unwrap().mode, Mode::Drive);
        let rocket = legs.iter().find(|l| l.mode == Mode::Rocket).unwrap();
        assert_eq!(rocket.to_name, "Tanegashima");
        let total: f64 = legs.iter().map(Leg::est_minutes).sum();
        assert!(total < 45.0, "trip should be a session, not a day: {total} min");
    }

    #[test]
    fn local_delivery_just_drives() {
        let a = Geodetic::new(48.85, 2.35, 0.0);
        let b = Geodetic::new(48.87, 2.30, 0.0);
        assert_eq!(plan(a, b, "x", AIRPORTS).len(), 1);
    }
}
