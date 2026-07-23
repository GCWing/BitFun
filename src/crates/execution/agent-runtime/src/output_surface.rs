use bitfun_runtime_ports::DialogTriggerSource;

pub const TOOL_CONTEXT_INLINE_MARKDOWN_IMAGE_DISPLAY_KEY: &str = "inline_markdown_image_display";

pub const fn supports_inline_markdown_images_for_source(source: DialogTriggerSource) -> bool {
    matches!(source, DialogTriggerSource::DesktopUi)
}

#[cfg(test)]
mod tests {
    use super::supports_inline_markdown_images_for_source;
    use bitfun_runtime_ports::DialogTriggerSource;

    #[test]
    fn inline_markdown_images_are_scoped_to_desktop_ui() {
        assert!(supports_inline_markdown_images_for_source(
            DialogTriggerSource::DesktopUi
        ));

        for source in [
            DialogTriggerSource::DesktopApi,
            DialogTriggerSource::AgentSession,
            DialogTriggerSource::ScheduledJob,
            DialogTriggerSource::RemoteRelay,
            DialogTriggerSource::Bot,
            DialogTriggerSource::Cli,
        ] {
            assert!(!supports_inline_markdown_images_for_source(source));
        }
    }
}
