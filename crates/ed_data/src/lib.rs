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
//! | Place search | Open-Meteo geocoding (GeoNames), OSM Nominatim | CC BY 4.0, ODbL |
//!
//! Every call here is blocking; the game runs them on Bevy's IO task pool.

pub mod cache;
pub mod climate;
pub mod canopy_height;
pub mod cog;
pub mod elevation;
pub mod geocode;
pub mod geology;
pub mod http;
pub mod mvt;
pub mod node;
pub mod osmtags;
pub mod overture;
pub mod roadz;
pub mod satellite;
pub mod vector;
pub mod weather;
pub mod worldcover;

pub use cache::DiskCache;

/// Pinned data snapshot version. Every cache key and generated tile is stamped
/// with this so all clients (and save files) agree on one world. Bump it when
/// a data source or its parsing changes; old caches simply miss and refetch.
/// Multiplayer relies on this being identical across machines.
pub const DATA_VERSION: &str = "2026-11-06";
