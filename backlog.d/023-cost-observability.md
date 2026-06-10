# Cost observability: spend ledger and per-spec economics

Priority: P2 · Status: pending · Estimate: S

## Goal
Spend survives state wipes and answers "what did this spec cost me" and "what's my burn this week".

## Oracle
- [ ] Durable append-only cost ledger (separate from renders dir) written on every real render.
- [ ] `specifi report` shows per-spec, per-day, and total spend from the ledger.
- [ ] Wallet gate reads the ledger, not just surviving render JSONs.

## Notes
Today spend is summed from render provenance — wiping .specifi/renders resets the meter while the money stays spent.
