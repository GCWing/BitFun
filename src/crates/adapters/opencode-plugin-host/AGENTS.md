# OpenCode Plugin Host Adapter

This private adapter owns the framed loopback JSON-RPC transport and maps the
OpenCode extension-host process onto OpenBitFun lifecycle operations. It may use the
managed process-tree primitive from `services-core`, but it must not own product
configuration selection, workspace/session policy, or plugin trust decisions.

It also owns OpenCode Client route/method/query matching, wire DTOs,
serialization, and protocol error mapping. Product Assembly may register the
adapter, keep opaque logical instance bindings, and call existing OpenBitFun owner
ports to satisfy a matched route; it must not duplicate these wire semantics or
implement physical process-tree supervision.

The backend always binds the loopback listener before spawning the child. The
first accepted frame must be an authenticated `backend.handshake` request.
