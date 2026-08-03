# Any Type Errors

## Error: Use explicit types instead of "any", "unknown"

### Error Message
```
Use explicit types instead of "any", "unknown" (arkts-no-any-unknown)
```

### Cause
ArkTS requires explicit type definitions and does not allow the use of `any` or `unknown` types. This is part of ArkTS's stricter type system designed to improve type safety and reduce runtime errors.

### Solution
Use explicit type definitions, interfaces, type aliases, union types, or generics instead of `any` or `unknown`.

### Key Points
- ArkTS does not allow `any` or `unknown` types
- Use explicit types for all variables and parameters
- Define interfaces for complex object types
- Use union types for multiple possible types
- Use generics for reusable type-safe code

### ❌ Wrong Usage
```typescript
// ❌ Wrong: Using any type
let data: any = { name: 'John', age: 30 };

// ❌ Wrong: Using any in function parameters
function processData(input: any): void {
  console.info(input.name);
}

// ❌ Wrong: Using any in event handlers
function handleEvent(event: any): void {
  console.info(event.target);
}

// ❌ Wrong: Using unknown type
let value: unknown = 'Hello';
```

### ✅ Correct Usage
```typescript
// ✅ Correct: Using explicit type
let data: { name: string; age: number } = { name: 'John', age: 30 };

// ✅ Correct: Using interface
interface UserData {
  name: string;
  age: number;
}

function processData(input: UserData): void {
  console.info(input.name);
}

// ✅ Correct: Using explicit event type
function handleEvent(event: ClickEvent): void {
  console.info(event.target.toString());
}
```

### Interface Definitions

#### Simple Interface
```typescript
interface UserData {
  name: string;
  age: number;
  email?: string;
}

@Component
struct UserComponent {
  @State user: UserData = { name: 'John', age: 30 };

  private displayUser(data: UserData): string {
    return `${data.name}, ${data.age}`;
  }

  build() {
    Text(this.displayUser(this.user))
  }
}
```

#### Nested Interface
```typescript
interface Address {
  street: string;
  city: string;
  zipCode: string;
}

interface User {
  id: number;
  name: string;
  address: Address;
}

@Component
struct AddressComponent {
  @State user: User = {
    id: 1,
    name: 'John',
    address: { street: '123 Main St', city: 'New York', zipCode: '10001' }
  };

  private displayAddress(user: User): string {
    return `${user.address.street}, ${user.address.city}`;
  }

  build() {
    Text(this.displayAddress(this.user))
  }
}
```

### Union Types

#### Simple Union
```typescript
type StringOrNumber = string | number;

@Component
struct UnionExample {
  @State value: StringOrNumber = 'Hello';

  private displayValue(value: StringOrNumber): string {
    if (typeof value === 'string') {
      return value;
    } else {
      return value.toString();
    }
  }

  build() {
    Text(this.displayValue(this.value))
  }
}
```

#### Complex Union
```typescript
type EventData = ClickEvent | TouchEvent | ScrollEvent;

@Component
struct EventExample {
  private handleEvent(event: EventData): void {
    if (event instanceof ClickEvent) {
      console.info('Click event');
    } else if (event instanceof TouchEvent) {
      console.info('Touch event');
    } else {
      console.info('Scroll event');
    }
  }

  build() {
    Column() {
      Button('Click')
        .onClick((event: ClickEvent) => this.handleEvent(event))
    }
  }
}
```

### Generics

#### Simple Generic
```typescript
@Component
struct GenericExample {
  @State items: number[] = [1, 2, 3, 4, 5];

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
      Button('Find Item')
        .onClick(() => {
          const found = this.findItem(this.items, (item) => item > 3);
          console.info(`Found: ${found}`);
        })
    }
  }
}
```

#### Generic Class
```typescript
class Storage<T> {
  private data: T[] = [];

  add(item: T): void {
    this.data.push(item);
  }

  get(index: number): T | undefined {
    return this.data[index];
  }

  find(predicate: (item: T) => boolean): T | undefined {
    return this.data.find(predicate);
  }
}

@Component
struct StorageExample {
  private storage: Storage<number> = new Storage<number>();

  aboutToAppear() {
    this.storage.add(1);
    this.storage.add(2);
    this.storage.add(3);
  }

  build() {
    Column() {
      Button('Get Item')
        .onClick(() => {
          const item = this.storage.get(0);
          console.info(`Item: ${item}`);
        })
    }
  }
}
```

### Type Guards

#### Discriminated Union
```typescript
interface StringData {
  type: 'string';
  value: string;
}

interface NumberData {
  type: 'number';
  value: number;
}

type Data = StringData | NumberData;

@Component
struct TypeGuardExample {
  @State data: Data = { type: 'string', value: 'Hello' };

  private processData(data: Data): string {
    if (data.type === 'string') {
      return data.value;
    } else {
      return data.value.toString();
    }
  }

  build() {
    Text(this.processData(this.data))
  }
}
```

#### Typeof Guard
```typescript
type Value = string | number | boolean;

@Component
struct TypeofExample {
  @State value: Value = 'Hello';

  private displayValue(value: Value): string {
    if (typeof value === 'string') {
      return `String: ${value}`;
    } else if (typeof value === 'number') {
      return `Number: ${value}`;
    } else {
      return `Boolean: ${value}`;
    }
  }

  build() {
    Text(this.displayValue(this.value))
  }
}
```

### Object Type

#### Using Object
```typescript
@Component
struct ObjectExample {
  @State config: Object = { width: 100, height: 200 };

  private applyConfig(config: Object): void {
    console.info(JSON.stringify(config));
  }

  private logArgs(...args: Object[]): void {
    args.forEach(arg => {
      console.info(JSON.stringify(arg));
    });
  }

  build() {
    Column() {
      Button('Apply Config')
        .onClick(() => {
          this.applyConfig(this.config);
          this.logArgs('arg1', 42, { key: 'value' });
        })
    }
  }
}
```

### Type Aliases

#### Simple Alias
```typescript
type UserId = number;
type UserName = string;
type UserEmail = string;

interface User {
  id: UserId;
  name: UserName;
  email: UserEmail;
}

@Component
struct AliasExample {
  @State user: User = { id: 1, name: 'John', email: 'john@example.com' };

  build() {
    Text(`${this.user.name} (${this.user.id})`)
  }
}
```

#### Function Alias
```typescript
type EventHandler = (event: ClickEvent) => void;
type ValueHandler = (value: string) => void;

@Component
struct FunctionAliasExample {
  private onClick: EventHandler = (event: ClickEvent): void => {
    console.info('Clicked');
  };

  private onInput: ValueHandler = (value: string): void => {
    console.info(`Input: ${value}`);
  };

  build() {
    Column() {
      Button('Click')
        .onClick(this.onClick)
    }
  }
}
```

### Common Patterns

#### Event Handlers
```typescript
type ClickHandler = (event: ClickEvent) => void;
type TouchHandler = (event: TouchEvent) => void;
type ScrollHandler = (event: ScrollEvent) => void;

@Component
struct EventHandlers {
  private onClick: ClickHandler = (event: ClickEvent): void => {
    console.info('Clicked');
  };

  private onTouch: TouchHandler = (event: TouchEvent): void => {
    console.info('Touched');
  };

  build() {
    Column() {
      Button('Click')
        .onClick(this.onClick)
    }
    .onTouch(this.onTouch)
  }
}
```

#### Data Processing
```typescript
interface DataProcessor<T, R> {
  process(data: T): R;
}

class StringProcessor implements DataProcessor<string, number> {
  process(data: string): number {
    return data.length;
  }
}

@Component
struct ProcessorExample {
  private processor: DataProcessor<string, number> = new StringProcessor();

  build() {
    Text(`Length: ${this.processor.process('Hello')}`)
  }
}
```

### Best Practices
1. **Use explicit types**: Always define types explicitly
2. **Define interfaces**: Use interfaces for complex object types
3. **Use union types**: Use union types for multiple possible types
4. **Use generics**: Use generics for reusable type-safe code
5. **Use type guards**: Use type guards for runtime type checking
6. **Avoid any**: Never use `any` or `unknown` types

### Related Files
- [Code Example](../assets/AnyTypeError.ets)
- [ArkTS Type System](https://developer.huawei.com/consumer/cn/doc/harmonyos-guides/arkts-type-system)
