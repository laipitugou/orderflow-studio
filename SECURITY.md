# Security policy

## Supported versions

Flowdepth is currently beta software. Security fixes are applied to the latest `develop` branch and, when practical, the newest `develop-<commit>` prerelease. Older beta builds and historical tags are not supported; reporters may be asked to verify an issue against the latest code.

## Reporting a vulnerability

Please do not immediately publish details of a vulnerability that could put users or their systems at risk.

Use this repository's [GitHub Security Advisories](https://github.com/Niketion/flowdepth/security/advisories/new) to report sensitive vulnerabilities privately, if private reporting is available. If it is unavailable, open a [GitHub issue](https://github.com/Niketion/flowdepth/issues/new) containing no sensitive technical details and ask the repository maintainers to arrange a private channel. Do not include exploit code, credentials, private data, or reproduction details in that public issue.

Include the affected version or commit, operating system, impact, reproduction conditions, and any proposed mitigation when it is safe to do so privately.

This volunteer project does not promise a response or remediation SLA. Maintainers will assess reports according to severity, reproducibility, and available capacity.

## Exchange access

Current Flowdepth market features consume public exchange REST APIs and WebSockets and do not require exchange trading credentials. Optional proxy authentication is stored separately and should still be treated as sensitive. Never attach credentials, API keys, saved secrets, or unredacted private configuration to an issue or advisory.
