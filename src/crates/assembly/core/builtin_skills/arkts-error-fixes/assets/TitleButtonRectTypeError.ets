import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';

// ❌ 错误示例：返回类型声明为 window.Rect
// async function getTitleButtonRect(context: common.UIAbilityContext): Promise<window.Rect> {
//   return new Promise((resolve, reject) => {
//     window.getLastWindow(context, (err, win) => {
//       if (err.code !== 0) {
//         reject(new Error(err.message));
//         return;
//       }
//       const titleButtonRect = win.getTitleButtonRect();
//       resolve(titleButtonRect); // 类型错误：TitleButtonRect 不能赋值给 Rect
//     });
//   });
// }

// ✅ 正确示例：返回类型声明为 window.TitleButtonRect
async function getTitleButtonRect(context: common.UIAbilityContext): Promise<window.TitleButtonRect> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(new Error(err.message));
        return;
      }
      const titleButtonRect = win.getTitleButtonRect();
      resolve(titleButtonRect);
    });
  });
}

// ✅ 如果需要 Rect 类型，进行类型转换
// 注意：TitleButtonRect 类型可能不包含 left 和 top 属性
// 如果需要完整的 Rect 信息，需要从其他 API 获取
async function getTitleButtonRectAsRect(context: common.UIAbilityContext): Promise<window.Rect> {
  return new Promise((resolve, reject) => {
    window.getLastWindow(context, (err, win) => {
      if (err.code !== 0) {
        reject(new Error(err.message));
        return;
      }
      const titleButtonRect = win.getTitleButtonRect();
      // 创建新的 Rect 对象
      // TitleButtonRect 只包含 width 和 height 属性
      resolve({
        left: 0,
        top: 0,
        width: titleButtonRect.width,
        height: titleButtonRect.height
      });
    });
  });
}

@Entry
@Component
struct TitleButtonRectTypeError {
  @State titleBarHeight: number = 0;
  @State titleBarWidth: number = 0;

  async aboutToAppear() {
    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    try {
      const titleButtonRect = await getTitleButtonRect(context);
      // TitleButtonRect 只包含 width 和 height 属性
      this.titleBarHeight = titleButtonRect.height;
      this.titleBarWidth = titleButtonRect.width;
    } catch (err) {
      console.error('获取标题栏按钮区域失败:', err instanceof Error ? err.message : String(err));
    }
  }

  build() {
    Column() {
      Text('TitleButtonRect 类型错误示例')
        .fontSize(20)
        .fontWeight(FontWeight.Bold)
        .margin({ bottom: 20 })
      
      Text(`标题栏高度: ${this.titleBarHeight}`)
        .margin({ bottom: 8 })
      Text(`标题栏宽度: ${this.titleBarWidth}`)
    }
    .padding(20)
  }
}
