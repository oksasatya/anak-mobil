# AnakMobil.id — repository instructions

Indonesia-first automotive platform: digital garage, community knowledge, and an AI assistant grounded in that community's data. Four client surfaces, one backend, one database.

Read [README.md](README.md) for the architecture and diagrams. This file holds the rules that are easy to violate without noticing.

## Layout

```
apps/api          Rust · axum · Cargo workspace (nested inside the JS workspace)
apps/landing      Astro                            — not scaffolded yet
apps/backoffice   Vite · React · shadcn · TanStack — not scaffolded yet
apps/mobile       React Native · Expo              — not scaffolded yet
packages/assets   brand marks
packages/tokens   design tokens                    — not scaffolded yet
packages/api-types  generated TS types             — not scaffolded yet
docs/             PRD, design system, feature breakdown
```

Backend conventions live in [apps/api/CLAUDE.md](apps/api/CLAUDE.md). Anything below applies repository-wide.

## Product rules that are not negotiable

These are product decisions with teeth. Each has cost a real argument already; none should be re-derived per task.

**Community contribution is never paywalled.** Sharing a build, reporting a problem, recording a service entry, answering someone's question, joining a community — free forever, at every subscription tier. Paid tiers buy AI depth and personal tooling, never the right to speak. This is the closing line of the PRD's monetisation section, and it is written as acceptance criteria on AM-308 and AM-315 so it cannot be quietly reversed.

**Private vehicle data is filtered server-side, including from admins.** Number plate, VIN, purchase price, and service costs never leave the server for anyone who should not see them — the client is never trusted to hide them, and an admin session is not a reason to expose them. Leaked plate and VIN data cannot be recalled.

**An AI answer never ships without its safety warning.** Questions about brakes, steering, structural damage, fuel leaks, electrical faults, or overheating carry a prominent warning and a recommendation to see a technician. Answers are persisted whole — prose, confidence, evidence, warnings — *before* being considered complete; streaming is transport for the typing experience only. A dropped connection must never leave someone reading an answer whose warning never arrived.

**Confidence comes from constraint match, not embedding distance.** Semantic similarity is not vehicle identity. Retrieval filters brand, model, generation, and variant *before* the vector search, and confidence is derived from how well the evidence matches the user's actual car plus the provenance of the source. A confident answer about the wrong car is worse than no answer.

**Reported, hidden, or deleted content is never cited as evidence.** Content status must be visible to the indexer, and there must be a path that pulls content back out of the index. A platform that frames community text as "evidence" with a confidence badge does more damage with bad content than a plain feed would.

**Nothing is seeded with fake data.** No invented community counts, no fabricated testimonials, no screenshots of data that never existed. The platform launches empty and says so — the low-data state is designed as a primary experience, not a fallback.

## Language

Product-facing text is **Bahasa Indonesia**: UI strings, error messages, Jira issue content. Automotive terms use what workshops and enthusiasts actually say, not literal translations.

Everything written for developers is **English**: code, comments, commit messages, this file, `docs/`, and README files.

## Jira

Project **AM** at `oksasatyaa.atlassian.net`. Hierarchy Epic → Story → Subtask.

Stories carry a user story, then Given/When/Then acceptance criteria in Bahasa Indonesia, then technical notes, then an explicit out-of-scope line. Subtasks carry a paragraph and a "Selesai ketika" definition of done — no Given/When/Then at that level.

Two traps worth knowing. The issue type named `Sub-Task` (id 10068) is **not** a subtask — it sits at hierarchy level 0 and creates a flat issue; the real one is `Subtask` (id 10063). And summaries take a literal `&`; passing `&amp;` stores the entity verbatim.

The Atlassian connector has Jira write access but Confluence read-only, and moving issues between Backlog and Board is a board operation it cannot perform at all. Say so rather than appearing to do it.

## Working here

**Verify, do not assert.** Run the gate and read its output. "It compiles" is the floor. A summary of what you expect a command to print is not evidence it printed that.

**The backlog is already specified.** 368 issues cover every surface. Before designing something, check whether a ticket already defines it — the acceptance criteria usually carry reasoning that is expensive to rediscover.

**Do not commit or push unless asked.** Work accumulates in the working tree for review.
