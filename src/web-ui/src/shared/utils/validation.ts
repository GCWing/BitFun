import { i18nService } from '@/infrastructure/i18n';

export function isValidEmail(email: string): boolean {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

 
export function isValidUrl(url: string): boolean {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

 
export function isValidFilePath(path: string): boolean {
  
  if (!path || path.trim().length === 0) {
    return false;
  }
  
  
  const illegalChars = /[<>:"|?*]/;
  return !illegalChars.test(path);
}

 
export function hasValidExtension(filename: string, allowedExtensions: string[]): boolean {
  const extension = filename.split('.').pop()?.toLowerCase();
  return extension ? allowedExtensions.includes(extension) : false;
}

 
export function isValidJson(str: string): boolean {
  try {
    JSON.parse(str);
    return true;
  } catch {
    return false;
  }
}

 
export function isInRange(value: number, min: number, max: number): boolean {
  return value >= min && value <= max;
}

 
export function isValidLength(str: string, minLength = 0, maxLength = Infinity): boolean {
  return str.length >= minLength && str.length <= maxLength;
}

 
export function isRequired(value: any): boolean {
  if (value === null || value === undefined) {
    return false;
  }
  
  if (typeof value === 'string') {
    return value.trim().length > 0;
  }
  
  if (Array.isArray(value)) {
    return value.length > 0;
  }
  
  return true;
}

 
export function matchesPattern(value: string, pattern: RegExp): boolean {
  return pattern.test(value);
}

 
export function isValidFileSize(file: File, maxSizeInMB: number): boolean {
  const maxSizeInBytes = maxSizeInMB * 1024 * 1024;
  return file.size <= maxSizeInBytes;
}

 
export function isValidPort(port: number): boolean {
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

 
export function isValidIPAddress(ip: string): boolean {
  const ipRegex = /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/;
  return ipRegex.test(ip);
}

 
export function validatePasswordStrength(password: string): {
  isValid: boolean;
  score: number;
  issues: string[];
} {
  const issues: string[] = [];
  let score = 0;
  
  if (password.length < 8) {
    issues.push(i18nService.t('common:validation.password.minLength', { min: 8 }));
  } else {
    score += 25;
  }
  
  if (!/[a-z]/.test(password)) {
    issues.push(i18nService.t('common:validation.password.lowercase'));
  } else {
    score += 25;
  }
  
  if (!/[A-Z]/.test(password)) {
    issues.push(i18nService.t('common:validation.password.uppercase'));
  } else {
    score += 25;
  }
  
  if (!/[0-9]/.test(password)) {
    issues.push(i18nService.t('common:validation.password.number'));
  } else {
    score += 25;
  }
  
  if (!/[^a-zA-Z0-9]/.test(password)) {
    issues.push(i18nService.t('common:validation.password.specialCharSuggested'));
  } else {
    score += 10;
  }
  
  return {
    isValid: issues.length === 0,
    score: Math.min(score, 100),
    issues
  };
}

