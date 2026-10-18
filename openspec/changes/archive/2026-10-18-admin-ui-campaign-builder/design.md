# Design

## Context

The one-form creation and the `createCampaign` action exist; the wizard is a different
presentation of the same fields plus a builder layer that produces rule fields. The
engine prices only the catalogue it holds, so "try it" is a dry run after creation.

## Goals / Non-Goals

**Goals:** guided creation that degrades to the full form; one summary function shared
by server and browser; templates as URLs; builder fields mapped to `Rule` variants in
the view model with tests.

**Non-Goals:** drafts on the server; previewing an unsaved campaign; replacing the raw
rule editor (it stays for everything the builder does not express).

## Decisions

1. **One form, stepped by script.** Sections carry `data-step`; `client/wizard.ts` shows
   one at a time with next/back and a step bar; values live in the same form, so nothing
   is lost and no-script renders every section. The review step is the summary plus the
   submit.
2. **Summary is a pure function over form fields** (`model/summary.ts`,
   `summarize(fields): string`) imported by the page for the review step and by the
   browser script, which calls it on every input and writes the sentence into the
   summary card. Dates are formatted by the browser in local time and by the server in
   UTC, as elsewhere.
3. **Builder fields → rules.** `model/builder.ts` defines the builder's field names
   (`b.first_order`, `b.once`, `b.dates`, `b.starts_at`, `b.ends_at`, `b.min_total.*`,
   `b.needs_code`, `b.allowlist`, `b.tags_any`, `b.attr.key`, `b.attr.values`,
   `b.max_total`) and `rulesFromBuilder(fields)` producing `rules.N.*` fields, which
   `campaignDraftFromForm` applies when `builder=1`; the promotion mapping stays the only
   place that knows the contract shape. Order: dates, order total, first order, once,
   tags, attribute, total uses, needs code, allowlist.
4. **Templates are query strings.** `model/templates.ts` maps a template id to field
   values; `/campaigns/new?template=welcome` applies them to the initial state. Linking
   and no-script both work; a template never submits.
5. **Try-it is a focused view of the explain result.** The overview tab's card posts a
   sample cart through the existing `evaluate` action and reads the campaign's entry from
   `applied` or `skipped`; for `NotEligible` the rule index is mapped to its plain name.
   The full result panel opens below on request.
6. **Templates that need other resources** (win-back needs a grant rule) prefill the
   campaign and link to the grant rule form with the campaign id, rather than creating
   two things at once.

## Risks / Trade-offs

- [Builder and raw editor diverge] → the raw editor renders whatever rules exist; the
  builder only creates. The rules tab shows the plain names next to the raw types.
- [Summary wording drifts from semantics] → every phrase comes from the glossary's terms
  and is tested against the scenario sentence.
