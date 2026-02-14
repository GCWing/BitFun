import sys
from pypdf import PdfReader


# 检测 PDF 是否包含交互式表单字段的工具。参见 form-handler.md。


doc = PdfReader(sys.argv[1])
if (doc.get_fields()):
    print("此 PDF 包含交互式表单字段")
else:
    print("此 PDF 不包含交互式表单字段；需要通过视觉分析确定数据输入位置")
