use slipbox_core::{RetainedSourceState, SourceChange, SourceChangeError, SourceId, SourceRecord};
use thiserror::Error;

pub const MAX_CATALOG_SOURCES: usize = 64;

/// Several configured sources with at most one explicit active selection.
///
/// Addition order is preserved; only `active` determines the selection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceCatalog {
    sources: Vec<SourceRecord>,
    active: Option<SourceId>,
}

impl SourceCatalog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstruct from caller-trusted records, checking bounds, unique IDs and
    /// selection. Repository lineage is not verified; updates to configured
    /// sources must use [`Self::apply`].
    pub fn from_parts(
        sources: Vec<SourceRecord>,
        active: Option<SourceId>,
    ) -> Result<Self, SourceCatalogError> {
        if sources.len() > MAX_CATALOG_SOURCES {
            return Err(SourceCatalogError::TooManySources {
                limit: MAX_CATALOG_SOURCES,
            });
        }
        for (position, record) in sources.iter().enumerate() {
            if sources[..position]
                .iter()
                .any(|earlier| earlier.id() == record.id())
            {
                return Err(SourceCatalogError::DuplicateSource(record.id().clone()));
            }
        }
        let unknown_active = active
            .as_ref()
            .filter(|active| !sources.iter().any(|record| record.id() == *active));
        if let Some(active) = unknown_active {
            return Err(SourceCatalogError::UnknownActiveSource(active.clone()));
        }
        Ok(Self { sources, active })
    }

    #[must_use]
    pub fn sources(&self) -> &[SourceRecord] {
        &self.sources
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    #[must_use]
    pub fn get(&self, id: &SourceId) -> Option<&SourceRecord> {
        self.sources.iter().find(|record| record.id() == id)
    }

    /// The selected source, if one is selected. A selection says which source
    /// the reader reads next, not that a readable generation of it is ready.
    #[must_use]
    pub fn active(&self) -> Option<&SourceRecord> {
        self.active.as_ref().and_then(|id| self.get(id))
    }

    #[must_use]
    pub fn active_id(&self) -> Option<&SourceId> {
        self.active.as_ref()
    }

    /// Add a source, which becomes the active selection when the catalog holds
    /// none. An addition never displaces an existing selection.
    pub fn add(&mut self, record: SourceRecord) -> Result<(), SourceCatalogError> {
        if self.get(record.id()).is_some() {
            return Err(SourceCatalogError::DuplicateSource(record.id().clone()));
        }
        if self.sources.len() == MAX_CATALOG_SOURCES {
            return Err(SourceCatalogError::TooManySources {
                limit: MAX_CATALOG_SOURCES,
            });
        }
        if self.active.is_none() {
            self.active = Some(record.id().clone());
        }
        self.sources.push(record);
        Ok(())
    }

    /// Apply a validated transition, preserving order and selection. Sync and
    /// storage must enforce the returned retention decision; reauthorization
    /// under another account retains identity alone.
    pub fn apply(
        &mut self,
        id: &SourceId,
        change: SourceChange,
    ) -> Result<RetainedSourceState, SourceCatalogError> {
        let Some(slot) = self.sources.iter_mut().find(|record| record.id() == id) else {
            return Err(SourceCatalogError::UnknownSource(id.clone()));
        };
        let transition = slot
            .apply(change)
            .map_err(|cause| SourceCatalogError::RefusedChange {
                id: id.clone(),
                cause,
            })?;
        let retained = transition.retained();
        *slot = transition.into_record();
        Ok(retained)
    }

    /// Select a configured source, replacing any current selection.
    pub fn set_active(&mut self, id: &SourceId) -> Result<(), SourceCatalogError> {
        if self.get(id).is_none() {
            return Err(SourceCatalogError::UnknownSource(id.clone()));
        }
        self.active = Some(id.clone());
        Ok(())
    }

    /// Clear the selection. The next addition becomes active.
    pub fn clear_active(&mut self) {
        self.active = None;
    }

    /// Remove a configured source, returning the record. Removing the active
    /// source clears the selection instead of promoting a neighbour.
    pub fn remove(&mut self, id: &SourceId) -> Result<SourceRecord, SourceCatalogError> {
        let Some(position) = self.sources.iter().position(|record| record.id() == id) else {
            return Err(SourceCatalogError::UnknownSource(id.clone()));
        };
        if self.active.as_ref() == Some(id) {
            self.active = None;
        }
        Ok(self.sources.remove(position))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceCatalogError {
    #[error("source {0} is already configured")]
    DuplicateSource(SourceId),
    #[error("source {0} is not configured")]
    UnknownSource(SourceId),
    #[error("the active source {0} is not configured")]
    UnknownActiveSource(SourceId),
    #[error("source {id} cannot be updated this way: {cause}")]
    RefusedChange {
        id: SourceId,
        cause: SourceChangeError,
    },
    #[error("a source catalog holds at most {limit} sources")]
    TooManySources { limit: usize },
}
