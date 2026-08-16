# AM-361 — Builds, parts, and structured fitment data

Design, 2026-08-16. Ticket [AM-361](https://oksasatyaa.atlassian.net/browse/AM-361), epic E16.

## What this is for

A user records what they have done to their car. The records are the product: a later fitment engine answers *"velg apa yang muat di mobil gue"* from what the community actually fitted, so a wheel's offset has to be a number the database can compare, not a sentence somebody typed.

That single requirement is what makes this ticket structural rather than CRUD. Everything below follows from it.

## Decisions, and what forced them

### A `parts` row is one exact configuration, not a product name

`Enkei RPF1 18x8.5 ET40` and `Enkei RPF1 18x9.5 ET45` share a brand and a product name and are different wheels. If a part is identified by its name, a curator merging "duplicates" collapses them, and every modification that genuinely used the first is read as having used the second.

The result is not a missing number. It is a **confident wrong number** — fitment evidence that reads as real and describes a wheel nobody fitted. That is the exact failure the platform's own rules call worse than no answer at all.

So: category + brand + product + the typed specs together identify a part. A merge is only legitimate when those match, or when one side carries no specs at all and a curator has verified the two are the same thing.

This was not the original design. It came out of the adversarial review and it is the most valuable thing in this document.

### Merge is an append-only log plus a cache, not a mutable pointer

The first design set `merged_into` on the losing part and resolved it at read time, claiming that undo was free because no row was ever rewritten.

It does not hold. Merge `A → B`, then merge `B → C`:

- Leave both pointers and the result is a chain `A → B → C`. A count for `C` that resolves one hop misses everything under `A`.
- Flatten `A → C` when `B → C` is applied, and undoing `B → C` cannot restore `A → B` — that edge no longer exists anywhere. "Undo restores the previous state exactly" is false.

Concurrency breaks it a second way: two curators merging `A → B` and `A → C` lock different rows, so the second silently overwrites the first and neither operation reports that anything was lost.

The design is therefore:

- **`part_merges`** — append-only: source, target, who, when, and whether it has been undone. This is the record undo reads to know what the previous state *was*. It is not event sourcing; it is the operation history that an undoable curation action requires anyway.
- **`parts.canonical_part_id`** — a one-hop cache, indexed, so reads stay a single join and never a recursive CTE.

Merge and unmerge lock the whole canonical component in one transaction and recompute `canonical_part_id` for the affected `parts` rows only. `modifications.part_id` is still never rewritten — that part of the original design survives, and it is what keeps evidence from being destroyed by a bad merge.

### An unknown part becomes a pending part row, immediately

The first design let a modification carry free text with `part_id NULL`. That breaks AC3: a NULL has no queue row, no suggester, and no per-part completeness state, so the "enters curation" half of the requirement quietly does not happen.

Writing a `parts` row with `status = 'pending'` in the same transaction as the modification satisfies both halves, and it **removes** code rather than adding it: no free-text columns on `modifications`, no NULL branch, and no `modifications.category` either — the category lives on the part now, so storing it twice only creates a way for the two to disagree.

### Typed columns, and a `CHECK` behind each one

Wide nullable typed columns on `parts`, not JSONB and not fifteen tables. The fitment engine wants indexed numeric range queries and type enforcement; JSONB gives neither well. Fifteen tables is fifteen joins for a problem that is currently three categories wide.

A type alone is not validity. `pcd_bolt_count = 0`, `offset_et_mm = -999`, and a tyre aspect ratio of `900` are all perfectly typed and all impossible, and a completeness check that only looks for NULL calls them complete. Every numeric spec gets a range `CHECK`.

Columns are added when a category earns them. Today three do.

### One taxonomy language, and the older one moves

`service_category` shipped with Indonesian values (`oli_mesin`, `kaki_kaki`). PRD §10 writes the modification categories in English. Following each source literally would put `oli_mesin` and `wheels` side by side in one API.

Both taxonomies become English. `service_category` migrates with `ALTER TYPE ... RENAME VALUE`, which renames in place and rewrites no data.

The owner was told this is a migration of shipped values for consistency alone, and chose it. The timing is what makes it cheap: this is a public contract change, there are no users, and the same change after launch costs a versioned API.

### `builds` is its own table — for publication, not for privacy

A vehicle is the private ownership record. A build is the thing its owner chooses to publish, with its own visibility and its own lifecycle.

An earlier draft justified the split as a privacy boundary. That was wrong and is corrected here: privacy already comes from `vehicle_private` being a separate table, and a public build query still has to join `vehicles` to read the variant. The split earns itself on publication lifecycle, and on nothing else.

`UNIQUE (vehicle_id)` is a product decision — one current build per car — and it forecloses separate street and track builds on one vehicle. Written down so that reversing it later is a decision rather than a surprise.

### Two evidence counts, each named for what it counts

"Community evidence" is not one number.

- **`active_build_count`** — builds where the part is currently fitted.
- **`ever_installed_build_count`** — including those where it was removed. A removed modification is still evidence that the part fitted; that is why removal sets `removed_at` instead of deleting the row.

Both are `COUNT(DISTINCT build_id)` over the canonical part, so a build that referenced two now-merged parts counts once rather than twice. Both must also filter on the reader's visibility and on the vehicle's `variant_id` — an Avanza build whose vehicle was never matched to a catalog variant is not evidence about a Civic, and `variant_id` is nullable precisely because free-text cars exist.

Calling either of them just "evidence count" is how the wrong one ends up on a screen.

### Two visibility settings, because they answer different questions

`builds.visibility` controls who sees the build at all. `vehicles.cost_visibility` controls who sees the money — service costs and modification costs alike, since both are costs of the same car.

They are separate because wanting to show the car without showing what it cost is an ordinary thing to want, not a hypothetical. One setting would force the two together, and the one people would give up is the sharing.

`cost_visibility` also closes the half of AM-360 AC3 that currently holds only by construction: today no non-owner read path exists, so nothing leaks, but nothing is *configured* either.

### A part is never rejected, only merged

The catalog suggestion queue has `rejected` and `duplicate` statuses. Parts deliberately do not.

A rejected part would orphan every modification already using it — and AC3's whole point is that the part is usable the moment it is typed. So a curator's options are to approve it, or to merge it into the part it duplicates. A part in use cannot be removed from under its users, which is the same rule the vehicle catalog already follows.

## Schema

```
builds
  id, vehicle_id UNIQUE → vehicles ON DELETE CASCADE
  notes, visibility (private|community|public)
  created_at, updated_at

build_photos                        -- table only; no upload path yet
  id, build_id → builds ON DELETE CASCADE
  object_key, position, created_at

parts
  id, category (modification_category), brand, product_name
  status (pending|approved)
  canonical_part_id → parts NULL          -- one-hop cache
  suggested_by → users NULL
  wheel_diameter_in, wheel_width_in, offset_et_mm,
  pcd_bolt_count, pcd_diameter_mm, center_bore_mm,
  tyre_width_mm, tyre_aspect_ratio, tyre_rim_diameter_in,
  suspension_type (coilover|lowering_spring|air), spring_rate_kgmm
  created_at, updated_at
  -- a range CHECK on every numeric spec

part_merges                          -- append-only
  id, source_part_id, target_part_id, merged_by, merged_at,
  undone_by NULL, undone_at NULL

modifications
  id, build_id → builds ON DELETE CASCADE, part_id → parts
  install_date, mileage_km, cost NUMERIC, garage_name, notes
  removed_at NULL
  created_at, updated_at

vehicles
  + cost_visibility (private|community|public)
```

Every foreign key is indexed. Money is `NUMERIC`. Ids are application-generated UUIDv7. Timestamps are `TIMESTAMPTZ`.

## Domain policy

`domain::build::policy` stays pure — no async, no I/O:

```rust
pub fn missing_specs(category: PartCategory, part: &PartSpecs) -> Vec<SpecField>
```

A wheel is complete when diameter, width, offset, bolt count, PCD diameter, and centre bore are all present. Derived rather than stored, so it can never go stale and so completing one part's specs propagates to every build using it with no backfill.

The SQL fitment queries must use a predicate equivalent to this function. Two definitions of "complete" that can disagree is a defect waiting for its first curator.

## Slices

Each ships through the full sequence — commits, a pull request into `dev`, CI green — before the next starts.

| # | Scope | AC | TDD |
|---|---|---|---|
| 1 | `service_category` → English; `parts` + specs + range checks; `missing_specs` policy; part search and pending-suggestion flow | AC2, AC3 | **yes** — `missing_specs` is a branching contract per category |
| 2 | `builds`, `modifications` with `removed_at`, `build_photos` table, `vehicles.cost_visibility` | AC1 | **no** — schema and CRUD; verified by integration tests after |
| 3 | Three-query build list; `active_build_count` and `ever_installed_build_count` with visibility and variant filters | AC5 | **no** — verified by integration tests, including cross-contamination |
| 4 | `merge` / `unmerge` use case, transactional, component-locked | AC4 | **yes** — the chain and concurrency cases are exactly what a test must pin |

Slice 3 runs **three** queries, not two: once photos exist, joining builds × modifications × photos multiplies rows (10 modifications × 20 photos = 200 rows for one build), and fetching photos separately per build is the N+1 the AC forbids. Three batched queries stay a constant number of round trips and `O(B + M + P)` in memory.

Slice 4 ships the use case and its tests but **no HTTP endpoint**. The endpoint belongs to [AM-88](https://oksasatyaa.atlassian.net/browse/AM-88), which already specifies this operation more completely, and it needs the admin role from [AM-83](https://oksasatyaa.atlassian.net/browse/AM-83), which does not exist yet. A merge endpoint without an access check is a destructive many-row operation with no guard.

## Not in this ticket

- **Photo upload.** The table exists; the path does not. Upload is its own problem — presigned URLs, size limits, EXIF stripping (a photo of a car carries the coordinates of the house it is parked at), moderation. Folding it in here would inflate the ticket and rush the privacy half of it.
- **A public slug.** `apps/landing` is not scaffolded, so there is nothing serving a public URL. A build id is already a URL-safe UUIDv7. A slug and its uniqueness constraint arrive with the surface that needs one.
- **Structured fitment context** — current suspension, ride height, brake setup, fender modification. PRD §16.2 needs all four, and without them a stock car and a lowered car with rolled fenders look like equivalent evidence for the same wheel. **This is a constraint on AM-363, recorded here rather than built: until that context is structured, the fitment engine must return `Insufficient Data` and must never parse it out of `notes`.**
- **The merge endpoint and the curation UI** — AM-88.
- **The fitment engine itself and seller price integration** — the ticket's own out-of-scope line.

## Anti-goals

- No JSONB spec bag. The numbers must be comparable by the database.
- No guessed values to make a part look complete. AC2 says incomplete is flagged, and a plausible invented offset is worse than a blank one.
- No rewriting of `modifications.part_id` during a merge. Evidence survives a wrong merge only because the original references are still there.
- No free-text part path that skips the curation queue.
- No second definition of "complete" in SQL that can drift from the policy function.
- No evidence count that ignores the reader's visibility or the vehicle's variant.
- No seeded or invented parts. The catalog starts empty.

## Quality gate

Rust, so clippy is the gate and Sonar is not used here.

```
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
make be-boundary                     # domain imports no framework
cargo sqlx prepare --check --workspace
cargo test --workspace
```

Domain crate keeps its dependency list: `thiserror`, `uuid`, `time`. No `serde`, no `sqlx`, no `axum`. `PartCategory` exists twice — once pure in `domain`, once in `adapter/postgres` with the sqlx and serde derives — and the two `From` impls go non-exhaustive the moment either side gains a variant.

No `.unwrap()`, `.expect()`, or `panic!` on a request path. Every duplicated literal appearing three or more times becomes a `const`. State the time and space complexity of any loop or query on the list path before writing it.

## Review history

The design was challenged by a second model (Codex, GPT-5.6-Terra) before it reached the owner. Eight objections were raised; the four that changed the design are recorded above as decisions — part identity, the merge log, the pending-part path, and the range checks. Two were accepted with reduced scope (evidence counts split in two; three queries instead of two). One was recorded as a constraint on a later ticket rather than built (structured fitment context). One correction was to the reasoning rather than the design: the `builds` split does not buy privacy, and claiming it did would have left a real privacy question looking answered.
