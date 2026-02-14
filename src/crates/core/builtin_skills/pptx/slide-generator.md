# HTML to PowerPoint Conversion Guide

Transform HTML slide designs into PowerPoint presentations with precise element positioning using the `slideConverter.js` library.

## Table of Contents

1. [Designing HTML Slides](#designing-html-slides)
2. [Using the slideConverter Library](#using-the-slideconverter-library)
3. [Working with PptxGenJS](#working-with-pptxgenjs)

---

## Designing HTML Slides

Each HTML slide requires proper body dimensions:

### Slide Dimensions

- **16:9** (default): `width: 720pt; height: 405pt`
- **4:3**: `width: 720pt; height: 540pt`
- **16:10**: `width: 720pt; height: 450pt`

### Supported HTML Elements

- `<p>`, `<h1>`-`<h6>` - Text content with styling
- `<ul>`, `<ol>` - Lists (avoid manual bullet characters)
- `<b>`, `<strong>` - Bold text (inline formatting)
- `<i>`, `<em>` - Italic text (inline formatting)
- `<u>` - Underlined text (inline formatting)
- `<span>` - Inline formatting with CSS styles (bold, italic, underline, color)
- `<br>` - Line breaks
- `<div>` with bg/border - Converts to shape
- `<img>` - Images
- `class="placeholder"` - Reserved space for charts (returns `{ id, x, y, w, h }`)

### Essential Text Formatting Rules

**ALL text MUST be inside `<p>`, `<h1>`-`<h6>`, `<ul>`, or `<ol>` tags:**
- Correct: `<div><p>Text here</p></div>`
- Incorrect: `<div>Text here</div>` - **Text will NOT appear in PowerPoint**
- Incorrect: `<span>Text</span>` - **Text will NOT appear in PowerPoint**
- Text in `<div>` or `<span>` without a text tag will be silently ignored

**AVOID manual bullet symbols** - Use `<ul>` or `<ol>` lists instead

**Use only universally available fonts:**
- Safe fonts: `Arial`, `Helvetica`, `Times New Roman`, `Georgia`, `Courier New`, `Verdana`, `Tahoma`, `Trebuchet MS`, `Impact`, `Comic Sans MS`
- Unsafe: `'Segoe UI'`, `'SF Pro'`, `'Roboto'`, custom fonts - **May cause rendering issues**

### Styling Guidelines

- Use `display: flex` on body to prevent margin collapse from breaking overflow validation
- Use `margin` for spacing (padding included in size)
- Inline formatting: Use `<b>`, `<i>`, `<u>` tags OR `<span>` with CSS styles
  - `<span>` supports: `font-weight: bold`, `font-style: italic`, `text-decoration: underline`, `color: #rrggbb`
  - `<span>` does NOT support: `margin`, `padding` (not supported in PowerPoint text runs)
  - Example: `<span style="font-weight: bold; color: #667eea;">Bold blue text</span>`
- Flexbox works - positions calculated from rendered layout
- Use hex colors with `#` prefix in CSS
- **Text alignment**: Use CSS `text-align` (`center`, `right`, etc.) when needed as a hint to PptxGenJS for text formatting if text lengths are slightly off

### Shape Styling (DIV elements only)

**NOTE: Backgrounds, borders, and shadows only work on `<div>` elements, NOT on text elements (`<p>`, `<h1>`-`<h6>`, `<ul>`, `<ol>`)**

- **Backgrounds**: CSS `background` or `background-color` on `<div>` elements only
  - Example: `<div style="background: #f0f0f0;">` - Creates a shape with background
- **Borders**: CSS `border` on `<div>` elements converts to PowerPoint shape borders
  - Supports uniform borders: `border: 2px solid #333333`
  - Supports partial borders: `border-left`, `border-right`, `border-top`, `border-bottom` (rendered as line shapes)
  - Example: `<div style="border-left: 8pt solid #E76F51;">`
- **Border radius**: CSS `border-radius` on `<div>` elements for rounded corners
  - `border-radius: 50%` or higher creates circular shape
  - Percentages <50% calculated relative to shape's smaller dimension
  - Supports px and pt units (e.g., `border-radius: 8pt;`, `border-radius: 12px;`)
  - Example: `<div style="border-radius: 25%;">` on 100x200px box = 25% of 100px = 25px radius
- **Box shadows**: CSS `box-shadow` on `<div>` elements converts to PowerPoint shadows
  - Supports outer shadows only (inset shadows are ignored to prevent corruption)
  - Example: `<div style="box-shadow: 2px 2px 8px rgba(0, 0, 0, 0.3);">`
  - Note: Inset/inner shadows are not supported by PowerPoint and will be skipped

### Icons and Gradients

- **ESSENTIAL: Never use CSS gradients (`linear-gradient`, `radial-gradient`)** - They don't convert to PowerPoint
- **ALWAYS create gradient/icon PNGs FIRST using Sharp, then reference in HTML**
- For gradients: Rasterize SVG to PNG background images
- For icons: Rasterize react-icons SVG to PNG images
- All visual effects must be pre-rendered as raster images before HTML rendering

### Image Assets for Slides

**NOTE**: Presentations should include relevant images to enhance visual communication. Prepare custom images from local assets or script-generated graphics before building slides.

**Image Workflow**:
1. **Before creating HTML slides**, analyze content and determine needed visuals
2. **Prepare images** using available local assets or script-generated graphics
3. **Reference images** in HTML using `<img>` tags with proper sizing

**Image Categories to Consider**:
- **Architecture diagrams**: System components, infrastructure layouts
- **Flowcharts**: Process flows, decision trees, user journeys
- **Illustrations**: Conceptual visuals, metaphorical images
- **Backgrounds**: Subtle patterns, gradient images, themed backgrounds
- **Icons**: Feature icons, category markers, decorative elements

**Image Sizing in HTML**:
```html
<!-- Full-width background image -->
<body style="background-image: url('background.png'); background-size: cover;">

<!-- Half-slide image in two-column layout -->
<div style="display: flex;">
  <div style="flex: 1;"><img src="diagram.png" style="width: 100%; height: auto;"></div>
  <div style="flex: 1; padding: 20pt;"><p>Text content here</p></div>
</div>

<!-- Centered diagram with margins -->
<img src="architecture.png" style="display: block; margin: 20pt auto; max-width: 600pt; height: auto;">

<!-- Small icon inline with text -->
<img src="icon.png" style="width: 40pt; height: 40pt; vertical-align: middle;">
```

**Image Quality Requirements**:
- **Minimum resolution**: 1920x1080 for full-slide backgrounds
- **Format**: PNG for diagrams/icons (transparency support), JPEG for photos
- **Aspect ratio**: Maintain original ratios; never stretch images

**Rasterizing Icons with Sharp:**

```javascript
const React = require('react');
const ReactDOMServer = require('react-dom/server');
const sharp = require('sharp');
const { FaHome } = require('react-icons/fa');

async function renderIconToPng(IconComponent, color, size = "256", filename) {
  const svgString = ReactDOMServer.renderToStaticMarkup(
    React.createElement(IconComponent, { color: `#${color}`, size: size })
  );

  // Convert SVG to PNG using Sharp
  await sharp(Buffer.from(svgString))
    .png()
    .toFile(filename);

  return filename;
}

// Usage: Rasterize icon before using in HTML
const iconPath = await renderIconToPng(FaHome, "4472c4", "256", "home-icon.png");
// Then reference in HTML: <img src="home-icon.png" style="width: 40pt; height: 40pt;">
```

**Rasterizing Gradients with Sharp:**

```javascript
const sharp = require('sharp');

async function generateGradientBackground(filename) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="562.5">
    <defs>
      <linearGradient id="g" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" style="stop-color:#COLOR1"/>
        <stop offset="100%" style="stop-color:#COLOR2"/>
      </linearGradient>
    </defs>
    <rect width="100%" height="100%" fill="url(#g)"/>
  </svg>`;

  await sharp(Buffer.from(svg))
    .png()
    .toFile(filename);

  return filename;
}

// Usage: Create gradient background before HTML
const bgPath = await generateGradientBackground("gradient-bg.png");
// Then in HTML: <body style="background-image: url('gradient-bg.png');">
```

### Example

```html
<!DOCTYPE html>
<html>
<head>
<style>
html { background: #ffffff; }
body {
  width: 720pt; height: 405pt; margin: 0; padding: 0;
  background: #f5f5f5; font-family: Arial, sans-serif;
  display: flex;
}
.content { margin: 30pt; padding: 40pt; background: #ffffff; border-radius: 8pt; }
h1 { color: #2d3748; font-size: 32pt; }
.box {
  background: #70ad47; padding: 20pt; border: 3px solid #5a8f37;
  border-radius: 12pt; box-shadow: 3px 3px 10px rgba(0, 0, 0, 0.25);
}
</style>
</head>
<body>
<div class="content">
  <h1>Recipe Title</h1>
  <ul>
    <li><b>Item:</b> Description</li>
  </ul>
  <p>Text with <b>bold</b>, <i>italic</i>, <u>underline</u>.</p>
  <div id="chart" class="placeholder" style="width: 350pt; height: 200pt;"></div>

  <!-- Text MUST be in <p> tags -->
  <div class="box">
    <p>5</p>
  </div>
</div>
</body>
</html>
```

## Using the slideConverter Library

### Dependencies

These libraries have been globally installed and are available to use:
- `pptxgenjs`
- `playwright`
- `sharp`

### Basic Usage

```javascript
const pptxgen = require('pptxgenjs');
const convertSlide = require('./slideConverter');

const pptx = new pptxgen();
pptx.layout = 'LAYOUT_16x9';  // Must match HTML body dimensions

const { slide, placeholders } = await convertSlide('slide1.html', pptx);

// Add chart to placeholder area
if (placeholders.length > 0) {
    slide.addChart(pptx.charts.LINE, chartData, placeholders[0]);
}

await pptx.writeFile('output.pptx');
```

### API Reference

#### Function Signature
```javascript
await convertSlide(htmlFile, pres, options)
```

#### Parameters
- `htmlFile` (string): Path to HTML file (absolute or relative)
- `pres` (pptxgen): PptxGenJS presentation instance with layout already set
- `options` (object, optional):
  - `tmpDir` (string): Temporary directory for generated files (default: `process.env.TMPDIR || '/tmp'`)
  - `slide` (object): Existing slide to reuse (default: creates new slide)

#### Returns
```javascript
{
    slide: pptxgenSlide,           // The created/updated slide
    placeholders: [                 // Array of placeholder positions
        { id: string, x: number, y: number, w: number, h: number },
        ...
    ]
}
```

### Validation

The library automatically validates and collects all errors before throwing:

1. **HTML dimensions must match presentation layout** - Reports dimension mismatches
2. **Content must not overflow body** - Reports overflow with exact measurements
3. **CSS gradients** - Reports unsupported gradient usage
4. **Text element styling** - Reports backgrounds/borders/shadows on text elements (only allowed on divs)

**All validation errors are collected and reported together** in a single error message, allowing you to fix all issues at once instead of one at a time.

### Working with Placeholders

```javascript
const { slide, placeholders } = await convertSlide('slide.html', pptx);

// Use first placeholder
slide.addChart(pptx.charts.BAR, data, placeholders[0]);

// Find by ID
const chartArea = placeholders.find(p => p.id === 'chart-area');
slide.addChart(pptx.charts.LINE, data, chartArea);
```

### Complete Example

```javascript
const pptxgen = require('pptxgenjs');
const convertSlide = require('./slideConverter');

async function buildPresentation() {
    const pptx = new pptxgen();
    pptx.layout = 'LAYOUT_16x9';
    pptx.author = 'Your Name';
    pptx.title = 'My Presentation';

    // Slide 1: Title
    const { slide: slide1 } = await convertSlide('slides/title.html', pptx);

    // Slide 2: Content with chart
    const { slide: slide2, placeholders } = await convertSlide('slides/data.html', pptx);

    const chartData = [{
        name: 'Sales',
        labels: ['Q1', 'Q2', 'Q3', 'Q4'],
        values: [4500, 5500, 6200, 7100]
    }];

    slide2.addChart(pptx.charts.BAR, chartData, {
        ...placeholders[0],
        showTitle: true,
        title: 'Quarterly Sales',
        showCatAxisTitle: true,
        catAxisTitle: 'Quarter',
        showValAxisTitle: true,
        valAxisTitle: 'Sales ($000s)'
    });

    // Save
    await pptx.writeFile({ fileName: 'presentation.pptx' });
    console.log('Presentation created successfully!');
}

buildPresentation().catch(console.error);
```

## Working with PptxGenJS

After converting HTML to slides with `convertSlide`, use PptxGenJS to add dynamic content like charts, images, and additional elements.

### Critical Rules

#### Colors
- **NEVER use `#` prefix** with hex colors in PptxGenJS - causes file corruption
- Correct: `color: "FF0000"`, `fill: { color: "0066CC" }`
- Incorrect: `color: "#FF0000"` (breaks document)

### Adding Images

Always calculate aspect ratios from actual image dimensions:

```javascript
// Get image dimensions: identify image.png | grep -o '[0-9]* x [0-9]*'
const imgWidth = 1860, imgHeight = 1519;  // From actual file
const aspectRatio = imgWidth / imgHeight;

const h = 3;  // Max height
const w = h * aspectRatio;
const x = (10 - w) / 2;  // Center on 16:9 slide

slide.addImage({ path: "chart.png", x, y: 1.5, w, h });
```

**Image Layout Patterns**:

```javascript
// Full-slide background image
slide.addImage({
    path: "background.png",
    x: 0, y: 0, w: 10, h: 5.625,  // 16:9 dimensions
    sizing: { type: 'cover' }
});

// Two-column layout: Image left, text right
slide.addImage({
    path: "diagram.png",
    x: 0.5, y: 1, w: 4.5, h: 3.5
});
slide.addText("Description text", {
    x: 5.5, y: 1, w: 4, h: 3.5
});

// Centered diagram with margins
slide.addImage({
    path: "architecture.png",
    x: 1.5, y: 1.5, w: 7, h: 3,
    sizing: { type: 'contain' }
});

// Image grid (2x2)
const gridImages = ["img1.png", "img2.png", "img3.png", "img4.png"];
const gridW = 4, gridH = 2.5, gap = 0.2;
gridImages.forEach((img, i) => {
    const col = i % 2, row = Math.floor(i / 2);
    slide.addImage({
        path: img,
        x: 0.5 + col * (gridW + gap),
        y: 0.8 + row * (gridH + gap),
        w: gridW, h: gridH
    });
});
```

**Image with Text Overlay**:

```javascript
// Background image with semi-transparent overlay for text
slide.addImage({ path: "hero-image.png", x: 0, y: 0, w: 10, h: 5.625 });
slide.addShape(pptx.shapes.RECTANGLE, {
    x: 0, y: 3.5, w: 10, h: 2.125,
    fill: { color: "000000", transparency: 50 }  // 50% transparent black
});
slide.addText("Title Over Image", {
    x: 0.5, y: 3.8, w: 9, h: 1,
    color: "FFFFFF", fontSize: 36, bold: true
});
```

### Adding Text

```javascript
// Rich text with formatting
slide.addText([
    { text: "Bold ", options: { bold: true } },
    { text: "Italic ", options: { italic: true } },
    { text: "Normal" }
], {
    x: 1, y: 2, w: 8, h: 1
});
```

### Adding Shapes

```javascript
// Rectangle
slide.addShape(pptx.shapes.RECTANGLE, {
    x: 1, y: 1, w: 3, h: 2,
    fill: { color: "4472C4" },
    line: { color: "000000", width: 2 }
});

// Circle
slide.addShape(pptx.shapes.OVAL, {
    x: 5, y: 1, w: 2, h: 2,
    fill: { color: "ED7D31" }
});

// Rounded rectangle
slide.addShape(pptx.shapes.ROUNDED_RECTANGLE, {
    x: 1, y: 4, w: 3, h: 1.5,
    fill: { color: "70AD47" },
    rectRadius: 0.2
});
```

### Adding Charts

**Required for most charts:** Axis labels using `catAxisTitle` (category) and `valAxisTitle` (value).

**Chart Data Format:**
- Use **single series with all labels** for simple bar/line charts
- Each series creates a separate legend entry
- Labels array defines X-axis values

**Time Series Data - Choose Correct Granularity:**
- **< 30 days**: Use daily grouping (e.g., "10-01", "10-02") - avoid monthly aggregation that creates single-point charts
- **30-365 days**: Use monthly grouping (e.g., "2024-01", "2024-02")
- **> 365 days**: Use yearly grouping (e.g., "2023", "2024")
- **Validate**: Charts with only 1 data point likely indicate incorrect aggregation for the time period

```javascript
const { slide, placeholders } = await convertSlide('slide.html', pptx);

// CORRECT: Single series with all labels
slide.addChart(pptx.charts.BAR, [{
    name: "Sales 2024",
    labels: ["Q1", "Q2", "Q3", "Q4"],
    values: [4500, 5500, 6200, 7100]
}], {
    ...placeholders[0],  // Use placeholder position
    barDir: 'col',       // 'col' = vertical bars, 'bar' = horizontal
    showTitle: true,
    title: 'Quarterly Sales',
    showLegend: false,   // No legend needed for single series
    // Required axis labels
    showCatAxisTitle: true,
    catAxisTitle: 'Quarter',
    showValAxisTitle: true,
    valAxisTitle: 'Sales ($000s)',
    // Optional: Control scaling (adjust min based on data range for better visualization)
    valAxisMaxVal: 8000,
    valAxisMinVal: 0,  // Use 0 for counts/amounts; for clustered data (e.g., 4500-7100), consider starting closer to min value
    valAxisMajorUnit: 2000,  // Control y-axis label spacing to prevent crowding
    catAxisLabelRotate: 45,  // Rotate labels if crowded
    dataLabelPosition: 'outEnd',
    dataLabelColor: '000000',
    // Use single color for single-series charts
    chartColors: ["4472C4"]  // All bars same color
});
```

#### Scatter Chart

**NOTE**: Scatter chart data format is unusual - first series contains X-axis values, subsequent series contain Y-values:

```javascript
// Prepare data
const data1 = [{ x: 10, y: 20 }, { x: 15, y: 25 }, { x: 20, y: 30 }];
const data2 = [{ x: 12, y: 18 }, { x: 18, y: 22 }];

const allXValues = [...data1.map(d => d.x), ...data2.map(d => d.x)];

slide.addChart(pptx.charts.SCATTER, [
    { name: 'X-Axis', values: allXValues },  // First series = X values
    { name: 'Series 1', values: data1.map(d => d.y) },  // Y values only
    { name: 'Series 2', values: data2.map(d => d.y) }   // Y values only
], {
    x: 1, y: 1, w: 8, h: 4,
    lineSize: 0,  // 0 = no connecting lines
    lineDataSymbol: 'circle',
    lineDataSymbolSize: 6,
    showCatAxisTitle: true,
    catAxisTitle: 'X Axis',
    showValAxisTitle: true,
    valAxisTitle: 'Y Axis',
    chartColors: ["4472C4", "ED7D31"]
});
```

#### Line Chart

```javascript
slide.addChart(pptx.charts.LINE, [{
    name: "Temperature",
    labels: ["Jan", "Feb", "Mar", "Apr"],
    values: [32, 35, 42, 55]
}], {
    x: 1, y: 1, w: 8, h: 4,
    lineSize: 4,
    lineSmooth: true,
    // Required axis labels
    showCatAxisTitle: true,
    catAxisTitle: 'Month',
    showValAxisTitle: true,
    valAxisTitle: 'Temperature (F)',
    // Optional: Y-axis range (set min based on data range for better visualization)
    valAxisMinVal: 0,     // For ranges starting at 0 (counts, percentages, etc.)
    valAxisMaxVal: 60,
    valAxisMajorUnit: 20,  // Control y-axis label spacing to prevent crowding (e.g., 10, 20, 25)
    // valAxisMinVal: 30,  // PREFERRED: For data clustered in a range (e.g., 32-55 or ratings 3-5), start axis closer to min value to show variation
    // Optional: Chart colors
    chartColors: ["4472C4", "ED7D31", "A5A5A5"]
});
```

#### Pie Chart (No Axis Labels Required)

**ESSENTIAL**: Pie charts require a **single data series** with all categories in the `labels` array and corresponding values in the `values` array.

```javascript
slide.addChart(pptx.charts.PIE, [{
    name: "Market Share",
    labels: ["Product A", "Product B", "Other"],  // All categories in one array
    values: [35, 45, 20]  // All values in one array
}], {
    x: 2, y: 1, w: 6, h: 4,
    showPercent: true,
    showLegend: true,
    legendPos: 'r',  // right
    chartColors: ["4472C4", "ED7D31", "A5A5A5"]
});
```

#### Multiple Data Series

```javascript
slide.addChart(pptx.charts.LINE, [
    {
        name: "Product A",
        labels: ["Q1", "Q2", "Q3", "Q4"],
        values: [10, 20, 30, 40]
    },
    {
        name: "Product B",
        labels: ["Q1", "Q2", "Q3", "Q4"],
        values: [15, 25, 20, 35]
    }
], {
    x: 1, y: 1, w: 8, h: 4,
    showCatAxisTitle: true,
    catAxisTitle: 'Quarter',
    showValAxisTitle: true,
    valAxisTitle: 'Revenue ($M)'
});
```

### Chart Colors

**ESSENTIAL**: Use hex colors **without** the `#` prefix - including `#` causes file corruption.

**Align chart colors with your chosen design palette**, ensuring sufficient contrast and distinctiveness for data visualization. Adjust colors for:
- Strong contrast between adjacent series
- Readability against slide backgrounds
- Accessibility (avoid red-green only combinations)

```javascript
// Example: Ocean palette-inspired chart colors (adjusted for contrast)
const chartColors = ["16A085", "FF6B9D", "2C3E50", "F39C12", "9B59B6"];

// Single-series chart: Use one color for all bars/points
slide.addChart(pptx.charts.BAR, [{
    name: "Sales",
    labels: ["Q1", "Q2", "Q3", "Q4"],
    values: [4500, 5500, 6200, 7100]
}], {
    ...placeholders[0],
    chartColors: ["16A085"],  // All bars same color
    showLegend: false
});

// Multi-series chart: Each series gets a different color
slide.addChart(pptx.charts.LINE, [
    { name: "Product A", labels: ["Q1", "Q2", "Q3"], values: [10, 20, 30] },
    { name: "Product B", labels: ["Q1", "Q2", "Q3"], values: [15, 25, 20] }
], {
    ...placeholders[0],
    chartColors: ["16A085", "FF6B9D"]  // One color per series
});
```

### Adding Tables

Tables can be added with basic or advanced formatting:

#### Basic Table

```javascript
slide.addTable([
    ["Header 1", "Header 2", "Header 3"],
    ["Row 1, Col 1", "Row 1, Col 2", "Row 1, Col 3"],
    ["Row 2, Col 1", "Row 2, Col 2", "Row 2, Col 3"]
], {
    x: 0.5,
    y: 1,
    w: 9,
    h: 3,
    border: { pt: 1, color: "999999" },
    fill: { color: "F1F1F1" }
});
```

#### Table with Custom Formatting

```javascript
const tableData = [
    // Header row with custom styling
    [
        { text: "Product", options: { fill: { color: "4472C4" }, color: "FFFFFF", bold: true } },
        { text: "Revenue", options: { fill: { color: "4472C4" }, color: "FFFFFF", bold: true } },
        { text: "Growth", options: { fill: { color: "4472C4" }, color: "FFFFFF", bold: true } }
    ],
    // Data rows
    ["Product A", "$50M", "+15%"],
    ["Product B", "$35M", "+22%"],
    ["Product C", "$28M", "+8%"]
];

slide.addTable(tableData, {
    x: 1,
    y: 1.5,
    w: 8,
    h: 3,
    colW: [3, 2.5, 2.5],  // Column widths
    rowH: [0.5, 0.6, 0.6, 0.6],  // Row heights
    border: { pt: 1, color: "CCCCCC" },
    align: "center",
    valign: "middle",
    fontSize: 14
});
```

#### Table with Merged Cells

```javascript
const mergedTableData = [
    [
        { text: "Q1 Results", options: { colspan: 3, fill: { color: "4472C4" }, color: "FFFFFF", bold: true } }
    ],
    ["Product", "Sales", "Market Share"],
    ["Product A", "$25M", "35%"],
    ["Product B", "$18M", "25%"]
];

slide.addTable(mergedTableData, {
    x: 1,
    y: 1,
    w: 8,
    h: 2.5,
    colW: [3, 2.5, 2.5],
    border: { pt: 1, color: "DDDDDD" }
});
```

### Table Options

Common table options:
- `x, y, w, h` - Position and size
- `colW` - Array of column widths (in inches)
- `rowH` - Array of row heights (in inches)
- `border` - Border style: `{ pt: 1, color: "999999" }`
- `fill` - Background color (no # prefix)
- `align` - Text alignment: "left", "center", "right"
- `valign` - Vertical alignment: "top", "middle", "bottom"
- `fontSize` - Text size
- `autoPage` - Auto-create new slides if content overflows
