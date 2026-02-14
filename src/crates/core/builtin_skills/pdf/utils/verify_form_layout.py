from dataclasses import dataclass
import json
import sys


# 验证分析 PDF 时创建的 `form_config.json` 文件是否存在重叠的边界框。参见 form-handler.md。


@dataclass
class BoundsAndEntry:
    bounds: list[float]
    bounds_type: str
    entry: dict


# 返回打印到标准输出供 Claude 读取的消息列表。
def validate_form_layout(config_json_stream) -> list[str]:
    messages = []
    config = json.load(config_json_stream)
    messages.append(f"已读取 {len(config['field_entries'])} 个字段条目")

    def bounds_overlap(b1, b2):
        no_horizontal_overlap = b1[0] >= b2[2] or b1[2] <= b2[0]
        no_vertical_overlap = b1[1] >= b2[3] or b1[3] <= b2[1]
        return not (no_horizontal_overlap or no_vertical_overlap)

    bounds_list = []
    for entry in config["field_entries"]:
        bounds_list.append(BoundsAndEntry(entry["label_bounds"], "标签", entry))
        bounds_list.append(BoundsAndEntry(entry["entry_bounds"], "输入", entry))

    found_error = False
    for i, bi in enumerate(bounds_list):
        # 时间复杂度 O(N^2)；如有需要可优化。
        for j in range(i + 1, len(bounds_list)):
            bj = bounds_list[j]
            if bi.entry["page_num"] == bj.entry["page_num"] and bounds_overlap(bi.bounds, bj.bounds):
                found_error = True
                if bi.entry is bj.entry:
                    messages.append(f"失败: `{bi.entry['description']}` 的标签和输入边界框重叠 ({bi.bounds}, {bj.bounds})")
                else:
                    messages.append(f"失败: `{bi.entry['description']}` 的{bi.bounds_type}边界框 ({bi.bounds}) 与 `{bj.entry['description']}` 的{bj.bounds_type}边界框 ({bj.bounds}) 重叠")
                if len(messages) >= 20:
                    messages.append("中止后续检查；请修正边界框后重试")
                    return messages
        if bi.bounds_type == "输入":
            if "text_content" in bi.entry:
                text_size = bi.entry["text_content"].get("text_size", 14)
                entry_height = bi.bounds[3] - bi.bounds[1]
                if entry_height < text_size:
                    found_error = True
                    messages.append(f"失败: `{bi.entry['description']}` 的输入边界框高度 ({entry_height}) 不足以容纳文本内容（文字大小: {text_size}）。请增加边界框高度或减小文字大小。")
                    if len(messages) >= 20:
                        messages.append("中止后续检查；请修正边界框后重试")
                        return messages

    if not found_error:
        messages.append("成功: 所有边界框均有效")
    return messages

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("用法: verify_form_layout.py [form_config.json]")
        sys.exit(1)
    # 输入文件应采用 form-handler.md 中描述的 `form_config.json` 格式。
    with open(sys.argv[1]) as f:
        messages = validate_form_layout(f)
    for msg in messages:
        print(msg)
