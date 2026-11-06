//! Public Earth data, fetched over HTTP and cached on disk.
//!
//! | Layer        | Source                                   | Licence          |
//! |--------------|------------------------------------------|------------------|
//! | Elevation    | AWS Terrain Tiles (Terrarium PNG)        | various, open    |
//! | Map vectors  | OpenFreeMap (OpenMapTiles schema, OSM)   | ODbL             |
//! | Buildings    | Overture Maps (baked offline, pinned)    | ODbL / CDLA      |
//! | Weather      | Open-Meteo                               | CC BY 4.0        |
//! | Climate      | WorldClim 2.1 normals (embedded)         | CC BY 4.0        |
//! | Land cover   | ESA WorldCover 2021 (COG range reads)    | CC BY 4.0        |
//! | Geology      | Macrostrat                               | CC BY 4.0        |
//! | Place search | Open-Meteo geocoding (GeoNames)          | CC BY 4.0        |
//!
//! Every call here is blocking; the game runs them on Bevy's IO task pool.

pub mod cache;
pub mod climate;
pub mod cog;
pub mod elevation;
pub mod geocode;
pub mod geology;
pub mod http;
pub mod mvt;
pub mod overture;
pub mod satellite;
pub mod vector;
pub mod weather;
pub mod worldcover;

pub use cache::DiskCache;
