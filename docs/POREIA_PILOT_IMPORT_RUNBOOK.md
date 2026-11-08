# Poreia pilot import

The pilot describes four Wikidata places and one exhibition. Every inserted row stays in `quarantine`. Nothing in this import publishes a site or edits `canonical_events`.

`DATABASE_URL_STAGING` must be a Postgres/PostGIS database that is not production. The variable is unset until an operator sets it. Do not point the command at `DATABASE_URL` when that URL is production.

```bash
python3 scripts/poreia/build_pilot_seed.py --check
psql "$DATABASE_URL_STAGING" -v ON_ERROR_STOP=1 -f scripts/poreia/pilot_stage.sql
```

Run the SQL a second time to confirm it does not create a second copy and does not change `publication_status`. Publication is only `POST /api/v1/admin/poreia/sites/{site_id}/review` with an allowlisted Talaria session and the header `X-Talaria-Operator-Intent: review-site`.
