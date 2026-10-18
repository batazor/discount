# Design

## 1. One file, two loaders
The catalogue is written once, in the `CatalogFixture` encoding the in-memory store already
reads, and extended with the application state the store already keeps (metadata,
allowlists, grant rules). The seed script reads the same file and PUTs it through the API,
so the in-memory server and the Postgres stack show the same campaigns. Alternative
rejected: a second format for the seed, which would drift from the fixture.

## 2. Dates relative to an `as_of` day
Campaign status is a function of the clock. A static file is right on one day only, so it
states that day (`as_of`) and the seed shifts every window by `now - as_of`, with `now` at
midnight UTC so a rerun on the same day repeats the same bodies and reservations stay
idempotent. The loader ignores `as_of`: without a database the console shows the file as
of its day, which is enough to see every status once.

## 3. Order history outside the file
Redemptions, grants and saved codes are not catalogue and have no fixture encoding; they
are produced by the script against the API, with fixed order ids (`DEMO-1001` ..
`DEMO-1015`) so Find and Explain have stable targets and reruns are no-ops.

## 4. Exhausted twice
`CARS-SPENT` ships with a zero budget (exhausted by budget, visible without the script);
`FLASH-48H` has `MaxUsesTotal 3` and the script confirms three flash orders (exhausted by
uses, visible only after the script), so both branches of the console's classification
are exercised.
