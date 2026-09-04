import Foundation
import OpenBitFunMobileCore

extension MobileAppModel {
    func apply(remoteTargetBound targetKey: String, epoch: UInt64, accountGeneration generation: UInt64) {
        guard generation == accountGeneration,
              !accountLoginPreview, !localActionPreview, !remoteCreatePreview else { return }
        let projection = RemoteTargetProjectionState(
            hasSessionRows: !remoteSessions.isEmpty,
            hasWorkspaceRows: !remoteWorkspaces.isEmpty || !workspaceCatalog.isEmpty,
            hasSelection: remoteSessionSelected,
            hasTimeline: (surface == .remote || remoteSessionSelected) &&
                (!timelineRows.isEmpty || !messages.isEmpty),
            hasActiveTurn: (surface == .remote || remoteSessionSelected) &&
                (activeTurnID != nil || isSending || busy),
            hasPendingNavigation: pendingDirectorySession != nil || pendingDirectoryWorkspace != nil ||
                pendingDirectoryRemoteDraft != nil,
            hasReadyAuthority: remoteInitialSessionReady || remoteInitialWorkspaceReady ||
                remoteLastAppliedAuthority != nil,
            hasCreateState: remoteCreateOpen || remoteCreateSubmitting || remoteCreateRequestID != nil ||
                committedRemoteCreate != nil
        )
        let transition = RemoteAuthorityGate.targetBoundTransition(
            currentTargetKey: remoteBoundTargetKey,
            currentEpoch: remoteBoundTargetEpoch,
            boundTargetKey: targetKey,
            boundEpoch: epoch,
            projection: projection
        )
        remoteBoundTargetKey = targetKey
        remoteBoundTargetEpoch = epoch
        remoteExpectedDeviceKey = targetKey
        remoteTargetEpoch = epoch
        guard transition.scopeChanged else { return }

        pairingRetainedAccountAuthority = nil
        clearTargetScopedRemoteProjection(boundTargetKey: targetKey, epoch: epoch)
    }

    func clearInvalidatedRemoteAuthorityProjection(adapterEpoch: UInt64) {
        clearTargetScopedRemoteProjection(boundTargetKey: "", epoch: adapterEpoch)
        remoteExpectedDeviceKey = nil
        remoteBoundTargetKey = nil
        remoteBoundTargetEpoch = nil
        remoteTargetEpoch = adapterEpoch
        pairingRetainedAccountAuthority = nil
    }

    private func clearTargetScopedRemoteProjection(boundTargetKey targetKey: String, epoch: UInt64) {
        invalidateTargetScopedFileTransfers()
        remoteInitialSessionReady = false
        remoteInitialWorkspaceReady = false
        remoteLastAppliedAuthority = nil
        remoteSessions = []
        remoteWorkspaces = []
        remoteAssistants = []
        workspaceCatalog = []
        selectedRemoteWorkspaceKind = ""
        workspaceLoading = !targetKey.isEmpty
        workspaceLoadFailed = false
        workspaceSelectionBusy = false
        remoteCreateWorkspacePhase = targetKey.isEmpty ? .unavailable : .loading
        let clearingVisibleRemoteConversation = surface == .remote || remoteSessionSelected
        remoteSessionSelected = false
        sessionDetails = nil
        if clearingVisibleRemoteConversation {
            selectedSessionID = ""
            activeTurnID = nil
            isSending = false
            busy = false
            timelineRows = []
            messages = []
            composerImages = []
        }

        if let pending = pendingDirectorySession,
           pending.epoch != epoch || directoryTargetKey(forRawDeviceKey: pending.deviceKey) != targetKey {
            pendingDirectorySession = nil
        }
        if let pending = pendingDirectoryWorkspace,
           pending.epoch != epoch || directoryTargetKey(forRawDeviceKey: pending.deviceKey) != targetKey {
            pendingDirectoryWorkspace = nil
        }
        if pendingDirectoryRemoteDraft?.targetKey != targetKey || pendingDirectoryRemoteDraft?.epoch != epoch {
            pendingDirectoryRemoteDraft = nil
        }
        pendingRemoteWorkspaceCreate = nil
        pendingRemoteAssistantCreate = false

        committedRemoteCreate = nil
        remoteCreateOpen = false
        remoteCreateSubmitting = false
        remoteCreateRequestID = nil
        remoteCreateRequestEpoch = 0
        remoteCreateRequestDeviceKey = nil
        remoteCreateError = nil
        remoteCreateDeviceError = nil
        if directPairingConnected && targetKey == "pairing" {
            updateDirectPairingDirectoryEntry()
        }
    }

    func apply(directoryState state: DeviceDirectoryUiState, generation: UInt64) {
        guard !accountLoginPreview, !localActionPreview, !remoteCreatePreview, !directoryFixturePreview,
              generation == accountDirectoryGeneration else { return }
        deviceDirectory = state.devices.map { entry in
            let deviceKey = entry.deviceId
            let sessions = entry.sessions.map { session in
                ChatSession(
                    id: session.id,
                    title: session.title.isEmpty ? localized("未命名会话") : session.title,
                    updatedLabel: session.updatedAt,
                    status: session.status,
                    agentType: session.agentType,
                    workspacePath: session.workspacePath,
                    workspaceName: session.workspaceName,
                    deviceKey: deviceKey,
                    createdAt: session.createdAt,
                    messageCount: Int(session.messageCount)
                )
            }
            let workspaces = entry.workspaces.map { workspace in
                MobileWorkspaceGroup(
                    path: workspace.path,
                    name: workspace.name.isEmpty ? workspace.path : workspace.name,
                    selected: remoteExpectedDeviceKey == deviceKey &&
                        normalizedSessionWorkspacePath(workspace.path) == normalizedSessionWorkspacePath(workspaceCatalog.first(where: { $0.selected })?.path ?? ""),
                    sessions: sessions.filter { normalizedSessionWorkspacePath($0.workspacePath ?? "") == normalizedSessionWorkspacePath(workspace.path) },
                    deviceKey: deviceKey
                )
            }
            return MobileDeviceDirectoryEntry(
                id: deviceKey,
                name: entry.deviceName,
                online: entry.online,
                expanded: entry.expanded,
                status: entry.status.name,
                error: entry.error?.name,
                workspaces: workspaces,
                sessions: sessions
            )
        }
    }

    func toggleDeviceDirectory(_ device: MobileDeviceDirectoryEntry) {
        if device.id == directPairingSidebarDeviceID {
            directPairingDirectoryEntry = MobileDeviceDirectoryEntry(
                id: device.id, name: device.name, online: device.online,
                expanded: !device.expanded, status: device.status, error: device.error,
                workspaces: device.workspaces, sessions: device.sessions
            )
            return
        }
        coreAdapter?.toggleDeviceDirectory(device.id, expanded: !device.expanded)
    }

    func retryDeviceDirectory(_ device: MobileDeviceDirectoryEntry) {
        if device.id == directPairingSidebarDeviceID {
            guard directPairingConnected else { return }
            coreAdapter?.loadRemoteWorkspaces()
            coreAdapter?.refreshRemoteSessions()
            return
        }
        coreAdapter?.retryDeviceDirectory(device.id)
    }

    private func directoryTargetKey(forRawDeviceKey rawDeviceKey: String) -> String {
        rawDeviceKey == directPairingSidebarDeviceID ? "pairing" : "account:\(rawDeviceKey)"
    }

    private var authoritativeDirectoryRawDeviceKey: String? {
        guard let targetKey = remoteExpectedDeviceKey else { return nil }
        if targetKey == "pairing" {
            return directPairingConnected ? directPairingSidebarDeviceID : nil
        }
        let prefix = "account:"
        guard targetKey.hasPrefix(prefix) else { return nil }
        let rawDeviceKey = String(targetKey.dropFirst(prefix.count))
        return rawDeviceKey.isEmpty ? nil : rawDeviceKey
    }

    func openDirectoryRemoteDraft(
        device: MobileDeviceDirectoryEntry,
        workspace: MobileWorkspaceGroup
    ) {
        guard !remoteCreateSubmitting, remoteCreateRequestID == nil else {
            showToast(localized("远程会话当前不可创建，请重试"))
            return
        }
        let targetKey: String
        let accountDevice: MobileAccountDevice?
        if device.id == directPairingSidebarDeviceID {
            guard device.online, directPairingConnected else {
                showToast(localized("这台桌面设备当前离线"))
                return
            }
            targetKey = "pairing"
            accountDevice = nil
        } else {
            guard let matched = accountDevices.first(where: { $0.id == device.id }) else {
                showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
                return
            }
            guard matched.online, device.online else {
                showToast(localized("这台桌面设备当前离线"))
                return
            }
            targetKey = "account:\(matched.id)"
            accountDevice = matched
        }

        pendingDirectorySession = nil
        pendingDirectoryWorkspace = nil
        pendingRemoteWorkspaceCreate = nil
        pendingRemoteAssistantCreate = false
        remoteCreateOpen = false

        let targetIsCurrent = remoteExpectedDeviceKey == targetKey
        let epoch = targetIsCurrent ? remoteTargetEpoch : remoteTargetEpoch &+ 1
        pendingDirectoryRemoteDraft = PendingDirectoryRemoteDraft(
            targetKey: targetKey,
            rawDeviceKey: device.id,
            workspacePath: workspace.path,
            normalizedWorkspacePath: normalizedSessionWorkspacePath(workspace.path),
            epoch: epoch,
            selectionRequested: false
        )

        if targetIsCurrent {
            if connectionPhase == .disconnected {
                pendingDirectoryRemoteDraft = nil
                showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            } else if workspaceLoadFailed {
                pendingDirectoryRemoteDraft = nil
                showToast(localized("工作区加载失败，点按重试"))
            } else {
                advancePendingDirectoryRemoteDraftIfReady()
            }
            return
        }
        guard let accountDevice else {
            pendingDirectoryRemoteDraft = nil
            showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            return
        }
        selectRemoteDevice(accountDevice)
    }

    func selectDirectoryWorkspace(_ workspace: MobileWorkspaceGroup) {
        pendingDirectoryRemoteDraft = nil
        guard let deviceKey = workspace.deviceKey else { return }
        let targetKey = directoryTargetKey(forRawDeviceKey: deviceKey)
        if remoteExpectedDeviceKey == targetKey {
            guard remoteConnected else {
                showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
                return
            }
            selectRemoteWorkspace(workspace)
            return
        }
        pendingDirectoryWorkspace = (deviceKey, workspace.path, remoteTargetEpoch &+ 1)
        guard targetKey != "pairing",
              let device = accountDevices.first(where: { $0.id == deviceKey }) else {
            pendingDirectoryWorkspace = nil
            showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            return
        }
        guard device.online else {
            pendingDirectoryWorkspace = nil
            showToast(localized("这台桌面设备当前离线"))
            return
        }
        selectRemoteDevice(device)
    }

    func selectDirectorySession(_ session: ChatSession) {
        pendingDirectoryRemoteDraft = nil
        guard let deviceKey = session.deviceKey else { return }
        let targetKey = directoryTargetKey(forRawDeviceKey: deviceKey)
        let targetIsCurrent = remoteExpectedDeviceKey == targetKey
        pendingDirectorySession = (
            deviceKey,
            session.id,
            remoteTargetEpoch &+ (targetIsCurrent ? 0 : 1)
        )
        if targetIsCurrent {
            guard remoteConnected else {
                pendingDirectorySession = nil
                showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
                return
            }
            openPendingDirectorySessionIfReady()
            return
        }
        guard targetKey != "pairing",
              let device = accountDevices.first(where: { $0.id == deviceKey }) else {
            pendingDirectorySession = nil
            showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            return
        }
        guard device.online else {
            pendingDirectorySession = nil
            showToast(localized("这台桌面设备当前离线"))
            return
        }
        selectRemoteDevice(device)
    }

    private func openPendingDirectorySessionIfReady() {
        guard let pending = pendingDirectorySession,
              pending.epoch == remoteTargetEpoch else { return }
        guard remoteExpectedDeviceKey == directoryTargetKey(forRawDeviceKey: pending.deviceKey) else {
            pendingDirectorySession = nil
            showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            return
        }
        guard remoteConnected,
              remoteInitialSessionReady,
              remoteInitialWorkspaceReady,
              !workspaceLoadFailed else { return }
        pendingDirectorySession = nil
        surface = .remote
        drawerOpen = false
        remoteSessionSelected = true
        selectedSessionID = pending.sessionID
        coreAdapter?.openRemoteSession(sessionID: pending.sessionID)
    }

    private func advancePendingDirectoryRemoteDraftIfReady() {
        guard var pending = pendingDirectoryRemoteDraft,
              pending.targetKey == remoteExpectedDeviceKey,
              pending.epoch == remoteTargetEpoch,
              remoteConnected,
              remoteInitialSessionReady,
              remoteInitialWorkspaceReady,
              !workspaceLoadFailed else { return }

        guard authoritativeDirectoryRawDeviceKey == pending.rawDeviceKey else {
            pendingDirectoryRemoteDraft = nil
            showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            return
        }
        let selectedPath = workspaceCatalog.first(where: { $0.selected })?.path ?? ""
        if selectedPath == pending.workspacePath {
            pendingDirectoryRemoteDraft = nil
            surface = .remote
            drawerOpen = false
            remoteCreateOpen = true
            return
        }
        guard !pending.selectionRequested else { return }
        guard workspaceCatalog.contains(where: { $0.path == pending.workspacePath }) else {
            pendingDirectoryRemoteDraft = nil
            showToast(localized("暂无可用工作区"))
            return
        }
        guard authoritativeDirectoryRawDeviceKey == pending.rawDeviceKey else {
            pendingDirectoryRemoteDraft = nil
            showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            return
        }
        pending.selectionRequested = true
        pendingDirectoryRemoteDraft = pending
        coreAdapter?.selectRemoteWorkspace(path: pending.workspacePath)
    }

    func selectRemoteWorkspace(_ workspace: MobileWorkspaceGroup) {
        pendingDirectoryRemoteDraft = nil
        guard remoteConnected, remoteCreateInteraction.canSelectWorkspace else {
            showToast(localized("请先连接桌面设备"))
            return
        }
        surface = .remote
        drawerOpen = false
        workspaceSelectionBusy = true
        coreAdapter?.selectRemoteWorkspace(path: workspace.path)
    }

    func createRemoteSession(in workspace: MobileWorkspaceGroup, agentType: String) {
        pendingDirectoryRemoteDraft = nil
        guard remoteCreateInteraction.canSubmit else { return }
        drawerOpen = false
        surface = .remote
        createRemoteSession(
            agentType: agentType,
            title: "",
            instruction: "",
            workspacePath: workspace.path
        )
    }

    func createRemoteAssistantSession() {
        pendingDirectoryRemoteDraft = nil
        guard remoteCreateInteraction.canSubmit else { return }
        drawerOpen = false
        surface = .remote
        if selectedRemoteWorkspaceKind.lowercased() == "assistant" {
            createRemoteSession(agentType: "Claw", title: "", instruction: "")
            return
        }
        guard let assistant = remoteAssistants.first else {
            showToast(localized("暂无可用工作区"))
            return
        }
        pendingRemoteAssistantCreate = true
        workspaceSelectionBusy = true
        coreAdapter?.selectRemoteAssistant(path: assistant.path)
    }

    func selectRemoteAssistant(_ assistant: MobileAssistantOption) {
        guard remoteConnected, remoteCreateInteraction.canSelectWorkspace else { return }
        workspaceSelectionBusy = true
        coreAdapter?.selectRemoteAssistant(path: assistant.path)
    }

    func createRemoteSession(
        agentType: String,
        title: String,
        instruction: String,
        modelID: String? = nil,
        workspacePath: String? = nil
    ) {
        guard remoteCreateInteraction.canSubmit else {
            remoteCreateError = localized("远程会话当前不可创建，请重试")
            return
        }
        let normalizedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalizedInstruction = instruction.trimmingCharacters(in: .whitespacesAndNewlines)
        let selectedModel = modelID ?? modelOptions.first(where: \.selected)?.id
        let requestID = UUID().uuidString
        guard let deviceKey = remoteExpectedDeviceKey else {
            remoteCreateError = localized("未选择远程设备")
            return
        }
        guard coreAdapter != nil else {
            remoteCreateError = localized("远程连接尚未准备好，请重试")
            return
        }
        remoteCreateSubmitting = true
        remoteCreateError = nil
        remoteCreateRequestID = requestID
        remoteCreateRequestEpoch = remoteTargetEpoch
        remoteCreateRequestDeviceKey = deviceKey
        if workspacePath == nil {
            guard let assistant = remoteAssistants.first else {
                remoteCreateSubmitting = false
                clearRemoteCreateRequestMetadata()
                remoteCreateError = localized("设备不支持助手会话")
                return
            }
            coreAdapter?.createRemoteAssistantSession(
                requestID: requestID,
                assistantPath: assistant.path,
                title: normalizedTitle,
                instruction: normalizedInstruction,
                modelID: selectedModel
            )
        } else {
            coreAdapter?.createRemoteSession(
                requestID: requestID,
                agentType: agentType,
                title: normalizedTitle,
                instruction: normalizedInstruction,
                modelID: selectedModel,
                workspacePath: workspacePath
            )
        }
        surface = .remote
    }

    private func clearRemoteCreateRequestMetadata() {
        remoteCreateRequestID = nil
        remoteCreateRequestEpoch = 0
        remoteCreateRequestDeviceKey = nil
    }

    func failRemoteCreate(requestID: String, targetKey: String?) {
        guard remoteCreateRequestID == requestID,
              remoteCreateRequestEpoch == remoteTargetEpoch,
              targetKey == nil || remoteCreateRequestDeviceKey == targetKey else { return }
        remoteCreateSubmitting = false
        clearRemoteCreateRequestMetadata()
        remoteCreateError = localized("远程会话连接已失效，请重新选择设备后重试")
    }

    func apply(createOperation state: CreateSessionOperationState, targetKey: String) {
        guard !remoteCreatePreview else { return }
        let operationRequestID: String?
        switch state {
        case let value as CreateSessionOperationStateInFlight: operationRequestID = value.requestId
        case let value as CreateSessionOperationStateSucceeded: operationRequestID = value.requestId
        case let value as CreateSessionOperationStateFailed: operationRequestID = value.requestId
        case let value as CreateSessionOperationStateCancelled: operationRequestID = value.requestId
        default: operationRequestID = nil
        }
        guard let requestID = remoteCreateRequestID,
              operationRequestID == requestID,
              remoteCreateRequestEpoch == remoteTargetEpoch,
              remoteCreateRequestDeviceKey == targetKey else {
            return
        }
        switch state {
        case is CreateSessionOperationStateInFlight:
            remoteCreateSubmitting = true
        case let succeeded as CreateSessionOperationStateSucceeded:
            guard let confirmed = succeeded.confirmedSession,
                  !succeeded.createdSessionId.isEmpty,
                  confirmed.id == succeeded.createdSessionId else {
                remoteCreateSubmitting = false
                remoteCreateRequestID = nil
                remoteCreateError = localized("远程会话创建结果无效，请刷新后重试")
                coreAdapter?.refreshRemoteSessions()
                return
            }
            let session = ChatSession(
                id: confirmed.id,
                title: confirmed.title.isEmpty ? localized("未命名会话") : confirmed.title,
                updatedLabel: confirmed.updatedAt,
                status: confirmed.status,
                agentType: confirmed.agentType,
                workspacePath: confirmed.workspacePath,
                workspaceName: confirmed.workspaceName,
                createdAt: confirmed.createdAt,
                messageCount: Int(confirmed.messageCount)
            )
            let authorityAlreadyApplied = RemoteAuthorityGate.succeededIsAlreadyAuthoritative(
                targetKey: targetKey,
                epoch: remoteTargetEpoch,
                commitRevision: succeeded.commitRevision,
                confirmedSessionVisible: remoteSessions.contains { $0.id == session.id },
                lastApplied: remoteLastAppliedAuthority
            )
            committedRemoteCreate = authorityAlreadyApplied ? nil : CommittedRemoteCreate(
                targetKey: targetKey,
                epoch: remoteTargetEpoch,
                session: session,
                minimumAuthorityRevision: succeeded.commitRevision
            )
            remoteCreateSubmitting = false
            remoteCreateError = nil
            remoteCreateRequestID = nil
            remoteCreateOpen = false
            selectedSessionID = session.id
            remoteSessionSelected = true
            surface = .remote
            remoteSessions.removeAll { $0.id == session.id }
            remoteSessions.insert(session, at: 0)
            rebuildRemoteWorkspaceGroups()
        case let failed as CreateSessionOperationStateFailed:
            remoteCreateSubmitting = false
            remoteCreateError = failed.unsupported
                ? localized("桌面端不支持创建此类会话")
                : localized("创建远程会话失败，请重试")
            remoteCreateRequestID = nil
        case is CreateSessionOperationStateCancelled:
            remoteCreateSubmitting = false
            remoteCreateError = localized("创建远程会话已取消")
            remoteCreateRequestID = nil
        case is CreateSessionOperationStateIdle:
            if remoteCreateSubmitting {
                remoteCreateSubmitting = false
                remoteCreateError = localized("创建远程会话已结束，请重试")
                remoteCreateRequestID = nil
            }
        default:
            break
        }
    }

    func deleteRemoteSession(_ session: ChatSession) {
        guard !busy else { return }
        coreAdapter?.deleteRemoteSession(sessionID: session.id)
        if selectedSessionID == session.id {
            remoteSessionSelected = false
            timelineRows = []
            messages = []
        }
    }

    func searchRemoteSessions(_ query: String) {
        remoteQuery = query
        guard remoteConnected else { return }
        coreAdapter?.searchRemoteSessions(query: query)
    }

    func loadMoreRemoteSessions() {
        guard remoteConnected, remoteHasMore, !busy else { return }
        coreAdapter?.loadMoreRemoteSessions()
    }

    func loadOlderRemoteMessages() {
        guard surface == .remote, remoteConnected, remoteHasMoreMessages, !busy else { return }
        coreAdapter?.loadOlderRemoteMessages()
    }

    func refreshRemoteSessions() {
        guard remoteConnected, !busy else { return }
        coreAdapter?.refreshRemoteSessions()
    }

    func setRemoteAgentFilter(_ name: String) {
        let filter: SessionAgentFilter
        switch name {
        case "CODE": filter = .code
        case "COWORK": filter = .cowork
        default: filter = .all
        }
        remoteAgentFilter = name
        coreAdapter?.setRemoteAgentFilter(filter)
    }

    func refreshRemotePermissionMode() {
        guard remoteConnected else { return }
        coreAdapter?.refreshRemotePermissionMode()
    }

    func setRemotePermissionMode(_ name: String) {
        let mode: SessionPermissionMode
        switch name {
        case "AUTO": mode = .auto
        case "FULL_ACCESS": mode = .fullAccess
        default: mode = .ask
        }
        coreAdapter?.setRemotePermissionMode(mode)
    }

    func retryRemoteWorkspaces() {
        guard remoteExpectedDeviceKey != nil,
              remoteCreateWorkspacePhase != .loading,
              !workspaceSelectionBusy else { return }
        remoteCreateWorkspacePhase = .loading
        workspaceLoading = true
        workspaceLoadFailed = false
        coreAdapter?.loadRemoteWorkspaces()
    }

    func sendRemote() {
        let value = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty || !composerImages.isEmpty,
              !isSending,
              connectionPhase != .disconnected,
              let sessionID = visibleSessions.first(where: { $0.id == selectedSessionID })?.id else { return }
        let images = composerImages
        draft = ""
        composerImages = []
        isSending = true
        busy = true
        coreAdapter?.sendRemote(sessionID: sessionID, content: value, images: images)
    }

    func approveTool(_ toolID: String) {
        guard surface == .remote, remoteSessionSelected, !toolID.isEmpty else { return }
        coreAdapter?.approveRemoteTool(sessionID: selectedSessionID, toolID: toolID)
    }

    func rejectTool(_ toolID: String) {
        guard surface == .remote, remoteSessionSelected, !toolID.isEmpty else { return }
        coreAdapter?.rejectRemoteTool(
            sessionID: selectedSessionID,
            toolID: toolID,
            reason: "Rejected from the iOS client"
        )
    }

    func cancelTool(_ toolID: String) {
        guard surface == .remote, remoteSessionSelected, !toolID.isEmpty else { return }
        coreAdapter?.cancelRemoteTool(
            sessionID: selectedSessionID,
            toolID: toolID,
            reason: "Cancelled from the iOS client"
        )
    }

    func answerTool(_ toolID: String, answer: String) {
        let normalized = answer.trimmingCharacters(in: .whitespacesAndNewlines)
        guard surface == .remote,
              remoteSessionSelected,
              !toolID.isEmpty,
              !normalized.isEmpty else { return }
        coreAdapter?.answerRemoteTool(
            sessionID: selectedSessionID,
            toolID: toolID,
            answer: normalized
        )
    }

    func answerTool(_ toolID: String, answers: [QuestionAnswer]) {
        guard surface == .remote,
              remoteSessionSelected,
              !toolID.isEmpty,
              !answers.isEmpty else { return }
        coreAdapter?.answerRemoteToolStructured(
            sessionID: selectedSessionID,
            toolID: toolID,
            answers: answers
        )
    }

    func apply(remoteState state: RemoteSessionUiState, targetKey: String, epoch: UInt64) {
        guard !localActionPreview, !accountLoginPreview, !remoteCreatePreview else { return }
        guard RemoteAuthorityGate.callbackMatchesAuthority(
            targetKey: targetKey,
            epoch: epoch,
            expectedTargetKey: remoteExpectedDeviceKey,
            expectedEpoch: remoteTargetEpoch
        ) else { return }
        guard let ready = state as? RemoteSessionUiStateReady else {
            remoteInitialSessionReady = false
            if let failed = state as? RemoteSessionUiStateFailed {
                let detail = failed.remoteMessage ?? failed.reason.name
                remoteConnected = false
                connectionPhase = .disconnected
                let clearingVisibleRemoteConversation = surface == .remote || remoteSessionSelected
                remoteSessionSelected = false
                if clearingVisibleRemoteConversation {
                    selectedSessionID = ""
                    activeTurnID = nil
                    isSending = false
                    busy = false
                    timelineRows = []
                    messages = []
                }
                pendingDirectorySession = nil
                pendingDirectoryWorkspace = nil
                if pendingDirectoryRemoteDraft?.targetKey == targetKey,
                   pendingDirectoryRemoteDraft?.epoch == epoch {
                    pendingDirectoryRemoteDraft = nil
                    showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
                }
                coreErrorMessage = detail
                remoteCreateOpen = false
                remoteCreateSubmitting = false
                clearRemoteCreateRequestMetadata()
                remoteCreateError = detail
                if directPairingConnected && targetKey == "pairing" {
                    updateDirectPairingDirectoryEntry()
                }
            }
            return
        }
        guard RemoteAuthorityGate.acceptsReady(
            targetKey: targetKey,
            epoch: epoch,
            revision: ready.revision,
            lastApplied: remoteLastAppliedAuthority
        ) else { return }
        remoteLastAppliedAuthority = RemoteAuthorityGate.updatedScope(
            targetKey: targetKey,
            epoch: epoch,
            revision: ready.revision,
            lastApplied: remoteLastAppliedAuthority
        )
        remoteInitialSessionReady = true
        remoteConnected = true
        surface = .remote
        connectionPhase = .connected
        let committed = committedRemoteCreate
        let projectionDecision = RemoteAuthorityGate.committedProjectionDecision(
            readyTargetKey: targetKey,
            readyEpoch: epoch,
            readyRevision: ready.revision,
            committedTargetKey: committed?.targetKey,
            committedEpoch: committed?.epoch,
            minimumAuthorityRevision: committed?.minimumAuthorityRevision,
            confirmedSessionVisible: committed.map { marker in
                ready.sessions.contains { $0.id == marker.session.id }
            } ?? false
        )
        if !projectionDecision.retainMarker {
            committedRemoteCreate = nil
        }
        remoteSessions = ready.sessions.map { session in
            ChatSession(
                id: session.id,
                title: session.title.isEmpty ? localized("未命名会话") : session.title,
                updatedLabel: session.updatedAt,
                status: session.status,
                agentType: session.agentType,
                workspacePath: session.workspacePath,
                workspaceName: session.workspaceName,
                createdAt: session.createdAt,
                messageCount: Int(session.messageCount),
            )
        }
        if let committed, projectionDecision.protectCommittedRowAndSelection {
            remoteSessions.removeAll { $0.id == committed.session.id }
            remoteSessions.insert(committed.session, at: 0)
        }
        rebuildRemoteWorkspaceGroups()
        if directPairingConnected {
            updateDirectPairingDirectoryEntry()
        }
        if let protected = committedRemoteCreate,
           protected.targetKey == targetKey,
           protected.epoch == epoch {
            selectedSessionID = protected.session.id
            remoteSessionSelected = true
        } else {
            if let selected = ready.selectedSessionId {
                selectedSessionID = selected
            }
            remoteSessionSelected = ready.selectedSessionId != nil
        }
        busy = ready.busy
        remoteQuery = ready.query
        remoteAgentFilter = ready.agentFilter.name
        remoteHasMore = ready.hasMore
        remoteHasMoreMessages = ready.hasMoreMessages
        remotePermissionMode = ready.permissionMode?.name ?? remotePermissionMode
        remotePermissionFailure = ready.permissionModeFailure?.name
        activeTurnID = ready.timeline?.activeTurn?.turnId
        isSending = ready.timeline?.activeTurn != nil
        modelOptions = ready.createModelOptions(fallbackLabel: localized("模型")).map { option in
            ComposerModelOption(
                id: option.id,
                primaryLabel: option.primaryLabel,
                secondaryLabel: option.secondaryLabel,
                source: "REMOTE",
                selected: option.selected
            )
        }
        if let timeline = ready.timeline {
            timelineRows = timeline.conversationRows().map(Self.mapConversationRow)
            messages = timelineRows.compactMap { row in
                guard row.kind != "EMPTY" else { return nil }
                return ChatMessage(
                    id: UUID(uuidString: row.id) ?? UUID(),
                    role: row.kind == "USER" ? .user : .assistant,
                    text: row.text
                )
            }
        } else {
            timelineRows = []
            messages = []
        }
        if let pending = pendingDirectoryWorkspace,
           remoteExpectedDeviceKey == directoryTargetKey(forRawDeviceKey: pending.deviceKey),
           pending.epoch == remoteTargetEpoch,
           remoteConnected,
           remoteInitialWorkspaceReady {
            pendingDirectoryWorkspace = nil
            coreAdapter?.selectRemoteWorkspace(path: pending.path)
        }
        openPendingDirectorySessionIfReady()
        advancePendingDirectoryRemoteDraftIfReady()
    }

    func apply(workspaceState state: RemoteWorkspaceUiState, targetKey: String, epoch: UInt64) {
        guard !localActionPreview, !accountLoginPreview, !remoteCreatePreview,
              RemoteAuthorityGate.callbackMatchesAuthority(
                targetKey: targetKey,
                epoch: epoch,
                expectedTargetKey: remoteExpectedDeviceKey,
                expectedEpoch: remoteTargetEpoch
              ) else { return }
        workspaceLoading = state is RemoteWorkspaceUiStateLoading
        workspaceLoadFailed = state is RemoteWorkspaceUiStateFailed
        workspaceSelectionBusy = (state as? RemoteWorkspaceUiStateReady)?.busy ?? false
        if state is RemoteWorkspaceUiStateLoading {
            remoteCreateWorkspacePhase = .loading
        } else if state is RemoteWorkspaceUiStateFailed {
            remoteCreateWorkspacePhase = .failed
        }
        if !(state is RemoteWorkspaceUiStateReady) {
            remoteInitialWorkspaceReady = false
        }
        if state is RemoteWorkspaceUiStateFailed {
            if pendingRemoteWorkspaceCreate != nil || pendingRemoteAssistantCreate ||
                pendingDirectoryRemoteDraft != nil {
                pendingRemoteWorkspaceCreate = nil
                pendingDirectoryRemoteDraft = nil
                pendingRemoteAssistantCreate = false
                showToast(localized("工作区加载失败，点按重试"))
            }
            return
        }
        guard let ready = state as? RemoteWorkspaceUiStateReady else { return }

        workspaceLoading = false
        workspaceLoadFailed = false
        workspaceSelectionBusy = ready.busy
        remoteCreateWorkspacePhase = .ready
        remoteInitialWorkspaceReady = true
        selectedRemoteWorkspaceKind = ready.selected?.kind ?? ""
        var seen = Set<String>()
        var catalog: [(path: String, name: String, selected: Bool)] = []
        if let selected = ready.selected,
           !selected.path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            let normalizedPath = normalizedSessionWorkspacePath(selected.path)
            seen.insert(normalizedPath)
            catalog.append((selected.path, selected.name, true))
        }
        for workspace in ready.workspaces {
            guard !workspace.path.isEmpty else { continue }
            let normalizedPath = normalizedSessionWorkspacePath(workspace.path)
            guard seen.insert(normalizedPath).inserted else { continue }
            catalog.append((workspace.path, workspace.name, false))
        }
        workspaceCatalog = catalog
        remoteAssistants = ready.assistants.map {
            MobileAssistantOption(path: $0.path, name: $0.name)
        }
        rebuildRemoteWorkspaceGroups()
        if directPairingConnected {
            updateDirectPairingDirectoryEntry()
        }
        apply(filePreviewState: ready.preview)
        apply(downloadState: ready.download)
        if let pending = pendingRemoteWorkspaceCreate,
           ready.selected?.path == pending.path {
            pendingRemoteWorkspaceCreate = nil
            createRemoteSession(agentType: pending.agentType, title: "", instruction: "")
        }
        if pendingRemoteAssistantCreate,
           ready.selected?.kind.lowercased() == "assistant" {
            pendingRemoteAssistantCreate = false
            createRemoteSession(agentType: "Claw", title: "", instruction: "")
        }
        advancePendingDirectoryRemoteDraftIfReady()
    }

    private func updateDirectPairingDirectoryEntry() {
        guard let name = directPairingDeviceName else { return }
        directPairingDirectoryEntry = MobileDeviceDirectoryEntry(
            id: directPairingSidebarDeviceID,
            name: name,
            online: remoteConnected,
            expanded: directPairingDirectoryEntry?.expanded ?? true,
            status: remoteConnected ? "READY" : "FAILED",
            error: remoteConnected ? nil : "DISCONNECTED",
            workspaces: remoteWorkspaces,
            sessions: remoteSessions
        )
    }

    func rebuildRemoteWorkspaceGroups() {
        let selectedPath = workspaceCatalog.first(where: { $0.selected })?.path
        remoteWorkspaces = workspaceCatalog.map { workspace in
            MobileWorkspaceGroup(
                path: workspace.path,
                name: workspace.name.isEmpty ? workspace.path : workspace.name,
                selected: workspace.selected,
                sessions: remoteSessions.filter { session in
                    normalizedSessionWorkspacePath(session.workspacePath ?? selectedPath ?? "") ==
                        normalizedSessionWorkspacePath(workspace.path)
                }
            )
        }
    }
}

extension MobileAppModel {
    var sessionListWorkspaceOptions: [MobileSessionWorkspaceOption] {
        SessionListPresentation.shared
            .workspaceOptions(sessions: sessionListCoreSessions, workspace: sessionListWorkspaceContext)
            .map { MobileSessionWorkspaceOption(path: $0.path, name: $0.name) }
    }

    var sessionListAgentGroups: [String] {
        SessionListPresentation.shared
            .agentGroups(sessions: sessionListCoreSessions, workspace: sessionListWorkspaceContext)
            .map(\.name)
    }

    var sessionListStatusOptions: [String] {
        SessionListPresentation.shared.statusOptions(sessions: sessionListCoreSessions)
    }

    var sessionListSections: [MobileSessionListSectionProjection] {
        let groupMode: SessionGroupMode = switch remoteGroupMode {
        case "TIME": .time
        case "CHAT": .chat
        default: .project
        }
        let agentFilter: SessionAgentGroup? = switch remoteViewAgentFilter {
        case "CHAT": .chat
        case "CODE": .code
        case "COWORK": .cowork
        default: nil
        }
        let view = SessionListPresentation.shared.view(
            sessions: sessionListCoreSessions,
            workspace: sessionListWorkspaceContext,
            options: SessionListOptions(
                groupMode: groupMode,
                query: "",
                workspaceFilter: remoteWorkspaceFilter,
                agentFilter: agentFilter,
                statusFilter: remoteStatusFilter
            ),
            nowMs: Int64(Date().timeIntervalSince1970 * 1_000)
        )
        let byID = Dictionary(uniqueKeysWithValues: remoteSessions.map { ($0.id, $0) })
        return view.sections.compactMap { section in
            switch onEnum(of: section) {
            case .chat(let value):
                return projection(id: "chat", kind: .chat, section: value, byID: byID)
            case .project(let value):
                return MobileSessionListSectionProjection(
                    id: "project:\(value.path)",
                    kind: .project,
                    path: value.path,
                    name: value.name,
                    sessions: value.sessions.compactMap { byID[$0.id] }
                )
            case .today(let value):
                return projection(id: "today", kind: .today, section: value, byID: byID)
            case .yesterday(let value):
                return projection(id: "yesterday", kind: .yesterday, section: value, byID: byID)
            case .earlier(let value):
                return projection(id: "earlier", kind: .earlier, section: value, byID: byID)
            }
        }
    }

    private var sessionListCoreSessions: [RemoteSession] {
        remoteSessions.map { session in
            RemoteSession(
                id: session.id,
                title: session.title,
                agentType: session.agentType,
                status: session.status,
                updatedAt: session.updatedLabel,
                createdAt: session.createdAt,
                messageCount: Int32(session.messageCount),
                workspacePath: session.workspacePath,
                workspaceName: session.workspaceName
            )
        }
    }

    private var sessionListWorkspaceContext: SessionWorkspaceContext {
        let assistantPaths = Set(remoteAssistants.map { normalizedSessionWorkspacePath($0.path) })
        let selected = remoteWorkspaces.first(where: \.selected)
        let recent = remoteWorkspaces.map { workspace in
            RecentWorkspace(
                path: workspace.path,
                name: workspace.name,
                lastOpened: "",
                kind: assistantPaths.contains(normalizedSessionWorkspacePath(workspace.path))
                    ? "assistant"
                    : "normal"
            )
        }
        let selectedKind = selected.map {
            assistantPaths.contains(normalizedSessionWorkspacePath($0.path)) ? "assistant" : "normal"
        } ?? ""
        return SessionWorkspaceContext(
            selectedPath: selected?.path ?? "",
            selectedName: selected?.name ?? "",
            selectedKind: selectedKind,
            recent: recent
        )
    }

    private func projection(
        id: String,
        kind: MobileSessionListSectionKind,
        section: any SessionListSection,
        byID: [String: ChatSession]
    ) -> MobileSessionListSectionProjection {
        MobileSessionListSectionProjection(
            id: id,
            kind: kind,
            path: "",
            name: "",
            sessions: section.sessions.compactMap { byID[$0.id] }
        )
    }

    private func normalizedSessionWorkspacePath(_ path: String) -> String {
        var result = path.trimmingCharacters(in: .whitespacesAndNewlines)
        while result.count > 1 && (result.hasSuffix("/") || result.hasSuffix("\\")) {
            result.removeLast()
        }
        return result
    }
}
