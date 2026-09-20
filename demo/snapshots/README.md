# demo/snapshots

Put compressed Postgres dumps here (`talaria-demo-*.dump`).

- Export from a filled machine: `./scripts/export_demo_snapshot.sh`
- Restore: `CONFIRM_RESTORE=1 ./scripts/restore_demo_snapshot.sh`
- Full local/VPS boot: `./scripts/bootstrap_demo.sh`

Large `.dump` files are gitignored. Ship them via release asset, object storage, or scp next to this folder as `talaria-demo-latest.dump`.
