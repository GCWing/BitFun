/** Portable, activation-scoped capabilities. Shell and transport adapters live outside this owner. */
export type CreationValue = null | boolean | number | string | CreationValue[] | { [key: string]: CreationValue };
type ParameterType = 'string' | 'number' | 'integer' | 'boolean' | 'object' | 'array';
export interface CreationParameter {
  type: ParameterType;
  description?: string;
  required?: boolean;
}
export interface CreationCommand {
  id: string;
  description: string;
  parameters?: Record<string, CreationParameter>;
}
type Handler = (args: Record<string, CreationValue>) => unknown | Promise<unknown>;
type Listener = (value: CreationValue) => void | Promise<void>;
const STATE_PREFIX = 'openbitfun.creation.state.v1:';
const TYPES: ParameterType[] = ['string', 'number', 'integer', 'boolean', 'object', 'array'];

function identifier(id: string): void {
  if (typeof id !== 'string' || !/^[a-z][a-z0-9_-]*(?:\.[a-z][a-z0-9_-]*)+$/.test(id)) {
    throw new Error('Creation identifiers must be namespaced, for example counter.increment');
  }
}

function jsonCopy(value: unknown): CreationValue {
  const text = JSON.stringify(value, (_key, item: unknown) => {
    if (typeof item === 'undefined' || typeof item === 'function' || typeof item === 'symbol'
      || (typeof item === 'number' && !Number.isFinite(item))) {
      throw new Error('Creation values must be JSON data');
    }
    return item;
  });
  if (text === undefined) throw new Error('Creation values must be JSON data');
  return JSON.parse(text) as CreationValue;
}

function matches(type: ParameterType, value: CreationValue): boolean {
  if (type === 'integer') return typeof value === 'number' && Number.isInteger(value);
  if (type === 'array') return Array.isArray(value);
  if (type === 'object') return value !== null && typeof value === 'object' && !Array.isArray(value);
  return typeof value === type;
}

export function createCreationCapabilities(options: { assertActive: () => void; storage: Storage }) {
  const commands = new Map<string, { definition: CreationCommand; handler: Handler }>();
  const listeners = new Map<string, Set<Listener>>();
  const diagnostics: { operation: string; message: string }[] = [];
  let disposed = false;
  const assertActive = () => {
    if (disposed) throw new Error('Creation runtime has been deactivated');
    options.assertActive();
  };
  const recordError = (operation: string, error: unknown) => {
    diagnostics.push({ operation, message: error instanceof Error ? error.message : String(error) });
    // A diagnostic tail is bounded independently of user state and execution.
    if (diagnostics.length > 100) diagnostics.shift();
  };
  const publish = async (id: string, value: CreationValue) => {
    assertActive(); identifier(id);
    const data = jsonCopy(value);
    const subscribers = [...(listeners.get(id) ?? [])];
    const results = await Promise.allSettled(subscribers.map(async listener => {
      assertActive();
      await listener(jsonCopy(data));
    }));
    const errors = results.flatMap(result => {
      if (result.status === 'fulfilled') return [];
      recordError(`event:${id}`, result.reason);
      return [result.reason instanceof Error ? result.reason.message : String(result.reason)];
    });
    return { delivered: results.length - errors.length, errors };
  };
  const stateKeys = () => {
    const keys: string[] = [];
    for (let i = 0; i < options.storage.length; i++) {
      const key = options.storage.key(i);
      if (key?.startsWith(STATE_PREFIX)) keys.push(key.slice(STATE_PREFIX.length));
    }
    return keys.sort();
  };
  const readState = (id: string, fallback: CreationValue = null): CreationValue => {
    assertActive(); identifier(id);
    const raw = options.storage.getItem(STATE_PREFIX + id);
    if (raw === null) return jsonCopy(fallback);
    try {
      const record = JSON.parse(raw);
      if (!record || record.schemaVersion !== 1 || !Object.prototype.hasOwnProperty.call(record, 'value')) throw new Error('Unsupported state format');
      return jsonCopy(record.value);
    } catch (error) {
      recordError(`state:${id}`, error);
      throw new Error(`Creation state ${id} is unreadable; its stored data was preserved`);
    }
  };
  const commandApi = Object.freeze({
    register(definition: CreationCommand, handler: Handler) {
      assertActive(); identifier(definition.id);
      if (!definition.description?.trim()) throw new Error('A command description is required');
      if (typeof handler !== 'function') throw new Error('A command handler is required');
      if (commands.has(definition.id)) throw new Error(`Creation command already registered: ${definition.id}`);
      const copy = jsonCopy({ ...definition, parameters: definition.parameters ?? {} }) as unknown as CreationCommand;
      for (const [name, parameter] of Object.entries(copy.parameters!)) {
        if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(name) || !parameter || !TYPES.includes(parameter.type)
          || (parameter.required !== undefined && typeof parameter.required !== 'boolean')
          || Object.keys(parameter).some(key => !['type', 'description', 'required'].includes(key))) {
          throw new Error(`Invalid Creation parameter: ${name}`);
        }
      }
      const entry = { definition: copy, handler };
      commands.set(copy.id, entry);
      return () => { if (commands.get(copy.id) === entry) commands.delete(copy.id); };
    },
    async invoke(id: string, args: Record<string, CreationValue> = {}) {
      assertActive();
      const command = commands.get(id);
      if (!command) throw new Error(`Unknown Creation command: ${id}`);
      try {
        const input = jsonCopy(args);
        if (!matches('object', input)) throw new Error('Command arguments must be an object');
        const values = input as Record<string, CreationValue>;
        const parameters = command.definition.parameters!;
        for (const name of Object.keys(values)) {
          if (!Object.prototype.hasOwnProperty.call(parameters, name)) throw new Error(`Unknown command argument: ${name}`);
        }
        for (const [name, parameter] of Object.entries(parameters)) {
          if (!Object.prototype.hasOwnProperty.call(values, name)) {
            if (parameter.required) throw new Error(`Missing command argument: ${name}`);
          } else if (!matches(parameter.type, values[name])) {
            throw new Error(`Command argument ${name} must be ${parameter.type}`);
          }
        }
        const result = await command.handler(values);
        assertActive();
        return jsonCopy(result === undefined ? null : result);
      } catch (error) { recordError(`command:${id}`, error); throw error; }
    },
  });
  return {
    commands: commandApi,
    events: Object.freeze({
      on(id: string, listener: Listener) {
        assertActive(); identifier(id);
        if (typeof listener !== 'function') throw new Error('An event listener is required');
        const set = listeners.get(id) ?? new Set<Listener>();
        set.add(listener); listeners.set(id, set);
        return () => { set.delete(listener); if (!set.size && listeners.get(id) === set) listeners.delete(id); };
      },
      emit: publish,
    }),
    state: Object.freeze({
      keys() { assertActive(); return stateKeys(); },
      get: readState,
      async set(id: string, value: CreationValue) {
        assertActive(); identifier(id);
        // Read first: never overwrite a corrupt or newer-format record implicitly.
        readState(id);
        const copy = jsonCopy(value);
        options.storage.setItem(STATE_PREFIX + id, JSON.stringify({ schemaVersion: 1, value: copy }));
        await publish('state.changed', { key: id, value: copy });
        return copy;
      },
      async delete(id: string) {
        assertActive(); identifier(id);
        options.storage.removeItem(STATE_PREFIX + id);
        await publish('state.changed', { key: id, deleted: true });
      },
    }),
    inspect() {
      assertActive();
      return {
        apiVersion: 1,
        commands: [...commands.values()].map(({ definition }) => ({
          id: definition.id, description: definition.description,
          inputSchema: {
            type: 'object', additionalProperties: false,
            required: Object.entries(definition.parameters!).filter(([, value]) => value.required).map(([name]) => name),
            properties: Object.fromEntries(Object.entries(definition.parameters!).map(([name, value]) =>
              [name, { type: value.type, ...(value.description ? { description: value.description } : {}) }])),
          },
        })),
        events: [...listeners.keys()].sort(),
        stateKeys: stateKeys(),
        diagnostics: diagnostics.map(entry => ({ ...entry })),
      };
    },
    dispose() { disposed = true; commands.clear(); listeners.clear(); },
  };
}
