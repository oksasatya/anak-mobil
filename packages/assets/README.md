# @anakmobil/assets

Brand assets shared by every surface: the landing page, the backoffice, and the mobile app.

```ts
import logo from "@anakmobil/assets/img/logo.png";
```

Importing through the package rather than by relative path is the point. A relative import that crosses an app boundary — `../../../packages/assets/img/logo.png` — breaks the moment an app moves, and gives no signal that the file is shared rather than local.

## Contents

| File | Use |
|---|---|
| `img/logo.png` | Horizontal lockup — headers, navigation |
| `img/logo-vertikal.png` | Vertical lockup — splash screens, square placements |
| `img/favicon-light.png` | Favicon for light backgrounds |
| `img/favicon-dark.png` | Favicon for dark backgrounds |

## What does not belong here

Design tokens. Colour, typography, radius, and spacing live in `@anakmobil/tokens`, generated from `docs/design.md` into CSS custom properties, a Tailwind theme, and React Native constants. Assets are files; tokens are values. Keeping them apart is what stops a hex code from being copied into three places and drifting.

Feature illustrations and screenshots also stay with the app that uses them. This package is for marks that must be identical everywhere.
