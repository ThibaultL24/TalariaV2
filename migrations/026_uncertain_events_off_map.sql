-- migrations/026_uncertain_events_off_map.sql
-- Repair rows that geocoding put back on the map after the tuple judge had
-- demoted them. Coordinates stay; only map visibility is withdrawn.
-- Legacy rows are left untouched: that lane is not judged.
UPDATE canonical_events
SET map_eligible = false
WHERE epistemic_status = 'uncertain'
  AND map_eligible
  AND pipeline IN ('quality', 'person');
