# Token Engine

Private, dependency-free token resolver used by the design-system build and authoring tools.

The initial implementation intentionally supports the DTCG-shaped `$type`, `$value`, and `{path.to.token}` alias contract needed by OpenBitFun. Unsupported composite token types fail instead of being silently stringified.
