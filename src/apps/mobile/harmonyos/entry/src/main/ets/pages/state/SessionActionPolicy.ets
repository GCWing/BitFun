export enum SessionActionScope {
  General = 'general',
  Remote = 'remote'
}

export class SessionActionCapabilities {
  readonly canViewDetails: boolean;
  readonly canArchive: boolean;
  readonly canExport: boolean;
  readonly canDelete: boolean;

  constructor(canViewDetails: boolean, canArchive: boolean, canExport: boolean, canDelete: boolean) {
    this.canViewDetails = canViewDetails;
    this.canArchive = canArchive;
    this.canExport = canExport;
    this.canDelete = canDelete;
  }
}

export class SessionActionPolicy {
  static resolve(scope: SessionActionScope, agentType: string, busy: boolean): SessionActionCapabilities {
    if (busy) {
      return new SessionActionCapabilities(false, false, false, false);
    }
    if (scope === SessionActionScope.Remote) {
      return new SessionActionCapabilities(true, false, false, true);
    }
    const isGeneralChat = agentType.toLowerCase() === 'chat';
    return new SessionActionCapabilities(true, isGeneralChat, isGeneralChat, true);
  }
}
