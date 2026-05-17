pub mod collisionpolicy;
pub mod globalconfig;
pub mod rootsetconfig;
pub mod slideconfig;
pub mod volumeconfig;

pub(crate) const DEFAULT_SLIDE_CONFIG_FILE: &str = ".slide.yml";
pub(crate) const DEFAULT_VOLUME_CONFIG_FILE: &str = ".volume.yml";

pub use collisionpolicy::CollisionPolicy;
pub use globalconfig::Algorithm;
pub use globalconfig::GlobalConfig;
pub use rootsetconfig::RootsetConfig;
pub use slideconfig::SlideConfig;
pub use volumeconfig::VolumeConfig;
