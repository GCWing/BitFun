import process from "@ohos.process";

/**
 * exit current application
 */
export const exit = (code: number) => {
  const pro = new process.ProcessManager();
  pro.exit(code);
};
