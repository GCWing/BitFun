import json
import sys

from pypdf import PdfReader


# 解析 PDF 中的交互式表单字段数据并输出 JSON 用于表单填写。参见 form-handler.md。


# 匹配 PdfReader `get_fields` 和 `update_page_form_field_values` 方法使用的格式。
def build_complete_element_id(annotation):
    parts = []
    while annotation:
        name = annotation.get('/T')
        if name:
            parts.append(name)
        annotation = annotation.get('/Parent')
    return ".".join(reversed(parts)) if parts else None


def build_element_dict(field, element_id):
    element_dict = {"element_id": element_id}
    field_type = field.get('/FT')
    if field_type == "/Tx":
        element_dict["element_type"] = "text_input"
    elif field_type == "/Btn":
        element_dict["element_type"] = "toggle_box"  # 选项组单独处理
        available_states = field.get("/_States_", [])
        if len(available_states) == 2:
            # "/Off" 通常是未选中值，参见 PDF 规范:
            # https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf#page=448
            # 它可能出现在 "/_States_" 列表的任一位置。
            if "/Off" in available_states:
                element_dict["on_value"] = available_states[0] if available_states[0] != "/Off" else available_states[1]
                element_dict["off_value"] = "/Off"
            else:
                print(f"切换框 `${element_id}` 的状态值异常。其开/关值可能不正确；请通过视觉检查验证结果。")
                element_dict["on_value"] = available_states[0]
                element_dict["off_value"] = available_states[1]
    elif field_type == "/Ch":
        element_dict["element_type"] = "dropdown"
        available_states = field.get("/_States_", [])
        element_dict["menu_items"] = [{
            "option_value": state[0],
            "display_text": state[1],
        } for state in available_states]
    else:
        element_dict["element_type"] = f"unknown ({field_type})"
    return element_dict


# Returns a list of interactive PDF form elements:
# [
#   {
#     "element_id": "name",
#     "page_num": 1,
#     "element_type": ("text_input", "toggle_box", "option_group", or "dropdown")
#     // Per-type additional properties described in form-handler.md
#   },
# ]
def parse_form_elements(reader: PdfReader):
    fields = reader.get_fields()

    elements_by_id = {}
    potential_option_groups = set()

    for element_id, field in fields.items():
        # Skip container fields with children, except possible parent groups for radio options.
        if field.get("/Kids"):
            if field.get("/FT") == "/Btn":
                potential_option_groups.add(element_id)
            continue
        elements_by_id[element_id] = build_element_dict(field, element_id)

    # Bounding rectangles are stored in annotations within page objects.

    # Radio option elements have a separate annotation for each choice;
    # all choices share the same element name.
    # See https://westhealth.github.io/exploring-fillable-forms-with-pdfrw.html
    option_groups_by_id = {}

    for page_idx, pg in enumerate(reader.pages):
        annotations = pg.get('/Annots', [])
        for ann in annotations:
            element_id = build_complete_element_id(ann)
            if element_id in elements_by_id:
                elements_by_id[element_id]["page_num"] = page_idx + 1
                elements_by_id[element_id]["bounds"] = ann.get('/Rect')
            elif element_id in potential_option_groups:
                try:
                    # ann['/AP']['/N'] should have two items. One is '/Off',
                    # the other is the active value.
                    active_values = [v for v in ann["/AP"]["/N"] if v != "/Off"]
                except KeyError:
                    continue
                if len(active_values) == 1:
                    bounds = ann.get("/Rect")
                    if element_id not in option_groups_by_id:
                        option_groups_by_id[element_id] = {
                            "element_id": element_id,
                            "element_type": "option_group",
                            "page_num": page_idx + 1,
                            "available_options": [],
                        }
                    # Note: macOS Preview.app may not display selected
                    # radio options correctly (removing leading slash helps there
                    # but breaks Chrome/Firefox/Acrobat/etc).
                    option_groups_by_id[element_id]["available_options"].append({
                        "option_value": active_values[0],
                        "bounds": bounds,
                    })

    # Some PDFs have form element definitions without corresponding annotations,
    # so we can't determine their location. Exclude these elements.
    elements_with_location = []
    for element in elements_by_id.values():
        if "page_num" in element:
            elements_with_location.append(element)
        else:
            print(f"无法确定元素 ID: {element.get('element_id')} 的位置，已排除")

    # Sort by page number, then Y position (flipped in PDF coordinate system), then X.
    def sort_key(elem):
        if "available_options" in elem:
            bounds = elem["available_options"][0]["bounds"] or [0, 0, 0, 0]
        else:
            bounds = elem.get("bounds") or [0, 0, 0, 0]
        adjusted_pos = [-bounds[1], bounds[0]]
        return [elem.get("page_num"), adjusted_pos]
    
    sorted_elements = elements_with_location + list(option_groups_by_id.values())
    sorted_elements.sort(key=sort_key)

    return sorted_elements


def export_form_structure(pdf_path: str, json_output_path: str):
    reader = PdfReader(pdf_path)
    elements = parse_form_elements(reader)
    with open(json_output_path, "w") as f:
        json.dump(elements, f, indent=2)
    print(f"已将 {len(elements)} 个表单元素写入 {json_output_path}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("用法: parse_form_structure.py [输入PDF] [输出JSON]")
        sys.exit(1)
    export_form_structure(sys.argv[1], sys.argv[2])
