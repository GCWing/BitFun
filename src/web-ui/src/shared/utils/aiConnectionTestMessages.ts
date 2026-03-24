type TranslateFn = (key: string) => string;

export type ConnectionTestMessageCode =
  | 'tool_calls_not_detected'
  | 'image_input_check_failed';

const MESSAGE_KEY_BY_CODE: Record<ConnectionTestMessageCode, string> = {
  tool_calls_not_detected: 'messages.connectionTestMessages.toolCallsNotDetected',
  image_input_check_failed: 'messages.connectionTestMessages.imageInputCheckFailed',
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
