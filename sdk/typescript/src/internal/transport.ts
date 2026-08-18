import type { Readable, Writable } from "node:stream";
import { StringDecoder } from "node:string_decoder";

import type {
  MessageTransport,
  MessageTransportObserver,
} from "../../../../src/crates/adapters/transport/typescript/src/index.js";

export interface HostTransport {
  readable: Readable;
  writable: Writable;
  close(): Promise<void>;
}

/** SDK-owned newline framing over a managed Host's stdio carrier. */
export class HostMessageTransport implements MessageTransport {
  readonly #host: HostTransport;
  readonly #maxLineBytes: number;
  #observer?: MessageTransportObserver;
  #closePromise?: Promise<void>;

  constructor(host: HostTransport, maxLineBytes: number) {
    this.#host = host;
    this.#maxLineBytes = maxLineBytes;
  }

  subscribe(observer: MessageTransportObserver): () => void {
    if (this.#observer !== undefined) {
      throw new Error("SDK Host transport already has a message owner");
    }
    this.#observer = observer;
    void this.#readLoop(observer);
    return () => {
      if (this.#observer === observer) {
        this.#observer = undefined;
      }
    };
  }

  send(message: string): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.#host.writable.write(`${message}\n`, (error) => {
          if (error !== null && error !== undefined) {
            reject(error);
          } else {
            resolve();
          }
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  close(): Promise<void> {
    this.#closePromise ??= this.#host.close();
    return this.#closePromise;
  }

  async #readLoop(observer: MessageTransportObserver): Promise<void> {
    try {
      for await (const line of readBoundedLines(
        this.#host.readable,
        this.#maxLineBytes,
      )) {
        if (this.#observer !== observer) {
          return;
        }
        observer.message(line);
      }
      if (this.#observer === observer) {
        observer.close(new Error("SDK Host closed its output stream"));
      }
    } catch (error) {
      if (this.#observer === observer) {
        observer.close(error);
      }
    }
  }
}

async function* readBoundedLines(
  readable: Readable,
  maxLineBytes: number,
): AsyncGenerator<string> {
  const decoder = new StringDecoder("utf8");
  let buffered = "";
  for await (const chunk of readable) {
    buffered +=
      typeof chunk === "string" ? chunk : decoder.write(chunk as Buffer);
    let newline = buffered.indexOf("\n");
    while (newline >= 0) {
      let line = buffered.slice(0, newline);
      buffered = buffered.slice(newline + 1);
      if (Buffer.byteLength(line, "utf8") > maxLineBytes) {
        throw new Error("SDK Host response line exceeds the size limit");
      }
      if (line.endsWith("\r")) {
        line = line.slice(0, -1);
      }
      yield line;
      newline = buffered.indexOf("\n");
    }
    if (Buffer.byteLength(buffered, "utf8") > maxLineBytes) {
      throw new Error("SDK Host response line exceeds the size limit");
    }
  }
  buffered += decoder.end();
  if (buffered.length > 0) {
    throw new Error("SDK Host output ended with an unterminated response line");
  }
}
