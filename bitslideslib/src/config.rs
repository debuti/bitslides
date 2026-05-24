pub mod collisionpolicy;
pub mod globalconfig;
pub mod slideconfig;
pub mod volumeconfig;

pub const DEFAULT_SLIDE_CONFIG_FILE: &str = ".slide.yml";
pub const DEFAULT_VOLUME_CONFIG_FILE: &str = ".volume.yml";

pub const EVENT_CHANNEL_CAPACITY: usize = 1024;
pub const COMMAND_CHANNEL_CAPACITY: usize = 1024;

/// Capacity of the internal synchronization trigger channel.
///
/// 1 slot for the current event and 1 slot for a possible next event.
///
pub const SYNCJOB_CHANNEL_CAPACITY: usize = 2;

/// Capacity of the trace channel.
///
/// Best effort oriented. Any message not fitting this will be discarded.
pub const TRACE_CHANNEL_CAPACITY: usize = 32;

pub use collisionpolicy::CollisionPolicy;
pub use globalconfig::Algorithm;
pub use globalconfig::GlobalConfig;
pub use slideconfig::SlideConfig;
pub use volumeconfig::VolumeConfig;
