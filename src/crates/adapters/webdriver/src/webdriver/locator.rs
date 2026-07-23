use crate::server::response::WebDriverErrorResponse;

#[derive(Debug, Clone, Copy)]
pub enum LocatorStrategy {
    CssSelector,
    LinkText,
    PartialLinkText,
    TagName,
    XPath,
    Id,
    Name,
    ClassName,
}

impl LocatorStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CssSelector => "css selector",
            Self::LinkText => "link text",
            Self::PartialLinkText => "partial link text",
            Self::TagName => "tag name",
            Self::XPath => "xpath",
            Self::Id => "id",
            Self::Name => "name",
            Self::ClassName => "class name",
        }
    }
}

impl TryFrom<&str> for LocatorStrategy {
    type Error = WebDriverErrorResponse;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "css selector" => Ok(Self::CssSelector),
            "link text" => Ok(Self::LinkText),
            "partial link text" => Ok(Self::PartialLinkText),
            "tag name" => Ok(Self::TagName),
            "xpath" => Ok(Self::XPath),
            "id" => Ok(Self::Id),
            "name" => Ok(Self::Name),
            "class name" => Ok(Self::ClassName),
            other => Err(WebDriverErrorResponse::invalid_selector(format!(
                "Unsupported locator strategy: {other}"
            ))),
        }
    }
}
