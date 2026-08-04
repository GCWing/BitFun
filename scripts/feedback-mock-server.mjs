import http from 'node:http';
import { randomUUID } from 'node:crypto';

const port = Number.parseInt(process.env.BITFUN_FEEDBACK_MOCK_PORT ?? '38971', 10);
const enrollments = new Map();
const refreshTokens = new Map();
const createRequests = new Map();
const replyRequests = new Map();
const records = new Map();
let nextFault = null;

const server = http.createServer(async (request, response) => {
  const requestId = normalizeRequestId(header(request, 'x-request-id'));
  const url = new URL(request.url ?? '/', `http://${request.headers.host ?? 'localhost'}`);
  logRequestStage(requestStage(request.method, url.pathname), requestId);

  if (request.method === 'GET' && url.pathname === '/health') {
    return send(response, 200, { status: 'ok' }, requestId);
  }
  if (request.method === 'POST' && url.pathname === '/__mock/control') {
    const body = await readJson(request);
    nextFault = typeof body.fault === 'string' ? body.fault : null;
    if (Number.isInteger(body.seed_inbox) && body.seed_inbox > 0) {
      seedInbox(Math.min(body.seed_inbox, 120));
    }
    if (body.admin_reply && typeof body.admin_reply.feedback_id === 'string') {
      addAdminReply(body.admin_reply.feedback_id, body.admin_reply.content ?? 'Mock support reply');
    }
    if (body.seed_messages && typeof body.seed_messages.feedback_id === 'string') {
      seedMessages(body.seed_messages.feedback_id, Math.min(body.seed_messages.count ?? 0, 250));
    }
    return send(response, 200, { next_fault: nextFault, records: records.size }, requestId);
  }

  const fault = takeFault();
  if (fault === 'timeout') {
    setTimeout(() => sendError(response, 500, 'INTERNAL_ERROR', requestId), 25_000);
    return;
  }
  if (fault === '403') return sendError(response, 403, 'SCOPE_INSUFFICIENT', requestId);
  if (fault === '429') return sendError(response, 429, 'RATE_LIMITED', requestId, { 'Retry-After': '30' });
  if (fault === '5xx') return sendError(response, 503, 'INTERNAL_ERROR', requestId);
  if (fault === '401') return sendError(response, 401, 'ACCESS_TOKEN_INVALID', requestId);
  if (fault === 'cursor_invalid') return sendError(response, 400, 'CURSOR_INVALID', requestId);
  if (fault === 'capability_invalid') return sendError(response, 403, 'CAPABILITY_INVALID', requestId);

  if (request.method === 'POST' && url.pathname === '/auth/v1/anonymous/enroll') {
    const body = await readJson(request);
    const idempotencyKey = requireUuidHeader(request, response, requestId);
    if (!idempotencyKey) return;
    if (typeof body.key !== 'string' || body.key.length === 0) {
      return sendError(response, 400, 'ENROLL_KEY_REQUIRED', requestId);
    }
    const identity = `${body.key}:${idempotencyKey}`;
    const existing = enrollments.get(identity);
    if (existing) return send(response, 201, existing, requestId, { 'Idempotency-Replayed': 'true' });
    const created = tokenPair(randomUUID());
    enrollments.set(identity, created);
    refreshTokens.set(created.refresh_token, created.anonymous_id);
    return send(response, 201, created, requestId, { 'Idempotency-Replayed': 'false' });
  }

  if (request.method === 'POST' && url.pathname === '/auth/v1/anonymous/token') {
    const body = await readJson(request);
    const anonymousId = refreshTokens.get(body.refresh_token);
    if (!anonymousId) return sendError(response, 401, 'REFRESH_TOKEN_INVALID', requestId);
    refreshTokens.delete(body.refresh_token);
    const refreshed = tokenPair(anonymousId);
    refreshTokens.set(refreshed.refresh_token, anonymousId);
    return send(response, 200, refreshed, requestId);
  }

  if (request.method === 'POST' && url.pathname === '/support/v1/feedback') {
    if (!validBearer(request)) return sendError(response, 401, 'ACCESS_TOKEN_INVALID', requestId);
    const body = await readJson(request);
    const idempotencyKey = requireUuidHeader(request, response, requestId);
    if (!idempotencyKey) return;
    const fingerprint = JSON.stringify(body);
    const existing = createRequests.get(idempotencyKey);
    if (existing && existing.fingerprint !== fingerprint) {
      return sendError(response, 409, 'FEEDBACK_IDEMPOTENT_CONFLICT', requestId);
    }
    if (existing) {
      return send(response, 201, { ...existing.result, idempotency_replayed: true }, requestId, {
        'Idempotency-Replayed': 'true',
      });
    }
    if (!['runtime_error', 'feature_request', 'usage_question', 'other'].includes(body.category)) {
      return sendError(response, 400, 'CATEGORY_INVALID', requestId);
    }
    if (typeof body.content !== 'string' || body.content.trim().length === 0) {
      return sendError(response, 400, 'CONTENT_EMPTY', requestId);
    }
    if (Array.from(body.content.trim()).length > 2_000) {
      return sendError(response, 400, 'CONTENT_TOO_LONG', requestId);
    }
    const feedbackId = randomUUID();
    const createdAt = new Date().toISOString();
    const result = {
      feedback_id: feedbackId,
      capability_token: randomUUID(),
      status: 'submitted',
      inbox_cursor: new Date().toISOString(),
      schema_version: '1.0.0',
    };
    if (fault === 'capability_missing') delete result.capability_token;
    createRequests.set(idempotencyKey, { fingerprint, result });
    records.set(feedbackId, {
      ...body,
      ...result,
      has_new_reply: false,
      created_at: createdAt,
      updated_at: createdAt,
      read_cursor: createdAt,
      messages: [{
        message_id: randomUUID(),
        sender_type: 'user',
        content: body.content.trim(),
        content_deleted: false,
        created_at: createdAt,
      }],
    });
    return send(response, 201, result, requestId, { 'Idempotency-Replayed': 'false' });
  }

  if (request.method === 'GET' && url.pathname === '/support/v1/feedback/inbox') {
    if (!validBearer(request)) return sendError(response, 401, 'ACCESS_TOKEN_INVALID', requestId);
    const limit = parseLimit(url.searchParams.get('limit'), 20, 100);
    if (limit === null) return sendError(response, 400, 'PAGE_SIZE_INVALID', requestId);
    const offset = decodeCursor(url.searchParams.get('cursor'));
    if (offset === null) return sendError(response, 400, 'CURSOR_INVALID', requestId);
    const ordered = [...records.values()].sort((left, right) =>
      right.created_at.localeCompare(left.created_at) || right.feedback_id.localeCompare(left.feedback_id));
    const page = ordered.slice(offset, offset + limit);
    const nextOffset = offset + page.length;
    return send(response, 200, {
      items: page.map(record => ({
        feedback_id: record.feedback_id,
        category: record.category,
        status: record.status,
        has_new_reply: record.has_new_reply,
        created_at: record.created_at,
        updated_at: record.updated_at,
      })),
      cursor: encodeCursor(nextOffset),
      has_more: nextOffset < ordered.length,
    }, requestId);
  }

  const messagesMatch = /^\/support\/v1\/feedback\/([^/]+)\/messages$/.exec(url.pathname);
  if (request.method === 'POST' && messagesMatch) {
    const record = authorizeRecord(request, response, requestId, messagesMatch[1]);
    if (!record) return;
    const idempotencyKey = requireUuidHeader(request, response, requestId);
    if (!idempotencyKey) return;
    const body = await readJson(request);
    const fingerprint = `${record.feedback_id}:${JSON.stringify(body)}`;
    const existing = replyRequests.get(idempotencyKey);
    if (existing && existing.fingerprint !== fingerprint) {
      return sendError(response, 409, 'IDEMPOTENCY_CONFLICT', requestId);
    }
    if (existing) {
      return send(response, 201, existing.result, requestId, {
        'Idempotency-Replayed': 'true',
      });
    }
    if (record.status === 'resolved') {
      return sendError(response, 409, 'FEEDBACK_ALREADY_RESOLVED', requestId);
    }
    if (typeof body.content !== 'string' || body.content.trim().length === 0) {
      return sendError(response, 400, 'CONTENT_EMPTY', requestId);
    }
    if (Array.from(body.content.trim()).length > 2_000) {
      return sendError(response, 400, 'CONTENT_TOO_LONG', requestId);
    }
    const previousTime = Date.parse(record.messages.at(-1)?.created_at ?? record.created_at);
    const createdAt = new Date(Math.max(Date.now(), previousTime + 1)).toISOString();
    const message = {
      message_id: randomUUID(),
      sender_type: 'user',
      content: body.content.trim(),
      content_deleted: false,
      created_at: createdAt,
    };
    record.messages.push(message);
    record.status = 'in_progress';
    record.has_new_reply = false;
    record.updated_at = createdAt;
    const result = {
      message_id: message.message_id,
      sender_type: message.sender_type,
      created_at: message.created_at,
      feedback_status: record.status,
    };
    replyRequests.set(idempotencyKey, { fingerprint, result });
    return send(response, 201, result, requestId, { 'Idempotency-Replayed': 'false' });
  }
  if (request.method === 'GET' && messagesMatch) {
    const record = authorizeRecord(request, response, requestId, messagesMatch[1]);
    if (!record) return;
    const limit = parseLimit(url.searchParams.get('limit'), 50, 200);
    if (limit === null) return sendError(response, 400, 'PAGE_SIZE_INVALID', requestId);
    const offset = decodeCursor(url.searchParams.get('cursor'), 'messages');
    if (offset === null) return sendError(response, 400, 'CURSOR_INVALID', requestId);
    const page = record.messages.slice(offset, offset + limit);
    const nextOffset = offset + page.length;
    return send(response, 200, {
      feedback_id: record.feedback_id,
      messages: page,
      cursor: encodeCursor(nextOffset, 'messages'),
      has_more: nextOffset < record.messages.length,
    }, requestId);
  }

  const ackMatch = /^\/support\/v1\/feedback\/([^/]+)\/ack$/.exec(url.pathname);
  if (request.method === 'POST' && ackMatch) {
    const record = authorizeRecord(request, response, requestId, ackMatch[1]);
    if (!record) return;
    const body = await readJson(request);
    const requested = Date.parse(body.read_cursor);
    if (!Number.isFinite(requested)) return sendError(response, 400, 'READ_CURSOR_INVALID', requestId);
    const latest = record.messages.at(-1)?.created_at ?? record.created_at;
    const effective = new Date(Math.max(
      Date.parse(record.read_cursor ?? record.created_at),
      Math.min(requested, Date.parse(latest)),
    )).toISOString();
    record.read_cursor = effective;
    if (record.status === 'waiting_user' && Date.parse(effective) >= Date.parse(latest)) {
      record.status = 'in_progress';
    }
    record.has_new_reply = record.messages.some(message =>
      message.sender_type === 'admin' && Date.parse(message.created_at) > Date.parse(effective));
    return send(response, 200, {
      feedback_id: record.feedback_id,
      read_cursor: effective,
      feedback_status: record.status,
    }, requestId);
  }

  return sendError(response, 404, 'NOT_FOUND', requestId);
});

server.listen(port, '127.0.0.1', () => {
  process.stdout.write(`Feedback mock listening at http://127.0.0.1:${port}\n`);
});

function tokenPair(anonymousId) {
  return {
    anonymous_id: anonymousId,
    access_token: `access-${randomUUID()}`,
    refresh_token: `refresh-${randomUUID()}`,
    expires_in: 3_600,
    refresh_expires_in: 2_592_000,
    scope: 'feedback:write,feedback:read',
    schema_version: '1.0.0',
  };
}

function seedInbox(count) {
  for (let index = 0; index < count; index += 1) {
    const feedbackId = randomUUID();
    const createdAt = new Date(Date.now() - index * 60_000).toISOString();
    records.set(feedbackId, {
      feedback_id: feedbackId,
      category: ['runtime_error', 'feature_request', 'usage_question', 'other'][index % 4],
      status: ['submitted', 'in_progress', 'waiting_user', 'resolved'][index % 4],
      has_new_reply: index % 3 === 0,
      created_at: createdAt,
      updated_at: createdAt,
    });
  }
}

function parseLimit(value, fallback, maximum) {
  if (value === null) return fallback;
  const parsed = Number.parseInt(value, 10);
  return Number.isInteger(parsed) && parsed >= 1 && parsed <= maximum ? parsed : null;
}

function encodeCursor(offset, kind = 'inbox') {
  return Buffer.from(`${kind}:${offset}`, 'utf8').toString('base64url');
}

function decodeCursor(cursor, kind = 'inbox') {
  if (cursor === null) return 0;
  try {
    const decoded = Buffer.from(cursor, 'base64url').toString('utf8');
    const match = new RegExp(`^${kind}:(\\d+)$`).exec(decoded);
    return match ? Number.parseInt(match[1], 10) : null;
  } catch {
    return null;
  }
}

function authorizeRecord(request, response, requestId, feedbackId) {
  if (!validBearer(request)) {
    sendError(response, 401, 'ACCESS_TOKEN_INVALID', requestId);
    return null;
  }
  const record = records.get(feedbackId);
  if (!record) {
    sendError(response, 404, 'FEEDBACK_NOT_FOUND', requestId);
    return null;
  }
  if (!record.capability_token
    || header(request, 'x-feedback-capability') !== record.capability_token) {
    sendError(response, 403, 'CAPABILITY_INVALID', requestId);
    return null;
  }
  return record;
}

function addAdminReply(feedbackId, content) {
  const record = records.get(feedbackId);
  if (!record || !Array.isArray(record.messages)) return;
  const createdAt = new Date(Date.now() + record.messages.length).toISOString();
  record.messages.push({
    message_id: randomUUID(),
    sender_type: 'admin',
    content: String(content),
    content_deleted: false,
    created_at: createdAt,
  });
  record.status = 'waiting_user';
  record.has_new_reply = true;
  record.updated_at = createdAt;
}

function seedMessages(feedbackId, count) {
  const record = records.get(feedbackId);
  if (!record || !Array.isArray(record.messages) || count <= 0) return;
  for (let index = 0; index < count; index += 1) {
    const createdAt = new Date(Date.parse(record.created_at) + (index + 1) * 1_000).toISOString();
    record.messages.push({
      message_id: randomUUID(),
      sender_type: index % 2 === 0 ? 'admin' : 'user',
      content: `Mock message ${index + 1}`,
      content_deleted: false,
      created_at: createdAt,
    });
  }
  record.status = 'waiting_user';
  record.has_new_reply = true;
  record.updated_at = record.messages.at(-1).created_at;
}

function takeFault() {
  const fault = nextFault;
  nextFault = null;
  return fault;
}

function validBearer(request) {
  return /^Bearer access-/.test(header(request, 'authorization') ?? '');
}

function requireUuidHeader(request, response, requestId) {
  const value = header(request, 'idempotency-key');
  if (!value || !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)) {
    sendError(response, 409, 'IDEMPOTENCY_KEY_INVALID', requestId);
    return null;
  }
  return value;
}

function header(request, name) {
  const value = request.headers[name];
  return Array.isArray(value) ? value[0] : value;
}

function normalizeRequestId(value) {
  return typeof value === 'string' && /^[a-zA-Z0-9._:-]{1,128}$/.test(value)
    ? value
    : randomUUID();
}

function requestStage(method, pathname) {
  if (method === 'GET' && pathname === '/health') return 'health';
  if (method === 'POST' && pathname === '/__mock/control') return 'control';
  if (method === 'POST' && pathname === '/auth/v1/anonymous/enroll') return 'enroll';
  if (method === 'POST' && pathname === '/auth/v1/anonymous/token') return 'refresh';
  if (method === 'POST' && pathname === '/support/v1/feedback') return 'create';
  if (method === 'GET' && pathname === '/support/v1/feedback/inbox') return 'inbox';
  if (/^\/support\/v1\/feedback\/[^/]+\/messages$/.test(pathname)) {
    return method === 'GET' ? 'message_history' : method === 'POST' ? 'reply' : 'unknown';
  }
  if (method === 'POST' && /^\/support\/v1\/feedback\/[^/]+\/ack$/.test(pathname)) {
    return 'acknowledge';
  }
  return 'unknown';
}

function logRequestStage(stage, requestId) {
  process.stdout.write(`${JSON.stringify({ stage, requestId })}\n`);
}

async function readJson(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  if (chunks.length === 0) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    return {};
  }
}

function sendError(response, status, code, requestId, headers = {}) {
  send(response, status, {
    error_code: code,
    error_message: 'Mock diagnostic text must never be shown directly by the client.',
    request_id: requestId,
  }, requestId, headers);
}

function send(response, status, body, requestId, headers = {}) {
  response.writeHead(status, {
    'Cache-Control': 'no-store',
    'Content-Type': 'application/json',
    'X-Request-ID': requestId,
    ...headers,
  });
  response.end(JSON.stringify(body));
}
