import json
import os
import sys
import tempfile

from pypdf import PdfReader, PdfWriter
from pypdf.annotations import FreeText
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.lib.colors import HexColor


# Completes a PDF by adding text overlays defined in `form_config.json`. See form-handler.md.


# Common CJK font paths for different operating systems
CJK_FONT_PATHS = {
    # macOS
    "/System/Library/Fonts/PingFang.ttc": "PingFang",
    "/System/Library/Fonts/STHeiti Light.ttc": "STHeiti",
    "/Library/Fonts/Arial Unicode.ttf": "ArialUnicode",
    # Windows
    "C:/Windows/Fonts/msyh.ttc": "MicrosoftYaHei",
    "C:/Windows/Fonts/simsun.ttc": "SimSun",
    "C:/Windows/Fonts/simhei.ttf": "SimHei",
    # Linux
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf": "DroidSansFallback",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc": "NotoSansCJK",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc": "WenQuanYi",
}


def has_cjk_characters(text):
    """Check if text contains CJK (Chinese/Japanese/Korean) characters"""
    for char in text:
        code = ord(char)
        # CJK Unified Ideographs and common CJK ranges
        if (0x4E00 <= code <= 0x9FFF or    # CJK Unified Ideographs
            0x3400 <= code <= 0x4DBF or    # CJK Unified Ideographs Extension A
            0x3000 <= code <= 0x303F or    # CJK Symbols and Punctuation
            0xFF00 <= code <= 0xFFEF or    # Halfwidth and Fullwidth Forms
            0x3040 <= code <= 0x309F or    # Hiragana
            0x30A0 <= code <= 0x30FF or    # Katakana
            0xAC00 <= code <= 0xD7AF):     # Hangul Syllables
            return True
    return False


def find_cjk_font():
    """Find an available CJK font on the system"""
    for font_path, font_name in CJK_FONT_PATHS.items():
        if os.path.exists(font_path):
            return font_path, font_name
    return None, None


def register_cjk_font():
    """Register a CJK font with reportlab if available"""
    font_path, font_name = find_cjk_font()
    if font_path:
        try:
            pdfmetrics.registerFont(TTFont(font_name, font_path))
            return font_name
        except Exception as e:
            print(f"警告: 注册 CJK 字体 {font_name} 失败: {e}")
    return None


def convert_image_to_pdf_coords(bbox, img_width, img_height, pdf_width, pdf_height):
    """Transform bounding box from image coordinates to PDF coordinates"""
    # Image coordinates: origin at top-left, y increases downward
    # PDF coordinates: origin at bottom-left, y increases upward
    x_scale = pdf_width / img_width
    y_scale = pdf_height / img_height
    
    left = bbox[0] * x_scale
    right = bbox[2] * x_scale
    
    # Flip Y coordinates for PDF
    top = pdf_height - (bbox[1] * y_scale)
    bottom = pdf_height - (bbox[3] * y_scale)
    
    return left, bottom, right, top


def apply_text_overlays(input_pdf_path, config_json_path, output_pdf_path):
    """Apply text overlays to PDF based on form_config.json"""
    
    # `form_config.json` format described in form-handler.md.
    with open(config_json_path, "r") as f:
        config = json.load(f)
    
    # Open the PDF
    reader = PdfReader(input_pdf_path)
    writer = PdfWriter()
    
    # Copy all pages to writer
    writer.append(reader)
    
    # Get PDF dimensions for each page
    pdf_dims = {}
    for idx, pg in enumerate(reader.pages):
        mediabox = pg.mediabox
        pdf_dims[idx + 1] = [float(mediabox.width), float(mediabox.height)]
    
    # Check if any text contains CJK characters
    has_cjk = False
    for entry in config["field_entries"]:
        if "text_content" in entry and "content" in entry["text_content"]:
            if has_cjk_characters(entry["text_content"]["content"]):
                has_cjk = True
                break
    
    # If CJK text detected, use reportlab method for proper font embedding
    if has_cjk:
        cjk_font_name = register_cjk_font()
        if cjk_font_name:
            print(f"检测到中日韩文字，使用嵌入字体: {cjk_font_name}")
            apply_text_overlays_with_reportlab(
                reader, writer, config, pdf_dims, cjk_font_name, output_pdf_path
            )
            return
        else:
            print("错误: 检测到中日韩文字，但系统中未找到 CJK 字体。")
            print("中文/日文/韩文将显示为方块（■）。")
            print("")
            print("请安装 CJK 字体后重试:")
            print("  macOS: 系统已预装 PingFang 字体")
            print("  Windows: 系统已预装 Microsoft YaHei 字体")
            print("  Linux: sudo apt-get install fonts-noto-cjk")
            print("")
            print("支持的字体路径:")
            for path, name in CJK_FONT_PATHS.items():
                print(f"  - {path} ({name})")
            sys.exit(1)
    
    # Process each field entry using standard FreeText annotation
    overlay_annotations = []
    for entry in config["field_entries"]:
        page_num = entry["page_num"]
        
        # Get page dimensions and transform coordinates.
        page_info = next(p for p in config["page_dimensions"] if p["page_num"] == page_num)
        img_width = page_info["img_width"]
        img_height = page_info["img_height"]
        pdf_width, pdf_height = pdf_dims[page_num]
        
        transformed_bounds = convert_image_to_pdf_coords(
            entry["entry_bounds"],
            img_width, img_height,
            pdf_width, pdf_height
        )
        
        # Skip empty fields
        if "text_content" not in entry or "content" not in entry["text_content"]:
            continue
        text_content = entry["text_content"]
        content = text_content["content"]
        if not content:
            continue
        
        font_name = text_content.get("font", "Arial")
        text_size = str(text_content.get("text_size", 14)) + "pt"
        text_color = text_content.get("text_color", "000000")

        # Font size/color may not render consistently across viewers:
        # https://github.com/py-pdf/pypdf/issues/2084
        annotation = FreeText(
            text=content,
            rect=transformed_bounds,
            font=font_name,
            font_size=text_size,
            font_color=text_color,
            border_color=None,
            background_color=None,
        )
        overlay_annotations.append(annotation)
        # page_num is 0-based for pypdf
        writer.add_annotation(page_number=page_num - 1, annotation=annotation)
        
    # Save the completed PDF
    with open(output_pdf_path, "wb") as out_file:
        writer.write(out_file)
    
    print(f"成功添加文本叠加层并保存到 {output_pdf_path}")
    print(f"共添加 {len(overlay_annotations)} 个文本叠加")


def apply_text_overlays_with_reportlab(reader, writer, config, pdf_dims, cjk_font_name, output_pdf_path):
    """Apply text overlays using reportlab for proper CJK font embedding"""
    
    # Group entries by page
    entries_by_page = {}
    for entry in config["field_entries"]:
        if "text_content" in entry and "content" in entry["text_content"]:
            content = entry["text_content"]["content"]
            if content:
                page_num = entry["page_num"]
                if page_num not in entries_by_page:
                    entries_by_page[page_num] = []
                entries_by_page[page_num].append(entry)
    
    total_overlays = 0
    
    # Create overlay PDF for each page with text
    for page_num, entries in entries_by_page.items():
        pdf_width, pdf_height = pdf_dims[page_num]
        page_info = next(p for p in config["page_dimensions"] if p["page_num"] == page_num)
        img_width = page_info["img_width"]
        img_height = page_info["img_height"]
        
        # Create temporary overlay PDF using reportlab
        with tempfile.NamedTemporaryFile(suffix='.pdf', delete=False) as tmp_file:
            tmp_path = tmp_file.name
        
        try:
            c = canvas.Canvas(tmp_path, pagesize=(pdf_width, pdf_height))
            
            for entry in entries:
                text_content = entry["text_content"]
                content = text_content["content"]
                text_size = text_content.get("text_size", 14)
                text_color = text_content.get("text_color", "000000")
                
                # Transform coordinates
                left, bottom, right, top = convert_image_to_pdf_coords(
                    entry["entry_bounds"],
                    img_width, img_height,
                    pdf_width, pdf_height
                )
                
                # Set font - use CJK font for CJK text, otherwise use specified font
                if has_cjk_characters(content):
                    c.setFont(cjk_font_name, text_size)
                else:
                    try:
                        c.setFont(text_content.get("font", "Helvetica"), text_size)
                    except:
                        c.setFont("Helvetica", text_size)
                
                # Set color
                try:
                    c.setFillColor(HexColor(f"#{text_color}"))
                except:
                    c.setFillColor(HexColor("#000000"))
                
                # Draw text at the position (left, bottom)
                c.drawString(left, bottom, content)
                total_overlays += 1
            
            c.save()
            
            # Merge overlay with the page
            overlay_reader = PdfReader(tmp_path)
            overlay_page = overlay_reader.pages[0]
            writer.pages[page_num - 1].merge_page(overlay_page)
            
        finally:
            # Clean up temp file
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)
    
    # Save the completed PDF
    with open(output_pdf_path, "wb") as out_file:
        writer.write(out_file)
    
    print(f"成功添加文本叠加层并保存到 {output_pdf_path}")
    print(f"共添加 {total_overlays} 个文本叠加（使用 CJK 字体嵌入）")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("用法: apply_text_overlays.py [输入PDF] [form_config.json] [输出PDF]")
        sys.exit(1)
    input_pdf = sys.argv[1]
    config_json = sys.argv[2]
    output_pdf = sys.argv[3]
    
    apply_text_overlays(input_pdf, config_json, output_pdf)
