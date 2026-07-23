pub(super) fn script() -> &'static str {
    r####"
    const W3C_KEY_MAP = {
      "\uE000": "Unidentified",
      "\uE001": "Cancel",
      "\uE002": "Help",
      "\uE003": "Backspace",
      "\uE004": "Tab",
      "\uE005": "Clear",
      "\uE006": "Enter",
      "\uE007": "Enter",
      "\uE008": "Shift",
      "\uE009": "Control",
      "\uE00A": "Alt",
      "\uE00B": "Pause",
      "\uE00C": "Escape",
      "\uE00D": " ",
      "\uE00E": "PageUp",
      "\uE00F": "PageDown",
      "\uE010": "End",
      "\uE011": "Home",
      "\uE012": "ArrowLeft",
      "\uE013": "ArrowUp",
      "\uE014": "ArrowRight",
      "\uE015": "ArrowDown",
      "\uE016": "Insert",
      "\uE017": "Delete",
      "\uE031": "F1",
      "\uE032": "F2",
      "\uE033": "F3",
      "\uE034": "F4",
      "\uE035": "F5",
      "\uE036": "F6",
      "\uE037": "F7",
      "\uE038": "F8",
      "\uE039": "F9",
      "\uE03A": "F10",
      "\uE03B": "F11",
      "\uE03C": "F12",
      "\uE03D": "Meta"
    };

    const normalizeKeyValue = (value) => W3C_KEY_MAP[String(value)] || String(value || "");

    const isModifierKey = (key) =>
      key === "Control" || key === "Shift" || key === "Alt" || key === "Meta";

    const updateModifierState = (modifiers, key, isDown) => {
      if (key === "Control") modifiers.ctrl = isDown;
      if (key === "Shift") modifiers.shift = isDown;
      if (key === "Alt") modifiers.alt = isDown;
      if (key === "Meta") modifiers.meta = isDown;
    };

    const eventCodeForKey = (key) => {
      const specialCodes = {
        " ": "Space",
        Backspace: "Backspace",
        Tab: "Tab",
        Enter: "Enter",
        Escape: "Escape",
        Delete: "Delete",
        Insert: "Insert",
        Home: "Home",
        End: "End",
        PageUp: "PageUp",
        PageDown: "PageDown",
        ArrowLeft: "ArrowLeft",
        ArrowRight: "ArrowRight",
        ArrowUp: "ArrowUp",
        ArrowDown: "ArrowDown",
        Shift: "ShiftLeft",
        Control: "ControlLeft",
        Alt: "AltLeft",
        Meta: "MetaLeft"
      };
      if (specialCodes[key]) {
        return specialCodes[key];
      }
      if (/^F\d{1,2}$/.test(key)) {
        return key;
      }
      if (key.length === 1) {
        if (/^[a-z]$/i.test(key)) {
          return `Key${key.toUpperCase()}`;
        }
        if (/^\d$/.test(key)) {
          return `Digit${key}`;
        }
      }
      return key || "Unidentified";
    };

    const getPrintableKey = (key, modifiers) => {
      if (key === " ") {
        return " ";
      }
      if (key.length !== 1) {
        return key;
      }
      if (modifiers.shift && /^[a-z]$/.test(key)) {
        return key.toUpperCase();
      }
      return key;
    };
"####
}
