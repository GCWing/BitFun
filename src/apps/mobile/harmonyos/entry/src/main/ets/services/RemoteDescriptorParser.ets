import { RemoteDescriptor } from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';

export class RemoteDescriptorParser {
  static parse(input: string): RemoteDescriptor {
    const value = input.trim();
    if (!value) {
      throw new Error(RemoteI18n.t('errors.remoteUrlRequired'));
    }

    const query = RemoteDescriptorParser.extractQuery(value);
    const params = RemoteDescriptorParser.parseQuery(query);
    const roomId = params.get('room') || '';
    const publicKey = params.get('pk') || '';
    const relay = params.get('relay') || '';

    if (!roomId || !publicKey) {
      throw new Error(RemoteI18n.t('errors.remoteUrlMissingParams'));
    }

    return {
      relayUrl: RemoteDescriptorParser.resolveRelayBaseUrl(value, relay),
      roomId,
      publicKey
    };
  }

  private static extractQuery(value: string): string {
    const pairIndex = value.indexOf('#/pair?');
    if (pairIndex >= 0) {
      return value.slice(pairIndex + '#/pair?'.length);
    }
    const questionIndex = value.indexOf('?');
    if (questionIndex >= 0) {
      return value.slice(questionIndex + 1);
    }
    return value;
  }

  private static parseQuery(query: string): Map<string, string> {
    const params = new Map<string, string>();
    const pairs = query.split('&');
    pairs.forEach((pair: string) => {
      const eq = pair.indexOf('=');
      const rawKey = eq >= 0 ? pair.slice(0, eq) : pair;
      const rawValue = eq >= 0 ? pair.slice(eq + 1) : '';
      if (rawKey) {
        params.set(decodeURIComponent(rawKey), decodeURIComponent(rawValue));
      }
    });
    return params;
  }

  private static resolveRelayBaseUrl(fullUrl: string, relayParam: string): string {
    const relay = relayParam
      .replace(/^wss:\/\//, 'https://')
      .replace(/^ws:\/\//, 'http://')
      .replace(/\/ws\/?$/, '')
      .replace(/\/$/, '');
    if (relay) {
      return relay;
    }

    const hashIndex = fullUrl.indexOf('#');
    const withoutHash = hashIndex >= 0 ? fullUrl.slice(0, hashIndex) : fullUrl;
    const questionIndex = withoutHash.indexOf('?');
    const withoutQuery = questionIndex >= 0 ? withoutHash.slice(0, questionIndex) : withoutHash;
    return withoutQuery
      .replace(/\/r\/[^/]*$/, '')
      .replace(/\/[^/]*$/, '')
      .replace(/\/$/, '');
  }
}
