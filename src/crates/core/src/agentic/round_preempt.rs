//! Compatibility re-exports for round-boundary scheduler owner state.

pub use bitfun_agent_runtime::scheduler::{
    DialogRoundInjectionInterrupt, NoopDialogRoundInjectionSource, NoopDialogRoundPreemptSource,
    SessionRoundInjectionBuffer, SessionRoundYieldFlags,
};
pub use bitfun_runtime_ports::{
    DialogRoundInjectionSource, DialogRoundPreemptSource, RoundInjection, RoundInjectionKind,
    RoundInjectionTarget,
};
