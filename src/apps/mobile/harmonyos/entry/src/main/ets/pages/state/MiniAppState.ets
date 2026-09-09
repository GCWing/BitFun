import { RemoteMiniAppSummary } from '../../model/RemoteMiniAppModels';

@ObservedV2
export class MiniAppState {
  @Trace visible: boolean = false;
  @Trace loading: boolean = false;
  @Trace error: string = '';
  @Trace apps: RemoteMiniAppSummary[] = [];
  @Trace appId: string = '';
  @Trace name: string = '';
  @Trace document: string = '';
  @Trace revision: number = 0;
}
