import unittest
import json
import io
from verify_form_layout import validate_form_layout


# 此测试目前不在 CI 中自动运行；仅用于文档和手动验证。
class TestValidateFormLayout(unittest.TestCase):
    
    def create_json_stream(self, data):
        """辅助方法：从数据创建 JSON 流"""
        return io.StringIO(json.dumps(data))
    
    def test_no_overlaps(self):
        """测试无边界框重叠的情况"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 30]
                },
                {
                    "description": "Email",
                    "page_num": 1,
                    "label_bounds": [10, 40, 50, 60],
                    "entry_bounds": [60, 40, 150, 60]
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("成功" in msg for msg in messages))
        self.assertFalse(any("失败" in msg for msg in messages))
    
    def test_label_entry_overlap_same_field(self):
        """测试同一字段的标签和输入框重叠"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 60, 30],
                    "entry_bounds": [50, 10, 150, 30]  # 与标签重叠
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("失败" in msg and "重叠" in msg for msg in messages))
        self.assertFalse(any("成功" in msg for msg in messages))
    
    def test_overlap_between_different_fields(self):
        """测试不同字段边界框之间的重叠"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 30]
                },
                {
                    "description": "Email",
                    "page_num": 1,
                    "label_bounds": [40, 20, 80, 40],  # 与 Name 的边界框重叠
                    "entry_bounds": [160, 10, 250, 30]
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("失败" in msg and "重叠" in msg for msg in messages))
        self.assertFalse(any("成功" in msg for msg in messages))
    
    def test_different_pages_no_overlap(self):
        """测试不同页面的边界框不算重叠"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 30]
                },
                {
                    "description": "Email",
                    "page_num": 2,
                    "label_bounds": [10, 10, 50, 30],  # 相同坐标但不同页面
                    "entry_bounds": [60, 10, 150, 30]
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("成功" in msg for msg in messages))
        self.assertFalse(any("失败" in msg for msg in messages))
    
    def test_entry_height_too_small(self):
        """测试输入框高度是否根据文字大小检查"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 20],  # 高度为 10
                    "text_content": {
                        "text_size": 14  # 文字大小大于高度
                    }
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("失败" in msg and "高度" in msg for msg in messages))
        self.assertFalse(any("成功" in msg for msg in messages))
    
    def test_entry_height_adequate(self):
        """测试输入框高度足够时通过验证"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 30],  # 高度为 20
                    "text_content": {
                        "text_size": 14  # 文字大小小于高度
                    }
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("成功" in msg for msg in messages))
        self.assertFalse(any("失败" in msg for msg in messages))
    
    def test_default_text_size(self):
        """测试未指定时使用默认文字大小"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 20],  # 高度为 10
                    "text_content": {}  # 未指定 text_size，应使用默认值 14
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("失败" in msg and "高度" in msg for msg in messages))
        self.assertFalse(any("成功" in msg for msg in messages))
    
    def test_no_text_content(self):
        """测试缺少 text_content 时不进行高度检查"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [60, 10, 150, 20]  # 高度较小但无 text_content
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("成功" in msg for msg in messages))
        self.assertFalse(any("失败" in msg for msg in messages))
    
    def test_multiple_errors_limit(self):
        """测试错误消息数量限制，防止输出过多"""
        entries = []
        # 创建多个重叠字段
        for i in range(25):
            entries.append({
                "description": f"Field{i}",
                "page_num": 1,
                "label_bounds": [10, 10, 50, 30],  # 全部重叠
                "entry_bounds": [20, 15, 60, 35]   # 全部重叠
            })
        
        data = {"field_entries": entries}
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        # 应在约 20 条消息后中止
        self.assertTrue(any("中止" in msg for msg in messages))
        # 应有一些失败消息但不应有数百条
        failure_count = sum(1 for msg in messages if "失败" in msg)
        self.assertGreater(failure_count, 0)
        self.assertLess(len(messages), 30)  # 应受限制
    
    def test_edge_touching_boxes(self):
        """测试边缘相接的边界框不算重叠"""
        data = {
            "field_entries": [
                {
                    "description": "Name",
                    "page_num": 1,
                    "label_bounds": [10, 10, 50, 30],
                    "entry_bounds": [50, 10, 150, 30]  # 在 x=50 处相接
                }
            ]
        }
        
        stream = self.create_json_stream(data)
        messages = validate_form_layout(stream)
        self.assertTrue(any("成功" in msg for msg in messages))
        self.assertFalse(any("失败" in msg for msg in messages))
    

if __name__ == '__main__':
    unittest.main()
