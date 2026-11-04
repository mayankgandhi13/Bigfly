//! Helpers shared by game implementations.

use crate::core::types::ConceptId;

/// Count hints before the first answer action, and return that answer.
/// Only the first answer counts: retrying until correct must not earn credit.
pub fn first_answer<A>(log: &[A], is_answer: impl Fn(&A) -> bool) -> (u16, Option<&A>)
where
    A: HintAction,
{
    let mut hints: u16 = 0;
    for a in log {
        if is_answer(a) {
            return (hints, Some(a));
        }
        if a.is_hint() {
            hints = hints.saturating_add(1);
        }
    }
    (hints, None)
}

pub trait HintAction {
    fn is_hint(&self) -> bool;
}

pub fn owned(ids: &[&str]) -> Vec<ConceptId> {
    ids.iter().map(|s| s.to_string()).collect()
}
