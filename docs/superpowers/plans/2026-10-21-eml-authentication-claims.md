# EML authentication claims implementation plan

> For agentic workers: use the `superpowers:executing-plans` skill to implement this plan task by task. Keep the approved design spec open while working.

**Goal:** Present Authentication-Results only as claims recorded in the message, never as checks Hexscope independently verified.

**Architecture:** The Rust EML parser will emit one `authclaim` fact per recognized status in each outer-message Authentication-Results header and preserve each source range. Web presentation will use neutral wording and tone, with no trust inference from Received headers. CLI inventory, parser explanation, share categories, knowledge/headline behavior, and both locales will use the new kind consistently.

**Tech Stack:** Rust workspace (`hexscope-core`, `hexscope-cli`), TypeScript/Vitest, Web UI, Ukrainian and English copy.

**Spec:** `docs/superpowers/specs/2026-10-21-eml-authentication-claims-design.md`

## Global constraints

- Never label a claimed pass or fail as verified or use green/red trust styling for it.
- Do not add live DNS, SPF, DKIM, or DMARC checks.
- Keep the original header and byte range available as parser evidence.
- Preserve unrelated verification findings and email-route behavior.

## Review focus

- Do repeated/contradictory headers survive without first-header selection or status-token substring matches?
- Is every user-facing statement explicit that the result is reported by the header and its author is unverified?
- Are CLI kind inventories, category ranking, and English/Ukrainian text consistent?

---

### Task 1: Parse all Authentication-Results as untrusted claims

**Files:** `crates/hexscope-core/src/eml/mod.rs`, `crates/hexscope-core/src/eml/tests.rs`

**Interfaces:** Produces one source-linked `authclaim` fact per recognized status in each header; Tasks 2 and 3 consume that kind and its header node.

- [x] Add failing parser cases for missing authserv-id, matching and forged Received hosts, pass/fail/softfail, duplicate and contradictory headers, comment text containing `spf=pass`, malformed tokens, and no header.
- [x] Run `cargo test -p hexscope-core eml::tests`; **Expected:** the new cases fail because the parser currently grants trust, ignores duplicate headers, or misses malformed/comment cases.
- [x] Replace the first-header and substring logic with recognition of supported method/status tokens across every outer-message header. Emit `authclaim` facts with the exact source node/range and neutral claim wording; do not infer trust from Received.
- [x] Keep unrecognized and malformed tokens out of authentication findings while preserving the original header bytes in the parsed tree.
- [x] Run `cargo test -p hexscope-core eml::tests` and inspect the resulting facts and byte ranges. **Expected:** all EML parser cases pass and every recognized claim points to its exact header evidence.

### Task 2: Make the email path describe claims neutrally

**Files:** `apps/web/src/emailpath.ts`, `apps/web/src/logic.test.ts`, `apps/web/src/headline.ts`, `apps/web/src/knowledge.ts`, `apps/web/src/advice.ts`, `apps/web/src/i18n-uk.ts`, English source strings in the feature code, and relevant Web rendering/styles.

**Interfaces:** Consumes Task 1's `authclaim` facts; produces neutral email-route/headline behavior and source strings used by Task 3's localization/share updates.

- [x] Add failing route assertions to the existing sample-backed email path tests for claimed pass and fail; assert neutral tone and wording that identifies the header as the source and Hexscope as unable to verify its author.
- [x] Run `cd apps/web && npm test -- src/logic.test.ts -t "an email's way"`; **Expected:** the two new assertions fail because current rendering expects `auth`/`authfail` and gives those results trust tone.
- [x] Point the email route at `authclaim`, remove positive/negative trust treatment for these claims, and keep evidence selection linked to the header node.
- [x] Update finding strength/headline routing so a claimed pass is not a positive trust signal and a claimed fail is not rendered as an independently established fact.
- [x] Add/update English and Ukrainian strings; run the focused checks `cd apps/web && npm test -- src/logic.test.ts -t "an email's way"` and `cd apps/web && npm test -- src/i18n.test.ts`. **Expected:** neutral source wording is available and both-language translation checks pass.

### Task 3: Align explanations, sharing, and CLI documentation

**Files:** `crates/hexscope-core/src/eml/docs.rs`, `crates/hexscope-core/src/eml/tests.rs`, `docs/cli.md`, `apps/web/src/share.ts`, `apps/web/src/logic.test.ts`, `apps/web/src/i18n-uk.ts`, `apps/web/src/analysis-i18n.ts`, `apps/web/src/guide-i18n.ts`, `apps/web/is-this-email-real.html`, and `apps/web/e2e/site.e2e.ts`.

**Interfaces:** Consumes Task 1's fact kind and Task 2's neutral copy; produces consistent parser explanation, share categories, and CLI inventory.

- [x] Add expectations for the EML format explanation, share category, and CLI kind inventory using `authclaim` and neutral language.
- [x] Run the focused Rust and Web tests to confirm these expectations fail before updating the consumers. **Observed RED:** the EML explanation described a receiving server's finding, the share category was missing, and the CLI contract reported that `docs/cli.md` did not list `authclaim`.
- [x] Update those descriptions and inventories together; remove obsolete `auth`/`authfail` wording only where it refers to Authentication-Results. Also align the guide, locale strings, and rendered metadata with the unverified-claim boundary.
- [x] Run `cargo test -p hexscope-core eml::tests`, `cargo test -p hexscope-cli`, `cd apps/web && npm test -- src/logic.test.ts src/i18n.test.ts`, and the targeted guide E2E. **Observed:** core 12 passed; CLI unit/integration/contract suites passed; focused Web suite 218 passed; production build passed; targeted guide E2E passed on computer and phone. Existing Vite config-loader warnings remain.
- [x] Review the final diff for stale authentication wording and confirm no unrelated verification state changed. Old `auth`/`authfail` references remain only in assertions that prohibit the parser from emitting them; no user-facing source retains trust wording.

**Acceptance:** Every Authentication-Results value remains visible as a source-linked claim, including duplicates; no value is displayed as trusted verification; parser, Web copy, share categories, locale strings, and CLI docs agree.

## Final review follow-up — 2026-10-21

- The independent review found that a present `Authentication-Results` header with no recognized claims was described as absent. The route now distinguishes header presence from recognized claims and has a focused regression test. The English and Ukrainian copy are covered in the full Web unit suite (354 tests).
- The Ukrainian guide E2E classifier now permits the literal protocol header name and the quoted `pass` status as technical tokens; it no longer reports translated copy as untranslated prose. The guide-completeness E2E passed on both computer and phone profiles.
