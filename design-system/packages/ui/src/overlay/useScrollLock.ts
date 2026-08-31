import { useEffect } from "react";

const lockCounts = new WeakMap<Document, number>();
const previousOverflow = new WeakMap<Document, string>();

export function useScrollLock(active: boolean, ownerDocument?: Document | null): void {
  useEffect(() => {
    if (!active) return;
    const documentOwner = ownerDocument
      ?? (typeof document === "undefined" ? null : document);
    if (!documentOwner) return;

    const nextCount = (lockCounts.get(documentOwner) ?? 0) + 1;
    if (nextCount === 1) {
      previousOverflow.set(documentOwner, documentOwner.body.style.overflow);
      documentOwner.body.style.overflow = "hidden";
    }
    lockCounts.set(documentOwner, nextCount);

    return () => {
      const count = Math.max(0, (lockCounts.get(documentOwner) ?? 1) - 1);
      if (count === 0) {
        documentOwner.body.style.overflow = previousOverflow.get(documentOwner) ?? "";
        previousOverflow.delete(documentOwner);
        lockCounts.delete(documentOwner);
      } else {
        lockCounts.set(documentOwner, count);
      }
    };
  }, [active, ownerDocument]);
}
