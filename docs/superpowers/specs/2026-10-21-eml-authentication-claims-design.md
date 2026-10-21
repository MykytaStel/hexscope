# EML authentication claims

**Status:** Proposed for review
**Date:** 2026-10-21

## Goal

Stop presenting an `Authentication-Results` header in a locally opened `.eml` as independently verified SPF, DKIM, or DMARC evidence. A standalone message file cannot establish which mail system added that header or whether it was preserved inside a trusted boundary.

## Behavior

- Treat recognized results in every outer-message `Authentication-Results` header as claims contained in the file.
- Do not infer trust from a missing `authserv-id`, a matching `Received` host, or the presence of a `pass` result.
- Do not give claimed pass or fail results a positive or negative trust tone. The email path says that the header reports the result and Hexscope cannot verify who added it.
- Keep the header byte range and claim visible as parser evidence. Ignore malformed or unrecognized status tokens rather than turning them into authentication findings.
- Use one `authclaim` fact kind for the recognized claims, and update Web, CLI summaries, the EML format explanation, share categories, and English/Ukrainian copy together. Do not change unrelated file-verification statuses or claim that Hexscope performs live DNS, SPF, DKIM, or DMARC checks.

## Acceptance

- A no-server-id `spf=pass`, a forged server name, and any matching `Received` host never yield a verified or green authentication state.
- Multiple authentication-result headers are represented without silently selecting only the first one.
- Tests cover pass/fail/softfail results, duplicate and contradictory headers, comments containing status-like text, malformed tokens, and an absent header.
- User-facing text distinguishes “header reports” from “Hexscope verified”; localization coverage includes both languages.

## Out of scope

Trust-anchor configuration, network checks, and authentication of the original mailbox transport.
