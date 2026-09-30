-- Read-only display projection: never update the historical occurrence.
CREATE VIEW person_timeline_projection AS
SELECT ce.*,
  CASE
    WHEN event_type IN ('publication','work') THEN 'works'
    WHEN event_type IN ('office','diplomatic','treaty','battle','siege') THEN 'public_life'
    WHEN event_type IN ('arrival','departure','travel','residence','exile') THEN 'places'
    ELSE 'life'
  END AS timeline_lane,
  CASE
    WHEN event_type IN ('birth','death') THEN 1.0
    WHEN event_type IN ('publication','office','exile','marriage','battle') THEN 0.8
    WHEN event_type IN ('residence','education','employment','diplomatic','treaty') THEN 0.6
    ELSE 0.4
  END AS timeline_importance
FROM canonical_events ce
WHERE is_active AND timeline_eligible AND pipeline='person';
COMMENT ON VIEW person_timeline_projection IS
'Deterministic display prioritization v1; importance is editorial, not historical confidence.';
