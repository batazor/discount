# Design

1. **Symmetry with the customer side.** `CustomerAttributeRange` exists; `AttributeRange`
   is the same function over `cart.attributes`. Half-open `AttributeDateWindow` stays for
   time; closed ranges for amounts and counts.
2. **No new money type on attributes.** The second amount travels as a decimal string of
   minor units in the cart currency; it is the caller's number, not something the engine
   converts or rounds. If a second currency-aware amount is ever needed it is a new field
   on `CartSnapshot`, which is a separate decision.
