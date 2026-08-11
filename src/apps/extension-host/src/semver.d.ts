declare module "semver" {
  export function satisfies(version: string, range: string): boolean
}
