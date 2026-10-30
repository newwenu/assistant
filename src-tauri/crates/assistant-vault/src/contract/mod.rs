#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precondition {
    None,
    AnyState,
    Unlocked,
}
