/**
 * 文件名校验工具。
 *
 * 被"新建文件/文件夹"对话框与文件树重命名共用，规则对齐 VSCode：
 * 空名 / 非法字符 / Windows 保留名 / 同级重名 / 名称过长。
 */

// eslint-disable-next-line no-control-regex -- 文件名规则明确禁止 ASCII 控制字符。
const INVALID_CHARS_LOCAL = /^[^<>:"/\\|?*\x00-\x1F]+$/;
// eslint-disable-next-line no-control-regex -- 文件名规则明确禁止 ASCII 控制字符。
const INVALID_CHARS_REMOTE = /^[^/\x00-\x1F]+$/;

/**
 * Windows 保留设备名（大小写不敏感）。匹配 `CON`、`CON.txt` 这类前缀。
 * 仅在本地（Windows）工作区生效。
 */
const RESERVED_NAME = /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.|$)/i;

/** 单路径段名称长度上限（NTFS / ext4 通用上限）。 */
export const MAX_NAME_LENGTH = 255;

export interface ValidateFileNameContext {
  /** 当前是否为远程工作区（远程放宽非法字符集合，且不检查 Windows 保留名）。 */
  isRemote: boolean;
  /** 被校验对象是否为文件夹，用于选择准确的错误文案。 */
  isDirectory?: boolean;
  /** 同级已存在的名称（大小写不敏感比较），用于检测重名冲突。需已排除被校验对象自身原名。 */
  siblings?: string[];
  /** 单路径段名称最大长度，默认 {@link MAX_NAME_LENGTH}。 */
  maxLength?: number;
}

/**
 * 校验一个文件/文件夹名是否合法。
 *
 * @param raw 输入框原始值
 * @param ctx 校验上下文
 * @returns 错误时返回 i18n key（`validation.*`，相对 `panels/files.json`），合法返回 null。
 */
export function validateFileName(raw: string, ctx: ValidateFileNameContext): string | null {
  const trimmed = raw.trim();
  const max = ctx.maxLength ?? MAX_NAME_LENGTH;

  // 1. 空名（含纯空白）
  if (!trimmed) {
    return 'validation.emptyName';
  }

  // 2. 非法字符
  const validPattern = ctx.isRemote ? INVALID_CHARS_REMOTE : INVALID_CHARS_LOCAL;
  if (!validPattern.test(trimmed)) {
    return ctx.isDirectory
      ? 'validation.invalidFolderName'
      : 'validation.invalidFilename';
  }

  // 3. Windows 保留名（仅本地）
  if (!ctx.isRemote && RESERVED_NAME.test(trimmed)) {
    return 'validation.reservedName';
  }

  // 4. 同级重名（大小写不敏感，覆盖原名已由调用方排除）
  if (ctx.siblings && ctx.siblings.length > 0) {
    const lower = trimmed.toLowerCase();
    if (ctx.siblings.some((s) => s.toLowerCase() === lower)) {
      return 'validation.duplicateName';
    }
  }

  // 5. 名称过长
  if (trimmed.length > max) {
    return 'validation.nameTooLong';
  }

  return null;
}
