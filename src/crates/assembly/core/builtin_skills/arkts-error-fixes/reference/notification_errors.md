# Notification API Type Errors

## Error: `notificationManager.ContentType` type incompatibility

### Error Message
```
Type 'notificationManager.ContentType' is not assignable to type 'notification.ContentType'
```

### Cause
Type mismatch between `notificationManager.ContentType` and `notification.ContentType`. The `notificationManager.ContentType` enum has additional properties that are not present in `notification.ContentType`, causing type incompatibility.

### Solution
Cast the ContentType value to `number` type to resolve the type incompatibility.

### Key Points
- Import from `@kit.NotificationKit`: `import { notificationManager } from '@kit.NotificationKit'`
- Cast to `number`: `as number` when assigning to `contentType`
- Use proper error handling with `BusinessError`
- Use `hilog` for logging instead of `console`

### Notification Request Structure
```typescript
interface NotificationRequest {
  id: number;
  content: NotificationContent;
  notificationSlotType?: SlotType;
  isOngoing?: boolean;
  isUnremovable?: boolean;
  deliveryTime?: number;
  tapDismissed?: boolean;
  autoDeletedTime?: number;
  classification?: Classification;
  groupName?: string;
  slotType?: SlotType;
  ...
}
```

### Content Types
```typescript
notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT
notificationManager.ContentType.NOTIFICATION_CONTENT_LONG_TEXT
notificationManager.ContentType.NOTIFICATION_CONTENT_PICTURE
notificationManager.ContentType.NOTIFICATION_CONTENT_CONVERSATION
notificationManager.ContentType.NOTIFICATION_CONTENT_MULTILINE
notificationManager.ContentType.NOTIFICATION_CONTENT_MEDIA
notificationManager.ContentType.NOTIFICATION_CONTENT_LOCAL_LIVE_VIEW
notificationManager.ContentType.NOTIFICATION_CONTENT_LIVE_VIEW
```

### Basic Text Content
```typescript
interface NotificationBasicContent {
  title: string;
  text: string;
  additionalText?: string;
  largeIcon?: string;
  briefText?: string;
  expandedTitle?: string;
}
```

### Publishing Pattern
```typescript
let notificationRequest: notificationManager.NotificationRequest = {
  id: 1,
  content: {
    contentType: notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT as number,
    normal: {
      title: 'Test Notification',
      text: 'This is a test notification',
      additionalText: 'Additional text'
    }
  }
};

notificationManager.publish(notificationRequest)
  .then(() => {
    hilog.info(0x0000, 'testTag', 'Publish notification success');
  })
  .catch((err: BusinessError) => {
    hilog.error(0x0000, 'testTag', 'Publish notification failed: %{public}s', JSON.stringify(err));
  });
```

### Error Handling
```typescript
// Common error codes
if (err.code === 1600001) {
  // Notification not enabled
} else if (err.code === 1600002) {
  // Too many notifications
} else if (err.code === 1600003) {
  // Invalid notification
} else if (err.code === 1600004) {
  // Notification slot not found
}
```

### Best Practices
1. **Cast to number**: Always cast ContentType to `number` to avoid type errors
2. **Use hilog**: Use `hilog` instead of `console` for better logging
3. **Handle errors**: Always catch and handle BusinessError
4. **Use unique IDs**: Ensure notification IDs are unique
5. **Test permissions**: Verify notification permissions are granted

### Related Files
- [Code Example](../assets/NotificationError.ets)
- [Notification Kit Documentation](https://developer.huawei.com/consumer/cn/doc/harmonyos-references-V5/ts-api-notificationmanager-V5)
