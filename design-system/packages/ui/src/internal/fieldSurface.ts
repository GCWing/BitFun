import { createContext, useContext } from "react";

export type FieldSurface = "ambient" | "default";

export const FieldSurfaceContext = createContext<FieldSurface>("default");

export function useFieldSurface(): FieldSurface {
  return useContext(FieldSurfaceContext);
}
