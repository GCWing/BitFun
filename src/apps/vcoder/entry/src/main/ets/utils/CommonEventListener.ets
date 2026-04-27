import commonEventManager from "@ohos.commonEventManager";
import { hilog } from "@kit.PerformanceAnalysisKit";
import Base from '@ohos.base';
import NativeModule from 'libbitfun_desktop_lib.so';

export class CommonEventListener {
  protected subscriber: commonEventManager.CommonEventSubscriber | null = null;
  protected subscriberInfo: commonEventManager.CommonEventSubscribeInfo = {
    events: ["DEVECO_BUILD_END"]
  };

  constructor() {
    hilog.info(0x0000, 'vnext', 'create CommonEventListener');
    this.subscriber = commonEventManager.createSubscriberSync(this.subscriberInfo);
    if (this.subscriber !== null) {
      commonEventManager.subscribe(this.subscriber, (err: Base.BusinessError, data: commonEventManager.CommonEventData) => {
        if (err) {
          hilog.error(0x0000, 'vnext', 'receive message error: ' + err);
          return;
        }

        hilog.info(0x0000, 'vnext', 'receive data ' + JSON.stringify(data));
        switch (data.event) {
          case "DEVECO_BUILD_END":
            let msg = getMessageParam(data, "msg");
            hilog.info(0x0000, 'vnext', 'receive message DEVECO_BUILD_END: ' + msg);
            NativeModule.setBuildResult(msg);
        }
      })
    } else {
      hilog.error(0x0000, 'vnext', 'subscriber is null!');
    }
  }
}

export function getMessageParam<T = string>(data: commonEventManager.CommonEventData, key: string): T | undefined {
  let value: T | undefined = undefined;
  let param = data.parameters;
  if (param) {
    value = param[key];
  }
  return value;
}