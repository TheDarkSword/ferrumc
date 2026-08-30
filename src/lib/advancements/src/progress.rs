//! What a player has done, and what that earns them.

use crate::{Advancement, Advancements};
use bitcode_derive::{Decode, Encode};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How far along one advancement a player is.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Encode, Decode)]
pub struct AdvancementProgress {
    /// When each criterion was granted, as milliseconds since the epoch. Vanilla keeps the same
    /// instant, and shows it in the advancement screen.
    pub criteria: BTreeMap<String, i64>,
}

impl AdvancementProgress {
    #[must_use]
    pub fn is_granted(&self, criterion: &str) -> bool {
        self.criteria.contains_key(criterion)
    }

    /// Whether enough criteria are granted for the advancement to be finished.
    #[must_use]
    pub fn is_done(&self, advancement: &Advancement) -> bool {
        advancement
            .requirements
            .met(|criterion| self.is_granted(criterion))
    }

    #[must_use]
    pub fn has_progress(&self) -> bool {
        !self.criteria.is_empty()
    }
}

/// What came of granting a criterion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Award {
    /// It was already granted, so nothing happened.
    Already,
    /// It was granted, and the advancement is still unfinished.
    Progressed,
    /// It was granted, and that finished the advancement.
    Completed,
}

/// Everything one player has done.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Encode, Decode)]
pub struct PlayerAdvancements {
    /// Only the advancements with something to say are kept, as vanilla writes them.
    pub progress: BTreeMap<String, AdvancementProgress>,
}

impl PlayerAdvancements {
    /// Grants one criterion, and says whether that finished the advancement.
    pub fn award(
        &mut self,
        advancements: &Advancements,
        name: &str,
        criterion: &str,
        now: i64,
    ) -> Award {
        let Some(advancement) = advancements.get_by_name(name) else {
            return Award::Already;
        };
        let progress = self.progress.entry(name.to_owned()).or_default();
        if progress.is_granted(criterion) {
            return Award::Already;
        }
        let was_done = progress.is_done(advancement);
        progress.criteria.insert(criterion.to_owned(), now);
        if !was_done && progress.is_done(advancement) {
            Award::Completed
        } else {
            Award::Progressed
        }
    }

    /// Takes a criterion back, which is what `/advancement revoke` does.
    pub fn revoke(&mut self, name: &str, criterion: &str) -> bool {
        let Some(progress) = self.progress.get_mut(name) else {
            return false;
        };
        let removed = progress.criteria.remove(criterion).is_some();
        if !progress.has_progress() {
            self.progress.remove(name);
        }
        removed
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&AdvancementProgress> {
        self.progress.get(name)
    }

    #[must_use]
    pub fn is_done(&self, advancements: &Advancements, name: &str) -> bool {
        match (self.progress.get(name), advancements.get_by_name(name)) {
            (Some(progress), Some(advancement)) => progress.is_done(advancement),
            _ => false,
        }
    }

    /// Offers an event to every criterion waiting for it, granting the ones it meets.
    ///
    /// Returns what was granted: the advancements that moved, and which of them that finished.
    pub fn offer(
        &mut self,
        advancements: &Advancements,
        now: i64,
        mut meets: impl FnMut(&crate::Trigger) -> bool,
    ) -> Granted {
        let mut granted = Granted::default();
        // Which criteria to grant is worked out first, so the borrow of the advancements ends
        // before anything is written back.
        let mut granting = Vec::new();
        for (name, advancement) in advancements.iter() {
            for (criterion, condition) in &advancement.criteria {
                if meets(&condition.trigger) {
                    granting.push((name.to_owned(), criterion.clone()));
                }
            }
        }
        for (name, criterion) in granting {
            match self.award(advancements, &name, &criterion, now) {
                Award::Completed => {
                    granted.moved.push(name.clone());
                    granted.completed.push(name);
                }
                Award::Progressed => granted.moved.push(name),
                Award::Already => {}
            }
        }
        granted
    }
}

/// What an event granted.
#[derive(Clone, Debug, Default)]
pub struct Granted {
    /// Every advancement that gained a criterion, finished or not.
    pub moved: Vec<String>,
    /// The ones that were finished by it.
    pub completed: Vec<String>,
}

impl Granted {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.moved.is_empty()
    }
}
