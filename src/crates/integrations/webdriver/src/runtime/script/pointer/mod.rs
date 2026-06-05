mod key_source;
mod mouse;
mod perform;
mod pointer_source;
mod release;
mod wheel;
mod wheel_source;

pub(super) fn script() -> String {
    format!(
        "{mouse}{wheel}{pointer_source}{wheel_source}{key_source}{perform}{release}",
        mouse = mouse::script(),
        wheel = wheel::script(),
        pointer_source = pointer_source::script(),
        wheel_source = wheel_source::script(),
        key_source = key_source::script(),
        perform = perform::script(),
        release = release::script()
    )
}
