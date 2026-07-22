#!/usr/bin/env node

/**
 * Console style utilities
 * Unified log formatting and coloring
 */

const colors = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  
  black: '\x1b[30m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
  white: '\x1b[37m',
  
  brightBlack: '\x1b[90m',
  brightRed: '\x1b[91m',
  brightGreen: '\x1b[92m',
  brightYellow: '\x1b[93m',
  brightBlue: '\x1b[94m',
  brightMagenta: '\x1b[95m',
  brightCyan: '\x1b[96m',
  brightWhite: '\x1b[97m',
  
  bgBlue: '\x1b[44m',
  bgGreen: '\x1b[42m',
  bgYellow: '\x1b[43m',
  bgRed: '\x1b[41m',
  bgCyan: '\x1b[46m',
  bgMagenta: '\x1b[45m',
};

const LINE_WIDTH = 60;
const THIN_LINE = '-'.repeat(LINE_WIDTH);
const THICK_LINE = '='.repeat(LINE_WIDTH);
const DOUBLE_LINE = '═'.repeat(LINE_WIDTH);

function colorize(text, ...colorCodes) {
  return colorCodes.join('') + text + colors.reset;
}

function printHeader(title) {
  console.log('');
  console.log(colorize(THICK_LINE, colors.cyan));
  console.log(colorize(`  ${title}`, colors.bold, colors.cyan));
  console.log(colorize(THICK_LINE, colors.cyan));
}

function printSubHeader(title) {
  console.log('');
  console.log(colorize(THIN_LINE, colors.dim));
  console.log(colorize(`  ${title}`, colors.bold, colors.white));
  console.log(colorize(THIN_LINE, colors.dim));
}

function printSuccess(message) {
  console.log(colorize('  [OK] ', colors.green) + message);
}

function printInfo(message) {
  console.log(colorize('  [--] ', colors.blue) + message);
}

function printWarning(message) {
  console.log(colorize('  [!!] ', colors.yellow) + message);
}

function printError(message) {
  console.log(colorize('  [XX] ', colors.red) + message);
}

function printKeyValue(key, value, indent = 2) {
  const spaces = ' '.repeat(indent);
  console.log(
    spaces + 
    colorize(key + ':', colors.dim) + 
    ' ' + 
    colorize(String(value), colors.white)
  );
}

function printListItem(text, indent = 4) {
  const spaces = ' '.repeat(indent);
  console.log(spaces + colorize('-', colors.dim) + ' ' + text);
}

function printComplete(message) {
  console.log('');
  console.log(colorize(THICK_LINE, colors.green));
  console.log(colorize(`  ${message}`, colors.bold, colors.green));
  console.log(colorize(THICK_LINE, colors.green));
  console.log('');
}

function printStep(step, total, message) {
  const stepText = `[${step}/${total}]`;
  console.log(
    colorize(stepText, colors.cyan, colors.bold) + 
    ' ' + 
    message
  );
}

function printBlank() {
  console.log('');
}

module.exports = {
  colors,
  colorize,
  printHeader,
  printSubHeader,
  printSuccess,
  printInfo,
  printWarning,
  printError,
  printKeyValue,
  printListItem,
  printComplete,
  printStep,
  printBlank,
  THIN_LINE,
  THICK_LINE,
  DOUBLE_LINE,
  LINE_WIDTH,
};
