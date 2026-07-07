#[cfg(feature = "canvas-runtime")]
use super::*;
#[cfg(feature = "canvas-runtime")]
use bitfun_product_domains::canvas::types::{CanvasId, CanvasRevision};

#[cfg(feature = "canvas-runtime")]
mod enabled {
    use super::*;

    #[cfg(feature = "canvas-runtime")]
    fn source(source: &str) -> CanvasSource {
        CanvasSource::new_tsx(
            CanvasId::new("canvas_1"),
            CanvasRevision::new("rev_1"),
            "canvas.tsx",
            source,
            BITFUN_CANVAS_SDK_VERSION,
            1,
        )
    }

    #[test]
    fn canvas_compiler_transforms_default_component_jsx() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Stack, Text } from 'bitfun/canvas';
const rows = ['a', 'b'];
export default function Canvas() {
  return <Stack gap={8}><Text tone="muted">Ready</Text>{rows.map(row => <Text>{row}</Text>)}</Stack>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("window.BitfunCanvasRuntime.mount"));
        assert!(html.contains("const h = __BitfunCanvasRuntime.h"));
        assert!(html.contains("const Fragment = __BitfunCanvasRuntime.Fragment"));
        assert!(html.contains("h(Stack"));
        assert!(html.contains("rows.map"));
        assert!(html.contains("h(Text"));
        assert!(html.contains("bitfun-canvas-save-state"));
        assert!(html.contains("bitfun-canvas-state"));
        assert!(html.contains("bitfun-canvas-theme"));
        assert!(html.contains("applyHostTheme"));
        assert!(html.contains("bitfun-canvas-design-mode"));
        assert!(html.contains("bitfun-canvas-element-selected"));
        assert!(html.contains("data-bitfun-canvas-node"));
        assert!(html.contains("bitfun-canvas-action-result"));
        assert!(html.contains("pendingActions"));
        assert!(html.contains("stack: error?.stack ? String(error.stack) : undefined"));
        assert!(html.contains("connect-src 'none'"));
    }

    #[test]
    fn canvas_compiler_rejects_local_sdk_component_shadowing() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Grid, Stack, Text } from 'bitfun/canvas';

export default function Canvas() {
  return <Stack><Grid columns={2}><Text>Ready</Text></Grid></Stack>;
}

function Grid() {
  return <div />;
}
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code.as_deref() == Some("canvas.compile.sdk_name_shadowed")
            })
            .expect("shadow diagnostic should be present");
        assert!(diagnostic.message.contains("Grid"), "{diagnostic:?}");
    }

    #[test]
    fn canvas_compiler_preserves_named_import_alias_bindings() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Text as T, Stack } from 'bitfun/canvas';

export default function Canvas() {
  return <Stack><T tone="muted">Ready</T></Stack>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("const T = __BitfunCanvasSDK.Text;"));
        assert!(html.contains("h(T"));
        assert!(!html.contains("from 'bitfun/canvas'"));
    }

    #[test]
    fn canvas_compiler_validates_props_through_named_import_alias() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Pill as P } from 'bitfun/canvas';

export default function Canvas() {
  return <P label="1" />;
}
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("canvas.sdk.invalid_prop"))
            .expect("alias prop diagnostic should be present");
        assert!(diagnostic.message.contains("Pill"), "{diagnostic:?}");
    }

    #[test]
    fn canvas_compiler_preserves_namespace_import_bindings() {
        let result = compile_canvas_source(
            &source(
                r#"
import * as Canvas from 'bitfun/canvas';

export default function CanvasView() {
  return <Canvas.Stack><Canvas.Text tone="muted">Ready</Canvas.Text></Canvas.Stack>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("const Canvas = __BitfunCanvasSDK;"));
        assert!(html.contains("h(Canvas.Stack"));
        assert!(html.contains("h(Canvas.Text"));
    }

    #[test]
    fn canvas_compiler_validates_props_through_namespace_import() {
        let result = compile_canvas_source(
            &source(
                r#"
import * as C from 'bitfun/canvas';

export default function Canvas() {
  return <C.Table columns={[]} rows={[]} />;
}
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_deref() == Some("canvas.sdk.invalid_prop"))
            .expect("namespace prop diagnostic should be present");
        assert!(diagnostic.message.contains("Table"), "{diagnostic:?}");
    }

    #[test]
    fn canvas_compiler_strips_imports_by_ast_span_not_semicolon_scan() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Text as T } from 'bitfun/canvas' with { note: "semi;colon" };

export default function Canvas() {
  return <T>Ready</T>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("const T = __BitfunCanvasSDK.Text;"));
        assert!(!html.contains("semi;colon"));
    }

    #[test]
    fn canvas_compiler_preserves_react_namespace_and_default_compat_bindings() {
        let result = compile_canvas_source(
            &source(
                r#"
import React, * as R from 'react';
import { Text } from 'bitfun/canvas';

export default function Canvas() {
  const [count] = React.useState(1);
  return R.createElement(Text, null, String(count));
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("const React = __BitfunCanvasReactCompat;"));
        assert!(html.contains("const R = __BitfunCanvasReactCompat;"));
        assert!(!html.contains("from 'react'"));
    }

    #[test]
    fn canvas_compiler_supports_named_arrow_default_export() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Stack } from 'bitfun/canvas';
const Canvas = () => <Stack />;
export default Canvas;
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("const Canvas"));
        assert!(html.contains("h(Stack"));
        assert!(html.contains("const __BitfunCanvasComponent = Canvas;"));
        assert!(html.contains("window.BitfunCanvasRuntime.mount(__BitfunCanvasComponent)"));
    }

    #[test]
    fn canvas_compiler_reports_missing_default_component_declaration() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Stack } from 'bitfun/canvas';
export default Canvas;
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        assert_eq!(
            result.diagnostics[0].code.as_deref(),
            Some("canvas.compile.default_function_required")
        );
    }

    #[test]
    fn canvas_compiler_reports_invalid_host_theme_tokens() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Stack, useHostTheme } from 'bitfun/canvas';
export default function Canvas() {
  const theme = useHostTheme();
  return <Stack style={{ background: theme.surface.primary, color: theme.interactive.accent }}>Body</Stack>;
}
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        let diagnostics = result
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.code.as_deref() == Some("canvas.sdk.invalid_theme_token")
            })
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 2, "{:?}", result.diagnostics);
        assert!(diagnostics[0].message.contains("theme.surface.primary"));
        assert!(diagnostics[0]
            .suggested_fix
            .as_deref()
            .is_some_and(|fix| fix.contains("theme.bg.editor")));
        assert!(diagnostics[1].message.contains("theme.interactive.accent"));
        assert!(diagnostics[1]
            .suggested_fix
            .as_deref()
            .is_some_and(|fix| fix.contains("theme.accent.primary")));
    }

    #[test]
    fn canvas_compiler_reports_located_jsx_diagnostics() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Stack } from 'bitfun/canvas';
export default function Canvas() {
  return <Stack><Missing></Stack>;
}
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        let diagnostic = &result.diagnostics[0];
        assert!(diagnostic
            .code
            .as_deref()
            .is_some_and(|code| code.starts_with("canvas.compile.oxc.")));
        assert!(diagnostic.line.is_some(), "{diagnostic:?}");
        assert!(diagnostic.column.is_some(), "{diagnostic:?}");
    }

    #[test]
    fn canvas_compiler_sanitizes_script_close_tags() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Text } from 'bitfun/canvas';
export default function Canvas() {
  return <Text>{"</script><div>"}</Text>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        assert!(!result.payload.unwrap().html.contains("</script><div>"));
    }

    #[test]
    fn canvas_compiler_supports_fragments() {
        let result = compile_canvas_source(
            &source(
                r#"
import { H1, Text } from 'bitfun/canvas';
export default function Canvas() {
  return <><H1>Title</H1><Text>Body</Text></>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("h(Fragment"));
        assert!(html.contains("h(H1"));
        assert!(html.contains("h(Text"));
    }

    #[test]
    fn canvas_runtime_exports_box_component() {
        let result = compile_canvas_source(
            &source(
                r##"
import { Box, Text } from 'bitfun/canvas';
export default function Canvas() {
  return <Box padding={{ x: 12, y: 8 }} background="#fff"><Text>Body</Text></Box>;
}
"##,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("Stack, Row, Grid, Box, Divider"));
        assert!(html.contains("const Box ="));
        assert!(html.contains("window.BitfunCanvasSDK = { Stack, Row, Grid, Box"));
        assert!(html.contains("h(Box"));
    }

    #[test]
    fn canvas_runtime_exports_cursor_style_canvas_components() {
        let result = compile_canvas_source(
            &source(
                r#"
import {
  Alert,
  Card,
  CardBody,
  CardHeader,
  CollapsibleSection,
  DiffStats,
  DiffView,
  DependencyGraph,
  Empty,
  FileTree,
  FlowDiagram,
  Grid,
  H1,
  Input,
  KeyValueList,
  Pill,
  ProgressBar,
  Row,
  Spacer,
  Stack,
  Swatch,
  Table,
  Text,
  TextArea,
  Tabs,
  Timeline,
  TodoListCard,
  UsageBar,
  canvasTokens,
  colorPalette,
  computeDAGLayout,
  mergeStyle,
  useCanvasState,
  useHostTheme,
} from 'bitfun/canvas';

const lines = [
  { type: 'unchanged', content: 'fn main() {}', lineNumber: 1 },
  { type: 'added', content: 'println!("ready");', lineNumber: 2 },
];

export default function Canvas() {
  const theme = useHostTheme();
  const [note, setNote] = useCanvasState('note', '');
  const merged = mergeStyle({ maxWidth: 900 }, { padding: 4 });
  const layout = computeDAGLayout({
nodes: [{ id: 'web-ui' }, { id: 'core' }],
edges: [{ from: 'web-ui', to: 'core' }],
nodeWidth: 120,
nodeHeight: 40,
  });
  return (
<Stack gap={16}>
  <H1>Cursor-style canvas</H1>
  <Grid columns={2}>
    <Text style={{ color: theme.text.primary }}>{layout.width}</Text>
    <Pill active size="sm">OPEN</Pill>
  </Grid>
  <Alert type="warning" title="Risk" message="Check runtime boundaries" />
  <KeyValueList items={{ Runtime: 'ready' }} columns={1} />
  <Timeline items={[{ title: 'Compiled', description: 'SDK loaded', time: 'now', tone: 'success' }]} />
  <FileTree items={[{ name: 'src', type: 'folder', children: [{ name: 'main.rs', meta: '+1' }] }]} />
  <ProgressBar value={75} max={100} label="Coverage" tone="success" />
  <Row style={merged}>
    <Swatch color="purple" title="Runtime" />
    <Text style={{ color: canvasTokens.textSecondary }}>{colorPalette.length} palette colors</Text>
  </Row>
  <UsageBar
    topLeftLabel="Context"
    topRightLabel="75%"
    total={100}
    segments={[{ id: 'input', value: 40, color: 'blue' }, { id: 'output', value: 35, color: 'green' }]}
  />
  <TodoListCard
    defaultExpanded
    todos={[{ id: 'one', content: 'Map SDK surface', status: 'completed' }, { id: 'two', content: 'Align docs skill', status: 'in_progress' }]}
  />
  <DependencyGraph nodes={[{ id: 'runtime', label: 'Runtime' }, { id: 'sdk', label: 'SDK' }]} edges={[{ from: 'runtime', to: 'sdk' }]} />
  <FlowDiagram steps={['Compile', { title: 'Render', description: 'iframe' }]} />
  <Tabs items={[{ key: 'graph', label: 'Graph', children: <Text>Graph tab</Text> }]} defaultActiveKey="graph" />
  <CollapsibleSection title="Files" count={1} defaultOpen>
    <Card>
      <CardHeader trailing={<DiffStats additions={1} deletions={0} />}>
        src/main.rs
      </CardHeader>
      <CardBody style={{ padding: 0 }}>
        <DiffView path="src/main.rs" lines={lines} />
      </CardBody>
    </Card>
  </CollapsibleSection>
  <Row>
    <Input value={note} onChange={setNote} placeholder="Summary" label="Note" hint="Session scoped" />
    <TextArea value={note} onChange={setNote} rows={4} />
    <Spacer />
  </Row>
  <Table headers={['A', 'B']} rows={[[theme.bg.elevated, 'ok']]} striped rowTone={['success']} />
  <Empty description="No remaining gaps" />
</Stack>
  );
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("CollapsibleSection"));
        assert!(html.contains("const Alert"));
        assert!(html.contains("function computeDAGLayout"));
        assert!(html.contains("const DiffView"));
        assert!(html.contains("const KeyValueList"));
        assert!(html.contains("const Timeline"));
        assert!(html.contains("const FileTree"));
        assert!(html.contains("const ProgressBar"));
        assert!(html.contains("const Swatch"));
        assert!(html.contains("const UsageBar"));
        assert!(html.contains("const TodoListCard"));
        assert!(html.contains("function mergeStyle"));
        assert!(html.contains("function DependencyGraph"));
        assert!(html.contains("function FlowDiagram"));
        assert!(html.contains("const Tabs"));
        assert!(html.contains("const Input"));
        assert!(html.contains("const Empty"));
        assert!(html.contains("const TextArea"));
        assert!(html.contains("const Spacer"));
        assert!(html.contains("theme.text.primary"));
    }

    #[test]
    fn canvas_runtime_supports_cursor_canvas_compat_hooks_and_theme_tokens() {
        let result = compile_canvas_source(
            &source(
                r#"
import { useEffect, useMemo, useRef, useState } from 'react';
import { Button, Stack, Text, useHostTheme } from 'cursor/canvas';

export default function Canvas() {
  const theme = useHostTheme();
  const rowRef = useRef<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);
  const label = useMemo(() => open ? 'Open' : 'Closed', [open]);
  useEffect(() => {
rowRef.current?.setAttribute('data-effect', label);
return () => rowRef.current?.removeAttribute('data-effect');
  }, [label]);
  return (
<Stack gap={8}>
  <div ref={rowRef} style={{ color: theme.category.gray, background: theme.bg.editor }}>
    <Text style={{ color: theme.text.quaternary }}>{label}</Text>
  </div>
  <Button onClick={() => setOpen(value => !value)}>Toggle</Button>
</Stack>
  );
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("useState, useRef, useEffect, useCallback, useMemo"));
        assert!(html.contains("category:"));
        assert!(html.contains("gray: '#7a8087'"));
        assert!(html.contains("tokens: theme"));
        assert!(html.contains("key === 'ref'"));
        assert!(html.contains("function flushEffects()"));
        assert!(html.contains("h(Stack"));
        assert!(html.contains("h(Button"));
        assert!(!html.contains("from 'react'"));
        assert!(!html.contains("from 'cursor/canvas'"));
    }

    #[test]
    fn canvas_compiler_rejects_invalid_canvas_sdk_props() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Pill, Stack, Table } from 'bitfun/canvas';
export default function Canvas() {
  return (
<Stack>
  <Pill label="1" />
  <Table columns={[{ key: 'name', title: 'Name' }]} rows={[]} />
</Stack>
  );
}
"#,
            ),
            2,
        );

        assert!(!result.compiled);
        let messages = result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(messages.contains("label"), "{messages}");
        assert!(messages.contains("columns"), "{messages}");
        assert!(result.diagnostics.iter().all(|diagnostic| {
            diagnostic.category == CanvasDiagnosticCategory::TypeScript
                && diagnostic.code.as_deref() == Some("canvas.sdk.invalid_prop")
        }));
    }

    #[test]
    fn canvas_compiler_accepts_valid_cursor_style_sdk_props() {
        let result = compile_canvas_source(
            &source(
                r#"
import { Pill, Stack, Table, computeDAGLayout } from 'bitfun/canvas';
export default function Canvas() {
  const layout = computeDAGLayout({
nodes: [{ id: 'a' }, { id: 'b' }],
edges: [{ from: 'a', to: 'b' }],
  });
  return (
<Stack>
  <Pill>1</Pill>
  <Table headers={['Name']} rows={[[String(layout.width)]]} />
</Stack>
  );
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
    }

    #[test]
    fn canvas_runtime_includes_svg_chart_components() {
        let result = compile_canvas_source(
            &source(
                r#"
import { BarChart, LineChart, PieChart } from 'bitfun/canvas';
const rows = [{ label: 'A', value: 10 }, { label: 'B', value: 16 }];
export default function Canvas() {
  return <><BarChart data={rows} /><LineChart data={rows} /><PieChart data={rows} /></>;
}
"#,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("document.createElementNS('http://www.w3.org/2000/svg'"));
        assert!(html.contains("function BarChart"));
        assert!(html.contains("function LineChart"));
        assert!(html.contains("function PieChart"));
        assert!(!html.contains("function simpleChart"));
    }

    #[test]
    fn canvas_compiler_handles_real_architecture_sample() {
        let result = compile_canvas_source(
            &source(
                r##"
import { Stack, Text, Box, Row, Divider } from 'bitfun/canvas';

const LAYER_COLORS = [
  { bg: '#1a1a2e', border: '#6c63ff', text: '#c4b5fd' },
  { bg: '#16213e', border: '#0ea5e9', text: '#7dd3fc' },
];

const layers = [
  {
label: 'L1 接口与入口',
subtitle: 'Interfaces & Entrypoints',
modules: ['Desktop App', 'CLI', 'Server'],
tech: 'React · Tauri · Vite · Node',
  },
  {
label: 'L2 产品组装',
subtitle: 'Product Assembly',
modules: ['assembly/core', 'product-capabilities'],
tech: '兼容门面 · 能力选择 · 服务注册',
  },
];

export default function Canvas() {
  return (
<Stack gap={0}>
  {/* Title */}
  <Box padding={{ x: 24, y: 20 }} background="#0f0f1a" borderBottom="1px solid #2d2d4a">
    <Text size={22} weight={700} color="#e2e8f0">
      BitFun 架构总览
    </Text>
    <Text size={13} color="#94a3b8" margin={{ top: 4 }}>
      Rust 工作空间 + React 前端
    </Text>
  </Box>

  <Stack gap={0} padding={{ x: 24, y: 16 }}>
    {layers.map((layer, i) => (
      <Stack key={i} gap={0}>
        <Box
          background={LAYER_COLORS[i].bg}
          border={`2px solid ${LAYER_COLORS[i].border}`}
          borderRadius={8}
          padding={16}
        >
          <Row gap={12} align="center" margin={{ bottom: 10 }}>
            <Box background={LAYER_COLORS[i].border} borderRadius={4} padding={{ x: 8, y: 3 }}>
              <Text size={12} weight={700} color="#fff">
                {layer.label}
              </Text>
            </Box>
            <Text size={12} color={LAYER_COLORS[i].text} opacity={0.8}>
              {layer.subtitle}
            </Text>
            <Box flex={1} />
            <Text size={11} color={LAYER_COLORS[i].text} opacity={0.6}>
              {layer.tech}
            </Text>
          </Row>

          <Row gap={6} wrap="wrap">
            {layer.modules.map((mod, j) => (
              <Box
                key={j}
                background={`${LAYER_COLORS[i].border}22`}
                border={`1px solid ${LAYER_COLORS[i].border}55`}
                borderRadius={4}
                padding={{ x: 8, y: 4 }}
              >
                <Text size={11} color={LAYER_COLORS[i].text}>
                  {mod}
                </Text>
              </Box>
            ))}
          </Row>
        </Box>

        {i < layers.length - 1 && (
          <Row justify="center" padding={{ y: 2 }}>
            <Text size={16} color="#4a4a6a">
              ↓
            </Text>
          </Row>
        )}
      </Stack>
    ))}
  </Stack>
  <Divider />
</Stack>
  );
}
"##,
            ),
            2,
        );

        assert!(result.compiled, "{:?}", result.diagnostics);
        let html = result.payload.unwrap().html;
        assert!(html.contains("h(Box"));
        assert!(html.contains("layers.map"));
        assert!(html.contains("window.BitfunCanvasRuntime.mount(__BitfunCanvasComponent)"));
        assert!(html.contains("reportRuntimeError"));
    }
}

#[cfg(not(feature = "canvas-runtime"))]
use super::*;

#[cfg(not(feature = "canvas-runtime"))]
#[test]
fn canvas_compiler_reports_feature_disabled_without_canvas_feature() {
    let diagnostics =
        compile_canvas_component_js("export default function Canvas() { return null; }")
            .expect_err("canvas feature should be required");

    assert_eq!(
        diagnostics[0].code.as_deref(),
        Some("canvas.compile.feature_disabled")
    );
}
