---
status: accepted
---

# Deleted accounts are retained, not erased

Deleting an account marks it dead and blocks sign-in; it does not remove the row or the personal data on it. The person may register again with the same email, because uniqueness applies only to live accounts. We chose this so that audit trails — starting with `role_changes`, which is append-only by trigger and therefore cannot be scrubbed later — stay readable after somebody leaves, rather than degrading into opaque UUIDs at exactly the moment accountability matters.

## Considered options

**Anonymise on deletion.** Null the retained name and email when the account is deleted, letting the trigger permit that one transition and nothing else. This was the recommendation, and it keeps AM-296's original promise intact with no legal question left open. Rejected because it trades away readability of the historical trail — the case the trail exists for is precisely the one where the actor is gone.

**Store only UUIDs and lean on the free-text `reason`.** Cleanest against the erasure promise, and the reason field is written by a human so it usually carries the human context anyway. Rejected because it makes the trail depend on the discipline of whoever typed the reason, and because a free-text field is not a reliable place to look somebody up.

## Consequences

**AM-296 no longer describes what we are building and must be rewritten.** Its definition of done reads "tidak ada data pribadi tersisa di permukaan mana pun" — no personal data left on any surface. That is now false by design. Leaving the ticket as written would mean the backlog claims one thing while the schema does another, which is the failure this project has already paid for once.

**Email uniqueness becomes conditional.** `users.email` is `CITEXT NOT NULL UNIQUE` today, so a retained row would block the same person from ever registering again. The constraint becomes a partial unique index over live accounts only, letting one dead row and one live row hold the same address.

**This is a position on data-protection law, taken deliberately.** Indonesia's UU PDP No. 27/2022 establishes a right to erasure of personal data. "Deleted, but we keep your data" is a stance the product owner has chosen with that named, not a side effect of an audit-table design. It is recorded here rather than buried in a migration so that a future review can find it and reopen it. No legal advice was sought or given.

**Retention has no stated bound.** Nothing here says how long a dead account's data is kept, or whether a person can ask for real erasure by another route. Both are open and belong to AM-296.

## None of this is built yet

Stated plainly because the decision reads as though it describes the system, and it does not. `users` has no `deleted_at` column, and sign-in resolves an account with `WHERE email = $1` and no live-account filter. Until AM-296 adds both, a delete-then-re-register cycle would resolve sign-in against whichever row the index reached first — so the partial unique index and the sign-in filter land together or not at all.

AM-355 does not depend on any of it. Its audit rows resolve identity through `ON DELETE RESTRICT`, which refuses to delete a user who has role history — the database enforces the join rather than trusting this decision to have shipped.

## Scope

Nothing in AM-355 implements this. `deleted_at`, the partial unique index, the sign-in block, and re-registration are AM-296's work; this ADR records the decision they will follow.

**And retention is what lets the audit trail stay thin.** The `role_changes` table was going to carry a copy of each person's email so the trail survived their deletion. Retention removes the need: the row it points at is always there, so identity comes from a join and the table stores only ids. The copies were considered for a second reason — a point-in-time record that does not change when somebody later edits their email — and dropped as not worth a second, unerasable copy of an address for a two-role system. If that faithfulness ever matters, the columns are additive.
