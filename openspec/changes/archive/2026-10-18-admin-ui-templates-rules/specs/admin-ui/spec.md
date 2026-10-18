# Spec Delta

## ADDED Requirements

### Requirement: Templates gallery and duplication
`/campaigns/templates` SHALL list every template with its sentence and link into the
wizard, and SHALL list existing campaigns to start from. `/campaigns/new?from=<id>` SHALL
prefill the wizard from that campaign's action, cap, conditions, budget currency, title,
source and labels, with an empty id, and SHALL create nothing until submit.

#### Scenario: Duplicate
- **WHEN** `SUMMER` gives `10 %` to first orders with a code and the wizard is opened with
  `from=SUMMER`
- **THEN** the percentage is `10`, "first order only" and "needs a code" are on, the id
  is empty and the title reads `SUMMER` copy

### Requirement: Rule builder on the rules tab
The campaign's rules tab SHALL show the builder's conditions prefilled from the stored
rules; rules the builder cannot express SHALL appear as advanced rules in the raw editor
beside it; saving SHALL send the builder's rules followed by the advanced ones.

#### Scenario: Round trip
- **WHEN** a campaign has `MaxCompletedOrders 0`, `RequiresCoupon` and a `TagsMatch` with
  `none = ["#partner"]`
- **THEN** the tab shows "first order only" and "needs a code" on, the tag rule as an
  advanced rule, and saving unchanged stores the same three rules

### Requirement: More conditions
The builder SHALL also offer cart attribute (`AttributeIn`), cart date attribute
(`AttributeDateWindow`), customer attribute range (`CustomerAttributeRange`) and maximum
order total (`OrderTotalRange` with `max`); minimum and maximum order total SHALL become one
`OrderTotalRange` when both are on.

#### Scenario: Both bounds
- **WHEN** minimum `50.00 EUR` and maximum `500.00 EUR` are on
- **THEN** one rule `OrderTotalRange { min: 5000 EUR, max: 50000 EUR }` is sent

### Requirement: Builder sanity checks
Before sending, the builder SHALL refuse an end before its start and a minimum above the
maximum as field errors, and SHALL warn on the campaign page when "needs a code" is on
and no code exists.

#### Scenario: End before start
- **WHEN** dates are on with an end before the start
- **THEN** the end field shows an error and nothing is sent

### Requirement: Who gets it
The rules tab and the wizard SHALL show the conditions as one sentence, updated as they
change.

#### Scenario: Sentence of conditions
- **WHEN** "first order only" and "needs a code" are on
- **THEN** the sentence reads `for first orders only, with a code`
