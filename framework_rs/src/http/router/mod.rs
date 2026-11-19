pub(crate) mod health;
pub(crate) mod spa;
pub(crate) mod version;

pub use health::health_router;
pub use spa::spa_router;
pub use version::{VERSION_RANDOM_FAILED, version_router};
