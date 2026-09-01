import { useContext } from "react";
import {
  DesignSystemContext,
  type DesignSystemContextValue,
} from "../providers/DesignSystemProvider.context";

export function useDesignSystem(): DesignSystemContextValue {
  return useContext(DesignSystemContext);
}
