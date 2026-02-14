import json
import sys

from pypdf import PdfReader, PdfWriter

from parse_form_structure import parse_form_elements


# 填充 PDF 中的交互式表单字段。参见 form-handler.md。


def populate_form_fields(input_pdf_path: str, form_data_path: str, output_pdf_path: str):
    with open(form_data_path) as f:
        form_data = json.load(f)
    # Group by page number.
    data_by_page = {}
    for entry in form_data:
        if "fill_value" in entry:
            element_id = entry["element_id"]
            page_num = entry["page_num"]
            if page_num not in data_by_page:
                data_by_page[page_num] = {}
            data_by_page[page_num][element_id] = entry["fill_value"]
    
    reader = PdfReader(input_pdf_path)

    has_errors = False
    elements = parse_form_elements(reader)
    elements_by_id = {e["element_id"]: e for e in elements}
    for entry in form_data:
        existing_element = elements_by_id.get(entry["element_id"])
        if not existing_element:
            has_errors = True
            print(f"错误: `{entry['element_id']}` 不是有效的元素 ID")
        elif entry["page_num"] != existing_element["page_num"]:
            has_errors = True
            print(f"错误: `{entry['element_id']}` 的页码不正确（得到 {entry['page_num']}，期望 {existing_element['page_num']}）")
        else:
            if "fill_value" in entry:
                err = validate_element_value(existing_element, entry["fill_value"])
                if err:
                    print(err)
                    has_errors = True
    if has_errors:
        sys.exit(1)

    writer = PdfWriter(clone_from=reader)
    for page_num, field_values in data_by_page.items():
        writer.update_page_form_field_values(writer.pages[page_num - 1], field_values, auto_regenerate=False)

    # Required for many PDF viewers to format form values correctly.
    # May cause "save changes" dialog even without user modifications.
    writer.set_need_appearances_writer(True)
    
    with open(output_pdf_path, "wb") as f:
        writer.write(f)


def validate_element_value(element_info, fill_value):
    element_type = element_info["element_type"]
    element_id = element_info["element_id"]
    if element_type == "toggle_box":
        on_val = element_info["on_value"]
        off_val = element_info["off_value"]
        if fill_value != on_val and fill_value != off_val:
            return f'错误: 切换元素 "{element_id}" 的值 "{fill_value}" 无效。开启值为 "{on_val}"，关闭值为 "{off_val}"'
    elif element_type == "option_group":
        valid_values = [opt["option_value"] for opt in element_info["available_options"]]
        if fill_value not in valid_values:
            return f'错误: 选项组 "{element_id}" 的值 "{fill_value}" 无效。有效值为: {valid_values}' 
    elif element_type == "dropdown":
        menu_values = [item["option_value"] for item in element_info["menu_items"]]
        if fill_value not in menu_values:
            return f'错误: 下拉框 "{element_id}" 的值 "{fill_value}" 无效。有效值为: {menu_values}'
    return None


# pypdf (at least version 5.7.0) has a bug when setting values for selection list fields.
# In _writer.py around line 966:
#
# if field.get(FA.FT, "/Tx") == "/Ch" and field_flags & FA.FfBits.Combo == 0:
#     txt = "\n".join(annotation.get_inherited(FA.Opt, []))
#
# The issue is that for selection lists, `get_inherited` returns a list of two-element lists like
# [["value1", "Text 1"], ["value2", "Text 2"], ...]
# This causes `join` to throw a TypeError because it expects an iterable of strings.
# The workaround is to patch `get_inherited` to return a list of value strings.
# We call the original method and adjust the return value only if the argument is
# `FA.Opt` and if the return value is a list of two-element lists.
def apply_pypdf_workaround():
    from pypdf.generic import DictionaryObject
    from pypdf.constants import FieldDictionaryAttributes

    original_get_inherited = DictionaryObject.get_inherited

    def patched_get_inherited(self, key: str, default = None):
        result = original_get_inherited(self, key, default)
        if key == FieldDictionaryAttributes.Opt:
            if isinstance(result, list) and all(isinstance(v, list) and len(v) == 2 for v in result):
                result = [r[0] for r in result]
        return result

    DictionaryObject.get_inherited = patched_get_inherited


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("用法: populate_interactive_form.py [输入PDF] [form_data.json] [输出PDF]")
        sys.exit(1)
    apply_pypdf_workaround()
    input_pdf = sys.argv[1]
    form_data_path = sys.argv[2]
    output_pdf = sys.argv[3]
    populate_form_fields(input_pdf, form_data_path, output_pdf)
