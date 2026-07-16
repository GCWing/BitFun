export declare function registerArktsFunction(funcName: string, callback: ((err: Error | null, arg: string) => Promise<string>)): void;
export declare function setBuildResult(msg: string): void;
export declare function ohosMarkCleanShutdown(): void;
export declare function getAppConfigBool(path: string): boolean;