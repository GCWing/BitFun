 

export interface TauriCommandErrorContext {
  command: string;
  request?: any;
  originalError?: any;
  timestamp?: string;
}

export class TauriCommandError extends Error {
  public readonly context: TauriCommandErrorContext;
  public readonly isTauriCommandError = true;

  constructor(
    message: string,
    context: TauriCommandErrorContext
  ) {
    super(message);
    this.name = 'TauriCommandError';
    this.context = {
      ...context,
      timestamp: new Date().toISOString()
    };

    
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, TauriCommandError);
    }
  }

   
  public getFormattedMessage(): string {
    const { command, request, originalError, timestamp } = this.context;
    
    let message = `[${timestamp}] Tauri command failed: ${command}\n`;
    message += `Error message: ${this.message}\n`;
    
    if (request) {
      message += `Request payload: ${JSON.stringify(request, null, 2)}\n`;
    }
    
    if (originalError) {
      message += `Original error: ${originalError.message || originalError}\n`;
    }
    
    return message;
  }

   
  public isNetworkError(): boolean {
    const message = this.message.toLowerCase();
    return message.includes('network') || 
           message.includes('connection') || 
           message.includes('timeout') ||
           message.includes('fetch');
  }

   
  public isPermissionError(): boolean {
    const message = this.message.toLowerCase();
    return message.includes('permission') || 
           message.includes('access') || 
           message.includes('unauthorized') ||
           message.includes('forbidden');
  }

   
  public isParameterError(): boolean {
    const message = this.message.toLowerCase();
    return message.includes('parameter') || 
           message.includes('argument') || 
           message.includes('invalid') ||
           message.includes('missing');
  }
}

 
export function createTauriCommandError(
  command: string,
  originalError: any,
  request?: any
): TauriCommandError {
  let message = 'Unknown error';
  
  if (originalError?.message) {
    message = originalError.message;
  } else if (typeof originalError === 'string') {
    message = originalError;
  }

  return new TauriCommandError(message, {
    command,
    request,
    originalError
  });
}

 
export function isTauriCommandError(error: any): error is TauriCommandError {
  return error && error.isTauriCommandError === true;
}

const SESSION_IN_USE_PREFIX = 'session_in_use:';

/** Recognizes the stable Desktop/Peer error code without parsing localized prose. */
export function isSessionInUseError(error: unknown): boolean {
  const pending: unknown[] = [error];
  const seen = new Set<object>();

  for (let inspected = 0; pending.length > 0 && inspected < 12; inspected += 1) {
    const current = pending.shift();
    if (typeof current === 'string') {
      if (current.trimStart().startsWith(SESSION_IN_USE_PREFIX)) return true;
      continue;
    }
    if (!current || typeof current !== 'object' || seen.has(current)) continue;
    seen.add(current);

    const candidate = current as {
      message?: unknown;
      originalError?: unknown;
      context?: { originalError?: unknown };
      details?: { originalError?: unknown };
    };
    pending.push(
      candidate.message,
      candidate.originalError,
      candidate.context?.originalError,
      candidate.details?.originalError,
    );
  }

  return false;
}
