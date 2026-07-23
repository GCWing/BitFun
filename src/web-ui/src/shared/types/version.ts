/**
 * Build/version metadata types.
 */
export interface VersionInfo {
   
  name: string;
   
  version: string;
   
  buildDate: string;
   
  buildTimestamp: number;
   
  gitCommit?: string;
   
  gitCommitFull?: string;
   
  gitBranch?: string;
   
  isDev: boolean;
   
  buildEnv: 'development' | 'production' | 'preview';
}

 
export interface LicenseInfo {
   
  type: string;
   
  text?: string;
   
  url?: string;
}

 
 
export interface AboutInfo {
   
  version: VersionInfo;
   
  license: LicenseInfo;
   
  links: {
    homepage?: string;
    repository?: string;
    documentation?: string;
    issues?: string;
  };
}

