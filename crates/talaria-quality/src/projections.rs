// crates/talaria-quality/src/projections.rs
//! BuildProjections interface only (Livrable 1).
//! Full API/OpenAPI contract changes belong to Livrable 3 / Phase 2.
//!
//! Invariant: projections must not treat persisted `title` as source of truth;
//! derive display labels from typed fields (subject, event_type, time, place).

use crate::model::{EventCandidate, TypedTime};

/// Minimal projection input assembled from quality events (not DB title).
#[derive(Debug, Clone)]
pub struct ProjectionEvent {
    pub subject_label: String,
    pub event_type: String,
    pub predicate: String,
    pub time: TypedTime,
    pub place_label: Option<String>,
    pub map_eligible: bool,
}

pub trait BuildProjections {
    fn display_label(&self, event: &ProjectionEvent) -> String;
    fn from_candidate(&self, candidate: &EventCandidate, subject_label: &str) -> ProjectionEvent;
}

/// Default label builder — title is derived, never stored as authority.
pub struct DerivedLabelProjections;

impl BuildProjections for DerivedLabelProjections {
    fn display_label(&self, event: &ProjectionEvent) -> String {
        let time = match &event.time {
            TypedTime::Exact { year, .. } | TypedTime::Approx { year, .. } => year.to_string(),
            TypedTime::Range {
                start_year,
                end_year,
                ..
            } => format!("{start_year}–{end_year}"),
            TypedTime::Unknown { .. } => "?".into(),
        };
        match &event.place_label {
            Some(place) => format!(
                "{} — {} ({}) @ {}",
                event.subject_label, event.event_type, time, place
            ),
            None => format!("{} — {} ({})", event.subject_label, event.event_type, time),
        }
    }

    fn from_candidate(&self, candidate: &EventCandidate, subject_label: &str) -> ProjectionEvent {
        ProjectionEvent {
            subject_label: subject_label.to_string(),
            event_type: candidate.event_type.clone(),
            predicate: candidate.predicate.clone(),
            time: candidate.time.clone(),
            place_label: candidate.place_label.clone(),
            map_eligible: candidate.place_label.is_some()
                && !matches!(candidate.time, TypedTime::Unknown { .. }),
        }
    }
}
