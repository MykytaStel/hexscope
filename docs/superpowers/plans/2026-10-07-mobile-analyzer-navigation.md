# Mobile Analyzer Navigation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution in the current checkout.

**Goal:** Make all five Analyzer views and secondary actions reachable and readable on phones.
**Architecture:** Keep the existing workspace view state and rendering. Reuse the current navigation buttons in a responsive bottom bar, group secondary actions in a compact disclosure menu, and preserve the two-button switcher for wide screens only.
**Tech Stack:** TypeScript, HTML/CSS, Vite, Playwright.
**Spec:** `/Users/mykyta/Downloads/hexscope-visual-direction/spec/page-map.md` (mobile Analyzer: one work surface, bottom/tab navigation or compact mode switcher, inspector drawer).

## Global Constraints

Continue on `feat/analyzer-layout-readability` and preserve its pending desktop/tablet layout regression and CSS changes. Do not change parser semantics, file handling, or byte-selection behavior. Keep the top file header, reserve space for the bottom navigation including safe areas, support Ukrainian and English labels, and prevent horizontal page overflow. User requested implementation and local testing; do not pause for approval or deploy this slice.

## Review Focus

1. All five views remain reachable with correct current-state accessibility and existing metadata/content/byte navigation.
2. Compression and Compare remain available through a compact “More” disclosure; menu actions close the disclosure before acting.
3. The bottom navigation fits narrow phone widths, respects safe areas, and does not cover the active work surface or create page overflow.
4. Desktop navigation and the previous 901/1024/1280 Analyzer sizing regression remain unchanged.

### Task 1: Mobile navigation regression

**Files:** `apps/web/e2e/site.e2e.ts`.

- [x] Add a phone regression for the five workspace views, the More actions, and viewport bounds.
- [x] Run only the new phone test and confirm it fails because mobile navigation is currently hidden.

### Task 2: Responsive workspace navigation

**Files:** `apps/web/index.html`, `apps/web/src/main.ts`, `apps/web/src/app-nav-icons.ts`, `apps/web/src/styles/app-shell.css`, `apps/web/src/styles/narrow-layout.css`, `apps/web/src/i18n.ts`, `apps/web/e2e/site.e2e.ts`.

- [x] Expose the five existing view controls as a fixed mobile bottom bar and group secondary actions in More.
- [x] Wire icon installation and disclosure behavior for mobile without changing desktop navigation semantics; keep the bottom bar above the first-run tour so choosing a view dismisses it as before.
- [x] Hide the old two-item mobile switcher and reserve safe-area-aware space beneath the workspace.
- [x] Update the existing compact-switcher regression to cover its replacement while retaining summary/bytes behavior.
- [x] Run focused phone and desktop navigation checks, then build, unit tests, full E2E and visual checks at 390px/320px.

### Task 3: Review and handoff

- [x] Review the final diff against `origin/main`, run RepoPilot review, and reconcile any scoped findings.
- [x] Keep the result local and unmerged; report the preview URL, screenshots, verification evidence, and remaining limitations.
