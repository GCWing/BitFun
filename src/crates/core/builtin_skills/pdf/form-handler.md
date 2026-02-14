**IMPORTANT: Follow these steps sequentially. Do not proceed to code writing without completing earlier steps.**

When you need to complete a PDF form, first determine whether it contains interactive form fields. Execute this utility from this file's directory:
 `python utils/detect_interactive_fields.py <file.pdf>`, then proceed to either "Interactive Form Fields" or "Static Form Layout" sections based on the output.

# Interactive Form Fields
If the document contains interactive form fields:
- Execute from this file's directory: `python utils/parse_form_structure.py <input.pdf> <structure_info.json>`. This generates a JSON file listing all fields:
```
[
  {
    "element_id": (unique identifier for the field),
    "page_num": (page number, 1-indexed),
    "bounds": ([left, bottom, right, top] bounding box in PDF coordinates, y=0 is page bottom),
    "element_type": ("text_input", "toggle_box", "option_group", or "dropdown"),
  },
  // Toggle boxes include "on_value" and "off_value" properties:
  {
    "element_id": (unique identifier for the field),
    "page_num": (page number, 1-indexed),
    "element_type": "toggle_box",
    "on_value": (Use this value to activate the toggle),
    "off_value": (Use this value to deactivate the toggle),
  },
  // Option groups contain "available_options" array with selectable choices:
  {
    "element_id": (unique identifier for the field),
    "page_num": (page number, 1-indexed),
    "element_type": "option_group",
    "available_options": [
      {
        "option_value": (set field to this value to select this option),
        "bounds": (bounding box for this option's selector)
      },
      // Additional options
    ]
  },
  // Dropdown fields contain "menu_items" array:
  {
    "element_id": (unique identifier for the field),
    "page_num": (page number, 1-indexed),
    "element_type": "dropdown",
    "menu_items": [
      {
        "option_value": (set field to this value to select this item),
        "display_text": (visible text of the item)
      },
      // Additional menu items
    ],
  }
]
```
- Generate page images (one PNG per page) with this utility (run from this file's directory):
`python utils/render_pages_to_png.py <file.pdf> <output_folder>`
Then examine the images to understand each field's purpose (convert bounding box PDF coordinates to image coordinates as needed).
- Create a `form_data.json` file with values for each field:
```
[
  {
    "element_id": "surname", // Must match element_id from `parse_form_structure.py`
    "description": "Family name of the applicant",
    "page_num": 1, // Must match "page_num" from structure_info.json
    "fill_value": "Johnson"
  },
  {
    "element_id": "Checkbox12",
    "description": "Toggle to mark if applicant is an adult",
    "page_num": 1,
    "fill_value": "/On" // For toggles, use "on_value" to activate. For option groups, use "option_value" from available_options.
  },
  // additional fields
]
```
- Execute the `populate_interactive_form.py` utility from this file's directory to generate the completed PDF:
`python utils/populate_interactive_form.py <input pdf> <form_data.json> <output pdf>`
This utility validates field IDs and values; if errors appear, correct them and retry.

# Static Form Layout
For PDFs without interactive form fields, you must visually identify data entry locations and create text overlays. Execute these steps *precisely*. All steps are mandatory for accurate form completion. Detailed instructions follow.
- Render the PDF as PNG images and determine field positioning.
- Create a JSON configuration with field data and generate overlay preview images.
- Validate the positioning.
- Apply the text overlays to complete the form.

## Step 1: Visual Inspection (MANDATORY)
- Render the PDF as PNG images. Execute from this file's directory:
`python utils/render_pages_to_png.py <file.pdf> <output_folder>`
This creates one PNG per page.
- Carefully review each image and locate all data entry areas. For each field, determine bounding boxes for both the field label and the data entry area. These boxes MUST NOT OVERLAP; the entry area should only cover the space for data input. Typically, entry areas are positioned adjacent to, above, or below their labels. Entry boxes must accommodate the expected text.

Common form patterns you may encounter:

*Label within bordered area*
```
+------------------------+
| Full Name:             |
+------------------------+
```
The data entry area should be to the right of "Full Name" and extend to the border.

*Label preceding underline*
```
Email: _______________________
```
The data entry area should be above the underline spanning its width.

*Label below underline*
```
_________________________
Signature
```
The data entry area should be above the underline across its full width. Common for signatures and dates.

*Label above underline*
```
Additional comments:
________________________________________________
```
The data entry area extends from below the label to the underline, spanning its width.

*Toggle boxes*
```
Are you employed? Yes []  No []
```
For toggle boxes:
- Identify small square markers ([]) - these are the target elements. They may appear before or after their labels.
- Distinguish between label text ("Yes", "No") and the actual toggle squares.
- The entry bounding box should cover ONLY the square marker, not the label text.

### Step 2: Create form_config.json and preview images (MANDATORY)
- Create `form_config.json` with field positioning data:
```
{
  "page_dimensions": [
    {
      "page_num": 1,
      "img_width": (page 1 image width in pixels),
      "img_height": (page 1 image height in pixels),
    },
    {
      "page_num": 2,
      "img_width": (page 2 image width in pixels),
      "img_height": (page 2 image height in pixels),
    }
    // more pages
  ],
  "field_entries": [
    // Text field example
    {
      "page_num": 1,
      "description": "Enter applicant's surname here",
      // Bounding boxes use [left, top, right, bottom] format. Label and entry boxes must not overlap.
      "label_text": "Last name",
      "label_bounds": [30, 125, 95, 142],
      "entry_bounds": [100, 125, 280, 142],
      "text_content": {
        "content": "Smith", // Text to overlay at entry_bounds location
        "text_size": 14, // optional, defaults to 14
        "text_color": "000000", // optional, RRGGBB format, defaults to 000000 (black)
      }
    },
    // Toggle box example. TARGET THE SQUARE MARKER, NOT THE TEXT
    {
      "page_num": 2,
      "description": "Mark if applicant is over 21",
      "entry_bounds": [140, 525, 155, 540],  // Small area over toggle square
      "label_text": "Yes",
      "label_bounds": [100, 525, 132, 540],  // Area containing "Yes" label
      // Use "X" to mark a toggle box.
      "text_content": {
        "content": "X",
      }
    }
    // more field entries
  ]
}
```

Generate preview images by running this utility from this file's directory for each page:
`python utils/generate_preview_overlay.py <page_number> <path_to_form_config.json> <source_image_path> <preview_image_path>

Preview images display red rectangles for data entry areas and blue rectangles for label areas.

### Step 3: Verify Positioning (MANDATORY)
#### Automated overlap detection
- Verify no bounding boxes overlap and entry boxes have sufficient height using the `verify_form_layout.py` utility (run from this file's directory):
`python utils/verify_form_layout.py <JSON file>`

If errors occur, re-examine the affected fields, adjust positioning, and iterate until all errors are resolved. Remember: label (blue) boxes contain text labels; entry (red) boxes should not.

#### Manual preview inspection
**CRITICAL: Do not continue without visually reviewing preview images**
- Red rectangles must cover ONLY data entry areas
- Red rectangles MUST NOT contain any existing text
- Blue rectangles should encompass label text
- For toggle boxes:
  - Red rectangle MUST be centered on the toggle square
  - Blue rectangle should cover the toggle's text label

- If any positioning appears incorrect, update form_config.json, regenerate previews, and verify again. Repeat until all positioning is accurate.


### Step 4: Apply text overlays to the PDF
Execute this utility from this file's directory to generate the completed PDF using form_config.json:
`python utils/apply_text_overlays.py <input_pdf_path> <path_to_form_config.json> <output_pdf_path>

# CJK (Chinese/Japanese/Korean) Font Support

## Important: CJK Text Display Issues

**Warning**: Standard PDF fonts (Arial, Helvetica, etc.) do not support CJK characters. Without a proper CJK font, Chinese/Japanese/Korean text will display as black boxes (■).

The `apply_text_overlays.py` utility:
1. Automatically detects CJK characters in your text content
2. Searches for available CJK fonts on your system
3. **Exits with an error if CJK characters are detected but no CJK font is found**

## Supported System Fonts

The utility searches for CJK fonts in these locations:

**macOS:**
- `/System/Library/Fonts/PingFang.ttc` (PingFang) - pre-installed
- `/System/Library/Fonts/STHeiti Light.ttc` (STHeiti)
- `/Library/Fonts/Arial Unicode.ttf` (Arial Unicode)

**Windows:**
- `C:/Windows/Fonts/msyh.ttc` (Microsoft YaHei) - pre-installed
- `C:/Windows/Fonts/simsun.ttc` (SimSun)
- `C:/Windows/Fonts/simhei.ttf` (SimHei)

**Linux:**
- `/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf`
- `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`
- `/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc`

## If You See "No CJK Font Found" Error

The script will exit with an error if CJK text is detected but no font is available. Install a CJK font:

```bash
# Ubuntu/Debian
sudo apt-get install fonts-noto-cjk
# or
sudo apt-get install fonts-wqy-zenhei

# Fedora/RHEL
sudo dnf install google-noto-sans-cjk-fonts

# macOS - PingFang is pre-installed, no action needed
# Windows - Microsoft YaHei is pre-installed, no action needed
```

You can also add a custom font path by modifying the `CJK_FONT_PATHS` dictionary in `apply_text_overlays.py`.

## Example with CJK Text

```json
{
  "page_dimensions": [{"page_num": 1, "img_width": 800, "img_height": 1000}],
  "field_entries": [
    {
      "page_num": 1,
      "description": "Applicant name in Chinese",
      "label_text": "姓名",
      "label_bounds": [30, 125, 70, 145],
      "entry_bounds": [80, 125, 280, 145],
      "text_content": {
        "content": "张三",
        "text_size": 14
      }
    }
  ]
}
```

The utility will automatically detect the Chinese characters and use an appropriate CJK font for rendering.
