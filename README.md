# Talaria

Talaria is a Rails 8 API-first historical intelligence platform. It ingests source material, creates structured candidates in a raw/staging layer, and promotes only evidence-backed records into canonical tables.

## Setup

1. Copy env template: `cp .env.example .env`
2. Start DB: `docker compose up -d db`
3. Install gems: `bundle install`
4. Prepare DB: `bin/rails db:prepare`
5. Run tests: `bin/rails spec`

## Key Commands

- `bin/rails db:migrate`
- `bin/rails jobs:work`
- `bin/rails rswag:specs:swaggerize`
- `bin/rails spec`

## Background jobs

Active Job uses Solid Queue. Queue storage remains Postgres-only by default; Redis is optional and not required.

## Storage

Active Storage uses disk in development/test and S3-compatible object storage in production via env vars.
