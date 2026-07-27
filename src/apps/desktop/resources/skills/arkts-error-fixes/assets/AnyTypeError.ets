/**
 * ❌ 错误示例：使用 any 类型
 * 
 * 错误原因：ArkTS 不允许使用 any 类型
 * 错误信息：Use explicit types instead of "any", "unknown" (arkts-no-any-unknown)
 */
// @Entry
// @Component
// struct AnyTypeError {
//   @State data: any = { name: 'John', age: 30 };
//
//   // ❌ 错误：使用 any 类型
//   private processData(input: any): void {
//     console.info(input.name);
//     console.info(input.age);
//   }
//
//   // ❌ 错误：使用 any 类型
//   private handleEvent(event: any): void {
//     console.info(event.target);
//     console.info(event.value);
//   }
//
//   build() {
//     Column() {
//       Text('Any Type Error')
//         .fontSize(20)
//         .margin({ bottom: 20 })
//
//       Button('Process Data')
//         .onClick(() => {
//           this.processData(this.data);
//         })
//     }
//   }
// }

/**
 * ✅ 正确示例：使用明确的类型
 */
@Entry
@Component
struct ExplicitTypeCorrect {
  @State data: UserData = { name: 'John', age: 30 };

  // ✅ 正确：使用明确的类型
  private processData(input: UserData): void {
    console.info(input.name);
    console.info(input.age.toString());
  }

  // ✅ 正确：使用明确的类型
  private handleEvent(event: ClickEvent): void {
    console.info(event.target.toString());
    console.info(event.timestamp.toString());
  }

  build() {
    Column() {
      Text('Explicit Type Correct')
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Process Data')
        .onClick(() => {
          this.processData(this.data);
        })
    }
  }
}

/**
 * ✅ 正确示例：使用接口定义类型
 */
interface UserData {
  name: string;
  age: number;
  email?: string;
}

@Entry
@Component
struct InterfaceTypeCorrect {
  @State data: UserData = { name: 'John', age: 30 };

  // ✅ 正确：使用接口定义类型
  private processData(input: UserData): void {
    console.info(input.name);
    console.info(input.age.toString());
    if (input.email) {
      console.info(input.email);
    }
  }

  build() {
    Column() {
      Text('Interface Type Correct')
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Process Data')
        .onClick(() => {
          this.processData(this.data);
        })
    }
  }
}

/**
 * ✅ 正确示例：使用联合类型
 */
type StringOrNumber = string | number;

@Entry
@Component
struct UnionTypeCorrect {
  @State value: StringOrNumber = 'Hello';

  // ✅ 正确：使用联合类型
  private displayValue(value: StringOrNumber): string {
    if (typeof value === 'string') {
      return value;
    } else {
      return value.toString();
    }
  }

  build() {
    Column() {
      Text(this.displayValue(this.value))
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Toggle Value')
        .onClick(() => {
          if (typeof this.value === 'string') {
            this.value = 42;
          } else {
            this.value = 'Hello';
          }
        })
    }
  }
}

/**
 * ✅ 正确示例：使用泛型
 */
@Entry
@Component
struct GenericTypeCorrect {
  @State items: number[] = [1, 2, 3, 4, 5];

  // ✅ 正确：使用泛型
  private findItem<T>(array: T[], predicate: (item: T) => boolean): T | undefined {
    for (const item of array) {
      if (predicate(item)) {
        return item;
      }
    }
    return undefined;
  }

  build() {
    Column() {
      Text('Generic Type Correct')
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Find Item')
        .onClick(() => {
          const found = this.findItem(this.items, (item) => item > 3);
          console.info(`Found: ${found}`);
        })
    }
  }
}

/**
 * ✅ 正确示例：使用 Object 类型
 */
interface Config {
  width: number;
  height: number;
}

interface LogArg {
  key: string;
}

@Entry
@Component
struct ObjectTypeCorrect {
  @State config: Config = { width: 100, height: 200 };

  // ✅ 正确：使用 Object 类型
  private applyConfig(config: Object): void {
    console.info(JSON.stringify(config));
  }

  // ✅ 正确：使用 Object[] 作为参数类型
  private logArgs(...args: Object[]): void {
    args.forEach(arg => {
      console.info(JSON.stringify(arg));
    });
  }

  build() {
    Column() {
      Text('Object Type Correct')
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Apply Config')
        .onClick(() => {
          this.applyConfig(this.config);
          const logArg: LogArg = { key: 'value' };
          this.logArgs('arg1', 42, logArg);
        })
    }
  }
}

/**
 * ✅ 正确示例：使用类型守卫
 */
interface StringData {
  type: 'string';
  value: string;
}

interface NumberData {
  type: 'number';
  value: number;
}

type Data = StringData | NumberData;

@Entry
@Component
struct TypeGuardCorrect {
  @State data: Data = { type: 'string', value: 'Hello' };

  // ✅ 正确：使用类型守卫
  private processData(data: Data): string {
    if (data.type === 'string') {
      return data.value;
    } else {
      return data.value.toString();
    }
  }

  build() {
    Column() {
      Text(this.processData(this.data))
        .fontSize(20)
        .margin({ bottom: 20 })

      Button('Toggle Data')
        .onClick(() => {
          if (this.data.type === 'string') {
            this.data = { type: 'number', value: 42 };
          } else {
            this.data = { type: 'string', value: 'Hello' };
          }
        })
    }
  }
}
