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

## JavaScript toolchain

**Bun, not npm.** `bun install`, `bun run`, `bun test`. There is no `package-lock.json` and no `npm` invocation anywhere — including inside a `package.json` script, where one would run silently on any machine that happens to have npm installed.

```bash
bun install                              # from the repository root
bun run --filter @anakmobil/landing dev  # a single workspace
make fe-dev                              # the same thing, through the wrapper
```

Bun's `--filter <package>` is the equivalent of npm's `--workspace <package>`, and `bun install --frozen-lockfile` is the equivalent of `npm ci` — it fails rather than updating `bun.lock`, so CI cannot quietly resolve a different dependency tree than the one that was reviewed.

Node is not required. Bun runs the Astro build and the `node:test` suites in `packages/tokens` unchanged.

**One exception is expected, and it is not a failure of this decision.** `apps/mobile` will use Expo, which is the slowest part of the JavaScript ecosystem to accommodate Bun — `bun install` generally works, but Metro and parts of the Expo CLI have historically assumed npm or yarn. If that bites, the answer is that **`apps/mobile` alone** uses a different package manager, not that the repository reverts. Verify before scaffolding it rather than assuming either way.

### No task runner, and what would change that

There is no Turborepo, Nx, or equivalent, and adding one now would be worse than not having one. Measured 2026-08-15:

```
tokens  build       42 ms
landing build    1,676 ms      total JS ≈ 1.7 s
cargo build     17,706 ms      incremental, no changes
```

Three reasons, in order of weight. **A task runner cannot touch Cargo**, and Rust is the slow half — CI already runs 1m45s for the backend gate against 15s for the frontend. **The cache has almost nothing to cache**: three JavaScript workspaces, not twenty. And **"only run what is affected" already exists**, one layer up, in the workflow path filters — a backend commit does not trigger the frontend job at all.

Remote caching also assumes a team. Its pitch is that a colleague's build warms your CI; there is one developer.

**Revisit when any one of these is true**, not when the repository merely feels large:

- `packages/tokens` has three or more consumers **and** the build order can no longer be stated honestly as Make dependencies;
- the `frontend` CI job passes ~90 seconds;
- one token change forces four or more downstream packages to rebuild on every pull request.

The first is the one to watch. What breaks first is not speed but **ordering** — `fe-build: ds-build` is a single honest line today because tokens has one consumer, and it stops being honest once `landing`, `backoffice`, and `mobile` all consume tokens while two of them also consume generated `api-types`. A task graph earns its place there, before it earns it on time saved.

Projected full scope is six JavaScript workspaces, five with builds. That is still below where a task runner's cache pays for its configuration, so expect the ordering trigger to fire before the timing ones.

## Branching

```
main ← dev ← feat/<ticket>-<slug>
```

**`main` is the release branch.** Nothing is committed to it directly and nothing is merged into it except from `dev`. It is what a stranger sees on the repository's front page and what a deploy will eventually track.

**`dev` is the integration branch.** Every feature merges here first. It is branched from `main` and must stay green.

**Work happens on a branch cut from `dev`**, never from `main` — a branch cut from `main` while `dev` is ahead produces a merge conflict that has nothing to do with the change:

```bash
git checkout dev && git pull
git checkout -b feat/AM-353-database-schema
```

Name it after the ticket: `feat/AM-353-database-schema`, `fix/AM-361-fitment-null`, `chore/…`, `docs/…`.

**A feature branch is opened as a pull request against `dev`.** The base is not the default, so pass it explicitly or the PR silently targets `main`:

```bash
gh pr create --base dev --title "…" --body "…"
```

**`dev` reaches `main` as its own pull request**, when the work on it is worth releasing rather than every time something lands.

### Rules that hold on any branch

**Commits are small and each one builds.** A commit that does not compile makes `git bisect` useless, which is most of the reason to split them at all. Where ordering makes that awkward — a module that references something not yet added — write the intermediate version rather than shipping a broken commit.

**Commit messages are English and say *why*.** The diff already says what. A message that explains a decision, a reversal, or a defect found along the way is worth more than a list of files.

**A red CI is not done.** Watch the run to green after pushing; fix, re-run locally, push again. Never merge or report completion on a run nobody watched finish.

**Merge with a merge commit, not a squash**, when the individual commits were built to be readable. Squashing collapses work that was deliberately sequenced.

## Working a Jira ticket

Every ticket runs the same sequence. No step is skipped, and none of them starts before the one before it finished.

```
brainstorming  →  grill  →  spec on disk  →  writing-plans  →  /executing-plans-hybrid
                                                                        ↓
                                       separate commits  →  PR into dev  →  merge
```

**1. Read the backlog before designing anything.** 368 issues are already specified, and more than one ticket covers the same ground. AM-361's AC4 turned out to be [AM-88](https://oksasatyaa.atlassian.net/browse/AM-88) in full — plus one requirement AM-361 never mentions — and it depends on a role story that does not exist. Half an hour of reading moved that work to the ticket that already specified it properly. The acceptance criteria also carry reasoning that is expensive to rediscover.

**2. `superpowers:brainstorming` first, before any code.** Not after a design has already formed in the writing. The skill classifies the work itself — a spike, a bounded change, or something architectural — and the ceremony scales to that classification. What does not scale is the approval: nothing is implemented until the design has been shown and agreed, however short that design is.

**3. Grill the approved design before it becomes a spec.** `grill-with-docs` against this repository's own documents, and a genuinely different model — `codex:codex-rescue` via the Agent tool — briefed to **refute** the design rather than approve it. A round-one "looks good" is not an answer; make it land an objection or explain concretely why each attack fails.

This is the step that pays. On AM-361 it killed the merge design outright — the pointer scheme could not be flat, concurrency-safe, and exactly undoable at once — and it caught that identifying a part by its product name would let a curator collapse two different wheels into one, turning community evidence into confident fiction. Neither was visible from inside the design. Record which objections changed the design and which were rejected, so the next reader knows the design survived something.

**4. A spec, written to disk and committed.** `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`. On disk rather than in the conversation, because a conversation is lost to compaction and the next session needs to know what was decided and why. It carries the decisions and their reasoning, what is deliberately absent, and an anti-goals block — so a later reader can tell a gap from an oversight.

**5. `superpowers:writing-plans`, dispatched to its own context, never run inline.** A planner that has lived through the brainstorm inherits every assumption made in it. A fresh one does not, and that is the whole point: on AM-361 the dispatched planner corrected the brief it was given — the enum rename is eight statements, not the nine I had counted, because renaming a value to itself errors.

The brief must carry two things or the dispatch makes the plan worse rather than better: **everything discovered that the spec does not state** (environment facts, traps, the test-framework dialect, call sites that will break), and an instruction to **read the code before writing code into the plan**, naming the nearest existing analogue by path. A plan written from a spec alone confidently invents signatures that do not exist, and that error surfaces only at implementation time.

**6. `/executing-plans-hybrid` runs the plan.** It carries the per-task verdicts the plan wrote down — what is tested first, what is verified by running it, where the risk sits. It maps the whole plan before task 1, dispatches a writer per task on the tier that task earns, re-runs the gates itself rather than trusting a writer's report, and dispatches an independent reviewer that never blocks the next task. Findings go to the plan's ledger and are worked in one pass at the end.

**7. Separate commits, each one building.** One logical change per commit. A commit that does not compile makes `git bisect` useless, which is most of the reason to split them; where ordering makes that awkward, write the intermediate version rather than shipping a broken commit.

**8. A pull request into `dev`, never into `main`.** `gh pr create --base dev` — the base is not the default, so it is passed explicitly or the PR silently targets the release branch.

**9. Merge once CI is green.** Watched to green, not assumed. A red run is not done.

**A ticket too large for one pass is split, and the split is said out loud.** AM-360 arrived carrying two mobile epics plus the catalog — four tables and eighteen endpoints. Attempting that in one pass produces a pull request nobody can review and rushes whichever part of it carries the most risk. Slice it in dependency order, ship each slice through the full sequence above, and leave the ticket open until the last one lands.

## Working here

**Verify, do not assert.** Run the gate and read its output. "It compiles" is the floor. A summary of what you expect a command to print is not evidence it printed that.

**The backlog is already specified** — 368 issues, and reading them is step 1 of working a ticket above, not optional context.

**Do not commit or push unless asked.** Work accumulates in the working tree for review. When asked, follow the branching rules above — "commit and push" means a feature branch and a pull request into `dev`, not a commit on whatever branch happens to be checked out.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
