import { Module } from "../ability/type";

export class Loadable {
  static mod: ESObject;

  static async load(libName: string, mode: "async" | "sync" = "async"): Promise<Module> {
    if (!!Loadable.mod) {
      return Loadable.mod;
    }
    let currentMod: ESObject;
    if (mode === "async") {
      currentMod = await import(libName);
    } else {
      currentMod = loadNativeModule(libName);
    }
    if (
      typeof currentMod?.default?.render === "function" &&
      typeof currentMod?.default?.init === "function"
    ) {
      Loadable.mod = currentMod.default;
      return Loadable.mod;
    } else if (typeof currentMod?.render === "function" && typeof currentMod?.init === "function") {
      Loadable.mod = currentMod;
      return Loadable.mod;
    }
    throw new Error(`${libName} is not a valid dynamic library`);
  }
}
