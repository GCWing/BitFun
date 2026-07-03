use bitfun_product_domains::canvas::types::CanvasSource;

const CANVAS_RUNTIME_STYLE: &str = include_str!("runtime_style.css");
const CANVAS_RUNTIME_BOOTSTRAP: &str = include_str!("runtime_bootstrap.js");

pub fn compile_canvas_html(source: &CanvasSource, component_js: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; font-src 'none'; frame-src 'none';">
  <style>{style}</style>
</head>
<body>
  <div id="bitfun-canvas-root"></div>
  <script>
{runtime}
  </script>
  <script type="module" data-revision="{revision}">
{component_js}
  </script>
</body>
</html>"#,
        style = CANVAS_RUNTIME_STYLE,
        runtime = sanitize_script_body(CANVAS_RUNTIME_BOOTSTRAP),
        revision = escape_html_attr(source.revision.as_str()),
        component_js = sanitize_script_body(component_js),
    )
}

pub(super) fn stable_content_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn sanitize_script_body(value: &str) -> String {
    value.replace("</script", "<\\/script")
}

fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(value: &str) -> String {
    escape_html_text(value).replace('"', "&quot;")
}
