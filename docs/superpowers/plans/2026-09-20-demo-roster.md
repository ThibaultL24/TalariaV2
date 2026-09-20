# Demo roster Implementation Plan

> **For Claude:** execute task-by-task. Roster locked; approach B.

**Goal:** Ship a publishable 10-person demo with home grid, modern ingest path, and Intuition testnet publish opt-in.

**Architecture:** Static roster in web + thin `/api/v1/demo/roster` enrichment from DB; seed script for moderns; env-gated live publish.

### Task 1: Roster + home grid
- Add `web/src/lib/demo-roster.ts`
- API `GET /api/v1/demo/roster` → counts per QID
- Home section cards linking to explorer/agora

### Task 2: Seed moderns
- `scripts/seed_demo_roster.sh` — ingest Macron, Trump, De Gaulle, Obama via explorer HTTP or CLI

### Task 3: Intuition live opt-in
- Gate `live_publish_guard` on `INTUITION_ALLOW_LIVE=1`
- Agora UI: publish CTA when modeled + allow flag exposed via status/signals

### Task 4: Smoke
- Home shows 10; Napoleon Intuition bar still works; modern search resolves
