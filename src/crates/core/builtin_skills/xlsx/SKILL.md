---
name: xlsx
description: "Advanced spreadsheet toolkit for content extraction, document generation, data manipulation, and formula processing. Use when you need to parse Excel data and formulas, create professional spreadsheets, handle complex formatting, or evaluate formula expressions programmatically."
description_zh: "高级电子表格工具包，用于内容提取、文档生成、数据操作和公式处理。当需要解析 Excel 数据和公式、创建专业电子表格、处理复杂格式或以编程方式计算公式表达式时使用。"
---

# Output Standards

## General Excel Requirements

### Formula Integrity
- All Excel deliverables MUST contain ZERO formula errors (#REF!, #DIV/0!, #VALUE!, #N/A, #NAME?)

### Template Preservation (for existing files)
- Carefully match existing formatting, styling, and conventions when editing files
- Never override established patterns with standardized formatting
- Existing file conventions take precedence over these guidelines

## Financial Spreadsheet Standards

### Color Conventions
Unless specified by user or existing template conventions

#### Standard Color Coding
- **Blue text (RGB: 0,0,255)**: Input values, scenario parameters
- **Black text (RGB: 0,0,0)**: All formula cells and computed values
- **Green text (RGB: 0,128,0)**: Cross-sheet references within workbook
- **Red text (RGB: 255,0,0)**: External file references
- **Yellow background (RGB: 255,255,0)**: Key assumptions or cells requiring updates

### Numeric Formatting

#### Formatting Guidelines
- **Years**: Format as text (e.g., "2024" not "2,024")
- **Currency**: Apply $#,##0 format; specify units in headers ("Revenue ($mm)")
- **Zeros**: Display all zeros as "-", including percentages (e.g., "$#,##0;($#,##0);-")
- **Percentages**: Use 0.0% format (single decimal) as default
- **Multiples**: Apply 0.0x format for valuation metrics (EV/EBITDA, P/E)
- **Negative values**: Use parentheses (123) instead of minus -123

### Formula Guidelines

#### Assumptions Organization
- Position ALL assumptions (growth rates, margins, multiples) in dedicated assumption cells
- Reference cells instead of embedding hardcoded values in formulas
- Example: Use =B5*(1+$B$6) rather than =B5*1.05

#### Error Prevention
- Validate all cell references
- Check for off-by-one range errors
- Maintain consistent formulas across projection periods
- Test with edge cases (zeros, negatives, large values)
- Avoid unintended circular references

#### Documentation for Hardcoded Values
- Add comments or adjacent cells with format: "Source: [System/Document], [Date], [Reference], [URL if applicable]"
- Examples:
  - "Source: Company 10-K, FY2024, Page 45, Revenue Note, [SEC EDGAR URL]"
  - "Source: Company 10-Q, Q2 2025, Exhibit 99.1, [SEC EDGAR URL]"
  - "Source: Bloomberg Terminal, 8/15/2025, AAPL US Equity"
  - "Source: FactSet, 8/20/2025, Consensus Estimates Screen"

# Spreadsheet Operations

## Overview

Users may request creation, modification, or analysis of .xlsx files. Different tools and approaches are available for various tasks.

## Prerequisites

**LibreOffice Required for Formula Evaluation**: LibreOffice must be available for evaluating formula values using the `formula_processor.py` script. The script handles LibreOffice configuration automatically on first execution

## Data Analysis

### Using pandas for Analysis
For data analysis, visualization, and bulk operations, leverage **pandas**:

```python
import pandas as pd

# Load Excel
df = pd.read_excel('file.xlsx')  # Default: first sheet
sheets_dict = pd.read_excel('file.xlsx', sheet_name=None)  # All sheets as dictionary

# Analyze
df.head()      # Preview rows
df.info()      # Column details
df.describe()  # Summary statistics

# Export Excel
df.to_excel('output.xlsx', index=False)
```

## Spreadsheet Workflows

## CRITICAL: Formulas Over Hardcoded Values

**Always prefer Excel formulas instead of Python-calculated hardcoded values.** This maintains spreadsheet dynamism and editability.

### Incorrect - Hardcoded Calculations
```python
# Avoid: Python calculation with hardcoded result
total = df['Sales'].sum()
sheet['B10'] = total  # Hardcodes 5000

# Avoid: Computing growth in Python
growth = (df.iloc[-1]['Revenue'] - df.iloc[0]['Revenue']) / df.iloc[0]['Revenue']
sheet['C5'] = growth  # Hardcodes 0.15

# Avoid: Python average
avg = sum(values) / len(values)
sheet['D20'] = avg  # Hardcodes 42.5
```

### Correct - Excel Formulas
```python
# Preferred: Excel performs the sum
sheet['B10'] = '=SUM(B2:B9)'

# Preferred: Growth formula in Excel
sheet['C5'] = '=(C4-C2)/C2'

# Preferred: Excel average function
sheet['D20'] = '=AVERAGE(D2:D19)'
```

This principle applies to ALL calculations - totals, percentages, ratios, differences. The spreadsheet should recalculate when source data changes.

## Standard Workflow
1. **Select library**: pandas for data work, openpyxl for formulas/formatting
2. **Initialize**: Create new workbook or open existing file
3. **Modify**: Add/update data, formulas, and formatting
4. **Save**: Write to file
5. **Evaluate formulas (REQUIRED WHEN USING FORMULAS)**: Run the formula_processor.py script
   ```bash
   python formula_processor.py output.xlsx
   ```
6. **Review and correct errors**: 
   - Script returns JSON with error information
   - If `status` is `errors_detected`, check `error_breakdown` for specific error types and locations
   - Correct identified errors and re-evaluate
   - Common error types:
     - `#REF!`: Invalid cell references
     - `#DIV/0!`: Division by zero
     - `#VALUE!`: Type mismatch in formula
     - `#NAME?`: Unknown formula name

### Creating Spreadsheets

```python
# Using openpyxl for formulas and formatting
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill, Alignment

wb = Workbook()
sheet = wb.active

# Add data
sheet['A1'] = 'Hello'
sheet['B1'] = 'World'
sheet.append(['Row', 'of', 'data'])

# Add formula
sheet['B2'] = '=SUM(A1:A10)'

# Formatting
sheet['A1'].font = Font(bold=True, color='FF0000')
sheet['A1'].fill = PatternFill('solid', start_color='FFFF00')
sheet['A1'].alignment = Alignment(horizontal='center')

# Column width
sheet.column_dimensions['A'].width = 20

wb.save('output.xlsx')
```

### Modifying Spreadsheets

```python
# Using openpyxl to preserve formulas and formatting
from openpyxl import load_workbook

# Open existing file
wb = load_workbook('existing.xlsx')
sheet = wb.active  # or wb['SheetName'] for specific sheet

# Iterate sheets
for sheet_name in wb.sheetnames:
    sheet = wb[sheet_name]
    print(f"Sheet: {sheet_name}")

# Update cells
sheet['A1'] = 'New Value'
sheet.insert_rows(2)  # Insert row at position 2
sheet.delete_cols(3)  # Delete column 3

# Add sheet
new_sheet = wb.create_sheet('NewSheet')
new_sheet['A1'] = 'Data'

wb.save('modified.xlsx')
```

## Formula Evaluation

Excel files created or modified by openpyxl contain formulas as text but not computed values. Use the provided `formula_processor.py` script to evaluate formulas:

```bash
python formula_processor.py <excel_file> [timeout_seconds]
```

Example:
```bash
python formula_processor.py output.xlsx 30
```

The script:
- Configures LibreOffice macro automatically on initial run
- Evaluates all formulas across all sheets
- Scans ALL cells for Excel errors (#REF!, #DIV/0!, etc.)
- Returns JSON with detailed error locations and counts
- Compatible with both Linux and macOS

## Formula Validation Checklist

Quick checks to ensure formulas function correctly:

### Essential Checks
- [ ] **Verify sample references**: Check 2-3 sample references pull correct values before building full model
- [ ] **Column mapping**: Confirm Excel columns match (e.g., column 64 = BL, not BK)
- [ ] **Row offset**: Remember Excel rows are 1-indexed (DataFrame row 5 = Excel row 6)

### Common Issues
- [ ] **NaN handling**: Check for null values with `pd.notna()`
- [ ] **Far-right columns**: FY data often in columns 50+ 
- [ ] **Multiple matches**: Search all occurrences, not just first
- [ ] **Division by zero**: Check denominators before using `/` in formulas (#DIV/0!)
- [ ] **Wrong references**: Verify all cell references point to intended cells (#REF!)
- [ ] **Cross-sheet references**: Use correct format (Sheet1!A1) for linking sheets

### Formula Testing Approach
- [ ] **Start small**: Test formulas on 2-3 cells before applying broadly
- [ ] **Verify dependencies**: Check all cells referenced in formulas exist
- [ ] **Test edge cases**: Include zero, negative, and very large values

### Understanding formula_processor.py Output
The script returns JSON with error details:
```json
{
  "status": "success",           // or "errors_detected"
  "error_count": 0,              // Total error count
  "formula_count": 42,           // Number of formulas in file
  "error_breakdown": {           // Only present if errors found
    "#REF!": {
      "count": 2,
      "cells": ["Sheet1!B5", "Sheet1!C10"]
    }
  }
}
```

## Best Practices

### Library Selection
- **pandas**: Optimal for data analysis, bulk operations, simple data export
- **openpyxl**: Optimal for complex formatting, formulas, Excel-specific features

### openpyxl Guidelines
- Cell indices are 1-based (row=1, column=1 refers to cell A1)
- Use `data_only=True` to read computed values: `load_workbook('file.xlsx', data_only=True)`
- **Warning**: Saving with `data_only=True` replaces formulas with values permanently
- For large files: Use `read_only=True` for reading or `write_only=True` for writing
- Formulas are preserved but not evaluated - use formula_processor.py to update values

### pandas Guidelines
- Specify data types to avoid inference issues: `pd.read_excel('file.xlsx', dtype={'id': str})`
- For large files, read specific columns: `pd.read_excel('file.xlsx', usecols=['A', 'C', 'E'])`
- Handle dates properly: `pd.read_excel('file.xlsx', parse_dates=['date_column'])`

## Code Style Guidelines
**IMPORTANT**: When generating Python code for Excel operations:
- Write minimal, concise Python code without unnecessary comments
- Avoid verbose variable names and redundant operations
- Avoid unnecessary print statements

**For Excel files themselves**:
- Add comments to cells with complex formulas or important assumptions
- Document data sources for hardcoded values
- Include notes for key calculations and model sections
