# Design

One function, `tenantLabel(user, authEnabled)`, gives the label and whether it is the
`default` fallback; the layout renders a chip from it and pages reuse it in descriptions.
No request changes: the service decides the tenant from the token.
