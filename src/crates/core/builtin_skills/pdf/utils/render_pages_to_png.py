import os
import sys

from pdf2image import convert_from_path


# 将 PDF 文档的每一页渲染为 PNG 图片。


def render_document(pdf_path, output_folder, max_dimension=1000):
    page_images = convert_from_path(pdf_path, dpi=200)

    for idx, img in enumerate(page_images):
        # 如果图片尺寸超过 max_dimension，则进行缩放
        w, h = img.size
        if w > max_dimension or h > max_dimension:
            scale = min(max_dimension / w, max_dimension / h)
            new_w = int(w * scale)
            new_h = int(h * scale)
            img = img.resize((new_w, new_h))
        
        img_path = os.path.join(output_folder, f"page_{idx+1}.png")
        img.save(img_path)
        print(f"已保存第 {idx+1} 页为 {img_path}（尺寸: {img.size}）")

    print(f"共渲染 {len(page_images)} 页为 PNG 图片")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("用法: render_pages_to_png.py [输入PDF] [输出目录]")
        sys.exit(1)
    pdf_path = sys.argv[1]
    output_folder = sys.argv[2]
    render_document(pdf_path, output_folder)
