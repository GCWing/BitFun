use bitfun_product_domains::canvas::types::{
    CanvasDiagnostic, CanvasDiagnosticCategory, CanvasDiagnosticSeverity,
};

#[cfg(feature = "canvas-runtime")]
use oxc::ast::ast::{
    BindingPattern, CallExpression, Expression, JSXAttributeItem, JSXAttributeName, JSXElementName,
    JSXOpeningElement, Program, StaticMemberExpression, VariableDeclarator,
};
#[cfg(feature = "canvas-runtime")]
use oxc::ast_visit::{
    walk::{walk_jsx_opening_element, walk_static_member_expression, walk_variable_declarator},
    Visit,
};
#[cfg(feature = "canvas-runtime")]
use std::collections::BTreeSet;

#[cfg(feature = "canvas-runtime")]
use super::analysis::{CanvasSdkImportBindings, CanvasSdkImportSource};
use super::line_column;

#[cfg(feature = "canvas-runtime")]
pub(super) fn validate_canvas_sdk_contracts(
    source: &str,
    program: &Program<'_>,
    import_bindings: &CanvasSdkImportBindings,
) -> Vec<CanvasDiagnostic> {
    let mut visitor = CanvasSdkContractVisitor {
        source,
        import_bindings,
        diagnostics: Vec::new(),
        host_theme_locals: BTreeSet::new(),
    };
    visitor.visit_program(program);
    visitor.diagnostics
}

#[cfg(feature = "canvas-runtime")]
struct CanvasSdkContractVisitor<'a> {
    source: &'a str,
    import_bindings: &'a CanvasSdkImportBindings,
    diagnostics: Vec<CanvasDiagnostic>,
    host_theme_locals: BTreeSet<String>,
}

#[cfg(feature = "canvas-runtime")]
impl<'a> Visit<'a> for CanvasSdkContractVisitor<'_> {
    fn visit_jsx_opening_element(&mut self, element: &JSXOpeningElement<'a>) {
        self.validate_opening_element(element);
        walk_jsx_opening_element(self, element);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        self.collect_host_theme_local(declarator);
        walk_variable_declarator(self, declarator);
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        self.validate_host_theme_member(expression);
        walk_static_member_expression(self, expression);
    }
}

#[cfg(feature = "canvas-runtime")]
impl CanvasSdkContractVisitor<'_> {
    fn validate_opening_element(&mut self, element: &JSXOpeningElement<'_>) {
        let Some(component) = self.jsx_component_name(&element.name) else {
            return;
        };
        let Some(allowed_props) = sdk_component_allowed_props(component.as_str()) else {
            return;
        };

        for item in &element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            let Some(prop) = jsx_attribute_name(&attribute.name) else {
                continue;
            };
            if prop == "key"
                || common_canvas_style_prop(prop.as_str())
                || allowed_props.iter().any(|allowed| *allowed == prop)
            {
                continue;
            }

            let (line, column) = line_column(self.source, attribute.span.start as usize);
            self.diagnostics.push(CanvasDiagnostic {
                severity: CanvasDiagnosticSeverity::Error,
                category: CanvasDiagnosticCategory::TypeScript,
                message: format!(
                    "`{}` is not a valid prop for `{}` in bitfun/canvas",
                    prop, component
                ),
                code: Some("canvas.sdk.invalid_prop".to_string()),
                line: Some(line),
                column: Some(column),
                suggested_fix: Some(
                    sdk_invalid_prop_fix(component.as_str(), prop.as_str()).to_string(),
                ),
            });
        }
    }

    fn jsx_component_name(&self, name: &JSXElementName<'_>) -> Option<String> {
        match name {
            JSXElementName::IdentifierReference(identifier) => self
                .import_bindings
                .canonical_component_for_local(identifier.name.as_str())
                .map(str::to_string)
                .or_else(|| Some(identifier.name.to_string())),
            JSXElementName::MemberExpression(member) => {
                self.import_bindings.canonical_component_for_member(member)
            }
            _ => None,
        }
    }

    fn collect_host_theme_local(&mut self, declarator: &VariableDeclarator<'_>) {
        let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
            return;
        };
        let Some(Expression::CallExpression(call)) = declarator.init.as_ref() else {
            return;
        };
        if self.is_use_host_theme_call(call) {
            self.host_theme_locals.insert(identifier.name.to_string());
        }
    }

    fn is_use_host_theme_call(&self, call: &CallExpression<'_>) -> bool {
        match &call.callee {
            Expression::Identifier(identifier) => {
                self.import_bindings
                    .canonical_for_local(identifier.name.as_str())
                    .unwrap_or(identifier.name.as_str())
                    == "useHostTheme"
            }
            Expression::StaticMemberExpression(member) => {
                let Expression::Identifier(namespace) = &member.object else {
                    return false;
                };
                member.property.name.as_str() == "useHostTheme"
                    && self.import_bindings.namespaces.iter().any(|binding| {
                        binding.source == CanvasSdkImportSource::Canvas
                            && binding.local == namespace.name.as_str()
                    })
            }
            _ => false,
        }
    }

    fn validate_host_theme_member(&mut self, expression: &StaticMemberExpression<'_>) {
        let token = expression.property.name.as_str();
        let Expression::StaticMemberExpression(group_expression) = &expression.object else {
            return;
        };
        let group = group_expression.property.name.as_str();
        let Expression::Identifier(root) = &group_expression.object else {
            return;
        };
        if !self.host_theme_locals.contains(root.name.as_str()) {
            return;
        }
        if canvas_theme_token_group_allows(group, token) {
            return;
        }

        let (line, column) = line_column(self.source, expression.property.span.start as usize);
        self.diagnostics.push(CanvasDiagnostic {
            severity: CanvasDiagnosticSeverity::Error,
            category: CanvasDiagnosticCategory::TypeScript,
            message: format!(
                "`{}.{}.{}` is not a valid Canvas host theme token",
                root.name, group, token
            ),
            code: Some("canvas.sdk.invalid_theme_token".to_string()),
            line: Some(line),
            column: Some(column),
            suggested_fix: Some(canvas_theme_token_fix(group, token).to_string()),
        });
    }
}

#[cfg(feature = "canvas-runtime")]
fn jsx_attribute_name(name: &JSXAttributeName<'_>) -> Option<String> {
    match name {
        JSXAttributeName::Identifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

#[cfg(feature = "canvas-runtime")]
fn sdk_component_allowed_props(component: &str) -> Option<&'static [&'static str]> {
    match component {
        "Stack" => Some(&["children", "gap", "style"]),
        "Row" => Some(&["children", "gap", "align", "justify", "wrap", "style"]),
        "Grid" => Some(&["children", "columns", "gap", "align", "style"]),
        "Box" => Some(&[
            "children",
            "padding",
            "background",
            "border",
            "radius",
            "style",
        ]),
        "Divider" => Some(&["style"]),
        "H1" | "H2" | "H3" | "Code" | "Link" => Some(&["children", "href", "style"]),
        "Text" => Some(&[
            "children", "tone", "size", "as", "weight", "italic", "truncate", "style", "color",
        ]),
        "Card" => Some(&[
            "children",
            "variant",
            "size",
            "stickyHeader",
            "collapsible",
            "defaultOpen",
            "open",
            "onOpenChange",
            "style",
        ]),
        "CardHeader" => Some(&["children", "trailing", "style"]),
        "CardBody" => Some(&["children", "style"]),
        "Alert" => Some(&[
            "children",
            "type",
            "tone",
            "title",
            "message",
            "description",
            "showIcon",
            "style",
        ]),
        "Callout" => Some(&["children", "tone", "title", "icon", "style"]),
        "CollapsibleSection" => Some(&[
            "children",
            "title",
            "leading",
            "count",
            "trailing",
            "defaultOpen",
            "style",
        ]),
        "Empty" => Some(&["description", "image", "imageSize", "children", "style"]),
        "Tabs" => Some(&[
            "items",
            "activeKey",
            "defaultActiveKey",
            "onChange",
            "children",
            "type",
            "size",
            "stretch",
            "style",
        ]),
        "Pill" => Some(&[
            "children",
            "active",
            "tone",
            "size",
            "leadingContent",
            "keyboardHint",
            "disabled",
            "title",
            "style",
            "onClick",
        ]),
        "Stat" => Some(&["value", "label", "tone", "style"]),
        "Table" => Some(&[
            "headers",
            "rows",
            "columnAlign",
            "rowTone",
            "framed",
            "striped",
            "stickyHeader",
            "style",
            "emptyMessage",
        ]),
        "KeyValueList" => Some(&["items", "columns", "compact", "emptyMessage", "style"]),
        "Timeline" => Some(&["items", "emptyMessage", "style"]),
        "FileTree" => Some(&["items", "defaultExpanded", "emptyMessage", "style"]),
        "ProgressBar" => Some(&["value", "max", "label", "tone", "showValue", "style"]),
        "Swatch" => Some(&["color", "style", "title", "className"]),
        "UsageBar" => Some(&[
            "segments",
            "total",
            "topLeftLabel",
            "topRightLabel",
            "style",
        ]),
        "TodoList" => Some(&["todos", "dimmedTodoIds", "onTodoClick", "style"]),
        "TodoListCard" => Some(&[
            "todos",
            "dimmedTodoIds",
            "defaultExpanded",
            "onTodoClick",
            "style",
        ]),
        "DependencyGraph" => Some(&[
            "nodes",
            "edges",
            "direction",
            "nodeWidth",
            "nodeHeight",
            "rankGap",
            "nodeGap",
            "padding",
            "title",
            "height",
            "style",
        ]),
        "FlowDiagram" => Some(&[
            "steps",
            "nodes",
            "edges",
            "direction",
            "nodeWidth",
            "nodeHeight",
            "rankGap",
            "nodeGap",
            "padding",
            "title",
            "height",
            "style",
        ]),
        "Button" => Some(&[
            "children", "variant", "disabled", "type", "style", "onClick",
        ]),
        "Toggle" => Some(&["checked", "onChange", "disabled", "size", "style"]),
        "Checkbox" => Some(&["checked", "onChange", "disabled", "label", "style"]),
        "Select" => Some(&[
            "value",
            "onChange",
            "options",
            "placeholder",
            "disabled",
            "style",
        ]),
        "TextInput" => Some(&[
            "value",
            "onChange",
            "placeholder",
            "disabled",
            "type",
            "style",
        ]),
        "Input" => Some(&[
            "value",
            "onChange",
            "placeholder",
            "disabled",
            "type",
            "label",
            "hint",
            "prefix",
            "suffix",
            "error",
            "errorMessage",
            "size",
            "style",
        ]),
        "TextArea" => Some(&[
            "value",
            "onChange",
            "placeholder",
            "disabled",
            "rows",
            "style",
        ]),
        "IconButton" => Some(&[
            "children", "onClick", "disabled", "title", "variant", "size", "style",
        ]),
        "DiffStats" => Some(&["additions", "deletions", "style"]),
        "DiffView" => Some(&[
            "lines",
            "path",
            "language",
            "showLineNumbers",
            "coloredLineNumbers",
            "showAccentStrip",
            "style",
        ]),
        "BarChart" | "LineChart" | "PieChart" => Some(&[
            "data",
            "categories",
            "series",
            "height",
            "style",
            "stacked",
            "horizontal",
            "normalized",
            "valueSuffix",
            "valuePrefix",
            "showValues",
            "beginAtZero",
            "yMin",
            "yMax",
            "referenceLines",
            "fill",
            "showHoverGuide",
            "size",
            "donut",
        ]),
        "Spacer" => Some(&[]),
        _ => None,
    }
}

#[cfg(feature = "canvas-runtime")]
fn common_canvas_style_prop(prop: &str) -> bool {
    matches!(
        prop,
        "padding"
            | "margin"
            | "background"
            | "border"
            | "borderTop"
            | "borderRight"
            | "borderBottom"
            | "borderLeft"
            | "borderRadius"
            | "width"
            | "height"
            | "flex"
            | "display"
            | "opacity"
            | "minWidth"
            | "maxWidth"
            | "minHeight"
            | "maxHeight"
    )
}

#[cfg(feature = "canvas-runtime")]
fn canvas_theme_token_group_allows(group: &str, token: &str) -> bool {
    match group {
        "bg" => matches!(token, "editor" | "chrome" | "elevated" | "canvas"),
        "text" => matches!(
            token,
            "primary" | "secondary" | "tertiary" | "quaternary" | "link" | "onAccent"
        ),
        "fill" => matches!(token, "primary" | "secondary" | "tertiary" | "quaternary"),
        "stroke" => matches!(token, "primary" | "secondary" | "tertiary" | "focused"),
        "accent" => matches!(
            token,
            "primary" | "control" | "controlHover" | "success" | "warning" | "danger" | "info"
        ),
        "diff" => matches!(
            token,
            "insertedLine" | "removedLine" | "stripAdded" | "stripRemoved"
        ),
        "category" => matches!(
            token,
            "blue" | "cyan" | "gray" | "green" | "orange" | "pink" | "purple" | "yellow"
        ),
        "status" => matches!(token, "success" | "warning" | "danger" | "info"),
        "tokens" | "palette" => true,
        _ => false,
    }
}

#[cfg(feature = "canvas-runtime")]
fn canvas_theme_token_fix(group: &str, token: &str) -> &'static str {
    match (group, token) {
        ("surface", "primary") => {
            "Use theme.bg.editor for the main background or theme.fill.primary for a filled surface."
        }
        ("surface", "secondary") => {
            "Use theme.bg.elevated for raised panels or theme.fill.secondary for tinted fills."
        }
        ("surface", _) => {
            "Canvas theme has no `surface` group. Use theme.bg.* for backgrounds or theme.fill.* for tinted fills."
        }
        ("interactive", "accent") => "Use theme.accent.primary.",
        ("interactive", _) => "Canvas theme has no `interactive` group. Use theme.accent.* tokens.",
        (_, _) => {
            "Use one of the declared useHostTheme() token paths: bg, text, fill, stroke, accent, diff, category, or status."
        }
    }
}

#[cfg(feature = "canvas-runtime")]
fn sdk_invalid_prop_fix(component: &str, prop: &str) -> &'static str {
    match (component, prop) {
        ("Pill", "label") => "Put the label inside the Pill children, e.g. <Pill>Label</Pill>.",
        ("Table", "columns") => "Use <Table headers={...} rows={...} />; the Canvas SDK does not support a columns prop.",
        _ => "Use props declared by the bitfun/canvas SDK for this component.",
    }
}
