use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use super::{
    element_action_response, element_boolean_response, element_value_response, ensure_session,
    find_elements, run_script,
};
use crate::server::response::{WebDriverErrorResponse, WebDriverResponse, WebDriverResult};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct ElementLocationRequest {
    using: String,
    value: String,
}

#[derive(Debug, Deserialize)]
pub struct ElementValueRequest {
    text: Option<String>,
    value: Option<Vec<String>>,
}

pub async fn find(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<ElementLocationRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let result = find_elements(state, &session_id, None, request.using, request.value).await?;
    let Some(first) = result.first().cloned() else {
        return Err(WebDriverErrorResponse::no_such_element(
            "No element matched the selector",
        ));
    };

    Ok(WebDriverResponse::success(first))
}

pub async fn find_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<ElementLocationRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let result = find_elements(state, &session_id, None, request.using, request.value).await?;
    Ok(WebDriverResponse::success(Value::Array(result)))
}

pub async fn get_active(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let active = run_script(
        state,
        &session_id,
        "() => document.activeElement",
        Vec::new(),
        false,
    )
    .await?;
    Ok(WebDriverResponse::success(active))
}

pub async fn find_from_element(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
    Json(request): Json<ElementLocationRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let result = find_elements(
        state,
        &session_id,
        Some(element_id),
        request.using,
        request.value,
    )
    .await?;
    let Some(first) = result.first().cloned() else {
        return Err(WebDriverErrorResponse::no_such_element(
            "No child element matched the selector",
        ));
    };

    Ok(WebDriverResponse::success(first))
}

pub async fn find_all_from_element(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
    Json(request): Json<ElementLocationRequest>,
) -> WebDriverResult {
    ensure_session(&state, &session_id).await?;
    let result = find_elements(
        state,
        &session_id,
        Some(element_id),
        request.using,
        request.value,
    )
    .await?;
    Ok(WebDriverResponse::success(Value::Array(result)))
}

pub async fn is_selected(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_boolean_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); return !!el && !!(el.selected || el.checked); }",
    )
    .await
}

pub async fn is_displayed(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_boolean_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); return window.__bitfunWd.isDisplayed(el); }",
    )
    .await
}

pub async fn get_attribute(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id, name)): Path<(String, String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id, name) => { const el = window.__bitfunWd.getElement(id); if (!el) { return null; } const attrName = String(name || '').toLowerCase(); const tagName = String(el.tagName || '').toLowerCase(); if (attrName === 'value' && (tagName === 'input' || tagName === 'textarea')) { return el.value; } if (attrName === 'checked' && tagName === 'input' && (el.type === 'checkbox' || el.type === 'radio')) { return el.checked ? 'true' : null; } if (attrName === 'selected' && tagName === 'option') { return el.selected ? 'true' : null; } return el.getAttribute(name); }",
        vec![Value::String(name)],
    )
    .await
}

pub async fn get_property(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id, name)): Path<(String, String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id, name) => { const el = window.__bitfunWd.getElement(id); return el ? el[name] : null; }",
        vec![Value::String(name)],
    )
    .await
}

pub async fn get_css_value(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id, property_name)): Path<(String, String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id, propertyName) => { const el = window.__bitfunWd.getElement(id); return el ? window.getComputedStyle(el).getPropertyValue(propertyName) : ''; }",
        vec![Value::String(property_name)],
    )
    .await
}

pub async fn get_text(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); return el ? (el.innerText ?? el.textContent ?? '') : ''; }",
        Vec::new(),
    )
    .await
}

pub async fn get_computed_role(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); if (!el) { return ''; } const explicitRole = el.getAttribute('role'); if (explicitRole) { return explicitRole; } const tag = String(el.tagName || '').toLowerCase(); if (tag === 'button') return 'button'; if (tag === 'a' && el.hasAttribute('href')) return 'link'; if (tag === 'input') { const type = String(el.getAttribute('type') || 'text').toLowerCase(); if (type === 'checkbox') return 'checkbox'; if (type === 'radio') return 'radio'; if (type === 'submit' || type === 'button' || type === 'reset') return 'button'; return 'textbox'; } if (tag === 'select') return 'combobox'; if (tag === 'textarea') return 'textbox'; return ''; }",
        Vec::new(),
    )
    .await
}

pub async fn get_computed_label(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); if (!el) { return ''; } const labelledBy = el.getAttribute('aria-labelledby'); if (labelledBy) { return labelledBy.split(/\\s+/).map((labelId) => document.getElementById(labelId)?.innerText?.trim() || '').filter(Boolean).join(' ').trim(); } const ariaLabel = el.getAttribute('aria-label'); if (ariaLabel) { return ariaLabel; } const htmlFor = el.id ? document.querySelector(`label[for=\"${el.id}\"]`) : null; if (htmlFor) { return (htmlFor.innerText || htmlFor.textContent || '').trim(); } return (el.innerText || el.textContent || el.getAttribute('value') || '').trim(); }",
        Vec::new(),
    )
    .await
}

pub async fn get_name(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); return el ? String(el.tagName || '').toLowerCase() : ''; }",
        Vec::new(),
    )
    .await
}

pub async fn get_rect(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_value_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); if (!el) { return null; } const rect = el.getBoundingClientRect(); return { x: rect.x + window.scrollX, y: rect.y + window.scrollY, width: rect.width, height: rect.height, top: rect.top + window.scrollY, left: rect.left + window.scrollX, right: rect.right + window.scrollX, bottom: rect.bottom + window.scrollY }; }",
        Vec::new(),
    )
    .await
}

pub async fn is_enabled(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_boolean_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); return !!el && !el.disabled; }",
    )
    .await
}

pub async fn click(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_action_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); if (!el) { throw new Error('Element not found'); } window.__bitfunWd.dispatchPointerClick(el, 0, false); return null; }",
        Vec::new(),
    )
    .await
}

pub async fn clear(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> WebDriverResult {
    element_action_response(
        state,
        &session_id,
        &element_id,
        "(id) => { const el = window.__bitfunWd.getElement(id); if (!el) { throw new Error('Element not found'); } window.__bitfunWd.clearElement(el); return null; }",
        Vec::new(),
    )
    .await
}

pub async fn send_keys(
    State(state): State<Arc<AppState>>,
    Path((session_id, element_id)): Path<(String, String)>,
    Json(request): Json<ElementValueRequest>,
) -> WebDriverResult {
    let text = request
        .text
        .or_else(|| request.value.map(|items| items.join("")))
        .unwrap_or_default();

    element_action_response(
        state,
        &session_id,
        &element_id,
        "(id, text) => { const el = window.__bitfunWd.getElement(id); if (!el) { throw new Error('Element not found'); } window.__bitfunWd.insertText(el, text); return null; }",
        vec![Value::String(text)],
    )
    .await
}
