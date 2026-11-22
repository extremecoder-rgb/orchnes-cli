# Security policy

Please report vulnerabilities privately via **Security → Report a vulnerability** on this repository, not in public issues.
Include steps to reproduce, the `orchnes` version (`orchnes --version`), and your OS.

Relevant areas: handling of API keys (they must only come from environment variables and never appear in output or logs),
file writes by `install`/`export` (paths must stay inside the chosen destination), and requests sent by the model gateway.
We aim to acknowledge reports within 7 days.
