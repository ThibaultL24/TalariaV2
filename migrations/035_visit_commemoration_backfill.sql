-- Visit lens: commemoration rows are heritage, not biography timeline.
UPDATE canonical_events
SET
    visit_eligible = true,
    timeline_eligible = false
WHERE pipeline = 'person'
  AND is_active
  AND event_type = 'commemoration'
  AND NOT visit_eligible;
