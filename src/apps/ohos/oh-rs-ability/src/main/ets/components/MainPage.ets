import { DefaultXComponent } from "./DefaultXComponent";

export const RouteName = "RustAbility";

@Entry({ routeName: RouteName })
@Component
struct Index {
  build() {
    Row() {
      Column() {
        DefaultXComponent()
      }.width("100%")
    }.height("100%");
  }
}
