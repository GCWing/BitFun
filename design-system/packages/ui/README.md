# @bitfun/ui

Theme-independent React primitives and components.

```tsx
import "@bitfun/theme-bitfun/default.css";
import "@bitfun/ui/styles.css";
import { Button, ThemeRoot } from "@bitfun/ui";

export function Example() {
  return (
    <ThemeRoot colorScheme="light" density="comfortable">
      <Button>Continue</Button>
    </ThemeRoot>
  );
}
```

The package owns component anatomy, behavior, accessibility, and stable variants. It does not own theme selection persistence, product state, routes, locale resources, or platform APIs.
