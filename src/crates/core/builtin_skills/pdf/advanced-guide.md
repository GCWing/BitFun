# PDF Document Toolkit - Advanced Guide

This document covers advanced PDF operations, detailed examples, and supplementary libraries beyond the main toolkit instructions.

## pypdfium2 Library

### Overview
pypdfium2 provides Python bindings for PDFium (Chromium's PDF engine). It excels at fast rendering, image conversion, and serves as an alternative to PyMuPDF.

### Render Pages to Images
```python
import pypdfium2 as pdfium
from PIL import Image

# Load document
doc = pdfium.PdfDocument("sample.pdf")

# Render first page
pg = doc[0]
bitmap = pg.render(
    scale=2.0,  # Higher DPI
    rotation=0  # No rotation
)

# Convert to PIL Image
img = bitmap.to_pil()
img.save("pg_1.png", "PNG")

# Process all pages
for idx, pg in enumerate(doc):
    bitmap = pg.render(scale=1.5)
    img = bitmap.to_pil()
    img.save(f"pg_{idx+1}.jpg", "JPEG", quality=90)
```

### Extract Text with pypdfium2
```python
import pypdfium2 as pdfium

doc = pdfium.PdfDocument("sample.pdf")
for idx, pg in enumerate(doc):
    content = pg.get_text()
    print(f"Page {idx+1} content length: {len(content)} chars")
```

## JavaScript Libraries

### pdf-lib

pdf-lib is a robust JavaScript library for creating and editing PDF documents across JavaScript environments.

#### Load and Edit Existing PDF
```javascript
import { PDFDocument } from 'pdf-lib';
import fs from 'fs';

async function editDocument() {
    // Load existing document
    const existingBytes = fs.readFileSync('source.pdf');
    const pdfDoc = await PDFDocument.load(existingBytes);

    // Get page count
    const totalPages = pdfDoc.getPageCount();
    console.log(`Document contains ${totalPages} pages`);

    // Append new page
    const newPg = pdfDoc.addPage([600, 400]);
    newPg.drawText('Added via pdf-lib', {
        x: 100,
        y: 300,
        size: 16
    });

    // Save changes
    const pdfBytes = await pdfDoc.save();
    fs.writeFileSync('edited.pdf', pdfBytes);
}
```

#### Generate Professional Documents from Scratch
```javascript
import { PDFDocument, rgb, StandardFonts } from 'pdf-lib';
import fs from 'fs';

async function generateDocument() {
    const pdfDoc = await PDFDocument.create();

    // Embed fonts
    const helvetica = await pdfDoc.embedFont(StandardFonts.Helvetica);
    const helveticaBold = await pdfDoc.embedFont(StandardFonts.HelveticaBold);

    // Create page
    const pg = pdfDoc.addPage([595, 842]); // A4 dimensions
    const { width, height } = pg.getSize();

    // Add styled text
    pg.drawText('Invoice #12345', {
        x: 50,
        y: height - 50,
        size: 18,
        font: helveticaBold,
        color: rgb(0.2, 0.2, 0.8)
    });

    // Add header background
    pg.drawRectangle({
        x: 40,
        y: height - 100,
        width: width - 80,
        height: 30,
        color: rgb(0.9, 0.9, 0.9)
    });

    // Add tabular data
    const rows = [
        ['Item', 'Qty', 'Price', 'Total'],
        ['Widget', '2', '$50', '$100'],
        ['Gadget', '1', '$75', '$75']
    ];

    let yPos = height - 150;
    rows.forEach(row => {
        let xPos = 50;
        row.forEach(cell => {
            pg.drawText(cell, {
                x: xPos,
                y: yPos,
                size: 12,
                font: helvetica
            });
            xPos += 120;
        });
        yPos -= 25;
    });

    const pdfBytes = await pdfDoc.save();
    fs.writeFileSync('generated.pdf', pdfBytes);
}
```

#### Advanced Document Combination
```javascript
import { PDFDocument } from 'pdf-lib';
import fs from 'fs';

async function combineDocuments() {
    // Create output document
    const combinedPdf = await PDFDocument.create();

    // Load source documents
    const doc1Bytes = fs.readFileSync('first.pdf');
    const doc2Bytes = fs.readFileSync('second.pdf');

    const doc1 = await PDFDocument.load(doc1Bytes);
    const doc2 = await PDFDocument.load(doc2Bytes);

    // Copy all pages from first document
    const doc1Pages = await combinedPdf.copyPages(doc1, doc1.getPageIndices());
    doc1Pages.forEach(pg => combinedPdf.addPage(pg));

    // Copy selected pages from second document (pages 0, 2, 4)
    const doc2Pages = await combinedPdf.copyPages(doc2, [0, 2, 4]);
    doc2Pages.forEach(pg => combinedPdf.addPage(pg));

    const combinedBytes = await combinedPdf.save();
    fs.writeFileSync('combined.pdf', combinedBytes);
}
```

### pdfjs-dist

PDF.js is Mozilla's JavaScript library for browser-based PDF rendering.

#### Basic Document Loading and Rendering
```javascript
import * as pdfjsLib from 'pdfjs-dist';

// Configure worker for performance
pdfjsLib.GlobalWorkerOptions.workerSrc = './pdf.worker.js';

async function displayDocument() {
    // Load document
    const loadingTask = pdfjsLib.getDocument('sample.pdf');
    const doc = await loadingTask.promise;

    console.log(`Loaded document with ${doc.numPages} pages`);

    // Get first page
    const pg = await doc.getPage(1);
    const viewport = pg.getViewport({ scale: 1.5 });

    // Render to canvas
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    canvas.height = viewport.height;
    canvas.width = viewport.width;

    const renderConfig = {
        canvasContext: ctx,
        viewport: viewport
    };

    await pg.render(renderConfig).promise;
    document.body.appendChild(canvas);
}
```

#### Extract Text with Position Data
```javascript
import * as pdfjsLib from 'pdfjs-dist';

async function extractTextContent() {
    const loadingTask = pdfjsLib.getDocument('sample.pdf');
    const doc = await loadingTask.promise;

    let fullContent = '';

    // Extract from all pages
    for (let i = 1; i <= doc.numPages; i++) {
        const pg = await doc.getPage(i);
        const textData = await pg.getTextContent();

        const pageContent = textData.items
            .map(item => item.str)
            .join(' ');

        fullContent += `\n--- Page ${i} ---\n${pageContent}`;

        // Get text with coordinates for advanced processing
        const textWithPositions = textData.items.map(item => ({
            text: item.str,
            x: item.transform[4],
            y: item.transform[5],
            width: item.width,
            height: item.height
        }));
    }

    console.log(fullContent);
    return fullContent;
}
```

#### Extract Annotations and Form Elements
```javascript
import * as pdfjsLib from 'pdfjs-dist';

async function extractAnnotations() {
    const loadingTask = pdfjsLib.getDocument('annotated.pdf');
    const doc = await loadingTask.promise;

    for (let i = 1; i <= doc.numPages; i++) {
        const pg = await doc.getPage(i);
        const annotations = await pg.getAnnotations();

        annotations.forEach(ann => {
            console.log(`Annotation type: ${ann.subtype}`);
            console.log(`Content: ${ann.contents}`);
            console.log(`Coordinates: ${JSON.stringify(ann.rect)}`);
        });
    }
}
```

## Advanced Shell Operations

### poppler-utils Advanced Features

#### Extract Text with Bounding Boxes
```bash
# Extract text with coordinate data (essential for structured processing)
pdftotext -bbox-layout sample.pdf result.xml

# The XML output contains precise coordinates for each text element
```

#### Advanced Image Conversion
```bash
# Convert to PNG with specific resolution
pdftoppm -png -r 300 sample.pdf output_prefix

# Convert page range at high resolution
pdftoppm -png -r 600 -f 1 -l 3 sample.pdf highres_pages

# Convert to JPEG with quality setting
pdftoppm -jpeg -jpegopt quality=85 -r 200 sample.pdf jpeg_output
```

#### Extract Embedded Images
```bash
# Extract all embedded images with metadata
pdfimages -j -p sample.pdf page_images

# List image info without extraction
pdfimages -list sample.pdf

# Extract images in original format
pdfimages -all sample.pdf images/img
```

### qpdf Advanced Features

#### Complex Page Manipulation
```bash
# Split document into page groups
qpdf --split-pages=3 source.pdf output_group_%02d.pdf

# Extract pages with complex range specifications
qpdf source.pdf --pages source.pdf 1,3-5,8,10-end -- extracted.pdf

# Combine specific pages from multiple documents
qpdf --empty --pages doc1.pdf 1-3 doc2.pdf 5-7 doc3.pdf 2,4 -- combined.pdf
```

#### Document Optimization and Repair
```bash
# Optimize for web streaming (linearize)
qpdf --linearize source.pdf optimized.pdf

# Remove unused objects and compress
qpdf --optimize-level=all source.pdf compressed.pdf

# Attempt to repair corrupted document structure
qpdf --check source.pdf
qpdf --fix-qdf damaged.pdf repaired.pdf

# Display detailed document structure for debugging
qpdf --show-all-pages source.pdf > structure.txt
```

#### Advanced Encryption
```bash
# Add password protection with specific permissions
qpdf --encrypt user_pass admin_pass 256 --print=none --modify=none -- source.pdf secured.pdf

# Check encryption status
qpdf --show-encryption secured.pdf

# Remove password protection (requires password)
qpdf --password=secret123 --decrypt secured.pdf unlocked.pdf
```

## Advanced Python Techniques

### pdfplumber Advanced Features

#### Extract Text with Precise Coordinates
```python
import pdfplumber

with pdfplumber.open("sample.pdf") as doc:
    pg = doc.pages[0]
    
    # Extract all text with coordinates
    chars = pg.chars
    for char in chars[:10]:  # First 10 characters
        print(f"Char: '{char['text']}' at x:{char['x0']:.1f} y:{char['y0']:.1f}")
    
    # Extract text by bounding box (left, top, right, bottom)
    region_text = pg.within_bbox((100, 100, 400, 200)).extract_text()
```

#### Advanced Table Extraction with Custom Settings
```python
import pdfplumber
import pandas as pd

with pdfplumber.open("complex_table.pdf") as doc:
    pg = doc.pages[0]
    
    # Extract tables with custom settings for complex layouts
    table_config = {
        "vertical_strategy": "lines",
        "horizontal_strategy": "lines",
        "snap_tolerance": 3,
        "intersection_tolerance": 15
    }
    tables = pg.extract_tables(table_config)
    
    # Visual debugging for table extraction
    img = pg.to_image(resolution=150)
    img.save("debug_layout.png")
```

### reportlab Advanced Features

#### Create Professional Reports with Tables
```python
from reportlab.platypus import SimpleDocTemplate, Table, TableStyle, Paragraph
from reportlab.lib.styles import getSampleStyleSheet
from reportlab.lib import colors

# Sample data
data = [
    ['Product', 'Q1', 'Q2', 'Q3', 'Q4'],
    ['Widgets', '120', '135', '142', '158'],
    ['Gadgets', '85', '92', '98', '105']
]

# Create document with table
doc = SimpleDocTemplate("report.pdf")
elements = []

# Add title
styles = getSampleStyleSheet()
title = Paragraph("Quarterly Sales Report", styles['Title'])
elements.append(title)

# Add table with advanced styling
table = Table(data)
table.setStyle(TableStyle([
    ('BACKGROUND', (0, 0), (-1, 0), colors.grey),
    ('TEXTCOLOR', (0, 0), (-1, 0), colors.whitesmoke),
    ('ALIGN', (0, 0), (-1, -1), 'CENTER'),
    ('FONTNAME', (0, 0), (-1, 0), 'Helvetica-Bold'),
    ('FONTSIZE', (0, 0), (-1, 0), 14),
    ('BOTTOMPADDING', (0, 0), (-1, 0), 12),
    ('BACKGROUND', (0, 1), (-1, -1), colors.beige),
    ('GRID', (0, 0), (-1, -1), 1, colors.black)
]))
elements.append(table)

doc.build(elements)
```

## Complex Workflows

### Extract Figures/Images from PDF

#### Method 1: Using pdfimages (fastest)
```bash
# Extract all images with original quality
pdfimages -all sample.pdf images/img
```

#### Method 2: Using pypdfium2 + Image Processing
```python
import pypdfium2 as pdfium
from PIL import Image
import numpy as np

def extract_figures(pdf_path, output_dir):
    doc = pdfium.PdfDocument(pdf_path)
    
    for page_num, pg in enumerate(doc):
        # Render high-resolution page
        bitmap = pg.render(scale=3.0)
        img = bitmap.to_pil()
        
        # Convert to numpy for processing
        img_array = np.array(img)
        
        # Simple figure detection (non-white regions)
        mask = np.any(img_array != [255, 255, 255], axis=2)
        
        # Find contours and extract bounding boxes
        # (This is simplified - real implementation would need more sophisticated detection)
        
        # Save detected figures
        # ... implementation depends on specific needs
```

### Batch Document Processing with Error Handling
```python
import os
import glob
from pypdf import PdfReader, PdfWriter
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def batch_process(input_dir, operation='merge'):
    pdf_files = glob.glob(os.path.join(input_dir, "*.pdf"))
    
    if operation == 'merge':
        output = PdfWriter()
        for pdf_file in pdf_files:
            try:
                doc = PdfReader(pdf_file)
                for pg in doc.pages:
                    output.add_page(pg)
                logger.info(f"Processed: {pdf_file}")
            except Exception as e:
                logger.error(f"Failed to process {pdf_file}: {e}")
                continue
        
        with open("batch_combined.pdf", "wb") as out_file:
            output.write(out_file)
    
    elif operation == 'extract_text':
        for pdf_file in pdf_files:
            try:
                doc = PdfReader(pdf_file)
                content = ""
                for pg in doc.pages:
                    content += pg.extract_text()
                
                output_file = pdf_file.replace('.pdf', '.txt')
                with open(output_file, 'w', encoding='utf-8') as f:
                    f.write(content)
                logger.info(f"Extracted text from: {pdf_file}")
                
            except Exception as e:
                logger.error(f"Failed to extract text from {pdf_file}: {e}")
                continue
```

### Advanced Page Cropping
```python
from pypdf import PdfWriter, PdfReader

doc = PdfReader("source.pdf")
output = PdfWriter()

# Crop page (left, bottom, right, top in points)
pg = doc.pages[0]
pg.mediabox.left = 50
pg.mediabox.bottom = 50
pg.mediabox.right = 550
pg.mediabox.top = 750

output.add_page(pg)
with open("cropped.pdf", "wb") as out_file:
    output.write(out_file)
```

## Performance Optimization Tips

### 1. For Large Documents
- Use streaming approaches instead of loading entire document in memory
- Use `qpdf --split-pages` for splitting large files
- Process pages individually with pypdfium2

### 2. For Text Extraction
- `pdftotext -bbox-layout` is fastest for plain text extraction
- Use pdfplumber for structured data and tables
- Avoid `pypdf.extract_text()` for very large documents

### 3. For Image Extraction
- `pdfimages` is much faster than rendering pages
- Use low resolution for previews, high resolution for final output

### 4. For Form Filling
- pdf-lib maintains form structure better than most alternatives
- Pre-validate form fields before processing

### 5. Memory Management
```python
# Process documents in chunks
def process_large_document(pdf_path, chunk_size=10):
    doc = PdfReader(pdf_path)
    total_pages = len(doc.pages)
    
    for start_idx in range(0, total_pages, chunk_size):
        end_idx = min(start_idx + chunk_size, total_pages)
        output = PdfWriter()
        
        for i in range(start_idx, end_idx):
            output.add_page(doc.pages[i])
        
        # Process chunk
        with open(f"chunk_{start_idx//chunk_size}.pdf", "wb") as out_file:
            output.write(out_file)
```

## Troubleshooting Common Issues

### Encrypted Documents
```python
# Handle password-protected documents
from pypdf import PdfReader

try:
    doc = PdfReader("secured.pdf")
    if doc.is_encrypted:
        doc.decrypt("password")
except Exception as e:
    print(f"Failed to decrypt: {e}")
```

### Corrupted Documents
```bash
# Use qpdf to repair
qpdf --check corrupted.pdf
qpdf --replace-input corrupted.pdf
```

### Text Extraction Issues
```python
# Fallback to OCR for scanned documents
import pytesseract
from pdf2image import convert_from_path

def extract_text_with_ocr(pdf_path):
    pages = convert_from_path(pdf_path)
    content = ""
    for idx, img in enumerate(pages):
        content += pytesseract.image_to_string(img)
    return content
```
