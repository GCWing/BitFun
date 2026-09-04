use openbitfun_product_domains::canvas::types::{
    CanvasDiagnostic, CanvasDiagnosticCategory, CanvasDiagnosticSeverity,
};
#[cfg(feature = "canvas-runtime")]
use openbitfun_product_domains::canvas::{
    canvas_appearance_token_group_allows as generated_appearance_token_group_allows,
    canvas_sdk_component_props as generated_component_props, CANVAS_SDK_COMMON_PROPS,
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
        host_appearance_locals: BTreeSet::new(),
    };
    visitor.visit_program(program);
    visitor.diagnostics
}

#[cfg(feature = "canvas-runtime")]
struct CanvasSdkContractVisitor<'a> {
    source: &'a str,
    import_bindings: &'a CanvasSdkImportBindings,
    diagnostics: Vec<CanvasDiagnostic>,
    host_appearance_locals: BTreeSet<String>,
}

#[cfg(feature = "canvas-runtime")]
impl<'a> Visit<'a> for CanvasSdkContractVisitor<'_> {
    fn visit_jsx_opening_element(&mut self, element: &JSXOpeningElement<'a>) {
        self.validate_opening_element(element);
        walk_jsx_opening_element(self, element);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        self.collect_host_appearance_local(declarator);
        walk_variable_declarator(self, declarator);
    }

    fn visit_static_member_expression(&mut self, expression: &StaticMemberExpression<'a>) {
        self.validate_host_appearance_member(expression);
        walk_static_member_expression(self, expression);
    }
}

#[cfg(feature = "canvas-runtime")]
impl CanvasSdkContractVisitor<'_> {
    fn validate_opening_element(&mut self, element: &JSXOpeningElement<'_>) {
        let Some(component) = self.jsx_component_name(&element.name) else {
            return;
        };
        let Some(allowed_props) = generated_component_props(component.as_str()) else {
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
                || common_canvas_prop(prop.as_str())
                || allowed_props.iter().any(|allowed| *allowed == prop)
            {
                continue;
            }

            let (line, column) = line_column(self.source, attribute.span.start as usize);
            self.diagnostics.push(CanvasDiagnostic {
                severity: CanvasDiagnosticSeverity::Error,
                category: CanvasDiagnosticCategory::TypeScript,
                message: format!(
                    "`{}` is not a valid prop for `{}` in openbitfun/canvas",
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

    fn collect_host_appearance_local(&mut self, declarator: &VariableDeclarator<'_>) {
        let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
            return;
        };
        let Some(Expression::CallExpression(call)) = declarator.init.as_ref() else {
            return;
        };
        if self.is_use_host_appearance_call(call) {
            self.host_appearance_locals
                .insert(identifier.name.to_string());
        }
    }

    fn is_use_host_appearance_call(&self, call: &CallExpression<'_>) -> bool {
        match &call.callee {
            Expression::Identifier(identifier) => {
                self.import_bindings
                    .canonical_for_local(identifier.name.as_str())
                    .unwrap_or(identifier.name.as_str())
                    == "useHostAppearance"
            }
            Expression::StaticMemberExpression(member) => {
                let Expression::Identifier(namespace) = &member.object else {
                    return false;
                };
                member.property.name.as_str() == "useHostAppearance"
                    && self.import_bindings.namespaces.iter().any(|binding| {
                        binding.source == CanvasSdkImportSource::Canvas
                            && binding.local == namespace.name.as_str()
                    })
            }
            _ => false,
        }
    }

    fn validate_host_appearance_member(&mut self, expression: &StaticMemberExpression<'_>) {
        let token = expression.property.name.as_str();
        let Expression::StaticMemberExpression(group_expression) = &expression.object else {
            return;
        };
        let group = group_expression.property.name.as_str();
        let Expression::Identifier(root) = &group_expression.object else {
            return;
        };
        if !self.host_appearance_locals.contains(root.name.as_str()) {
            return;
        }
        if generated_appearance_token_group_allows(group, token) {
            return;
        }

        let (line, column) = line_column(self.source, expression.property.span.start as usize);
        self.diagnostics.push(CanvasDiagnostic {
            severity: CanvasDiagnosticSeverity::Error,
            category: CanvasDiagnosticCategory::TypeScript,
            message: format!(
                "`{}.{}.{}` is not a valid Canvas host appearance token",
                root.name, group, token
            ),
            code: Some("canvas.sdk.invalid_appearance_token".to_string()),
            line: Some(line),
            column: Some(column),
            suggested_fix: Some(canvas_appearance_token_fix(group, token).to_string()),
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
fn common_canvas_prop(prop: &str) -> bool {
    CANVAS_SDK_COMMON_PROPS.contains(&prop)
        || prop.starts_with("aria-")
        || prop.starts_with("data-")
}

#[cfg(feature = "canvas-runtime")]
fn canvas_appearance_token_fix(group: &str, token: &str) -> &'static str {
    match (group, token) {
        ("surface", "primary") => {
            "Use appearance.bg.editor for the main background or appearance.fill.primary for a filled surface."
        }
        ("surface", "secondary") => {
            "Use appearance.bg.elevated for raised panels or appearance.fill.secondary for tinted fills."
        }
        ("surface", _) => {
            "Canvas appearance has no `surface` group. Use appearance.bg.* for backgrounds or appearance.fill.* for tinted fills."
        }
        ("interactive", "accent") => "Use appearance.accent.primary.",
        ("interactive", _) => "Canvas appearance has no `interactive` group. Use appearance.accent.* tokens.",
        (_, _) => {
            "Use one of the declared useHostAppearance() token paths: bg, text, fill, stroke, accent, diff, category, or status."
        }
    }
}

#[cfg(feature = "canvas-runtime")]
fn sdk_invalid_prop_fix(component: &str, prop: &str) -> &'static str {
    match (component, prop) {
        ("Pill", "label") => "Put the label inside the Pill children, e.g. <Pill>Label</Pill>.",
        ("Table", "columns") => "Use <Table headers={...} rows={...} />; the Canvas SDK does not support a columns prop.",
        _ => "Use props declared by the openbitfun/canvas SDK for this component.",
    }
}
