//! Compatibility re-exports for round-boundary injection state.

pub use openbitfun_agent_runtime::scheduler::{
    DialogRoundInjectionInterrupt, NoopDialogRoundInjectionSource, SessionRoundInjectionBuffer,
};
pub use openbitfun_runtime_ports::{
    DialogRoundInjectionSource, RoundInjection, RoundInjectionKind, RoundInjectionTarget,
};
