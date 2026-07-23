mod edit;
mod event;
mod focus;
mod mapping;

pub(super) fn script() -> String {
    format!(
        "{mapping}{focus}{event}{edit}",
        mapping = mapping::script(),
        focus = focus::script(),
        event = event::script(),
        edit = edit::script()
    )
}
