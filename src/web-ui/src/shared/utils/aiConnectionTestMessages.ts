type TranslateFn = (key: string) => string;

export type ConnectionTestMessageCode =
  | 'tool_calls_not_detected'
  | 'image_input_check_failed'
  | 'tls_or_certificate_issue'
  | 'proxy_issue'
  | 'network_issue';

const MESSAGE_KEY_BY_CODE: Record<ConnectionTestMessageCode, string> = {
  tool_calls_not_detected: 'messages.connectionTestMessages.toolCallsNotDetected',
  image_input_check_failed: 'messages.connectionTestMessages.imageInputCheckFailed',
  tls_or_certificate_issue: 'messages.connectionTestMessages.tlsOrCertificateIssue',
  proxy_issue: 'messages.connectionTestMessages.proxyIssue',
  network_issue: 'messages.connectionTestMessages.networkIssue',
};

export function translateConnectionTestMessage(
  messageCode: ConnectionTestMessageCode | undefined,
  t: TranslateFn
): string | undefined {
  if (!messageCode) {
    return undefined;
  }

  const translationKey = MESSAGE_KEY_BY_CODE[messageCode];
  return translationKey ? t(translationKey) : undefined;
}
