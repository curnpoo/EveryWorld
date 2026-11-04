//! Public Earth data, fetched over HTTP and cached on disk.
//!
//! | Layer        | Source                                   | Licence          |
//! |--------------|------------------------------------------|------------------|
//! | Elevation    | AWS Terrain Tiles (Terrarium PNG)        | various, open    |
//! | Map vectors  | OpenFreeMap (OpenMapTiles schema, OSM)   | ODbL             |
//! | Weather      | Open-Meteo                               | CC BY 4.0        |
//! | Place search | Open-Meteo geocoding (GeoNames)          | CC BY 4.0        |
//!
//! Every call here is blocking; the game runs them on Bevy's IO task pool.

pub mod cache;
pub mod elevation;
pub mod geocode;
pub mod http;
pub mod mvt;
pub mod satellite;
pub mod vector;
pub mod weather;

pub use cache::DiskCache;
