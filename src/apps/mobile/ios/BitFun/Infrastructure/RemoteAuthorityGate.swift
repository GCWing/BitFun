import Foundation

struct RemoteAuthorityScope: Equatable {
    let targetKey: String
    let epoch: UInt64
    let revision: Int64
}

struct RemoteCommittedProjectionDecision: Equatable {
    let retainMarker: Bool
    let protectCommittedRowAndSelection: Bool
}

struct PairingAttemptProjectionTransition: Equatable {
    let clearBoundRemoteProjection: Bool
    let remoteConnected: Bool
}

struct RemoteTargetProjectionState: Equatable {
    var hasSessionRows: Bool
    var hasWorkspaceRows: Bool
    var hasSelection: Bool
    var hasTimeline: Bool
    var hasActiveTurn: Bool
    var hasPendingNavigation: Bool
    var hasReadyAuthority: Bool
    var hasCreateState: Bool

    static let cleared = RemoteTargetProjectionState(
        hasSessionRows: false,
        hasWorkspaceRows: false,
        hasSelection: false,
        hasTimeline: false,
        hasActiveTurn: false,
        hasPendingNavigation: false,
        hasReadyAuthority: false,
        hasCreateState: false
    )
}

struct RemoteTargetBoundTransition: Equatable {
    let scopeChanged: Bool
    let projection: RemoteTargetProjectionState
}

struct RetainedAccountAuthority: Equatable {
    let targetKey: String
    let epoch: UInt64
}

enum RemoteAuthorityInvalidationResult: Equatable {
    case invalidated(newEpoch: UInt64)
    case notMatched(currentTargetKey: String?, currentEpoch: UInt64)
}

enum RemoteAuthorityGate {
    static func targetBoundTransition(
        currentTargetKey: String?,
        currentEpoch: UInt64?,
        boundTargetKey: String,
        boundEpoch: UInt64,
        projection: RemoteTargetProjectionState
    ) -> RemoteTargetBoundTransition {
        let changed = currentTargetKey != boundTargetKey || currentEpoch != boundEpoch
        return RemoteTargetBoundTransition(
            scopeChanged: changed,
            projection: changed ? .cleared : projection
        )
    }

    static func callbackMatchesAuthority(
        targetKey: String,
        epoch: UInt64,
        expectedTargetKey: String?,
        expectedEpoch: UInt64
    ) -> Bool {
        targetKey == expectedTargetKey && epoch == expectedEpoch
    }

    static func fileTransferCallbackMatchesAuthority(
        requestTargetKey: String?,
        requestEpoch: UInt64?,
        adapterTargetKey: String?,
        adapterEpoch: UInt64
    ) -> Bool {
        guard let requestTargetKey, let requestEpoch else { return false }
        return requestTargetKey == adapterTargetKey && requestEpoch == adapterEpoch
    }

    static func exactInvalidationMatchesAuthority(
        expectedTargetKey: String,
        expectedEpoch: UInt64,
        currentTargetKey: String?,
        currentEpoch: UInt64
    ) -> Bool {
        expectedTargetKey == currentTargetKey && expectedEpoch == currentEpoch
    }

    static func shouldRetainAccountAfterPairingFailure(
        captured: RetainedAccountAuthority?,
        adapterTargetKey: String?,
        adapterEpoch: UInt64,
        modelTargetKey: String?,
        modelEpoch: UInt64,
        healthyConnected: Bool
    ) -> Bool {
        guard let captured, captured.targetKey.hasPrefix("account:") else { return false }
        return healthyConnected &&
            adapterTargetKey == captured.targetKey && adapterEpoch == captured.epoch &&
            modelTargetKey == captured.targetKey && modelEpoch == captured.epoch
    }

    static func pairingAttemptTransition(
        authoritativeTargetKey: String?,
        remoteConnected: Bool
    ) -> PairingAttemptProjectionTransition {
        let replacesPairing = authoritativeTargetKey == "pairing"
        return PairingAttemptProjectionTransition(
            clearBoundRemoteProjection: replacesPairing,
            remoteConnected: replacesPairing ? false : remoteConnected
        )
    }

    static func acceptsReady(
        targetKey: String,
        epoch: UInt64,
        revision: Int64,
        lastApplied: RemoteAuthorityScope?
    ) -> Bool {
        guard let lastApplied,
              lastApplied.targetKey == targetKey,
              lastApplied.epoch == epoch,
              lastApplied.revision > 0 else {
            return true
        }
        return revision > 0 && revision >= lastApplied.revision
    }

    static func updatedScope(
        targetKey: String,
        epoch: UInt64,
        revision: Int64,
        lastApplied: RemoteAuthorityScope?
    ) -> RemoteAuthorityScope? {
        guard revision > 0 else {
            if let lastApplied,
               lastApplied.targetKey == targetKey,
               lastApplied.epoch == epoch {
                return lastApplied
            }
            return nil
        }
        return RemoteAuthorityScope(targetKey: targetKey, epoch: epoch, revision: revision)
    }

    static func succeededIsAlreadyAuthoritative(
        targetKey: String,
        epoch: UInt64,
        commitRevision: Int64,
        confirmedSessionVisible: Bool,
        lastApplied: RemoteAuthorityScope?
    ) -> Bool {
        guard confirmedSessionVisible,
              commitRevision > 0,
              let lastApplied,
              lastApplied.targetKey == targetKey,
              lastApplied.epoch == epoch else {
            return false
        }
        return lastApplied.revision >= commitRevision
    }

    static func readyIncludesCommit(
        readyRevision: Int64,
        minimumAuthorityRevision: Int64,
        confirmedSessionVisible: Bool
    ) -> Bool {
        if minimumAuthorityRevision > 0 {
            return readyRevision >= minimumAuthorityRevision && confirmedSessionVisible
        }
        return confirmedSessionVisible
    }

    static func committedProjectionDecision(
        readyTargetKey: String,
        readyEpoch: UInt64,
        readyRevision: Int64,
        committedTargetKey: String?,
        committedEpoch: UInt64?,
        minimumAuthorityRevision: Int64?,
        confirmedSessionVisible: Bool
    ) -> RemoteCommittedProjectionDecision {
        guard let committedTargetKey,
              let committedEpoch,
              let minimumAuthorityRevision,
              committedTargetKey == readyTargetKey,
              committedEpoch == readyEpoch else {
            return RemoteCommittedProjectionDecision(
                retainMarker: false,
                protectCommittedRowAndSelection: false
            )
        }
        let authoritative = readyIncludesCommit(
            readyRevision: readyRevision,
            minimumAuthorityRevision: minimumAuthorityRevision,
            confirmedSessionVisible: confirmedSessionVisible
        )
        return RemoteCommittedProjectionDecision(
            retainMarker: !authoritative,
            protectCommittedRowAndSelection: !authoritative
        )
    }
}
