pub mod health;
pub mod spa;
pub mod version;

pub use health::health_router;
pub use spa::spa_router;
pub use version::{VERSION_RANDOM_FAILED, version_router};
