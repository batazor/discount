# Spec Delta

## MODIFIED Requirements

### Requirement: Provisioned alert rules
The local stack SHALL provision Grafana-managed alert rules from
`ops/grafana/alerting/*.yaml` into folder `Discount`, group `discount` evaluated every
minute. Each rule SHALL carry labels `service=discount` and `severity` and link to the
dashboard panel it is derived from. No contact point SHALL be provisioned.

#### Scenario: Rules present after start
- **WHEN** Grafana is up
- **THEN** `GET /api/v1/provisioning/alert-rules` lists the rules with uids prefixed
  `discount-`, all in folder `Discount`, including `discount-exchange-rates-stale`

#### Scenario: Rule set
- **WHEN** the rules are listed
- **THEN** they are: HTTP 5xx ratio per route above 5 % for 5 minutes (`warning`); p95
  latency of `POST /v1/redeem` above 500 ms for 5 minutes (`warning`); pricing error ratio
  above 20 % for 10 minutes (`warning`); a promotion with `discount.budget.remaining` at or
  below zero for 1 minute (`critical`); redemption storage errors (`critical`); exchange
  rates stale, `discount.exchange_rates.stale == 1` for 5 minutes (`warning`)

#### Scenario: Exhausted budget fires
- **WHEN** a promotion's remaining budget is set to `0` and the gauge has been exported
- **THEN** within three minutes the rule `Promotion budget exhausted` is `Alerting` with
  label `promotion_id` naming that promotion, and returns to `Normal` after the budget is
  restored

#### Scenario: Stale rates fire
- **WHEN** a source is set and its provider stays unreachable for three intervals
- **THEN** the rule `Exchange rates stale` is `Alerting` and returns to `Normal` after a
  successful fetch
