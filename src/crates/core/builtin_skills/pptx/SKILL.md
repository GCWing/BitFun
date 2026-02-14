---
name: pptx
description: "PowerPoint document toolkit for slide generation, content modification, and presentation analysis. Ideal for: (1) Building new slide decks from scratch, (2) Editing existing presentation content, (3) Managing slide layouts and templates, (4) Inserting notes and annotations, or handling other presentation-related operations"
description_zh: "PowerPoint 文档工具包，用于幻灯片生成、内容修改和演示文稿分析。适用于：(1) 从头构建新的幻灯片，(2) 编辑现有演示文稿内容，(3) 管理幻灯片布局和模板，(4) 插入备注和批注，或处理其他演示文稿相关操作"
---

# PowerPoint Document Generation and Editing Toolkit

## Introduction

Users may request you to generate, modify, or analyze .pptx files. A .pptx file is fundamentally a ZIP container with XML documents and associated resources that can be inspected or altered. Different utilities and processes are available depending on the task requirements.

## Extracting and Analyzing Content

### Text Content Extraction
When you only need to retrieve textual content from slides, convert the presentation to markdown format:

```bash
# Transform presentation to markdown
python -m markitdown path-to-file.pptx
```

### Direct XML Inspection
Direct XML inspection is required for: annotations, presenter notes, master layouts, transition effects, visual styling, and advanced formatting. For these capabilities, unpack the presentation and examine its XML structure.

#### Extracting Package Contents
`python openxml/scripts/extract.py <office_file> <output_dir>`

**Note**: In this repository, run from `src/crates/core/builtin_skills/pptx` so `openxml/scripts/extract.py` resolves correctly. Absolute path from project root: `src/crates/core/builtin_skills/pptx/openxml/scripts/extract.py`.

#### Essential File Hierarchy
* `ppt/presentation.xml` - Core presentation metadata and slide references
* `ppt/slides/slide{N}.xml` - Individual slide content (slide1.xml, slide2.xml, etc.)
* `ppt/notesSlides/notesSlide{N}.xml` - Presenter notes per slide
* `ppt/comments/modernComment_*.xml` - Slide-specific annotations
* `ppt/slideLayouts/` - Layout template definitions
* `ppt/slideMasters/` - Master slide configurations
* `ppt/theme/` - Theme and styling definitions
* `ppt/media/` - Embedded images and media assets

#### Typography and Color Extraction
**When provided with a reference design to replicate**: Analyze the presentation's typography and color scheme first using these approaches:
1. **Examine theme file**: Check `ppt/theme/theme1.xml` for color definitions (`<a:clrScheme>`) and font configurations (`<a:fontScheme>`)
2. **Inspect slide content**: Examine `ppt/slides/slide1.xml` for actual font usage (`<a:rPr>`) and color values
3. **Pattern search**: Use grep to locate color (`<a:solidFill>`, `<a:srgbClr>`) and font references across all XML files

## Building a New Presentation **from Scratch**

For creating new presentations without an existing template, use the **slideConverter** workflow to transform HTML slides into PowerPoint with precise element positioning.

### Design Philosophy

**ESSENTIAL**: Before building any presentation, evaluate the content and select appropriate visual elements:
1. **Analyze subject matter**: What is the presentation topic? What tone, industry context, or mood should it convey?
2. **Identify branding requirements**: If a company/organization is mentioned, consider their brand colors and visual identity
3. **Align palette with content**: Choose colors that complement the subject matter
4. **Plan visual elements**: Determine which slides require images, diagrams, or illustrations for better comprehension
5. **Document your approach**: Explain design decisions before writing code

**Guidelines**:
- State your content-driven design approach BEFORE writing code
- Use universally available fonts: Arial, Helvetica, Times New Roman, Georgia, Courier New, Verdana, Tahoma, Trebuchet MS, Impact
- Establish visual hierarchy through size, weight, and color variations
- Prioritize readability: strong contrast, appropriately sized text, clean alignment
- Maintain consistency: repeat patterns, spacing, and visual language across slides
- **Incorporate images proactively**: Enhance presentations with relevant visuals (architecture diagrams, flowcharts, icons, illustrations)

#### Color Palette Design

**Developing creative color schemes**:
- **Move beyond defaults**: What colors authentically match this specific topic? Avoid automatic choices.
- **Explore multiple dimensions**: Topic, industry, mood, energy level, target audience, brand identity (if applicable)
- **Experiment boldly**: Try unexpected combinations - a healthcare presentation doesn't require green, finance doesn't require navy
- **Construct your palette**: Select 3-5 harmonious colors (dominant colors + supporting tones + accent)
- **Verify contrast**: Text must remain clearly readable against backgrounds

**Sample color palettes** (use for inspiration - select one, adapt it, or create your own):

1. **Corporate Navy**: Deep navy (#1C2833), slate gray (#2E4053), silver (#AAB7B8), off-white (#F4F6F6)
2. **Ocean Breeze**: Teal (#5EA8A7), deep teal (#277884), coral (#FE4447), white (#FFFFFF)
3. **Vibrant Sunset**: Red (#C0392B), bright red (#E74C3C), orange (#F39C12), yellow (#F1C40F), green (#2ECC71)
4. **Soft Blush**: Mauve (#A49393), blush (#EED6D3), rose (#E8B4B8), cream (#FAF7F2)
5. **Rich Wine**: Burgundy (#5D1D2E), crimson (#951233), rust (#C15937), gold (#997929)
6. **Royal Amethyst**: Purple (#B165FB), dark blue (#181B24), emerald (#40695B), white (#FFFFFF)
7. **Natural Cream**: Cream (#FFE1C7), forest green (#40695B), white (#FCFCFC)
8. **Berry Fusion**: Pink (#F8275B), coral (#FF574A), rose (#FF737D), purple (#3D2F68)
9. **Garden Fresh**: Lime (#C5DE82), plum (#7C3A5F), coral (#FD8C6E), blue-gray (#98ACB5)
10. **Luxe Noir**: Gold (#BF9A4A), black (#000000), cream (#F4F6F6)
11. **Mediterranean**: Sage (#87A96B), terracotta (#E07A5F), cream (#F4F1DE), charcoal (#2C2C2C)
12. **Modern Mono**: Charcoal (#292929), red (#E33737), light gray (#CCCBCB)
13. **Energy Burst**: Orange (#F96D00), light gray (#F2F2F2), charcoal (#222831)
14. **Tropical Forest**: Black (#191A19), green (#4E9F3D), dark green (#1E5128), white (#FFFFFF)
15. **Retro Spectrum**: Purple (#722880), pink (#D72D51), orange (#EB5C18), amber (#F08800), gold (#DEB600)
16. **Autumn Harvest**: Mustard (#E3B448), sage (#CBD18F), forest green (#3A6B35), cream (#F4F1DE)
17. **Seaside Rose**: Old rose (#AD7670), beaver (#B49886), eggshell (#F3ECDC), ash gray (#BFD5BE)
18. **Citrus Splash**: Light orange (#FC993E), grayish turquoise (#667C6F), white (#FCFCFC)

#### Visual Design Elements

**Geometric Patterns**:
- Diagonal section dividers instead of horizontal
- Asymmetric column widths (30/70, 40/60, 25/75)
- Rotated text headers at 90 or 270 degrees
- Circular/hexagonal frames for images
- Triangular accent shapes in corners
- Overlapping shapes for depth

**Border and Frame Treatments**:
- Thick single-color borders (10-20pt) on one side only
- Double-line borders with contrasting colors
- Corner brackets instead of full frames
- L-shaped borders (top+left or bottom+right)
- Underline accents beneath headers (3-5pt thick)

**Typography Treatments**:
- Extreme size contrast (72pt headlines vs 11pt body)
- All-caps headers with wide letter spacing
- Numbered sections in oversized display type
- Monospace (Courier New) for data/stats/technical content
- Condensed fonts (Arial Narrow) for dense information
- Outlined text for emphasis

**Data Visualization Styling**:
- Monochrome charts with single accent color for key data
- Horizontal bar charts instead of vertical
- Dot plots instead of bar charts
- Minimal gridlines or none at all
- Data labels directly on elements (no legends)
- Oversized numbers for key metrics

**Layout Innovations**:
- Full-bleed images with text overlays
- Sidebar column (20-30% width) for navigation/context
- Modular grid systems (3x3, 4x4 blocks)
- Z-pattern or F-pattern content flow
- Floating text boxes over colored shapes
- Magazine-style multi-column layouts

**Background Treatments**:
- Solid color blocks occupying 40-60% of slide
- Gradient fills (vertical or diagonal only)
- Split backgrounds (two colors, diagonal or vertical)
- Edge-to-edge color bands
- Negative space as a design element
- **Background images**: Use subtle, low-contrast images as backgrounds with text overlays
- **Gradient overlays**: Combine background images with semi-transparent gradient overlays for readability

#### Visual Assets and Image Planning

**CRITICAL**: Proactively enhance presentations with relevant images to improve visual communication and audience engagement. Do NOT rely solely on text.

**When to Add Images**:
- **Architecture/System slides**: Always include system architecture diagrams, component diagrams, or infrastructure illustrations
- **Process/Workflow slides**: Add flowcharts, process diagrams, or step-by-step illustrations
- **Data flow slides**: Include data pipeline diagrams, ETL flow illustrations
- **Feature/Product slides**: Add UI mockups, screenshots, or product illustrations
- **Concept explanation slides**: Use metaphorical illustrations or conceptual diagrams
- **Team/About slides**: Include relevant icons or illustrations representing team activities
- **Comparison slides**: Use side-by-side visual comparisons or before/after images

**Image Categories to Prepare**:
1. **Architecture Diagrams**: System components, microservices layout, cloud infrastructure
2. **Flowcharts**: Business processes, user journeys, decision trees
3. **Data Visualizations**: Custom infographics, data flow diagrams
4. **Icons and Illustrations**: Conceptual icons, feature illustrations, decorative elements
5. **Backgrounds**: Subtle pattern backgrounds, gradient images, themed backgrounds
6. **UI/UX Elements**: Interface mockups, wireframe illustrations

**Image Asset Guidelines**:
- Prepare high-quality images tailored to slide content using available local assets or script-generated graphics
- Prefer PNG format for direct insertion into slides
- **NEVER use code-based diagrams** (like Mermaid) that require rendering - all images must be static PNG/SVG
- Match image style to presentation theme (colors, mood, professionalism level)
- Ensure generated images have sufficient resolution (at least 1920x1080 for full-slide backgrounds)

**Image Asset Workflow**:
```
When preparing images for presentations:
1. Analyze the slide content and determine what visual would enhance it
2. Choose an image source:
   - Existing project assets (screenshots, diagrams, brand images)
   - Script-generated assets (e.g., SVG/PNG produced with local tooling)
3. Place the image in the appropriate slide location and verify layout/readability
```

**Sample Asset Specs for Slides**:
- Architecture diagram: Flat design system architecture PNG, clean white background, no text labels
- Process flow: Minimalist 5-step flowchart PNG, professional style
- Background: Subtle geometric pattern PNG, low contrast for text overlay
- Icon set: 4 business icons PNG (innovation, teamwork, growth, technology), consistent style

#### Image Layout Patterns

**Image Placement Approaches**:
1. **Full-bleed background**: Image covers entire slide with text overlay
   - Use semi-transparent overlay (rgba) for text readability
   - Position text in areas with lower visual complexity
   
2. **Two-column (Image + Text)**: Most versatile layout
   - Image: 40-60% of slide width
   - Text: remaining space with adequate margins
   - Variations: image left/right, equal or unequal splits
   
3. **Image accent**: Small image as visual anchor
   - Corner placement (top-right, bottom-left common)
   - Size: 15-25% of slide area
   - Use for icons, logos, or supporting graphics
   
4. **Image grid**: Multiple images in organized layout
   - 2x2 or 3x2 grids for comparison or gallery views
   - Equal spacing between images
   - Consistent image dimensions within grid
   
5. **Hero image with caption**: Large central image
   - Image: 60-80% of slide height
   - Caption below or overlay at bottom
   - Ideal for showcasing products, screenshots, diagrams

**Image Sizing Recommendations**:
- **Full-slide background**: Match slide dimensions (720pt x 405pt for 16:9)
- **Half-slide image**: 360pt x 405pt (portrait) or 720pt x 200pt (landscape banner)
- **Quarter-slide image**: 350pt x 200pt
- **Icon/thumbnail**: 50-100pt x 50-100pt
- Always maintain aspect ratio to avoid distortion
- Leave 20-30pt margins from slide edges

**Text-Image Coordination**:
- Ensure sufficient contrast between text and image backgrounds
- Use text shadows or backdrop shapes when placing text over images
- Align text blocks to image edges for visual coherence
- Match text color to accent colors in the image

### Layout Strategies
**When creating slides with charts or tables:**
- **Two-column layout (PREFERRED)**: Use a header spanning the full width, then two columns below - text/bullets in one column and the featured content in the other. This provides better balance and makes charts/tables more readable. Use flexbox with unequal column widths (e.g., 40%/60% split) to optimize space for each content type.
- **Full-slide layout**: Let the featured content (chart/table) take up the entire slide for maximum impact and readability
- **NEVER vertically stack**: Do not place charts/tables below text in a single column - this causes poor readability and layout issues

### Process
1. **REQUIRED - READ COMPLETE FILE**: Read [`slide-generator.md`](slide-generator.md) entirely from start to finish. **NEVER set any range limits when reading this file.** Read the full file content for detailed syntax, critical formatting rules, and best practices before proceeding with presentation creation.
2. Create an HTML file for each slide with proper dimensions (e.g., 720pt x 405pt for 16:9)
   - Use `<p>`, `<h1>`-`<h6>`, `<ul>`, `<ol>` for all text content
   - Use `class="placeholder"` for areas where charts/tables will be added (render with gray background for visibility)
   - **CRITICAL**: Rasterize gradients and icons as PNG images FIRST using Sharp, then reference in HTML
   - **LAYOUT**: For slides with charts/tables/images, use either full-slide layout or two-column layout for better readability
3. Create and run a JavaScript file using the [`slideConverter.js`](scripts/slideConverter.js) library to convert HTML slides to PowerPoint and save the presentation
   - Use the `convertSlide()` function to process each HTML file
   - Add charts and tables to placeholder areas using PptxGenJS API
   - Save the presentation using `pptx.writeFile()`
4. **Visual validation**: Generate thumbnails and inspect for layout issues
   - Create thumbnail grid: `python scripts/slidePreview.py output.pptx workspace/thumbnails --cols 4`
   - Read and carefully examine the thumbnail image for:
     - **Text cutoff**: Text being cut off by header bars, shapes, or slide edges
     - **Text overlap**: Text overlapping with other text or shapes
     - **Positioning issues**: Content too close to slide boundaries or other elements
     - **Contrast issues**: Insufficient contrast between text and backgrounds
   - If issues found, adjust HTML margins/spacing/colors and regenerate the presentation
   - Repeat until all slides are visually correct

## Modifying an Existing Presentation

When editing slides in an existing PowerPoint presentation, work with the raw Office Open XML (OOXML) format. This involves extracting the .pptx file, modifying the XML content, and repackaging it.

### Process
1. **REQUIRED - READ COMPLETE FILE**: Read [`openxml.md`](openxml.md) (~500 lines) entirely from start to finish. **NEVER set any range limits when reading this file.** Read the full file content for detailed guidance on OOXML structure and editing workflows before any presentation editing.
2. Extract the presentation: `python openxml/scripts/extract.py <office_file> <output_dir>`
3. Modify the XML files (primarily `ppt/slides/slide{N}.xml` and related files)
4. **ESSENTIAL**: Validate immediately after each edit and fix any validation errors before proceeding: `python openxml/scripts/check.py <dir> --original <file>`
5. Repackage the final presentation: `python openxml/scripts/bundle.py <input_directory> <office_file>`

## Building a New Presentation **Using a Template**

When you need to create a presentation that follows an existing template's design, duplicate and re-arrange template slides before replacing placeholder content.

### Process
1. **Extract template text AND create visual thumbnail grid**:
   * Extract text: `python -m markitdown template.pptx > template-content.md`
   * Read `template-content.md`: Read the entire file to understand the contents of the template presentation. **NEVER set any range limits when reading this file.**
   * Create thumbnail grids: `python scripts/slidePreview.py template.pptx`
   * See [Generating Thumbnail Grids](#generating-thumbnail-grids) section for more details

2. **Analyze template and save inventory to a file**:
   * **Visual Analysis**: Review thumbnail grid(s) to understand slide layouts, design patterns, and visual structure
   * Create and save a template inventory file at `template-inventory.md` containing:
     ```markdown
     # Template Inventory Analysis
     **Total Slides: [count]**
     **IMPORTANT: Slides are 0-indexed (first slide = 0, last slide = count-1)**

     ## [Category Name]
     - Slide 0: [Layout code if available] - Description/purpose
     - Slide 1: [Layout code] - Description/purpose
     - Slide 2: [Layout code] - Description/purpose
     [... EVERY slide must be listed individually with its index ...]
     ```
   * **Using the thumbnail grid**: Reference the visual thumbnails to identify:
     - Layout patterns (title slides, content layouts, section dividers)
     - Image placeholder locations and counts
     - Design consistency across slide groups
     - Visual hierarchy and structure
   * This inventory file is REQUIRED for selecting appropriate templates in the next step

3. **Create presentation outline based on template inventory**:
   * Review available templates from step 2.
   * Choose an intro or title template for the first slide. This should be one of the first templates.
   * Choose safe, text-based layouts for the other slides.
   * **ESSENTIAL: Match layout structure to actual content**:
     - Single-column layouts: Use for unified narrative or single topic
     - Two-column layouts: Use ONLY when you have exactly 2 distinct items/concepts
     - Three-column layouts: Use ONLY when you have exactly 3 distinct items/concepts
     - Image + text layouts: Use ONLY when you have actual images to insert
     - Quote layouts: Use ONLY for actual quotes from people (with attribution), never for emphasis
     - Never use layouts with more placeholders than you have content
     - If you have 2 items, don't force them into a 3-column layout
     - If you have 4+ items, consider breaking into multiple slides or using a list format
   * Count your actual content pieces BEFORE selecting the layout
   * Verify each placeholder in the chosen layout will be filled with meaningful content
   * Select one option representing the **best** layout for each content section.
   * Save `outline.md` with content AND template mapping that leverages available designs
   * Example template mapping:
      ```
      # Template slides to use (0-based indexing)
      # WARNING: Verify indices are within range! Template with 73 slides has indices 0-72
      # Mapping: slide numbers from outline -> template slide indices
      template_mapping = [
          0,   # Use slide 0 (Title/Cover)
          34,  # Use slide 34 (B1: Title and body)
          34,  # Use slide 34 again (duplicate for second B1)
          50,  # Use slide 50 (E1: Quote)
          54,  # Use slide 54 (F2: Closing + Text)
      ]
      ```

4. **Duplicate, reorder, and delete slides using `reorder.py`**:
   * Use the `scripts/reorder.py` script to create a new presentation with slides in the desired order:
     ```bash
     python scripts/reorder.py template.pptx working.pptx 0,34,34,50,52
     ```
   * The script handles duplicating repeated slides, deleting unused slides, and reordering automatically
   * Slide indices are 0-based (first slide is 0, second is 1, etc.)
   * The same slide index can appear multiple times to duplicate that slide

5. **Extract ALL text using the `textExtractor.py` script**:
   * **Run inventory extraction**:
     ```bash
     python scripts/textExtractor.py working.pptx text-inventory.json
     ```
   * **Read text-inventory.json**: Read the entire text-inventory.json file to understand all shapes and their properties. **NEVER set any range limits when reading this file.**

   * The inventory JSON structure:
      ```json
        {
          "slide-0": {
            "shape-0": {
              "placeholder_type": "TITLE",  // or null for non-placeholders
              "left": 1.5,                  // position in inches
              "top": 2.0,
              "width": 7.5,
              "height": 1.2,
              "paragraphs": [
                {
                  "text": "Paragraph text",
                  // Optional properties (only included when non-default):
                  "bullet": true,           // explicit bullet detected
                  "level": 0,               // only included when bullet is true
                  "alignment": "CENTER",    // CENTER, RIGHT (not LEFT)
                  "space_before": 10.0,     // space before paragraph in points
                  "space_after": 6.0,       // space after paragraph in points
                  "line_spacing": 22.4,     // line spacing in points
                  "font_name": "Arial",     // from first run
                  "font_size": 14.0,        // in points
                  "bold": true,
                  "italic": false,
                  "underline": false,
                  "color": "FF0000"         // RGB color
                }
              ]
            }
          }
        }
      ```

   * Key features:
     - **Slides**: Named as "slide-0", "slide-1", etc.
     - **Shapes**: Ordered by visual position (top-to-bottom, left-to-right) as "shape-0", "shape-1", etc.
     - **Placeholder types**: TITLE, CENTER_TITLE, SUBTITLE, BODY, OBJECT, or null
     - **Default font size**: `default_font_size` in points extracted from layout placeholders (when available)
     - **Slide numbers are filtered**: Shapes with SLIDE_NUMBER placeholder type are automatically excluded from inventory
     - **Bullets**: When `bullet: true`, `level` is always included (even if 0)
     - **Spacing**: `space_before`, `space_after`, and `line_spacing` in points (only included when set)
     - **Colors**: `color` for RGB (e.g., "FF0000"), `theme_color` for theme colors (e.g., "DARK_1")
     - **Properties**: Only non-default values are included in the output

6. **Generate replacement text and save the data to a JSON file**
   Based on the text inventory from the previous step:
   - **ESSENTIAL**: First verify which shapes exist in the inventory - only reference shapes that are actually present
   - **VALIDATION**: The textReplacer.py script will validate that all shapes in your replacement JSON exist in the inventory
     - If you reference a non-existent shape, you'll get an error showing available shapes
     - If you reference a non-existent slide, you'll get an error indicating the slide doesn't exist
     - All validation errors are shown at once before the script exits
   - **NOTE**: The textReplacer.py script uses textExtractor.py internally to identify ALL text shapes
   - **AUTOMATIC CLEARING**: ALL text shapes from the inventory will be cleared unless you provide "paragraphs" for them
   - Add a "paragraphs" field to shapes that need content (not "replacement_paragraphs")
   - Shapes without "paragraphs" in the replacement JSON will have their text cleared automatically
   - Paragraphs with bullets will be automatically left aligned. Don't set the `alignment` property when `"bullet": true`
   - Generate appropriate replacement content for placeholder text
   - Use shape size to determine appropriate content length
   - **ESSENTIAL**: Include paragraph properties from the original inventory - don't just provide text
   - **NOTE**: When bullet: true, do NOT include bullet symbols in text - they're added automatically
   - **FORMATTING GUIDELINES**:
     - Headers/titles should typically have `"bold": true`
     - List items should have `"bullet": true, "level": 0` (level is required when bullet is true)
     - Preserve any alignment properties (e.g., `"alignment": "CENTER"` for centered text)
     - Include font properties when different from default (e.g., `"font_size": 14.0`, `"font_name": "Lora"`)
     - Colors: Use `"color": "FF0000"` for RGB or `"theme_color": "DARK_1"` for theme colors
     - The replacement script expects **properly formatted paragraphs**, not just text strings
     - **Overlapping shapes**: Prefer shapes with larger default_font_size or more appropriate placeholder_type
   - Save the updated inventory with replacements to `replacement-text.json`
   - **CAUTION**: Different template layouts have different shape counts - always check the actual inventory before creating replacements

   Example paragraphs field showing proper formatting:
   ```json
   "paragraphs": [
     {
       "text": "New presentation title text",
       "alignment": "CENTER",
       "bold": true
     },
     {
       "text": "Section Header",
       "bold": true
     },
     {
       "text": "First bullet point without bullet symbol",
       "bullet": true,
       "level": 0
     },
     {
       "text": "Red colored text",
       "color": "FF0000"
     },
     {
       "text": "Theme colored text",
       "theme_color": "DARK_1"
     },
     {
       "text": "Regular paragraph text without special formatting"
     }
   ]
   ```

   **Shapes not listed in the replacement JSON are automatically cleared**:
   ```json
   {
     "slide-0": {
       "shape-0": {
         "paragraphs": [...] // This shape gets new text
       }
       // shape-1 and shape-2 from inventory will be cleared automatically
     }
   }
   ```

   **Common formatting patterns for presentations**:
   - Title slides: Bold text, sometimes centered
   - Section headers within slides: Bold text
   - Bullet lists: Each item needs `"bullet": true, "level": 0`
   - Body text: Usually no special properties needed
   - Quotes: May have special alignment or font properties

7. **Apply replacements using the `textReplacer.py` script**
   ```bash
   python scripts/textReplacer.py working.pptx replacement-text.json output.pptx
   ```

   The script will:
   - First extract the inventory of ALL text shapes using functions from textExtractor.py
   - Validate that all shapes in the replacement JSON exist in the inventory
   - Clear text from ALL shapes identified in the inventory
   - Apply new text only to shapes with "paragraphs" defined in the replacement JSON
   - Preserve formatting by applying paragraph properties from the JSON
   - Handle bullets, alignment, font properties, and colors automatically
   - Save the updated presentation

   Example validation errors:
   ```
   ERROR: Invalid shapes in replacement JSON:
     - Shape 'shape-99' not found on 'slide-0'. Available shapes: shape-0, shape-1, shape-4
     - Slide 'slide-999' not found in inventory
   ```

   ```
   ERROR: Replacement text made overflow worse in these shapes:
     - slide-0/shape-2: overflow worsened by 1.25" (was 0.00", now 1.25")
   ```

## Generating Thumbnail Grids

To create visual thumbnail grids of PowerPoint slides for quick analysis and reference:

```bash
python scripts/slidePreview.py template.pptx [output_prefix]
```

**Capabilities**:
- Creates: `thumbnails.jpg` (or `thumbnails-1.jpg`, `thumbnails-2.jpg`, etc. for large decks)
- Default: 5 columns, max 30 slides per grid (5x6)
- Custom prefix: `python scripts/slidePreview.py template.pptx my-grid`
  - Note: The output prefix should include the path if you want output in a specific directory (e.g., `workspace/my-grid`)
- Adjust columns: `--cols 4` (range: 3-6, affects slides per grid)
- Grid limits: 3 cols = 12 slides/grid, 4 cols = 20, 5 cols = 30, 6 cols = 42
- Slides are zero-indexed (Slide 0, Slide 1, etc.)

**Use cases**:
- Template analysis: Quickly understand slide layouts and design patterns
- Content review: Visual overview of entire presentation
- Navigation reference: Find specific slides by their visual appearance
- Quality check: Verify all slides are properly formatted

**Examples**:
```bash
# Basic usage
python scripts/slidePreview.py presentation.pptx

# Combine options: custom name, columns
python scripts/slidePreview.py template.pptx analysis --cols 4
```

## Converting Slides to Images

To visually analyze PowerPoint slides, convert them to images using a two-step process:

1. **Convert PPTX to PDF**:
   ```bash
   soffice --headless --convert-to pdf template.pptx
   ```

2. **Convert PDF pages to JPEG images**:
   ```bash
   pdftoppm -jpeg -r 150 template.pdf slide
   ```
   This creates files like `slide-1.jpg`, `slide-2.jpg`, etc.

Options:
- `-r 150`: Sets resolution to 150 DPI (adjust for quality/size balance)
- `-jpeg`: Output JPEG format (use `-png` for PNG if preferred)
- `-f N`: First page to convert (e.g., `-f 2` starts from page 2)
- `-l N`: Last page to convert (e.g., `-l 5` stops at page 5)
- `slide`: Prefix for output files

Example for specific range:
```bash
pdftoppm -jpeg -r 150 -f 2 -l 5 template.pdf slide  # Converts only pages 2-5
```

## Code Style Guidelines
**CRITICAL**: When generating code for PPTX operations:
- Write concise code
- Avoid verbose variable names and redundant operations
- Avoid unnecessary print statements

## Dependencies

Required dependencies (should already be installed):

- **markitdown**: `pip install "markitdown[pptx]"` (for text extraction from presentations)
- **pptxgenjs**: `npm install -g pptxgenjs` (for creating presentations via slideConverter)
- **playwright**: `npm install -g playwright` (for HTML rendering in slideConverter)
- **react-icons**: `npm install -g react-icons react react-dom` (for icons)
- **sharp**: `npm install -g sharp` (for SVG rasterization and image processing)
- **LibreOffice**: `sudo apt-get install libreoffice` (for PDF conversion)
- **Poppler**: `sudo apt-get install poppler-utils` (for pdftoppm to convert PDF to images)
- **defusedxml**: `pip install defusedxml` (for secure XML parsing)
