import Foundation
import BitFunMobileCore

extension MobileAppModel {
    private func invalidateTerminalAccountAuthority() {
        invalidateTargetScopedFileTransfers()
        if let targetKey = coreAdapter?.currentRemoteTargetKey,
           targetKey.hasPrefix("account:") {
            let epoch = coreAdapter?.currentRemoteTargetEpoch ?? remoteTargetEpoch
            _ = coreAdapter?.invalidateRemoteAuthority(ifTargetKey: targetKey, epoch: epoch)
        }
        clearInvalidatedRemoteAuthorityProjection(
            adapterEpoch: coreAdapter?.currentRemoteTargetEpoch ?? remoteTargetEpoch
        )
        remoteConnected = false
        connectionPhase = .disconnected
    }

    private func invalidateRemoteTarget(for operation: (accountGeneration: UInt64, remoteTargetEpoch: UInt64, preservePairing: Bool)) {
        committedRemoteCreate = nil
        remoteLastAppliedAuthority = nil
        accountGeneration = operation.accountGeneration
        pendingAccountOperationPreservesPairing = (
            generation: operation.accountGeneration,
            preserve: operation.preservePairing
        )
        remoteTargetEpoch = operation.remoteTargetEpoch
        if operation.preservePairing && directPairingConnected {
            remoteExpectedDeviceKey = "pairing"
            remoteConnected = true
            pendingDirectorySession = nil
            pendingDirectoryWorkspace = nil
            pendingDirectoryRemoteDraft = nil
            return
        }
        remoteExpectedDeviceKey = nil
        remoteConnected = false
        pendingDirectorySession = nil
        pendingDirectoryWorkspace = nil
        pendingDirectoryRemoteDraft = nil
        remoteCreateSubmitting = false
        remoteCreateRequestID = nil
        remoteCreateRequestEpoch = remoteTargetEpoch
        remoteCreateRequestDeviceKey = nil
        pendingRemoteWorkspaceCreate = nil
        pendingRemoteAssistantCreate = false
        remoteSessionSelected = false
    }

    func selectRemoteDevice(_ device: MobileAccountDevice) {
        guard device.online else {
            showToast(localized("这台桌面设备当前离线"))
            return
        }
        surface = .remote
        drawerOpen = false
        let targetKey = "account:\(device.id)"
        guard remoteExpectedDeviceKey != targetKey else { return }
        invalidateTargetScopedFileTransfers()
        directPairingConnected = false
        directPairingDeviceName = nil
        directPairingDirectoryEntry = nil
        pairingIntentInFlight = false
        remoteTargetEpoch &+= 1
        remoteExpectedDeviceKey = "account:\(device.id)"
        remoteInitialSessionReady = false
        remoteInitialWorkspaceReady = false
        pendingDirectorySession = pendingDirectorySession.map { ($0.deviceKey, $0.sessionID, remoteTargetEpoch) }
        remoteCreateSubmitting = false
        remoteCreateRequestID = nil
        remoteCreateError = nil
        committedRemoteCreate = nil
        remoteLastAppliedAuthority = nil
        accountBusy = true
        remoteSessionSelected = false
        remoteConnected = directPairingConnected
        remoteSessions = []
        remoteWorkspaces = []
        workspaceCatalog = []
        pendingRemoteWorkspaceCreate = nil
        pendingRemoteAssistantCreate = false
        selectedRemoteWorkspaceKind = ""
        messages = []
        timelineRows = []
        coreAdapter?.selectAccountDevice(id: device.id)
    }

    func refreshRemoteDevices() {
        guard accountUser != nil else { return }
        coreAdapter?.refreshAccountDevices()
    }

    func logoutAccount() {
        var preservePairing = directPairingConnected && remoteExpectedDeviceKey == "pairing"
        if coreAdapter?.currentRemoteTargetKey != "pairing" {
            invalidateTargetScopedFileTransfers()
        }
        if let operation = coreAdapter?.beginAccountOperation() {
            preservePairing = operation.preservePairing
            invalidateRemoteTarget(for: operation)
        } else {
            accountGeneration &+= 1
            remoteTargetEpoch &+= 1
            committedRemoteCreate = nil
        remoteLastAppliedAuthority = nil
            remoteExpectedDeviceKey = nil
            remoteConnected = false
            pendingDirectorySession = nil
            pendingDirectoryWorkspace = nil
            pendingDirectoryRemoteDraft = nil
            remoteCreateSubmitting = false
            remoteCreateRequestID = nil
            remoteCreateRequestEpoch = remoteTargetEpoch
            remoteCreateRequestDeviceKey = nil
            pendingRemoteWorkspaceCreate = nil
            pendingRemoteAssistantCreate = false
            remoteSessionSelected = false
        }
        if !preservePairing {
            remoteExpectedDeviceKey = nil
            remoteConnected = false
            pendingDirectorySession = nil
            pendingDirectoryWorkspace = nil
            pendingDirectoryRemoteDraft = nil
        }
        accountDirectoryGeneration &+= 1
        coreAdapter?.logoutAccount(preservePairing: preservePairing)
        accountUser = nil
        accountUserID = nil
        accountDeviceName = nil
        accountDeviceCount = 0
        accountDevices = []
        accountSelectedDeviceID = nil
        coreAdapter?.syncDeviceDirectory([])
        if !preservePairing {
            remoteConnected = false
        }
        if !directPairingConnected {
            remoteSessionSelected = false
            remoteSessions = []
            remoteWorkspaces = []
            workspaceCatalog = []
            pendingRemoteWorkspaceCreate = nil
            pendingRemoteAssistantCreate = false
            selectedRemoteWorkspaceKind = ""
            surface = .local
        }
    }

    func loginAccount(relayURL: String, username: String, password: String) {
        if coreAdapter?.currentRemoteTargetKey != "pairing" {
            invalidateTargetScopedFileTransfers()
        }
        if let operation = coreAdapter?.beginAccountOperation() {
            invalidateRemoteTarget(for: operation)
        } else {
            accountGeneration &+= 1
            remoteTargetEpoch &+= 1
            committedRemoteCreate = nil
        remoteLastAppliedAuthority = nil
            remoteExpectedDeviceKey = nil
            remoteConnected = false
            pendingDirectorySession = nil
            pendingDirectoryWorkspace = nil
            pendingDirectoryRemoteDraft = nil
            remoteCreateSubmitting = false
            remoteCreateRequestID = nil
            remoteCreateRequestEpoch = remoteTargetEpoch
            remoteCreateRequestDeviceKey = nil
            pendingRemoteWorkspaceCreate = nil
            pendingRemoteAssistantCreate = false
            remoteSessionSelected = false
        }
        accountBusy = true
        accountFailureStage = nil
        accountFailureCanRetry = false
        coreErrorMessage = nil
        coreAdapter?.loginAccount(relayURL: relayURL, username: username, password: password)
    }

    func retryAccountFailure() {
        guard accountFailureStage == "DEVICE_LIST", accountFailureCanRetry, !accountBusy else { return }
        accountBusy = true
        coreAdapter?.retryAccountFailure()
    }

    func apply(accountState state: AccountUiState, generation: UInt64) {
        guard !accountLoginPreview, !localActionPreview, !remoteCreatePreview,
              generation == accountGeneration else { return }
        let preserveDirectPairing =
            (directPairingConnected && remoteExpectedDeviceKey == "pairing") ||
            (pendingAccountOperationPreservesPairing?.generation == generation &&
             pendingAccountOperationPreservesPairing?.preserve == true)
        let preservePendingPairing = preserveDirectPairing &&
            directPairingConnected &&
            remoteExpectedDeviceKey == "pairing" &&
            pendingDirectoryRemoteDraft?.targetKey == "pairing" &&
            pendingDirectoryRemoteDraft?.epoch == remoteTargetEpoch
        pendingAccountOperationPreservesPairing = nil
        accountGeneration = generation
        accountBusy = state is AccountUiStateSigningIn
        if let ready = state as? AccountUiStateReady {
            let readyTargetKey = ready.selectedDeviceId.map { "account:\($0)" }
            if let adapterTargetKey = coreAdapter?.currentRemoteTargetKey,
               adapterTargetKey.hasPrefix("account:"),
               adapterTargetKey != readyTargetKey {
                invalidateTargetScopedFileTransfers()
            }
            accountBusy = false
            accountFailureStage = nil
            accountFailureCanRetry = false
            coreErrorMessage = nil
            accountUser = ready.username
            accountUserID = ready.userId
            accountDeviceName = ready.selectedDeviceName
            accountDeviceCount = ready.devices.count
            accountSelectedDeviceID = ready.selectedDeviceId
            accountRefreshing = ready.refreshing
            remoteCreateDeviceError = ready.refreshFailure != nil
                ? localized("设备列表加载失败，请稍后重试。") : nil
            accountDevices = ready.devices.map { device in
                MobileAccountDevice(
                    id: device.id,
                    name: device.name,
                    online: device.online,
                    selected: device.id == ready.selectedDeviceId
                )
            }
            accountDirectoryGeneration = coreAdapter?.syncDeviceDirectory(accountDevices) ?? (accountDirectoryGeneration &+ 1)
            if !directPairingConnected,
               ready.selectedDeviceId == nil,
               let target = ready.devices.first(where: { $0.online }) {
                accountBusy = true
                coreAdapter?.selectAccountDevice(id: target.id)
                return
            }
            remoteConnected = directPairingConnected || ready.selectedDeviceId != nil
            surface = .remote
            connectionPhase = .connected
            if ready.refreshFailure != nil {
                showToast(localized("设备列表刷新失败，仍显示上次结果"))
            }
        } else if let failed = state as? AccountUiStateFailed {
            accountBusy = false
            accountFailureStage = failed.stage.name
            accountFailureCanRetry = failed.canRetry
            coreErrorMessage = accountErrorMessage(failed.reason.name, stage: failed.stage.name)
            if pendingDirectoryRemoteDraft != nil, !preservePendingPairing {
                pendingDirectoryRemoteDraft = nil
                showToast(localized("远程会话连接已失效，请重新选择设备后重试"))
            }
            if remoteCreateOpen {
                remoteCreateDeviceError = coreErrorMessage
            }
            if !preserveDirectPairing { connectionPhase = .disconnected }
            if failed.reason.name == "AUTHENTICATION" {
                accountUser = nil
                accountUserID = nil
                accountDevices = []
                accountSelectedDeviceID = nil
                accountDeviceName = nil
                accountDeviceCount = 0
                accountRefreshing = false
                pendingDirectorySession = nil
                pendingDirectoryWorkspace = nil
                if !preservePendingPairing {
                    pendingDirectoryRemoteDraft = nil
                }
                accountDirectoryGeneration = coreAdapter?.syncDeviceDirectory([]) ?? (accountDirectoryGeneration &+ 1)
                if !preserveDirectPairing {
                    invalidateTerminalAccountAuthority()
                }
            }
        } else if state is AccountUiStateSignedOut {
            accountBusy = false
            accountFailureStage = nil
            accountFailureCanRetry = false
            coreErrorMessage = nil
            accountUser = nil
            accountUserID = nil
            accountDevices = []
            accountSelectedDeviceID = nil
            accountDeviceName = nil
            accountDeviceCount = 0
            accountRefreshing = false
            pendingDirectorySession = nil
            pendingDirectoryWorkspace = nil
            if !preservePendingPairing {
                pendingDirectoryRemoteDraft = nil
            }
            accountDirectoryGeneration = coreAdapter?.syncDeviceDirectory([]) ?? (accountDirectoryGeneration &+ 1)
            if !preserveDirectPairing {
                invalidateTerminalAccountAuthority()
            }
        }
    }

    func accountErrorMessage(_ reason: String, stage: String? = nil) -> String {
        localized(AccountFailureCopy.localizationKey(reason: reason, stage: stage))
    }
}
