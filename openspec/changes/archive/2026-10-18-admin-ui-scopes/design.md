# Design

`model/access.ts` turns the claims into `{ read, write, pricing, customers, admin }`
(everything true without sign-in; `discount:admin` implies all). The middleware puts it on
`locals.access`; the layout, the overview and the pages read it. Forms are gated with a
disabled `<fieldset>` plus a note so that read-only pages keep their layout; controls that
start an action are not rendered without the scope. The service remains the authority.
