import { notificationManager } from '@kit.NotificationKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

@Entry
@Component
struct NotificationError {
  publishNotification() {
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

    notificationManager.publish(notificationRequest).then(() => {
      hilog.info(0x0000, 'testTag', 'Publish notification success');
    }).catch((err: BusinessError) => {
      hilog.error(0x0000, 'testTag', 'Publish notification failed: %{public}s', JSON.stringify(err));
    });
  }

  build() {
    Column() {
      Button('Publish Notification')
        .onClick(() => this.publishNotification())
    }
  }
}
