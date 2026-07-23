/**
 * Vite plugin: Inject version info into HTML
 */

import type { Plugin } from 'vite';
import fs from 'fs';
import path from 'path';

export function versionInjectionPlugin(): Plugin {
  return {
    name: 'version-injection',
    transformIndexHtml(html) {
      try {
        // Read the generated version info injection script
        const injectionScriptPath = path.resolve(__dirname, 'src/generated/version-injection.html');
        
        if (fs.existsSync(injectionScriptPath)) {
          const injectionScript = fs.readFileSync(injectionScriptPath, 'utf-8');
          
          // Inject the version info script before </head>
          return html.replace('</head>', `${injectionScript}\n</head>`);
        }
        
        console.warn('[version-injection] Version injection script not found, skipping injection');
        return html;
      } catch (error) {
        console.error('[version-injection] Failed to inject version info:', error);
        return html;
      }
    }
  };
}

