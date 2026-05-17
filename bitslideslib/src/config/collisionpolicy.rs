/// Policy to apply in case of a file collision
///
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CollisionPolicy {
    /// Overwrite the destination file
    Overwrite,
    /// Skip the file
    Skip,
    /// Rename the file
    Rename { suffix: String },
    /// Fail the operation
    Fail,
}
