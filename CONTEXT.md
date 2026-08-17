# AnakMobil

An Indonesia-first automotive platform: a digital garage for your own cars, community knowledge about them, and an assistant grounded in that community's data.

This file is a glossary and nothing else. It records the words this project has settled on, so that a term means one thing everywhere — in tickets, in code, and in conversation. Implementation lives in the code; decisions live in `docs/adr/`.

## Language

### Roles

Three unrelated things were all called "role" at some point. They are never stored in the same column and never substituted for one another.

**Platform role**:
What an account is allowed to do across the whole platform. Exactly two values, `admin` and `user`.
_Avoid_: role, user role, account type, permission level

**Process role**:
Which process a running binary is — `web`, `worker`, or `migrate`. A property of the deployment, not of any person.
_Avoid_: role, mode, service type

**Community membership role**:
What a person may do inside one community they belong to. Confined to that community and never a source of platform authority.
_Avoid_: role, admin, moderator

### Audit

**Actor**:
The person who performed a recorded change. Absent only when the change came from an operational command rather than a signed-in human.
_Avoid_: user, admin, author, performer

**Target**:
The person a recorded change was performed on. Distinct from the actor even when they are the same person.
_Avoid_: user, subject, recipient

**Bootstrap**:
Granting the first platform admin when the platform has none, through an operational command rather than any interface. Not a synonym for promotion — a bootstrap has no actor.
_Avoid_: seed, setup, initial admin, first-run

**Promotion / demotion**:
A change of platform role performed by an admin who is already signed in, and therefore always attributable to an actor.
_Avoid_: grant, elevate, upgrade, role assignment

### Accounts

**Deleted account**:
An account marked dead. It cannot be signed into, and its data is retained rather than erased — see [ADR-0001](docs/adr/0001-deleted-accounts-are-retained-not-erased.md). The address it used may be registered again by a new account.
_Avoid_: erased, removed, purged, closed

### Vehicles and parts

**Private vehicle data**:
Number plate, VIN, purchase price, and service costs. Never leaves the server for anyone who should not see it, including an admin.
_Avoid_: sensitive fields, PII, hidden fields

**Part**:
One exact configuration of a component — a specific wheel in a specific size, offset, and bolt pattern. Two wheels sharing a product name but differing in any measured value are two parts.
_Avoid_: product, SKU, item, component

**Evidence**:
Community content cited in support of an answer, carrying where it came from and how well it matches the asking vehicle. Reported, hidden, and deleted content is never evidence.
_Avoid_: source, reference, proof, citation
