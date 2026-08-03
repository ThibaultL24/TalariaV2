// crates/talaria-sources/src/density.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityTargets {
    pub target_timeline_events: u32,
    pub target_map_events: u32,
    pub max_documents: u32,
    pub max_linked_entities: u32,
    pub max_depth: u16,
    pub max_documents_per_source: u32,
}

impl Default for DensityTargets {
    fn default() -> Self {
        Self {
            target_timeline_events: 500,
            target_map_events: 500,
            max_documents: 10_000,
            max_linked_entities: 5_000,
            max_depth: 3,
            max_documents_per_source: 2_500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityProgress {
    pub timeline_events: u32,
    pub map_events: u32,
    pub documents_processed: u32,
    pub target_reached: bool,
    pub status: String,
}

impl DensityProgress {
    pub fn evaluate(&self, targets: &DensityTargets) -> Self {
        let mut out = self.clone();
        out.target_reached = self.timeline_events >= targets.target_timeline_events
            && self.map_events >= targets.target_map_events;
        out.status = if out.target_reached {
            "target_reached".into()
        } else {
            "target_not_reached".into()
        };
        out
    }
}
