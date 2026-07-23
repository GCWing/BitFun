import { Encoding } from '../Encoding';

interface AnthropicStreamDelta {
  type?: string;
  text?: string;
}

interface AnthropicStreamMessage {
  id?: string;
}

interface AnthropicStreamEvent {
  type?: string;
  delta?: AnthropicStreamDelta;
  message?: AnthropicStreamMessage;
}

interface OpenAiStreamDelta {
  content?: string;
}

interface OpenAiStreamChoice {
  delta?: OpenAiStreamDelta;
}

interface OpenAiStreamEvent {
  id?: string;
  choices?: OpenAiStreamChoice[];
}

export class ModelProviderSseParser {
  private readonly protocol: string;
  private buffer: string = '';
  private streamResponseId: string = '';

  constructor(protocol: string) {
    this.protocol = protocol;
  }

  feed(chunk: string): string[] {
    if (chunk.length === 0) {
      return [];
    }
    this.buffer += chunk;
    this.buffer = this.buffer.replace(/\r\n/g, '\n');
    const deltas: string[] = [];
    let separatorIndex = this.buffer.indexOf('\n\n');
    while (separatorIndex >= 0) {
      const eventText = this.buffer.slice(0, separatorIndex);
      this.buffer = this.buffer.slice(separatorIndex + 2);
      this.appendEventDeltas(eventText, deltas);
      separatorIndex = this.buffer.indexOf('\n\n');
    }
    return deltas;
  }

  finish(): string[] {
    const deltas: string[] = [];
    const remaining = this.buffer.trim();
    this.buffer = '';
    if (remaining.length > 0) {
      this.appendEventDeltas(remaining, deltas);
    }
    return deltas;
  }

  responseId(): string {
    return this.streamResponseId;
  }

  private appendEventDeltas(eventText: string, deltas: string[]): void {
    const lines = eventText.split('\n');
    let data = '';
    lines.forEach((line: string) => {
      if (line.startsWith('data:')) {
        const value = line.slice(5).trimStart();
        data += data.length > 0 ? `\n${value}` : value;
      }
    });
    if (data.length === 0 || data === '[DONE]') {
      return;
    }
    try {
      if (this.protocol === 'openai') {
        const event = Encoding.parseJsonObject<OpenAiStreamEvent>(data);
        if (event.id && this.streamResponseId.length === 0) {
          this.streamResponseId = event.id;
        }
        const choices = event.choices || [];
        choices.forEach((choice: OpenAiStreamChoice) => {
          const content = choice.delta && choice.delta.content ? choice.delta.content : '';
          if (content.length > 0) {
            deltas.push(content);
          }
        });
        return;
      }
      const event = Encoding.parseJsonObject<AnthropicStreamEvent>(data);
      if (event.message && event.message.id && this.streamResponseId.length === 0) {
        this.streamResponseId = event.message.id;
      }
      if (event.type === 'content_block_delta' && event.delta &&
        event.delta.type === 'text_delta' && event.delta.text) {
        deltas.push(event.delta.text);
      }
    } catch (_err) {
      // Ignore non-JSON keepalive events and incomplete provider metadata.
    }
  }
}
