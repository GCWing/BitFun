import {
  forwardRef,
  type CSSProperties,
  type HTMLAttributes,
} from "react";
import { classNames } from "../../internal/classNames";
import arrowLeftUrl from "./assets/arrow-left.svg";
import arrowRightUrl from "./assets/arrow-right.svg";
import arrowUpUrl from "./assets/arrow-up.svg";
import arrowUpRightUrl from "./assets/arrow-upright.svg";
import bellUrl from "./assets/bell.svg";
import browserUrl from "./assets/browser.svg";
import checkCircleUrl from "./assets/check-circle.svg";
import checkFillUrl from "./assets/check-fill.svg";
import checkLineUrl from "./assets/check-line.svg";
import chevronDownUrl from "./assets/chevron-down.svg";
import chevronRightUrl from "./assets/chevron-right.svg";
import chevronUpUrl from "./assets/chevron-up.svg";
import circleUrl from "./assets/circle.svg";
import clockUrl from "./assets/clock.svg";
import commandMacUrl from "./assets/command-mac.svg";
import commitUrl from "./assets/commit.svg";
import deviceMacUrl from "./assets/device-mac.svg";
import downloadUrl from "./assets/download.svg";
import duplicateUrl from "./assets/duplicate.svg";
import editUrl from "./assets/edit.svg";
import extensionUrl from "./assets/extension.svg";
import eyeUrl from "./assets/eye.svg";
import filesUrl from "./assets/files.svg";
import filterUrl from "./assets/filter.svg";
import floatingWindowUrl from "./assets/floating-window.svg";
import folderUrl from "./assets/folder.svg";
import gearUrl from "./assets/gear.svg";
import gitUrl from "./assets/git.svg";
import imageUrl from "./assets/image.svg";
import infoUrl from "./assets/info.svg";
import levelUrl from "./assets/level.svg";
import linkUrl from "./assets/link.svg";
import micUrl from "./assets/mic.svg";
import miniAppUrl from "./assets/mini-app.svg";
import moreUrl from "./assets/more.svg";
import paletteUrl from "./assets/palette.svg";
import pinUrl from "./assets/pin.svg";
import plusUrl from "./assets/plus.svg";
import progress25Url from "./assets/progress-25.svg";
import refreshUrl from "./assets/refresh.svg";
import searchUrl from "./assets/search.svg";
import sessionUrl from "./assets/session.svg";
import settingsUrl from "./assets/settings.svg";
import showSessionUrl from "./assets/show-session.svg";
import sideChatUrl from "./assets/side-chat.svg";
import sidebarLeftUrl from "./assets/sidebar-left.svg";
import sidebarRightUrl from "./assets/sidebar-right.svg";
import sparkUrl from "./assets/spark.svg";
import starUrl from "./assets/star.svg";
import storeUrl from "./assets/store.svg";
import terminalUrl from "./assets/terminal.svg";
import thinkingUrl from "./assets/thinking.svg";
import turnUrl from "./assets/turn.svg";
import uploadUrl from "./assets/upload.svg";
import userUrl from "./assets/user.svg";
import xmarkUrl from "./assets/xmark.svg";
import chevronLeftUrl from "./assets/chevron-left.svg";
import selectedUrl from "./assets/selected.svg";
import deleteUrl from "./assets/delete.svg";
import waitlistMessageUrl from "./assets/waitlist-message.svg";
import creativeUrl from "./assets/creative.svg";
import ultimateUrl from "./assets/ultimate.svg";
import standardUrl from "./assets/standard.svg";
import minimalUrl from "./assets/minimal.svg";
import styles from "./Icon.module.css";

export const iconNames = [
  "arrow-down",
  "unselected",
  "chevron-left",
  "selected",
  "delete",
  "waitlist-message",
  "creative",
  "ultimate",
  "standard",
  "minimal",
  "arrow-left",
  "arrow-right",
  "arrow-up",
  "arrow-up-right",
  "bell",
  "browser",
  "check-circle",
  "check-fill",
  "check-line",
  "chevron-down",
  "chevron-right",
  "chevron-up",
  "circle",
  "clock",
  "command-mac",
  "commit",
  "device-mac",
  "download",
  "duplicate",
  "edit",
  "extension",
  "eye",
  "files",
  "filter",
  "floating-window",
  "folder",
  "gear",
  "git",
  "image",
  "info",
  "level",
  "link",
  "mic",
  "mini-app",
  "more",
  "palette",
  "pin",
  "plus",
  "progress-25",
  "refresh",
  "search",
  "session",
  "settings",
  "show-session",
  "side-chat",
  "sidebar-left",
  "sidebar-right",
  "spark",
  "star",
  "store",
  "terminal",
  "thinking",
  "turn",
  "upload",
  "user",
  "xmark",
] as const;

/** Legacy names remain renderable; new previews use the canonical catalog. */
export const iconAliases = { download: "arrow-down", circle: "unselected" } as const;
export const canonicalIconNames = iconNames.filter(name => name !== "turn" && !(name in iconAliases)).sort();

export type IconName = (typeof iconNames)[number];
export type IconSize = "2xs" | "xs" | "sm" | "md" | "lg";
export type IconTone =
  | "inherit"
  | "primary"
  | "secondary"
  | "muted"
  | "disabled"
  | "info"
  | "success"
  | "warning"
  | "danger";

const iconSources = {
  "arrow-down": downloadUrl,
  unselected: circleUrl,
  "chevron-left": chevronLeftUrl,
  "selected": selectedUrl,
  "delete": deleteUrl,
  "waitlist-message": waitlistMessageUrl,
  "creative": creativeUrl,
  "ultimate": ultimateUrl,
  "standard": standardUrl,
  "minimal": minimalUrl,
  "arrow-left": arrowLeftUrl,
  "arrow-right": arrowRightUrl,
  "arrow-up": arrowUpUrl,
  "arrow-up-right": arrowUpRightUrl,
  bell: bellUrl,
  browser: browserUrl,
  "check-circle": checkCircleUrl,
  "check-fill": checkFillUrl,
  "check-line": checkLineUrl,
  "chevron-down": chevronDownUrl,
  "chevron-right": chevronRightUrl,
  "chevron-up": chevronUpUrl,
  circle: circleUrl,
  clock: clockUrl,
  "command-mac": commandMacUrl,
  commit: commitUrl,
  "device-mac": deviceMacUrl,
  download: downloadUrl,
  duplicate: duplicateUrl,
  edit: editUrl,
  extension: extensionUrl,
  eye: eyeUrl,
  files: filesUrl,
  filter: filterUrl,
  "floating-window": floatingWindowUrl,
  folder: folderUrl,
  gear: gearUrl,
  git: gitUrl,
  image: imageUrl,
  info: infoUrl,
  level: levelUrl,
  link: linkUrl,
  mic: micUrl,
  "mini-app": miniAppUrl,
  more: moreUrl,
  palette: paletteUrl,
  pin: pinUrl,
  plus: plusUrl,
  "progress-25": progress25Url,
  refresh: refreshUrl,
  search: searchUrl,
  session: sessionUrl,
  settings: settingsUrl,
  "show-session": showSessionUrl,
  "side-chat": sideChatUrl,
  "sidebar-left": sidebarLeftUrl,
  "sidebar-right": sidebarRightUrl,
  spark: sparkUrl,
  star: starUrl,
  store: storeUrl,
  terminal: terminalUrl,
  thinking: thinkingUrl,
  turn: turnUrl,
  upload: uploadUrl,
  user: userUrl,
  xmark: xmarkUrl,
} as const satisfies Record<IconName, string>;

export interface IconProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, "aria-label" | "children"> {
  label?: string;
  name: IconName;
  size?: IconSize;
  tone?: IconTone;
}

export const Icon = forwardRef<HTMLSpanElement, IconProps>(function Icon({
  className,
  label,
  name,
  size = "lg",
  style,
  tone = "inherit",
  ...props
}, ref) {
  const iconSource = `url("${iconSources[name]}")`;
  const iconStyle: CSSProperties = {
    ...style,
    WebkitMaskImage: iconSource,
    maskImage: iconSource,
  };

  return (
    <span
      {...props}
      aria-hidden={label ? undefined : "true"}
      aria-label={label}
      className={classNames(styles.icon, className)}
      data-bf-component="icon"
      data-bf-name={name}
      data-bf-tone={tone}
      data-size={size}
      ref={ref}
      role={label ? "img" : undefined}
      style={iconStyle}
    />
  );
});
