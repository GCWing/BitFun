import { describe, expect, it } from '@ohos/hypium';
import { MiniAppDocument } from '../main/ets/services/MiniAppDocument';
import { MiniAppViewModel } from '../main/ets/pages/viewmodel/MiniAppViewModel';
import { RemoteMiniAppClient, RemoteMiniAppPayload, RemoteMiniAppRequest } from '../main/ets/model/RemoteMiniAppModels';

class FakeMiniAppClient implements RemoteMiniAppClient {
  generation: number = 1;
  calls: RemoteMiniAppRequest[] = [];
  pendingOpen?: (payload: Object) => void;
  holdOpen: boolean = false;

  miniAppConnection(): number { return this.generation; }
  async miniApp(request: RemoteMiniAppRequest, connection: number): Promise<Object> {
    if (connection !== this.generation) throw new Error('Target changed');
    this.calls.push(request);
    if (request.action === 'list') {
      const value: RemoteMiniAppPayload = { apps: [{ id: 'game', name: 'Game', description: '' }] };
      return value;
    }
    if (request.action === 'open') {
      if (this.holdOpen) return new Promise<Object>((resolve) => { this.pendingOpen = resolve; });
      const value: RemoteMiniAppPayload = { id: 'game', name: 'Game', version: 3, html: '<p>Game</p>' };
      return value;
    }
    return 'saved';
  }
}

export default function miniAppUnitTest() {
  describe('MobileMiniApp', () => {
    it('binds_calls_to_the_open_app_and_refuses_a_replaced_device', 0, async () => {
      const client = new FakeMiniAppClient();
      const model = new MiniAppViewModel(client);
      await model.open();
      await model.select('game');
      const raw = '{"id":"rpc-1","method":"worker.call","params":{"method":"storage.get","params":{"key":"stats"}},"app_id":"attacker"}';
      const response = await model.request(raw);
      expect(response.includes('saved')).assertTrue();
      expect(client.calls[2].app_id).assertEqual('game');
      expect(client.calls[2].version).assertEqual(3);
      client.generation += 1;
      expect((await model.request(raw)).includes('Target changed')).assertTrue();
      expect(client.calls.length).assertEqual(3);
    });

    it('does_not_resurrect_a_closed_page_after_a_slow_open', 0, async () => {
      const client = new FakeMiniAppClient();
      const model = new MiniAppViewModel(client);
      await model.open();
      client.holdOpen = true;
      const loading = model.select('game');
      model.close();
      const payload: RemoteMiniAppPayload = { id: 'game', version: 3, html: '<p>Late</p>' };
      client.pendingOpen?.(payload);
      await loading;
      expect(model.state.visible).assertFalse();
      expect(model.state.document).assertEqual('');
    });

    it('rejects_unsupported_bridge_methods_before_transport', 0, async () => {
      const client = new FakeMiniAppClient();
      const model = new MiniAppViewModel(client);
      await model.open();
      await model.select('game');
      const result = await model.request('{"id":"bad","method":"host_invoke"}');
      expect(result.includes('error')).assertTrue();
      expect(client.calls.length).assertEqual(2);
    });

    it('keeps_app_markup_outside_the_trusted_wrapper_script', 0, () => {
      const html = MiniAppDocument.build('</script><script>window.pwned=1</script>', 'en-US');
      expect(html.includes('</script><script>window.pwned')).assertFalse();
      expect(html.includes('sandbox="allow-scripts"')).assertTrue();
      expect(html.includes('allow-same-origin')).assertFalse();
    });
  });
}
