import json
import sys

from PIL import Image, ImageDraw


# 生成带有边界框矩形的"预览"图片，用于在 PDF 中确定文本叠加位置。参见 form-handler.md。


def generate_preview(page_num, config_json_path, source_path, preview_path):
    # 输入文件应采用 form-handler.md 中描述的 `form_config.json` 格式。
    with open(config_json_path, 'r') as f:
        config = json.load(f)

        img = Image.open(source_path)
        draw = ImageDraw.Draw(img)
        box_count = 0
        
        for entry in config["field_entries"]:
            if entry["page_num"] == page_num:
                entry_box = entry['entry_bounds']
                label_box = entry['label_bounds']
                # 在输入区域绘制红色矩形，在标签区域绘制蓝色矩形。
                draw.rectangle(entry_box, outline='red', width=2)
                draw.rectangle(label_box, outline='blue', width=2)
                box_count += 2
        
        img.save(preview_path)
        print(f"已生成预览图片 {preview_path}，包含 {box_count} 个边界框")


if __name__ == "__main__":
    if len(sys.argv) != 5:
        print("用法: generate_preview_overlay.py [页码] [form_config.json文件] [源图片路径] [预览图片路径]")
        sys.exit(1)
    page_num = int(sys.argv[1])
    config_json_path = sys.argv[2]
    source_image_path = sys.argv[3]
    preview_image_path = sys.argv[4]
    generate_preview(page_num, config_json_path, source_image_path, preview_image_path)
