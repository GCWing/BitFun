# Offline migration adapters

Own concrete BitFun readers and converters against shared storage contracts.
No Core, product assembly, application host, or live Agent/session lifecycle.
Agent Runtime may be consumed only with definition-contracts for static Agent
and Skill parsing. Stored connection imports never connect to remote hosts.
Keep original source-read-only behavior, atomic domain writes, idempotent journal
recovery, reference repair and target conflict rules. Share storage formats with
config-contracts and services-core; do not copy private application schemas.

Focused verification:

```bash
cargo test -p openbitfun-legacy-migration-adapters --lib
```
