import type { ComponentMeta } from "../../registry.types";

export const switchMeta = {
  category: "form",
  description: "Toggles a single setting between on and off with native checkbox semantics.",
  maturity: "stable",
  name: "Switch",
  props: [
    { name: "checked", type: "boolean" },
    { name: "defaultChecked", type: "boolean" },
    { defaultValue: "false", name: "disabled", type: "boolean" },
    { name: "onChange", type: "ChangeEventHandler<HTMLInputElement>" },
    { name: "onCheckedChange", type: "(checked: boolean) => void" },
  ],
  states: ["off", "on", "focus-visible", "disabled"],
  tokens: [
    "color.control.switch.track",
    "color.control.switch.trackChecked",
    "color.control.switch.thumb",
    "color.focus.ring",
    "control.switch.trackWidth",
    "control.switch.trackHeight",
    "control.switch.thumbSize",
    "control.switch.thumbInset",
    "control.switch.thumbTravel",
    "control.switch.thumbTravelReverse",
    "radius.pill",
  ],
} as const satisfies ComponentMeta;
