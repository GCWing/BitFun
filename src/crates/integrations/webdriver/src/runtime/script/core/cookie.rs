pub(super) fn script() -> &'static str {
    r####"
    const parseDocumentCookies = (doc) => {
      const raw = doc.cookie || "";
      if (!raw.trim()) {
        return [];
      }
      return raw
        .split(/;\s*/)
        .filter(Boolean)
        .map((entry) => {
          const separator = entry.indexOf("=");
          const name = separator >= 0 ? entry.slice(0, separator) : entry;
          const value = separator >= 0 ? entry.slice(separator + 1) : "";
          return {
            name: decodeURIComponent(name),
            value: decodeURIComponent(value),
            path: null,
            domain: null,
            secure: false,
            httpOnly: false,
            expiry: null,
            sameSite: null
          };
        });
    };

    const getAllCookies = (frameContext = currentFrameContext) => parseDocumentCookies(getCurrentDocument(frameContext));

    const getCookie = (name, frameContext = currentFrameContext) =>
      getAllCookies(frameContext).find((cookie) => cookie.name === name) || null;

    const addCookie = (cookie, frameContext = currentFrameContext) => {
      if (!cookie || typeof cookie !== "object") {
        throw new Error("Invalid cookie payload");
      }
      if (!cookie.name) {
        throw new Error("Cookie name is required");
      }
      const doc = getCurrentDocument(frameContext);
      const parts = [
        `${encodeURIComponent(cookie.name)}=${encodeURIComponent(cookie.value ?? "")}`
      ];
      if (cookie.path) parts.push(`Path=${cookie.path}`);
      if (cookie.domain) parts.push(`Domain=${cookie.domain}`);
      if (cookie.expiry) parts.push(`Expires=${new Date(Number(cookie.expiry) * 1000).toUTCString()}`);
      if (cookie.secure) parts.push("Secure");
      if (cookie.sameSite) parts.push(`SameSite=${cookie.sameSite}`);
      doc.cookie = parts.join("; ");
      return null;
    };

    const deleteCookie = (name, frameContext = currentFrameContext) => {
      const doc = getCurrentDocument(frameContext);
      const expires = "Thu, 01 Jan 1970 00:00:00 GMT";
      doc.cookie = `${encodeURIComponent(name)}=; Expires=${expires}; Path=/`;
      doc.cookie = `${encodeURIComponent(name)}=; Expires=${expires}`;
      return null;
    };

    const deleteAllCookies = (frameContext = currentFrameContext) => {
      getAllCookies(frameContext).forEach((cookie) => {
        deleteCookie(cookie.name, frameContext);
      });
      return null;
    };
"####
}
