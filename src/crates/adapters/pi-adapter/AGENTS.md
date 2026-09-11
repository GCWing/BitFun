# PI adapter

Own PI settings, package entry selection, and static extension event discovery.
Read TypeScript with the parser; never import an extension, install a package,
execute a command, or infer native trust from its location. Dynamic registrations
remain opaque; literal events remain native-only with unknown activation.
Keep registration in Product Assembly and use shared bounded filesystem helpers.

Verification: `cargo test --locked -p openbitfun-pi-adapter --lib`
