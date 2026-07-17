# Security policy

## Supported versions

Security fixes are applied to the latest published stable line. Before 1.0, consumers should pin the exact `0.x` version and review release notes before upgrading because minor releases may still refine public contracts.

## Reporting a vulnerability

Do not publish malicious documents, exploit details or private user data in a public issue. Send a private report to the repository owner/security contact configured for the eventual hosting organization, including the affected version, format, smallest safe reproducer, impact and mitigation. If no private contact is configured yet, keep the report private and notify the maintainer through the hosting provider's private vulnerability-reporting feature.

The project should acknowledge a complete report within three business days, provide an initial severity assessment within seven, and coordinate disclosure after a fix or mitigation is available. No bounty is promised.

## Security model

See `docs/security.md` for trust boundaries, active-content policy, resource limits, fuzzing and audit commands. Password-protected files, document scripts/macros, external relationships and server-side uploads are not supported paths.
