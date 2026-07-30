# Licensing FAQ

Nahuali is source-available under the Functional Source License 1.1 with an MIT
future grant, abbreviated **FSL-1.1-MIT**. This FAQ is a practical summary, not
legal advice.

Primary sources:

- Repo license: `LICENSE`
- SPDX entry: https://spdx.org/licenses/FSL-1.1-MIT.html
- FSL license and FAQ: https://fsl.software/
- Open Source Definition: https://opensource.org/osd

## Is Nahuali open source?

Current Nahuali releases are source-available (FSL-1.1-MIT), not OSI open
source. The source is visible and the license grants broad use rights, but the
current license restricts competing commercial use until the future MIT grant
applies.

## What does FSL-1.1-MIT allow?

The license grants the right to use, copy, modify, create derivative works,
publicly perform, publicly display, and redistribute the software for any
Permitted Purpose (`LICENSE:21-23`). Permitted Purposes include internal use and
access, non-commercial education, non-commercial research, and professional
services provided to a licensee using the software under the license terms
(`LICENSE:35-43`).

## What does it block?

The license blocks a Competing Use. The license defines that as making the
software available to others in a commercial product or service that substitutes
for Nahuali, substitutes for another product or service offered using Nahuali as
of the version date, or offers the same or substantially similar functionality
(`LICENSE:25-33`).

If your planned use might be a competing hosted product or service, check with
counsel before relying on this FAQ.

## Does FSL prevent someone from copying Nahuali?

It restricts distributing the current code in a competing commercial product or
service. It does not make the source secret, protect the underlying ideas or
algorithms, prevent an independent implementation, or make enforcement
automatic. A party can also use each published version under MIT after that
version's two-year conversion date.

The practical trade-off is deliberate: FSL adds a contractual restriction on
direct competing commercial use of the current code that MIT and Apache-2.0 do
not contain, while being less permissive for adopters than an OSI-approved
open-source license. Whether and how that restriction can be enforced depends
on the facts and applicable law. Organizations whose policy requires an
OSI-approved license may decline or delay adoption. The license is one part of
a product strategy, not a substitute for execution, distribution, support, or
brand.

## What happens after two years?

Each version receives an irrevocable future MIT grant on the second anniversary
of the date that version is made available (`LICENSE:65-71`). On or after that
date, that version may be used under MIT terms. Newer versions have their own
two-year clocks.

## Can I self-host it?

Yes, self-hosted internal use is a Permitted Purpose when it does not become a
Competing Use. The README gives the same practical summary while identifying the
license text as binding (`README.md:279-288`).

## Can I sell consulting or integration work around it?

The FSL text permits professional services provided to a licensee using the
software in accordance with the license (`LICENSE:35-43`). That is different
from offering a competing commercial product or service.

## How does this compare to Sentry, Keygen, Liquibase, and Qdrant?

These are licensing precedents and market references, not legal equivalence:

- **Sentry** uses FSL for the Sentry and Codecov web applications, describes FSL
  as eventually open source, and states that it restricts offering a
  Sentry-like commercial service before the future open-source grant applies:
  https://open.sentry.io/licensing/.
- **Liquibase** moved Liquibase Community to FSL 1.1 with an Apache 2.0 future
  grant, describing production use, modification, contribution, and consulting
  as allowed while blocking competing commercialization:
  https://www.liquibase.com/blog/liquibase-community-for-the-future-fsl.
- **Keygen** is Fair Source under FCL rather than FSL. It is still relevant as a
  precedent for delayed open-source conversion and non-compete source-available
  licensing, but it is not the same license family:
  https://github.com/keygen-sh/keygen-api and https://fcl.dev/.
- **Qdrant is not an FSL precedent.** The Qdrant database is licensed under
  Apache-2.0 and is offered alongside Qdrant Cloud. That is an open-source core
  plus managed-service model, not the delayed MIT conversion used by Nahuali:
  https://github.com/qdrant/qdrant#license.

## How should public docs name the license?

Use **source-available (FSL-1.1-MIT)** when describing Nahuali's current license
posture. Avoid describing the current version as open source or OSS unless you
are specifically discussing the future MIT grant after the two-year conversion.
