# @ohos-rs/ability

This package provides a set of components and APIs for building OpenHarmony activities and it helps us create a application entry for rust application.

For OpenHarmony/HarmonyNext development, our application entry must be a ArkTS file and we need to forward some lifecycle event to rust code.

## Install

```
ohpm install @ohos-rs/ability
```

## API and Components

### RustAbility

The Activity component is a wrapper of the OpenHarmony Activity class. It will load the native code and forward the lifecycle event to rust code by default.

If you want to use rust development OpenHarmony/HarmonyNext application, you must use this component to create your application entry.

```ts
// ets/entryability/EntryAbility.ets
import { RustAbility } from "@ohos-rs/ability";

export default class MyAbility extends RustAbility {
  public moduleName: string = "hello";

  onCreate() {
    super.onCreate();
  }
}
```

Here are some notes and tips:

1. For every lifecycle callback, you must call the super method to forward the event to rust code as first and then write your own logic.

2. `moduleName` is the name of your native module name which file name is `lib${moduleName}.so`. **You must define it in your project**.

### loadMode

Allow to define that how to load dynamic library in runtime.

- async
  In `async` mode, we will use `await import(${lib})` to load library. And this is default behavior.
- sync
  In `sync` mode, we will use `loadNativeModule` to load library. If you define it, you must add some configuration into your `build-profile.json5`.

  ```json
  {
    "buildOption": {
      "arkOptions": {
        "runtimeOnly": {
          "packages": ["libentry.so"]
        }
      }
    }
  }
  ```

  See more with [loadNativeModule](https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-common-load-native-module#loadnativemodule).

### DefaultXComponent

When using rust to develop OpenHarmony/HarmonyNext application, we use `XComponent` to render the UI by default. And `DefaultXComponent` loads the native module and forward the lifecycle event to rust code by default.

This component is a optional component, you may don't need it. If you don't need to use it, `RustAbility` will use it by default.

And if you want to add some custom logic, you can use it with the following code:

```ts
// ets/entryability/EntryAbility.ets
import { RustAbility } from "@ohos-rs/ability";
import Want from "@ohos.app.ability.Want";
import { AbilityConstant } from "@kit.AbilityKit";
import window from "@ohos.window";

export default class EntryAbility extends RustAbility {
  public moduleName: string = "example";

  // Must mark it as false to prevent the default page from loading
  public defaultPage: boolean = false;

  async onCreate(
    want: Want,
    launchParam: AbilityConstant.LaunchParam
  ): Promise<void> {
    super.onCreate(want, launchParam);
  }

  async onWindowStageCreate(windowStage: window.WindowStage): Promise<void> {
    // Must call super method to forward the event to rust code
    super.onWindowStageCreate(windowStage);
    // Jump to your custom page
    await windowStage.loadContent("pages/Index");
  }
}
```

```ts
// ets/pages/Index.ets
import { DefaultXComponent } from '@ohos-rs/ability'
import { ItemRestriction, SegmentButton, SegmentButtonOptions, SegmentButtonTextItem } from '@kit.ArkUI';
import { changeRender } from "libwgpu_in_app.so"

@Entry
@Component
struct Index {
  // Add some custom logic
  @State tabOptions: SegmentButtonOptions = SegmentButtonOptions.capsule({
    buttons: [{ text: 'boids' },
      { text: 'MSAA line' },
      { text: 'cube' },
      { text: "water" },
      { text: "shadow" }] as ItemRestriction<SegmentButtonTextItem>,
    backgroundBlurStyle: BlurStyle.BACKGROUND_THICK,
  })
  @State @Watch("handleChange") tabSelectedIndexes: number[] = [0]

  handleChange() {
    console.log(`changeIndex: ${this.tabSelectedIndexes}`)
    changeRender(this.tabSelectedIndexes[0])
  }

  build() {
    Row() {
      Column() {
        SegmentButton({ options: this.tabOptions, selectedIndexes: $tabSelectedIndexes })
        // Must use the default component to render the UI
        DefaultXComponent()
      }
      .width('100%')
    }
    .height('100%')
  }
}
```
