// A preset row: registers one model-facing tool named from config, so a test
// can read a session's composition off its tool list.
//
// Import-free on purpose — the Loader resolves a composition row through Node's
// ESM resolver, which cannot see this package's TypeScript sources.
export const name = 'contribute'
export const inject = ['tools']

export function apply(ctx, config) {
  ctx.effect(() => ctx.tools.register({
    name: config.tool,
    description: `fixture tool ${config.tool}`,
    parameters: { type: 'object', properties: {}, additionalProperties: false },
    output: { schema: { type: 'string' }, render: (_args, value) => [{ type: 'text', text: String(value) }] },
    execute: () => Promise.resolve(config.tool),
  }))
}
