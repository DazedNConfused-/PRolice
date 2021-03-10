use crate::scoring::score::Score;

/// Trait that reflects that a particular `type` is able to produce a [`Score`] based on its inner data.
pub trait Scorable {
    fn get_score(&self) -> Score;
}
