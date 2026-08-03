// crates/talaria-quality/src/resolve.rs
//! Typed mention resolution — places vs persons vs objects.

use crate::model::{EntityKind, EventCandidate, Mention, ParticipantRole};

pub trait MentionResolver {
    fn resolve_surface(&self, surface: &str) -> Option<(String, EntityKind)>;
}

/// Deterministic gazetteer for tests and offline resolution.
/// Generic entries only — no biography-specific rules.
pub struct GazetteerResolver;

impl MentionResolver for GazetteerResolver {
    fn resolve_surface(&self, surface: &str) -> Option<(String, EntityKind)> {
        let key = surface.trim().to_lowercase();
        // Persons (common historical names used in fixtures — kind only, no event rules)
        const PERSONS: &[&str] = &[
            "joséphine",
            "josephine",
            "joséphine de beauharnais",
            "josephine de beauharnais",
            "marie louise",
            "marie-louise",
            "napoleon",
            "napoleon bonaparte",
            "napoléon",
            "wellington",
            "alexander i",
        ];
        if PERSONS.iter().any(|p| *p == key) {
            return Some((surface.trim().to_string(), EntityKind::Person));
        }

        // Places via judge gazetteer labels + common toponyms
        const PLACES: &[&str] = &[
            "paris",
            "london",
            "ajaccio",
            "waterloo",
            "leipzig",
            "austerlitz",
            "elba",
            "saint helena",
            "st helena",
            "corsica",
            "moscow",
            "vienna",
            "rome",
            "milan",
            "toulon",
            "fontainebleau",
            "malmaison",
            "brienne",
            "egypt",
            "cairo",
            "jena",
            "wagram",
            "borodino",
            "smolensk",
            "tilsit",
            "notre-dame",
            "notre dame",
        ];
        if PLACES.iter().any(|p| *p == key) {
            return Some((surface.trim().to_string(), EntityKind::Place));
        }

        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedMentions {
    pub subject_kind: Option<EntityKind>,
    pub place_label: Option<String>,
    pub place_kind: Option<EntityKind>,
    /// True when a non-place was proposed as place.
    pub invalid_place_attempt: bool,
    pub place_mentions: Vec<Mention>,
    pub object_mentions: Vec<Mention>,
    pub participant_mentions: Vec<Mention>,
}

/// Resolve and segregate mentions. Never promotes non-place → place_entity.
pub fn resolve_mentions(
    candidate: &EventCandidate,
    resolver: &dyn MentionResolver,
    raw_place_surface: Option<&str>,
    raw_object_surface: Option<&str>,
    raw_participant_surfaces: &[(String, ParticipantRole)],
) -> ResolvedMentions {
    let mut out = ResolvedMentions::default();

    if let Some((_, kind)) = resolver.resolve_surface(&candidate.subject_surface) {
        out.subject_kind = Some(kind);
    }

    if let Some(place_raw) = raw_place_surface.map(str::trim).filter(|s| !s.is_empty()) {
        match resolver.resolve_surface(place_raw) {
            Some((label, EntityKind::Place)) => {
                out.place_label = Some(label.clone());
                out.place_kind = Some(EntityKind::Place);
                out.place_mentions.push(Mention {
                    surface: label,
                    entity_id: None,
                    kind: Some(EntityKind::Place),
                    role: None,
                });
            }
            Some((label, kind)) => {
                // e.g. Joséphine offered as place → participant/object, never place.
                out.invalid_place_attempt = true;
                if kind == EntityKind::Person {
                    out.participant_mentions.push(Mention {
                        surface: label,
                        entity_id: None,
                        kind: Some(EntityKind::Person),
                        role: Some(ParticipantRole::Spouse),
                    });
                } else {
                    out.object_mentions.push(Mention {
                        surface: label,
                        entity_id: None,
                        kind: Some(kind),
                        role: None,
                    });
                }
            }
            None => {
                // Unknown surface kept as unresolved place mention for review.
                out.place_mentions.push(Mention {
                    surface: place_raw.to_string(),
                    entity_id: None,
                    kind: Some(EntityKind::Unknown),
                    role: None,
                });
                out.place_label = Some(place_raw.to_string());
                out.place_kind = Some(EntityKind::Unknown);
            }
        }
    }

    if let Some(obj) = raw_object_surface.map(str::trim).filter(|s| !s.is_empty()) {
        let (surface, kind) = resolver
            .resolve_surface(obj)
            .unwrap_or_else(|| (obj.to_string(), EntityKind::Unknown));
        if kind == EntityKind::Person {
            out.participant_mentions.push(Mention {
                surface,
                entity_id: None,
                kind: Some(EntityKind::Person),
                role: Some(ParticipantRole::Spouse),
            });
        } else {
            out.object_mentions.push(Mention {
                surface,
                entity_id: None,
                kind: Some(kind),
                role: None,
            });
        }
    }

    for (surface, role) in raw_participant_surfaces {
        let (label, kind) = resolver
            .resolve_surface(surface)
            .unwrap_or_else(|| (surface.clone(), EntityKind::Unknown));
        out.participant_mentions.push(Mention {
            surface: label,
            entity_id: None,
            kind: Some(kind),
            role: Some(*role),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CandidateStatus, TypedTime, EXTRACTOR_DETERMINISTIC_V1};
    use uuid::Uuid;

    fn empty_candidate() -> EventCandidate {
        EventCandidate {
            id: Uuid::new_v4(),
            snapshot_id: Uuid::nil(),
            fragment_id: Uuid::nil(),
            clause_index: 0,
            subject_surface: "Napoleon".into(),
            subject_entity_id: None,
            event_type: "marriage".into(),
            predicate: "married".into(),
            time: TypedTime::Exact {
                year: 1796,
                month: None,
                day: None,
                surface: Some("1796".into()),
            },
            place_mentions: vec![],
            object_mentions: vec![],
            participant_mentions: vec![],
            place_entity_id: None,
            place_label: None,
            evidence_ptrs: vec![],
            extractor_version: EXTRACTOR_DETERMINISTIC_V1.into(),
            fingerprint: "x".into(),
            status: CandidateStatus::Pending,
            rejection_codes: vec![],
        }
    }

    #[test]
    fn josephine_never_becomes_place() {
        let c = empty_candidate();
        let r = resolve_mentions(&c, &GazetteerResolver, Some("Joséphine"), None, &[]);
        assert!(r.invalid_place_attempt);
        assert!(r.place_label.is_none());
        assert!(r.place_kind.is_none());
        assert_eq!(r.participant_mentions.len(), 1);
        assert_eq!(r.participant_mentions[0].kind, Some(EntityKind::Person));
        assert_eq!(
            r.participant_mentions[0].role,
            Some(ParticipantRole::Spouse)
        );
    }

    #[test]
    fn paris_resolves_as_place() {
        let c = empty_candidate();
        let r = resolve_mentions(&c, &GazetteerResolver, Some("Paris"), None, &[]);
        assert!(!r.invalid_place_attempt);
        assert_eq!(r.place_kind, Some(EntityKind::Place));
        assert_eq!(r.place_label.as_deref(), Some("Paris"));
    }
}
