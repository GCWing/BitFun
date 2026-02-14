---
name: pdf
description: Advanced PDF document toolkit for content extraction, document generation, page manipulation, and interactive form processing. Use when you need to parse PDF text and tables, create professional documents, combine or split files, or complete fillable forms programmatically.
description_zh: 高级 PDF 文档工具包，支持内容提取、文档生成、页面操作和交互式表单处理。适用于解析 PDF 文本和表格、创建专业文档、合并或拆分文件，或以编程方式完成可填写表单。
---

# PDF Document Toolkit

## Introduction

This toolkit provides comprehensive PDF document operations using Python libraries and shell utilities. For advanced usage, JavaScript APIs, and detailed code samples, refer to advanced-guide.md. For filling PDF forms, consult form-handler.md and follow its workflow.

## Important: Post-Completion Verification

**After generating or modifying PDF files, ALWAYS verify the output for CJK text rendering issues:**

1. **Open the generated PDF** and visually inspect all text content
2. **Check for garbled characters** - Look for:
   - Black boxes (■) or rectangles instead of CJK characters
   - Question marks (?) or replacement characters (�)
   - Missing text where CJK content should appear
   - Incorrectly rendered or overlapping characters
3. **If issues are found**, refer to the "CJK (Chinese/Japanese/Korean) Text Support" section below for font configuration solutions

This verification step is critical when the PDF contains Chinese, Japanese, or Korean text.

## Getting Started

```python
from pypdf import PdfReader, PdfWriter

# Open a PDF document
doc = PdfReader("sample.pdf")
print(f"Total pages: {len(doc.pages)}")

# Gather text content
content = ""
for pg in doc.pages:
    content += pg.extract_text()
```

## Python Libraries

### pypdf - Core Operations

#### Combine Multiple PDFs
```python
from pypdf import PdfWriter, PdfReader

output = PdfWriter()
for pdf in ["first.pdf", "second.pdf", "third.pdf"]:
    doc = PdfReader(pdf)
    for pg in doc.pages:
        output.add_page(pg)

with open("combined.pdf", "wb") as out_file:
    output.write(out_file)
```

#### Separate PDF Pages
```python
doc = PdfReader("source.pdf")
for idx, pg in enumerate(doc.pages):
    output = PdfWriter()
    output.add_page(pg)
    with open(f"part_{idx+1}.pdf", "wb") as out_file:
        output.write(out_file)
```

#### Read Document Properties
```python
doc = PdfReader("sample.pdf")
props = doc.metadata
print(f"Title: {props.title}")
print(f"Author: {props.author}")
print(f"Subject: {props.subject}")
print(f"Creator: {props.creator}")
```

#### Rotate Document Pages
```python
doc = PdfReader("source.pdf")
output = PdfWriter()

pg = doc.pages[0]
pg.rotate(90)  # 90 degrees clockwise
output.add_page(pg)

with open("turned.pdf", "wb") as out_file:
    output.write(out_file)
```

### pdfplumber - Content Extraction

#### Extract Text with Layout
```python
import pdfplumber

with pdfplumber.open("sample.pdf") as doc:
    for pg in doc.pages:
        content = pg.extract_text()
        print(content)
```

#### Extract Tabular Data
```python
with pdfplumber.open("sample.pdf") as doc:
    for pg_num, pg in enumerate(doc.pages):
        data_tables = pg.extract_tables()
        for tbl_num, tbl in enumerate(data_tables):
            print(f"Table {tbl_num+1} on page {pg_num+1}:")
            for row in tbl:
                print(row)
```

#### Export Tables to Excel
```python
import pandas as pd

with pdfplumber.open("sample.pdf") as doc:
    collected_tables = []
    for pg in doc.pages:
        data_tables = pg.extract_tables()
        for tbl in data_tables:
            if tbl:  # Verify table is not empty
                df = pd.DataFrame(tbl[1:], columns=tbl[0])
                collected_tables.append(df)

# Merge all tables
if collected_tables:
    merged_df = pd.concat(collected_tables, ignore_index=True)
    merged_df.to_excel("tables_export.xlsx", index=False)
```

### reportlab - Document Generation

#### Create Simple PDF
```python
from reportlab.lib.pagesizes import letter
from reportlab.pdfgen import canvas

c = canvas.Canvas("greeting.pdf", pagesize=letter)
width, height = letter

# Insert text
c.drawString(100, height - 100, "Welcome!")
c.drawString(100, height - 120, "Generated using reportlab library")

# Draw a separator line
c.line(100, height - 140, 400, height - 140)

# Save document
c.save()
```

#### Generate Multi-Page Document
```python
from reportlab.lib.pagesizes import letter
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, PageBreak
from reportlab.lib.styles import getSampleStyleSheet

doc = SimpleDocTemplate("document.pdf", pagesize=letter)
styles = getSampleStyleSheet()
elements = []

# Add content
heading = Paragraph("Document Title", styles['Title'])
elements.append(heading)
elements.append(Spacer(1, 12))

body_text = Paragraph("This is the main content section. " * 20, styles['Normal'])
elements.append(body_text)
elements.append(PageBreak())

# Second page
elements.append(Paragraph("Section 2", styles['Heading1']))
elements.append(Paragraph("Content for the second section", styles['Normal']))

# Generate PDF
doc.build(elements)
```

## Shell Utilities

### pdftotext (poppler-utils)
```bash
# Convert to text
pdftotext source.pdf result.txt

# Preserve layout formatting
pdftotext -layout source.pdf result.txt

# Convert specific page range
pdftotext -f 1 -l 5 source.pdf result.txt  # Pages 1-5
```

### qpdf
```bash
# Merge documents
qpdf --empty --pages doc1.pdf doc2.pdf -- result.pdf

# Extract page range
qpdf source.pdf --pages . 1-5 -- subset1-5.pdf
qpdf source.pdf --pages . 6-10 -- subset6-10.pdf

# Rotate specific page
qpdf source.pdf result.pdf --rotate=+90:1  # Rotate page 1 by 90 degrees

# Decrypt protected PDF
qpdf --password=secret --decrypt protected.pdf unlocked.pdf
```

### pdftk (if available)
```bash
# Merge documents
pdftk doc1.pdf doc2.pdf cat output result.pdf

# Split into individual pages
pdftk source.pdf burst

# Rotate page
pdftk source.pdf rotate 1east output turned.pdf
```

## Common Operations

### OCR for Scanned Documents
```python
# Requires: pip install pytesseract pdf2image
import pytesseract
from pdf2image import convert_from_path

# Convert PDF pages to images
pages = convert_from_path('scanned.pdf')

# Process each page with OCR
content = ""
for idx, img in enumerate(pages):
    content += f"Page {idx+1}:\n"
    content += pytesseract.image_to_string(img)
    content += "\n\n"

print(content)
```

### Apply Watermark
```python
from pypdf import PdfReader, PdfWriter

# Load watermark (or create one)
watermark_page = PdfReader("stamp.pdf").pages[0]

# Apply to all pages
doc = PdfReader("sample.pdf")
output = PdfWriter()

for pg in doc.pages:
    pg.merge_page(watermark_page)
    output.add_page(pg)

with open("stamped.pdf", "wb") as out_file:
    output.write(out_file)
```

### Export Embedded Images
```bash
# Using pdfimages (poppler-utils)
pdfimages -j source.pdf img_prefix

# Outputs: img_prefix-000.jpg, img_prefix-001.jpg, etc.
```

### Add Document Password
```python
from pypdf import PdfReader, PdfWriter

doc = PdfReader("source.pdf")
output = PdfWriter()

for pg in doc.pages:
    output.add_page(pg)

# Set passwords
output.encrypt("user_pwd", "admin_pwd")

with open("secured.pdf", "wb") as out_file:
    output.write(out_file)
```

## Quick Reference Table

| Operation | Recommended Tool | Example |
|-----------|------------------|---------|
| Merge documents | pypdf | `output.add_page(pg)` |
| Split document | pypdf | One page per output file |
| Extract text | pdfplumber | `pg.extract_text()` |
| Extract tables | pdfplumber | `pg.extract_tables()` |
| Create documents | reportlab | Canvas or Platypus |
| Shell merge | qpdf | `qpdf --empty --pages ...` |
| OCR scanned docs | pytesseract | Convert to image first |
| Fill PDF forms | pdf-lib or pypdf (see form-handler.md) | See form-handler.md |

## CJK (Chinese/Japanese/Korean) Text Support

**Important**: Standard PDF fonts (Arial, Helvetica, etc.) do not support CJK characters. If CJK text is used without a proper CJK font, characters will display as black boxes (■).

### Automatic Font Detection

The `apply_text_overlays.py` utility automatically:
1. Detects CJK characters in your text content
2. Searches for available CJK fonts on your system
3. **Exits with an error if CJK characters are detected but no CJK font is found**

### Supported System Fonts

| OS | Font Paths |
|----|------------|
| macOS | `/System/Library/Fonts/PingFang.ttc`, `/System/Library/Fonts/STHeiti Light.ttc` |
| Windows | `C:/Windows/Fonts/msyh.ttc` (Microsoft YaHei), `C:/Windows/Fonts/simsun.ttc` |
| Linux | `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`, `/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc` |

### If You See "No CJK Font Found" Error

Install a CJK font for your operating system:

```bash
# Ubuntu/Debian
sudo apt-get install fonts-noto-cjk

# Fedora/RHEL
sudo dnf install google-noto-sans-cjk-fonts

# macOS - PingFang is pre-installed
# Windows - Microsoft YaHei is pre-installed
```

### Manual Font Registration (for reportlab)

When using reportlab directly, register a CJK font before drawing text:

```python
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont

# Register CJK font (example for macOS)
# Note: For TTC (TrueType Collection) files, specify subfontIndex parameter
pdfmetrics.registerFont(TTFont('PingFang', '/System/Library/Fonts/PingFang.ttc', subfontIndex=0))

# Use the font for CJK text
c.setFont('PingFang', 14)
c.drawString(100, 700, '你好世界')      # Chinese
c.drawString(100, 680, 'こんにちは')    # Japanese
c.drawString(100, 660, '안녕하세요')    # Korean
```

**Common subfontIndex values for TTC files:**
- PingFang.ttc: 0 (Regular), 1 (Medium), 2 (Semibold), etc.
- msyh.ttc: 0 (Regular), 1 (Bold)
- NotoSansCJK-Regular.ttc: varies by language variant

For detailed CJK font configuration, see form-handler.md.

## Additional Resources

- For pypdfium2 advanced usage, see advanced-guide.md
- For JavaScript libraries (pdf-lib), see advanced-guide.md
- For filling PDF forms, follow instructions in form-handler.md
- For troubleshooting tips, see advanced-guide.md
