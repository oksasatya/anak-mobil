# AnakMobil.id — Product Requirements Document

**Version:** 1.0  
**Status:** Draft  
**Product:** AnakMobil.id  
**Market:** Indonesia-first  
**Product Type:** Automotive Community Platform + Digital Garage + AI Car Assistant  
**Platform Strategy:** Mobile App First + Public Web Companion  
**Last Updated:** 14 August 2026

---

# 1. Product Overview

**AnakMobil.id** adalah platform otomotif Indonesia yang menjadikan **mobil sebagai identitas digital utama pengguna**.

User membuat **My Garage**, menambahkan kendaraan, menyimpan build/modifikasi, service history, problem/repair history, bergabung ke komunitas yang relevan, melihat setup pengguna lain, menemukan bengkel dan event, serta menggunakan **AnakMobil AI** untuk bertanya seputar kendaraan mereka.

### Product Positioning

> **Mobil lo. Build lo. Komunitas lo.**

### Product Promise

AnakMobil.id harus mampu menjawab tiga kebutuhan utama:

1. **Kenali mobil gue**
2. **Hubungkan gue dengan pemilik mobil yang relevan**
3. **Bantu gue mengambil keputusan lebih baik soal mobil**

---

# 2. Vision

Menjadi **digital home untuk car culture Indonesia**.

Dalam jangka panjang AnakMobil.id menghubungkan:

- Pemilik mobil
- Mobil dan build-nya
- Komunitas
- Knowledge
- Bengkel
- Part aftermarket
- Event
- Touring
- Roadside help
- Seller
- Automotive creator

Semua berpusat pada **vehicle identity**.

---

# 3. Core Principles

## 3.1 Car First, Feed Second

Mobil adalah entry point utama.

User tidak datang untuk membuat social profile kosong.

Flow:

```text
Register
  ↓
Add Car
  ↓
Your Garage
  ↓
Relevant Builds
  ↓
Relevant Problems
  ↓
Relevant Community
  ↓
Relevant AI
```

---

## 3.2 Utility Before Social

AnakMobil.id harus tetap berguna meskipun user belum memiliki teman di platform.

Standalone utility:

- Digital garage
- Build history
- Service history
- Problem history
- AI car assistant
- Fitment advisor
- Community knowledge

---

## 3.3 Structured Automotive Data

Data utama tidak boleh hanya berupa post.

Data harus dikaitkan dengan:

- Brand
- Model
- Generation
- Variant
- Year
- Engine
- Transmission
- Wheel specification
- Tyre specification
- Suspension
- Modification
- Part
- Service
- Problem
- Diagnosis
- Solution
- Cost
- Mileage
- Garage

---

## 3.4 Community-Validated Intelligence

AnakMobil.id tidak boleh menyatakan data komunitas sebagai fakta absolut.

Gunakan wording seperti:

- "87 Civic FD builds menggunakan setup ini"
- "Commonly reported solution"
- "Community median price"
- "Likely compatible"
- "Needs verification"
- "Based on 42 similar vehicles"

---

## 3.5 AI Must Be Grounded

AI tidak boleh menjadi chatbot generic yang hanya menjawab dari LLM knowledge.

AI harus menggunakan context:

```text
User
+
Vehicle
+
Current Build
+
Service History
+
Community Data
+
Fitment Database
+
Repair Cases
+
Verified Automotive Data
```

---

## 3.6 Privacy by Default

Private by default:

- Nomor polisi
- VIN
- Exact realtime location
- Home address
- Purchase price
- Sensitive document
- Personal contact data

---

# 4. Primary Problems

| Problem | Frequency | Pain | Opportunity |
|---|---:|---:|---:|
| Cari bengkel yang benar-benar ngerti mobil kita | High | Very High | Very High |
| Cari part/modif yang compatible | High | Very High | Very High |
| Cari event/car meet | High | Medium | Very High |
| Tidak tahu komunitas/nongkrong yang relevan | High | Medium | High |
| Touring rombongan kepencar | Medium | High | High |
| Trouble di jalan dan butuh bantuan | Medium | Very High | Very High |
| Pertanyaan problem mobil berulang di group | High | Medium | High |
| Tidak tahu harga wajar jasa/part | High | High | Very High |
| Trust jual/beli part bekas | High | High | High |
| History modifikasi berantakan | Medium | Medium | High |
| Cari fotografer otomotif | Medium | Low-Medium | Medium |
| Cari komunitas yang cocok | Medium | Medium | High |

---

# 5. Target Users

## 5.1 Car Enthusiast

Characteristics:

- Memiliki minimal 1 mobil
- Aktif atau tertarik dengan komunitas otomotif
- Suka modifikasi, detailing, maintenance, meet, touring, atau car culture
- Sering bertanya mengenai bengkel, part, setup, problem, dan harga

### Jobs To Be Done

> Ketika saya ingin modifikasi mobil, saya ingin tahu setup apa yang sudah terbukti digunakan mobil seperti milik saya.

> Ketika mobil saya bermasalah, saya ingin melihat kasus serupa sebelum pergi ke bengkel.

> Ketika saya ingin bertanya soal mobil, saya ingin AI yang sudah mengetahui mobil dan build saya.

---

## 5.2 Community Admin

Needs:

- Manage community
- Manage members
- Manage events
- Moderate discussions
- Publish announcements
- Build trusted knowledge

---

## 5.3 Garage / Workshop — Future

Needs:

- Claim business profile
- Vehicle specialization
- Structured reviews
- Leads
- Booking
- CRM
- Analytics

---

## 5.4 Parts Seller — Future

Needs:

- Product catalog
- Vehicle compatibility
- Store verification
- Qualified automotive audience

---

# 6. Platform Strategy

## 6.1 Mobile App — Primary Product

Recommended:

- Flutter
- Android
- iOS

Mobile owns:

- Authentication
- My Garage
- Build
- Service
- Problem
- AI assistant
- Community
- Explore
- Events
- Camera
- Notifications
- Future location / touring / SOS

---

## 6.2 Public Web — Acquisition

Recommended:

- Next.js
- SSR
- SEO
- OpenGraph
- Deep links

Public pages:

```text
/@username
/car/:slug
/build/:slug
/community/:slug
/problems/:slug
/events/:slug
/garage/:slug
```

Examples:

```text
anakmobil.id/@oksa
anakmobil.id/@oksa/civic-fd
anakmobil.id/c/civic-fd-indonesia
anakmobil.id/problems/civic-fd-rack-steer
```

---

# 7. Core User Flow

## 7.1 First Time User

```text
Welcome
  ↓
Register / Login
  ↓
Add First Car
  ↓
Brand
  ↓
Model
  ↓
Generation
  ↓
Year
  ↓
Variant
  ↓
Optional Engine / Transmission
  ↓
Upload Photo
  ↓
Garage Ready
```

Then:

```text
Your Honda Civic FD

837 Builds
184 Known Issues
421 Parts
7 Communities

[ Ask AnakMobil AI ]
```

This screen is the first **Aha Moment**.

---

# 8. My Garage

One user can own multiple vehicles.

Example:

```text
MY GARAGE

Honda Civic FD1
2008

BMW E46 325i
2004
```

Each vehicle has:

- Identity
- Photos
- Specs
- Build
- Modification history
- Service history
- Problem history
- Expenses
- Community relationship

---

# 9. Vehicle Profile

Minimum fields:

- Brand
- Model
- Generation
- Variant
- Year
- Photo

Optional:

- Engine
- Engine code
- Transmission
- Color
- Mileage
- Drive type
- Power
- Torque
- Wheel drive
- Purchase date

Sensitive/private:

- Plate
- VIN
- Purchase price

---

# 10. Build & Modification

Each vehicle can contain structured modifications.

Categories:

- Wheels
- Tyres
- Suspension
- Brake
- Engine
- Intake
- Exhaust
- ECU
- Transmission
- Exterior
- Interior
- Lighting
- Audio
- Electronics
- Other

Example:

```text
Wheels
Enkei RPF1
18x8.5
ET40

Tyres
Michelin Pilot Sport 5
225/40 R18

Suspension
Tein Flex Z
```

Modification fields:

- Category
- Brand
- Product
- Specification
- Installation date
- Mileage
- Cost
- Garage
- Notes
- Photos

---

# 11. Build Timeline

Example:

```text
JAN 2026
Bought Car

FEB 2026
Enkei RPF1
Rp14.500.000

MAR 2026
Michelin PS5
Rp8.000.000

MAY 2026
Tein Flex Z
Rp11.000.000
```

Optional:

```text
Total Build Value
Rp33.500.000
```

Cost visibility:

- Private
- Friends/community
- Public

---

# 12. Service History

Service record fields:

- Vehicle
- Date
- Mileage
- Service category
- Parts replaced
- Garage
- Cost
- Notes
- Photos / invoice
- Next service mileage
- Next service date

Example:

```text
143,120 KM

CV Joint Replacement

Garage:
XYZ Garage

Cost:
Rp1.200.000
```

---

# 13. Problem & Repair Knowledge

User can record:

```text
Problem
Bunyi tek-tek saat belok

Vehicle
Honda Civic FD1

Mileage
143,000 KM
```

Optional:

- Symptoms
- Photos
- Audio
- Video
- Error code
- Diagnosis
- Final solution
- Parts changed
- Garage
- Cost

After solved:

```text
Status:
SOLVED

Diagnosis:
CV Joint

Solution:
Replace CV Joint
```

Structured solved cases contribute to community knowledge.

---

# 14. AnakMobil AI

## 14.1 Product Definition

**AnakMobil AI** adalah automotive assistant yang memiliki context terhadap kendaraan user.

AI entry points:

- Home
- Vehicle page
- Build page
- Problem page
- Parts page
- Dedicated AI tab/button

Primary CTA:

> **Tanya AnakMobil AI**

---

# 15. AI Context

Before answering, AI should resolve:

```text
Current User
  ↓
Selected Vehicle
  ↓
Vehicle Specifications
  ↓
Current Build
  ↓
Service History
  ↓
Problem History
  ↓
Relevant Community Knowledge
  ↓
Relevant Parts/Fitment Data
```

Therefore user can ask:

> "Velg yang cocok buat mobil gue apa?"

without repeating:

> Honda Civic FD1 2008.

---

# 16. AI Fitment Advisor

## 16.1 Example

User:

> Kalau gue pakai RPF1 18x8.5 ET35 aman nggak?

AI already knows:

```text
Honda Civic FD1
2008
Current suspension: Tein Flex Z
Current tyre: 225/40 R18
```

Response structure:

```text
FITMENT RESULT

Compatibility:
LIKELY COMPATIBLE

Confidence:
High

Community Evidence:
42 similar Civic FD builds

Potential Issue:
Front may sit aggressive

Common Setup:
18x8.5 ET38-40
225/40 R18

ET35:
May require additional clearance depending on ride height.

Recommended:
Check fender clearance before purchase.
```

---

## 16.2 Fitment Inputs

For wheels:

- Diameter
- Width
- Offset
- PCD
- Center bore

For tyres:

- Width
- Aspect ratio
- Diameter

Vehicle context:

- OEM fitment
- Current suspension
- Ride height
- Brake setup
- Fender modification
- Community builds

---

## 16.3 Fitment Confidence

AI must classify:

```text
Verified
High Confidence
Medium Confidence
Low Confidence
Insufficient Data
```

AI must never fabricate precise fitment certainty when data is incomplete.

---

# 17. AI Car Q&A

Users can ask:

> Oli apa yang cocok?

> Kenapa Civic FD gue bunyi pas belok?

> Service 150 ribu km harus cek apa?

> Setup harian yang nyaman tapi agak rendah gimana?

> Upgrade brake dulu atau suspension dulu?

> RPF1 atau CE28 untuk setup gue?

> Mobil gue sekarang modifnya apa aja?

> Total biaya build gue berapa?

> Kapan terakhir ganti engine mounting?

AI should understand both:

- General automotive knowledge
- User-specific vehicle history

---

# 18. AI Problem Assistant

Example:

User:

> Mobil gue kalau belok kiri bunyi tek tek.

AI workflow:

```text
Question
  ↓
Identify selected vehicle
  ↓
Retrieve similar cases
  ↓
Retrieve service history
  ↓
Generate possible causes
  ↓
Rank possibilities
  ↓
Recommend inspection
```

Response:

```text
Based on 63 similar Civic FD cases:

1. CV Joint
   51%

2. Rack End
   22%

3. Tie Rod
   14%

4. Other
   13%
```

Important:

AI must phrase these as **possibilities, not diagnosis**.

For safety-critical problems such as:

- Brake failure
- Steering issue
- Overheating
- Fuel leak
- Severe engine warning
- Accident damage

AI should recommend stopping driving and professional inspection where appropriate.

---

# 19. AI Build Advisor

User:

> Gue punya budget 15 juta. Enaknya upgrade apa dulu?

AI considers:

- Vehicle
- Existing modifications
- Build goal
- Budget
- Community setups

First ask or infer goal:

```text
Daily
Comfort
Track
Performance
Show
Stance
Touring
```

Example output:

```text
Budget:
Rp15.000.000

Goal:
Daily + handling

Recommended order:

1. Tyres
2. Suspension
3. Brake pads + fluid
4. Alignment

Avoid:
Engine modification first

Reason:
Your current build is still stock in handling-related areas.
```

---

# 20. AI Maintenance Assistant

Examples:

> Mobil gue service apa berikutnya?

> Kapan terakhir ganti oli?

> Ada service yang kelewat?

AI should inspect service history.

Example:

```text
Current mileage:
146,100 KM

Last oil change:
140,200 KM

Distance:
5,900 KM

Potential upcoming:
Engine oil
Brake inspection
Transmission fluid check
```

User can create reminder directly from AI response.

---

# 21. AI Community Knowledge Search

Instead of:

```text
Search forum
Scroll 200 messages
```

User asks:

> Civic FD biasanya masalah rack steer gimana?

AI retrieves:

- Solved cases
- Community discussions
- Service records
- Garage data

Answer should link back to source objects:

```text
Related:
31 solved cases
12 builds
8 garage reviews
```

AI must not replace community content.

It summarizes and routes users to underlying evidence.

---

# 22. AI Architecture

Recommended conceptual flow:

```text
Mobile / Web
      ↓
AI Gateway
      ↓
Context Builder
      ↓
Retrieval Layer
      ↓
LLM
      ↓
Structured Response
```

---

## 22.1 Context Builder

Responsibilities:

- Resolve selected vehicle
- Resolve vehicle specs
- Current modifications
- Service history
- Existing problems
- User preferences

---

## 22.2 Retrieval

Potential sources:

```text
vehicle_specs
parts
fitments
builds
modifications
service_records
problem_cases
garage_reviews
community_posts
verified_knowledge
```

Recommended:

- PostgreSQL
- pgvector
- Hybrid search
- Metadata filtering

Example metadata filter:

```text
brand = Honda
model = Civic
generation = FD
```

before semantic retrieval.

---

## 22.3 Model Provider Abstraction

Backend must not be tightly coupled to one LLM provider.

Interface concept:

```text
LLMProvider
  ├── chat()
  ├── structuredOutput()
  ├── embeddings()
  └── vision()
```

Allows future use of multiple model providers.

---

# 23. AI Responses

Prefer structured responses instead of unrestricted text.

Example response model:

```json
{
  "answer": "...",
  "confidence": "high",
  "vehicleContextUsed": true,
  "recommendations": [],
  "warnings": [],
  "evidence": []
}
```

This allows UI to render:

- Confidence
- Cards
- Parts
- Builds
- Garage links
- Warning
- Actions

---

# 24. AI Vision — Future

Future AI can analyze:

- Wheel photo
- Engine bay
- Tyre condition
- Dashboard warning
- Part photo
- Damage photo

Possible flow:

```text
Upload photo
  ↓
AI Vision
  ↓
Identify visual clues
  ↓
Cross-reference selected vehicle
  ↓
Suggest next inspection
```

AI must not claim certainty for mechanical diagnosis based only on image.

---

# 25. Community

Community types:

- Brand
- Model
- Generation
- Region
- Interest
- Club

Examples:

```text
Honda Indonesia
Civic FD Indonesia
Civic FD Jakarta
BMW E46 Indonesia
JDM Indonesia
Track Day Indonesia
```

---

# 26. Community Recommendation

After vehicle creation:

```text
Honda Civic FD

Recommended:

Civic FD Indonesia
Honda Civic Indonesia
Honda Jakarta
```

Relevance comes from vehicle data.

---

# 27. Community Content Types

Community supports structured content:

- Discussion
- Problem
- Build
- Modification
- Question
- Garage recommendation
- Event
- Marketplace — future

Avoid making generic social posts the primary product loop.

---

# 28. Explore Builds

Users can explore builds based on:

- Same vehicle
- Wheel setup
- Suspension
- Style
- Engine
- Popular
- Recent

Example:

```text
Civic FD
18x8.5 ET40
Tein Flex Z
225/40 R18

128 Builds
```

---

# 29. Parts & Fitment Intelligence

Parts database contains:

- Brand
- Product
- Category
- Specs

Relationship:

```text
Part
  ↓
Installed Build
  ↓
Vehicle
```

This generates community evidence.

Example:

```text
Enkei RPF1
18x8.5 ET40

Used by:
87 Civic FD builds

Most common tyre:
225/40 R18
```

---

# 30. Garage Discovery — Phase 2

Garage data:

- Name
- Address
- Location
- Specializations
- Vehicles handled
- Services
- Reviews
- Solved cases

Review must be structured.

Example:

```text
Garage:
XYZ Garage

Vehicle:
Honda Civic FD1

Job:
CV Joint Replacement

Cost:
Rp1.200.000

Rating:
5/5
```

---

# 31. Price Intelligence — Phase 2

Aggregate anonymized service prices.

Example:

```text
Civic FD
Rack Steer Repair

Low:
Rp1.200.000

Median:
Rp1.700.000

High:
Rp2.400.000

Based on:
83 repair records
```

Never present as mandatory market price.

---

# 32. Events & Car Meets — Phase 2

Event fields:

- Organizer
- Community
- Name
- Description
- Date/time
- Location
- Capacity
- Vehicle requirements
- Attendees

Example:

```text
Civic FD Sunday Meet

74 Cars Going
```

When joining:

```text
Joining With:

Honda Civic FD1 2008
```

---

# 33. Touring / Convoy — Future

Features:

- Create touring event
- Meeting point
- Destination
- Participant list
- Lead
- Sweep
- Temporary location sharing
- Regroup points

Realtime location sharing:

- Opt-in only
- Active only during session
- Auto expires

---

# 34. SOS / Roadside Community — Future

SOS categories:

- Flat tyre
- Battery
- Overheat
- Engine problem
- Accident
- Need towing
- Other

Recipients:

- Friends
- Community
- Verified garage
- Towing
- Selected contacts

SOS must not expose exact location publicly.

---

# 35. Marketplace — Future

Categories:

- Wheels
- Tyres
- Suspension
- Engine
- Brake
- Exterior
- Interior
- Audio
- OEM Parts
- Performance Parts

Marketplace advantage:

```text
Product
  ↓
Compatibility
  ↓
Vehicle
```

Potential:

> "Likely compatible with your Civic FD."

---

# 36. User Reputation — Future

Potential reputation signals:

- Account age
- Verified vehicle
- Community membership
- Completed transactions
- Garage reviews
- Helpful answers
- Solved cases

Do not use only follower count as trust.

---

# 37. Automotive Creator / Photographer — Future

Creator profiles:

- Automotive photography
- Rolling shots
- Event coverage
- Video
- Drone

Future monetization via booking/commission.

---

# 38. Product Navigation

Recommended mobile navigation:

```text
Home
Garage
Explore
Community
Profile
```

Global prominent action:

```text
+
```

Actions:

- Add modification
- Add service
- Add problem
- Add photo
- Create event

AI should remain easily accessible through:

```text
Ask AI
```

or persistent contextual entry point.

---

# 39. Home

Home is contextual.

Example:

```text
Good evening.

Your Civic FD
146,100 KM

Ask AnakMobil AI
"What do you want to know?"

For Your Car

Known Issues
Popular Builds
Community Updates
Upcoming Service
Events
```

Avoid generic infinite-feed-first homepage.

---

# 40. MVP — Phase 1

Must-have:

## Authentication
- Register
- Login
- OAuth optional
- Profile

## Vehicle
- Add vehicle
- Edit vehicle
- Multiple vehicles
- Select active vehicle
- Vehicle profile

## Digital Garage
- Vehicle photos
- Build summary
- Modification history

## Service
- Add service
- Service timeline

## Problems
- Create problem
- Mark solved
- Record diagnosis/solution

## Community
- Community discovery
- Join/leave
- Relevant discussions

## Explore
- Browse same-vehicle builds
- Build detail

## AnakMobil AI
- Car Q&A
- Vehicle-aware context
- Problem assistant
- Fitment Q&A
- Build advisor
- Maintenance assistant
- Community knowledge retrieval
- Confidence indicator
- Evidence links

---

# 41. Out of Scope for V1

Do NOT build initially:

- Marketplace transaction/payment
- Full garage booking
- Realtime convoy
- Realtime nearby users
- SOS dispatch network
- Creator marketplace
- Insurance
- Vehicle financing
- Full AI image diagnosis
- Social short-video feed
- Gamification-heavy system

---

# 42. Phase 2

Add:

- Garage discovery
- Structured garage reviews
- Price intelligence
- Events
- Car meets
- Community admin tools
- Expanded parts database
- Improved fitment engine
- Public SEO knowledge pages

---

# 43. Phase 3

Add:

- Marketplace
- Verified sellers
- Garage booking
- Garage dashboard
- Parts seller dashboard
- Event organizer tools
- Paid promotion

---

# 44. Phase 4

Add:

- Touring
- Convoy
- SOS
- Towing
- Roadside partnerships
- Location-based capabilities

---

# 45. Public Web SEO

Important landing entities:

```text
Honda Civic FD
BMW E46
Toyota Yaris Bakpao
Mazda 2
etc.
```

SEO pages may contain:

- Common issues
- Popular builds
- Popular wheels
- Popular suspension
- Service knowledge
- Community
- Garages

Example:

```text
anakmobil.id/cars/honda/civic/fd
```

---

# 46. Core Data Model

High-level entities:

```text
User
Vehicle
VehicleModel
VehicleGeneration
VehicleVariant

Build
Modification
Part
Fitment

ServiceRecord
ProblemCase
RepairSolution

Community
CommunityMember
CommunityPost

Garage
GarageReview

Event
EventAttendee

AIConversation
AIMessage
AIResponseEvidence
```

---

# 47. Relationship Overview

```text
User
  ↓
Vehicle
  ├── Build
  │    └── Modification
  │          └── Part
  │
  ├── ServiceRecord
  ├── ProblemCase
  └── Communities

ProblemCase
  └── RepairSolution
       └── Garage

Vehicle
  ↓
Fitment Evidence
  ↓
Part

AI
  ↓
Vehicle Context
  ↓
Retrieval
  ↓
Evidence
```

---

# 48. Suggested Backend Architecture

Use a **modular monolith** initially.

Suggested modules:

```text
auth
users
vehicles
builds
parts
services
problems
communities
events
garages
ai
media
notifications
```

Recommended:

- Go
- PostgreSQL
- PostGIS
- Redis
- pgvector
- Object Storage
- WebSocket when required

No microservices for MVP.

---

# 49. AI Module Boundary

AI module should not own core business data.

It reads through application interfaces.

Example:

```text
AIService
  ↓
VehicleContextProvider
BuildProvider
ServiceProvider
ProblemProvider
FitmentProvider
KnowledgeRetriever
```

AI produces recommendations, not authoritative database mutations without user confirmation.

---

# 50. AI Cost Controls

Implement from day one:

- Token limits
- Conversation summarization
- Context pruning
- Retrieval before generation
- Model routing
- Usage quota
- Rate limit
- Cache common automotive queries
- Cost logging per AI request

Potential free plan:

```text
20 AI questions / month
```

Paid plans can increase limits later.

Exact pricing is not part of MVP PRD.

---

# 51. AI Safety Requirements

AI must clearly distinguish:

```text
FACT
COMMUNITY EVIDENCE
INFERENCE
RECOMMENDATION
UNKNOWN
```

AI must avoid giving false certainty for:

- Braking
- Steering
- Structural damage
- Fuel leak
- Electrical fire
- Overheating
- Accident damage
- Safety-critical modifications

When safety risk is plausible, AI should recommend inspection by qualified technician.

---

# 52. Notifications

V1:

- Community activity
- Comment/reply
- Build interaction
- Service reminder
- AI reminder generated from service data

Future:

- Event reminder
- Marketplace
- Touring
- SOS

---

# 53. Product Metrics

## Activation

User is activated when:

```text
Registered
+
Added first vehicle
+
Performed at least one meaningful action
```

Meaningful action:

- Add modification
- Add service
- Add problem
- Join community
- Ask AI question

---

## North Star Candidate

**Weekly Active Vehicles (WAV)**

A vehicle is active when owner performs or receives meaningful value involving that vehicle.

Examples:

- AI query
- Build update
- Service record
- Problem
- Community interaction
- Fitment research

This is preferable to raw page views.

---

# 54. Supporting Metrics

Track:

- Vehicle creation rate
- Garage completion rate
- AI questions/user
- AI answer helpful rate
- Build creation rate
- Service logs/user
- Problems solved
- Community joins
- D7 retention
- D30 retention
- Public build shares
- Search-to-answer rate
- AI retrieval evidence coverage

---

# 55. AI Quality Metrics

Track:

- Helpful / not helpful
- Answer regenerated
- Evidence click-through
- Unsupported claim rate
- Low-confidence responses
- User corrections
- Fitment confirmation
- Problem eventually solved

Future:

When a user resolves a problem, compare final diagnosis against prior AI suggestions.

This creates a feedback loop.

---

# 56. Data Flywheel

```text
User adds car
      ↓
Adds build
      ↓
Adds part
      ↓
Fitment evidence improves
      ↓
AI answers improve
      ↓
More owners get value
      ↓
More owners contribute data
```

Repair:

```text
Problem
  ↓
Diagnosis
  ↓
Solution
  ↓
Cost
  ↓
Garage
  ↓
Community knowledge
  ↓
AI retrieval
```

This is a major long-term moat.

---

# 57. Monetization — Future

Consumer account should initially be free.

Potential revenue:

## Garage SaaS
- Verified profile
- Leads
- Booking
- CRM
- Analytics
- Featured listing

## Seller
- Verified store
- Listings
- Promoted product
- Transaction fee

## Events
- Promoted events
- Ticketing
- Registration
- Sponsorship

## AI Pro
Possible future premium:

- Higher AI limits
- Advanced build planning
- Comparison
- Deep service analysis
- Image analysis
- Export vehicle report

Do not gate basic community contribution behind payment.

---

# 58. Competitive Differentiation

AnakMobil.id should NOT compete primarily as:

> Automotive social media.

Differentiate via:

```text
Digital Vehicle Identity
+
Structured Build Data
+
Structured Repair Knowledge
+
Community Evidence
+
AI
```

Social interaction is a consequence of shared vehicle context.

---

# 59. Key Product Loop

```text
ADD CAR
   ↓
SEE OWNERS LIKE ME
   ↓
ASK AI / EXPLORE
   ↓
SEE RELEVANT BUILDS & PROBLEMS
   ↓
ADD MY BUILD / SERVICE / PROBLEM
   ↓
HELP NEXT OWNER
   ↓
DATA GETS BETTER
   ↓
AI GETS BETTER
   ↓
MORE VALUE FOR EVERY OWNER
```

---

# 60. MVP Success Criteria

MVP should demonstrate:

1. User understands that their car is the center of the product.
2. User can create a useful digital garage.
3. User can discover relevant owners/builds.
4. User can log service/problem history.
5. AnakMobil AI can answer contextual questions about the selected vehicle.
6. AI can surface supporting community evidence.
7. Community data improves future answers.
8. Public build/profile links are shareable.

---

# 61. Non-Goals

AnakMobil.id is not:

- Generic social media
- Generic marketplace
- Generic workshop directory
- Replacement for certified mechanic inspection
- Vehicle ECU diagnostic tool
- Navigation application
- Insurance platform
- Vehicle financing platform

---

# 62. Product Identity

**Brand:** AnakMobil.id

Primary phrase:

> **Mobil lo. Build lo. Komunitas lo.**

AI product:

> **AnakMobil AI**

Possible CTA:

> Tanya Mobil Lo

or

> Tanya AnakMobil AI

The product should feel:

- Automotive
- Modern
- Knowledgeable
- Community-driven
- Trustworthy
- Indonesia-native

---

# 63. Final Product Thesis

The core asset of AnakMobil.id is not the feed.

It is the structured graph connecting:

```text
OWNER
  ↓
CAR
  ↓
BUILD
  ↓
PART
  ↓
PROBLEM
  ↓
SOLUTION
  ↓
GARAGE
  ↓
COMMUNITY
```

**AnakMobil AI** becomes the conversational interface over that graph.

The long-term product advantage comes from answering questions that generic AI, Google, marketplace search, and generic social media cannot answer reliably because they do not know:

> **mobil user, build user, history user, dan pengalaman nyata pemilik kendaraan yang sama.**
