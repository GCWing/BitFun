import XCTest

final class RemoteCodeSessionSendUITests: XCTestCase {
    private let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")

    override func setUpWithError() throws {
        continueAfterFailure = false
        app.launch()
    }

    func testSendMessageInCurrentRemoteCodeSession() throws {
        let activeSession = app.descendants(matching: .any).matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "conversation.session.")
        ).firstMatch
        guard activeSession.waitForExistence(timeout: 20) else {
            recordDiagnostics(named: "NoActiveRemoteSession")
            XCTFail(
                "No active Remote Code Session is visible. Keep the phone unlocked and navigate OpenBitFun to the intended existing remote code session; this test will not choose a session automatically."
            )
            return
        }

        let composer = app.textFields.firstMatch
        guard composer.waitForExistence(timeout: 10), composer.isHittable else {
            recordDiagnostics(named: "ComposerUnavailable")
            XCTFail("The current remote session is visible, but its composer text field is unavailable or obstructed.")
            return
        }

        let message = "iPhone remote send E2E \(Int(Date().timeIntervalSince1970))"
        composer.tap()
        composer.typeText(message)
        XCTAssertEqual(composer.value as? String, message, "The complete harmless E2E message was not entered.")

        let sendButton = firstExistingElement([
            app.buttons["发送"],
            app.buttons["Send"],
        ])
        guard let sendButton else {
            recordDiagnostics(named: "SendButtonMissing")
            XCTFail("The composer contains the E2E message, but no localized Send button is visible.")
            return
        }
        guard sendButton.isEnabled else {
            recordDiagnostics(named: "SendButtonDisabled")
            XCTFail("The Send button is disabled; remote mutation authority may not be confirmed or the session may be busy. The draft was not dispatched.")
            return
        }

        sendButton.tap()

        let draftCleared = NSPredicate { evaluated, _ in
            guard let field = evaluated as? XCUIElement else { return false }
            return (field.value as? String) != message
        }
        expectation(for: draftCleared, evaluatedWith: composer)
        waitForExpectations(timeout: 10)

        let timelineMessage = app.staticTexts[message]
        guard timelineMessage.waitForExistence(timeout: 20) else {
            recordDiagnostics(named: "TimelineMessageMissing")
            XCTFail("The draft cleared after tapping Send, but the exact E2E message did not appear in the current timeline; dispatch or peer synchronization may have failed.")
            return
        }

        let evidence = XCTAttachment(string: "Sent message visible in current timeline: \(message)")
        evidence.name = "RemoteCodeSessionSendEvidence"
        evidence.lifetime = .keepAlways
        add(evidence)
        recordDiagnostics(named: "RemoteCodeSessionSendSucceeded")
    }

    private func firstExistingElement(_ elements: [XCUIElement]) -> XCUIElement? {
        elements.first { $0.waitForExistence(timeout: 2) }
    }

    private func recordDiagnostics(named name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)

        let hierarchy = XCTAttachment(string: app.debugDescription)
        hierarchy.name = "\(name)-Hierarchy"
        hierarchy.lifetime = .keepAlways
        add(hierarchy)
    }
}

/// Read-only real-device probe for the remote-create critical path. It opens
/// the workspace picker and observes its first usable row, but never selects a
/// workspace, creates a session, or sends content.
final class RemoteWorkspaceLoadingPerformanceUITests: XCTestCase {
    private let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")
    private var startedAt = Date()

    override func setUpWithError() throws {
        continueAfterFailure = false
        startedAt = Date()
        app.launch()
    }

    func testConnectedHomeToWorkspaceRowsReadOnly() throws {
        let connected = firstExistingElement([
            app.staticTexts["桌面端已连接"],
            app.staticTexts["Desktop connected"],
        ], timeout: 30)
        XCTAssertNotNil(connected, "The existing remote connection did not restore on the device.")
        recordMilestone("connected_home")

        guard let create = firstExistingElement([
            app.buttons["新建远程会话"],
            app.buttons["New remote session"],
        ], timeout: 10) else {
            recordDiagnostics(named: "RemoteCreateActionMissing")
            XCTFail("The connected home did not expose the remote-create action.")
            return
        }
        XCTAssertTrue(create.isHittable)
        create.tap()
        recordMilestone("create_opened")

        let selectors = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "remoteCreate.workspace.")
        )
        let selector = selectors.firstMatch
        let usableSelector = NSPredicate { evaluated, _ in
            guard let element = evaluated as? XCUIElement else { return false }
            return element.exists && element.isHittable && element.isEnabled
        }
        expectation(for: usableSelector, evaluatedWith: selector)
        waitForExpectations(timeout: 15)
        XCTAssertEqual(selectors.count, 1, "Workspace selector must be unambiguous.")
        recordMilestone("workspace_selector_usable")

        selector.tap()
        recordMilestone("workspace_picker_opened")

        let workspaceRows = app.buttons.matching(
            NSPredicate(
                format: "label BEGINSWITH %@ OR label BEGINSWITH %@",
                "工作区:",
                "Workspace:"
            )
        )
        let firstRow = workspaceRows.firstMatch
        let usableRow = NSPredicate { evaluated, _ in
            guard let element = evaluated as? XCUIElement else { return false }
            return element.exists && element.isHittable && element.isEnabled
        }
        expectation(for: usableRow, evaluatedWith: firstRow)
        waitForExpectations(timeout: 45)
        recordMilestone("first_workspace_row_usable")
        recordDiagnostics(named: "WorkspaceRowsReadOnlySucceeded")
    }

    private func firstExistingElement(_ elements: [XCUIElement], timeout: TimeInterval) -> XCUIElement? {
        elements.first { $0.waitForExistence(timeout: timeout) }
    }

    private func recordMilestone(_ name: String) {
        let elapsed = Date().timeIntervalSince(startedAt)
        let attachment = XCTAttachment(string: String(format: "%@ +%.3fs", name, elapsed))
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func recordDiagnostics(named name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)

        let hierarchy = XCTAttachment(string: app.debugDescription)
        hierarchy.name = "\(name)-Hierarchy"
        hierarchy.lifetime = .keepAlways
        add(hierarchy)
    }
}

final class RemoteCreateWorkspacePickerUITests: XCTestCase {
    private let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")

    override func setUpWithError() throws {
        continueAfterFailure = false
        app.launchArguments = ["--remote-create"]
        app.launch()
    }

    func testWorkspacePickerOpensAndExposesUsableRows() {
        assertWorkspacePickerUsable()
    }

    func testSessionDirectoryBusyDoesNotDisableWorkspacePicker() {
        app.terminate()
        app.launchArguments = ["--remote-create-session-loading"]
        app.launch()
        assertWorkspacePickerUsable()
    }

    private func assertWorkspacePickerUsable() {
        let selectors = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "remoteCreate.workspace.")
        )
        let selector = selectors.firstMatch
        XCTAssertTrue(selector.waitForExistence(timeout: 10))
        XCTAssertEqual(selectors.count, 1)
        XCTAssertTrue(selector.isHittable)
        XCTAssertTrue(selector.isEnabled, "Independent session loading must not disable the workspace selector.")

        selector.tap()

        let workspaceRows = app.buttons.matching(
            NSPredicate(
                format: "label BEGINSWITH %@ OR label BEGINSWITH %@",
                "工作区:",
                "Workspace:"
            )
        )
        let row = workspaceRows.firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        XCTAssertTrue(row.isHittable)
        XCTAssertTrue(row.isEnabled)

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = "RemoteCreateWorkspacePickerUsable"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}

/// Read-only responsiveness probes for the two composer implementations. The
/// tests enter a disposable character but never submit it or mutate a remote
/// session.
final class ComposerFocusResponsivenessUITests: XCTestCase {
    private let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")

    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    func testRemoteHomeComposerAcceptsFirstCharacter() {
        app.launchArguments = ["--remote", "--connected", "--remote-home-preview"]
        app.launch()
        assertFirstCharacterResponsiveness(
            identifier: "composer.input",
            named: "RemoteHomeComposer"
        )
    }

    func testRemoteCreateComposerAcceptsFirstCharacter() {
        app.launchArguments = ["--remote-create"]
        app.launch()
        assertFirstCharacterResponsiveness(
            identifier: "remoteCreate.composer.input",
            named: "RemoteCreateComposer"
        )
    }

    private func assertFirstCharacterResponsiveness(identifier: String, named name: String) {
        let field = app.textFields[identifier]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "\(name) text field did not appear.")
        XCTAssertTrue(field.isHittable, "\(name) text field is obstructed.")

        let tapStart = Date()
        field.tap()
        let tapElapsed = Date().timeIntervalSince(tapStart)

        let typeStart = Date()
        field.typeText("x")
        let typeElapsed = Date().timeIntervalSince(typeStart)

        XCTAssertEqual(field.value as? String, "x", "\(name) did not accept the first character.")
        XCTAssertLessThan(tapElapsed, 2, "\(name) focus transition blocked UI automation.")
        XCTAssertLessThan(typeElapsed, 2, "\(name) first character handling blocked UI automation.")
        let keyboard = app.keyboards.firstMatch
        if keyboard.exists {
            XCTAssertLessThanOrEqual(
                field.frame.maxY,
                keyboard.frame.minY + 1,
                "\(name) is covered by the software keyboard."
            )
        }

        let timing = XCTAttachment(
            string: String(format: "tap=%.3fs type=%.3fs keyboards=%d", tapElapsed, typeElapsed, app.keyboards.count)
        )
        timing.name = "\(name)-Timing"
        timing.lifetime = .keepAlways
        add(timing)

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = "\(name)-Focused"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}

/// Exercises the complete account-driven remote workflow against a real relay.
/// Credentials are supplied by the invoking process and are never persisted in
/// the test bundle or emitted in diagnostics.
final class RemoteAccountWorkflowPerformanceUITests: XCTestCase {
    private let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")
    private var workflowStartedAt = Date()

    override func setUpWithError() throws {
        continueAfterFailure = false
        workflowStartedAt = Date()
        app.launchArguments = ["--simplified-chinese"]
        app.launch()
    }

    func testLoginDeviceWorkspaceSessionSwitchAndSend() throws {
        let environment = ProcessInfo.processInfo.environment
        guard let username = environment["OPENBITFUN_E2E_USERNAME"], !username.isEmpty,
              let password = environment["OPENBITFUN_E2E_PASSWORD"], !password.isEmpty else {
            throw XCTSkip("Set OPENBITFUN_E2E_USERNAME and OPENBITFUN_E2E_PASSWORD to run the real relay workflow.")
        }

        let sidebarAlreadyOpen = try signIn(username: username, password: password)
        let deviceID = try selectFirstRemoteDevice(openSidebar: !sidebarAlreadyOpen)
        let sessionIDs = try exerciseWorkspaceDirectory(deviceID: deviceID)
        try exerciseSessionSwitching(sessionIDs: sessionIDs)
        try sendVerificationMessage()
        let idleStarted = Date()
        RunLoop.current.run(until: Date().addingTimeInterval(5))
        recordStep("post_workflow_idle", since: idleStarted)
        recordDiagnostics(named: "RemoteAccountWorkflowSucceeded")
    }

    private func signIn(username: String, password: String) throws -> Bool {
        let sidebar = try requireFirst([
            app.buttons["打开侧栏"],
            app.buttons["Open sidebar"],
        ], timeout: 20, failure: "The sidebar action did not become available.")
        sidebar.tap()

        let accountActions = [
            app.buttons["登录 OpenBitFun 账号"],
            app.buttons["Sign in to OpenBitFun"],
        ]
        if !waitUntil(timeout: 5, condition: {
            accountActions.contains(where: \.exists) || self.app.buttons.matching(
                NSPredicate(format: "identifier BEGINSWITH %@", "sidebar.device.")
            ).count > 0
        }) {
            recordDiagnostics(named: "AccountStateUnavailable")
            XCTFail("The sidebar exposed neither account login nor a restored device directory.")
            throw WorkflowError.requiredElementMissing
        }
        if !accountActions.contains(where: \.exists) {
            recordStep("account_restore_ready", since: workflowStartedAt)
            return true
        }
        let accountAction = accountActions.first(where: \.exists)!
        accountAction.tap()

        let usernameField = try requireFirst([
            app.textFields["用户名"],
            app.textFields["Username"],
        ], timeout: 10, failure: "The username field did not appear.")
        let passwordField = try requireFirst([
            app.secureTextFields["密码"],
            app.secureTextFields["Password"],
        ], timeout: 10, failure: "The password field did not appear.")
        usernameField.tap()
        usernameField.typeText(username)
        passwordField.tap()
        passwordField.typeText(password)

        let login = try requireFirst([
            app.buttons["登录"],
            app.buttons["Sign in"],
        ], timeout: 5, failure: "The login button did not become available.")
        XCTAssertTrue(login.isEnabled, "The completed login form remained disabled.")
        let started = Date()
        login.tap()

        _ = try requireFirst([
            app.staticTexts["个人资料"],
            app.staticTexts["Profile"],
        ], timeout: 45, failure: "Login did not reach the account profile.")
        recordStep("login_ready", since: started)

        let close = try requireFirst([
            app.buttons["关闭"],
            app.buttons["Close"],
        ], timeout: 5, failure: "The account sheet could not be closed.")
        close.tap()
        _ = try requireFirst([
            app.buttons["打开侧栏"],
            app.buttons["Open sidebar"],
        ], timeout: 10, failure: "The conversation surface did not return after login.")
        return false
    }

    private func selectFirstRemoteDevice(openSidebar: Bool) throws -> String {
        if openSidebar {
            let sidebar = try requireFirst([
                app.buttons["打开侧栏"],
                app.buttons["Open sidebar"],
            ], timeout: 10, failure: "The sidebar action was unavailable after login.")
            sidebar.tap()
        }

        let devices = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "sidebar.device.")
        )
        let device = try requireUsableElement(
            in: devices,
            timeout: 30,
            failure: "No online account device became selectable."
        )
        let identifier = device.identifier
        let deviceID = String(identifier.dropFirst("sidebar.device.".count))
        let started = Date()
        device.tap()

        let workspaces = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "sidebar.workspace.\(deviceID).")
        )
        _ = try requireUsableElement(
            in: workspaces,
            timeout: 45,
            failure: "The selected device did not expose a usable workspace."
        )
        recordStep("device_workspaces_ready", since: started)
        return deviceID
    }

    private func exerciseWorkspaceDirectory(deviceID: String) throws -> [String] {
        let workspacePrefix = "sidebar.workspace.\(deviceID)."
        let sessionPrefix = "sidebar.session.\(deviceID)."
        let workspaces = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", workspacePrefix)
        )
        XCTAssertGreaterThan(workspaces.count, 1, "At least two workspaces are required to validate switching.")

        var chosenWorkspace: XCUIElement?
        var sessionIDs: [String] = []
        let candidateCount = min(workspaces.count, 8)
        for index in 0..<candidateCount {
            let workspace = workspaces.element(boundBy: index)
            guard scrollToHittable(workspace) else { continue }
            let started = Date()
            workspace.tap()
            waitForDirectoryLoadingToFinish(timeout: 30)
            recordStep("workspace_\(index + 1)_expanded", since: started)

            let sessions = app.buttons.matching(
                NSPredicate(format: "identifier BEGINSWITH %@", sessionPrefix)
            )
            sessionIDs = uniqueIdentifiers(from: sessions)
            if sessionIDs.count >= 2 {
                chosenWorkspace = workspace
                break
            }

            if scrollToHittable(workspace) { workspace.tap() }
        }

        guard let chosenWorkspace else {
            recordDiagnostics(named: "NoWorkspaceWithTwoSessions")
            XCTFail("No inspected workspace exposed two sessions for rapid switching.")
            return []
        }

        if scrollToHittable(chosenWorkspace) { chosenWorkspace.tap() }
        let secondWorkspace = workspaces.element(boundBy: 1)
        guard scrollToHittable(secondWorkspace) else {
            XCTFail("A second workspace exists but could not be scrolled into view.")
            return []
        }
        let switchStarted = Date()
        secondWorkspace.tap()
        waitForDirectoryLoadingToFinish(timeout: 30)
        recordStep("workspace_switch_ready", since: switchStarted)

        if scrollToHittable(secondWorkspace) { secondWorkspace.tap() }
        guard scrollToHittable(chosenWorkspace) else {
            XCTFail("The source workspace could not be restored after switching.")
            return []
        }
        chosenWorkspace.tap()
        waitForDirectoryLoadingToFinish(timeout: 30)
        return Array(sessionIDs.prefix(3))
    }

    private func exerciseSessionSwitching(sessionIDs: [String]) throws {
        guard sessionIDs.count >= 2 else {
            XCTFail("Rapid switching requires at least two sessions.")
            return
        }

        try openSession(sessionIDs[0], step: "session_first_ready")
        try reopenSidebar()
        try selectSessionWithoutWaiting(sessionIDs[1], step: "rapid_switch_first_selected")
        try reopenSidebar()
        let finalSession = sessionIDs.count > 2 ? sessionIDs[2] : sessionIDs[0]
        try openSession(finalSession, step: "rapid_switch_final_ready")
        try reopenSidebar()
        try openSession(sessionIDs[0], step: "cached_session_return_ready")
        try reopenSidebar()
        try openSession(finalSession, step: "cached_final_session_ready")
        try selectIdleSession(from: sessionIDs)
    }

    private func openSession(_ sessionID: String, step: String) throws {
        let session = app.buttons[sessionID]
        guard scrollToHittable(session) else {
            recordDiagnostics(named: "SessionUnavailable")
            XCTFail("The requested session was not reachable in the expanded workspace.")
            return
        }
        let rawID = sessionID.components(separatedBy: ".").last ?? sessionID
        let started = Date()
        session.tap()

        let conversation = app.descendants(matching: .any)["conversation.session.\(rawID)"]
        XCTAssertTrue(
            waitUntil(timeout: 10) { conversation.exists },
            "The selected conversation identity did not update."
        )
        waitForConversationLoadingToFinish(timeout: 45)
        let composer = app.textFields["composer.input"]
        XCTAssertTrue(
            waitUntil(timeout: 10) { composer.exists && composer.isHittable },
            "The selected conversation composer did not become available."
        )
        XCTAssertTrue(
            waitUntil(timeout: 45) {
                let voice = self.app.buttons["语音输入"]
                let stop = self.app.buttons["停止"]
                return (voice.exists && voice.isEnabled) || (stop.exists && stop.isEnabled)
            },
            "The selected conversation remained busy after its content appeared."
        )
        recordStep(step, since: started)
    }

    private func selectSessionWithoutWaiting(_ sessionID: String, step: String) throws {
        let session = app.buttons[sessionID]
        guard scrollToHittable(session) else {
            XCTFail("The rapid-switch source session was unreachable.")
            return
        }
        let rawID = sessionID.components(separatedBy: ".").last ?? sessionID
        let started = Date()
        session.tap()
        let conversation = app.descendants(matching: .any)["conversation.session.\(rawID)"]
        XCTAssertTrue(
            waitUntil(timeout: 10) { conversation.exists },
            "The rapid-switch source selection was not projected."
        )
        recordStep(step, since: started)
    }

    private func selectIdleSession(from sessionIDs: [String]) throws {
        for (index, sessionID) in sessionIDs.enumerated() {
            try reopenSidebar()
            try openSession(sessionID, step: "send_candidate_\(index + 1)_ready")
            let voice = app.buttons["语音输入"]
            if voice.exists && voice.isEnabled { return }
            let send = app.buttons["发送"]
            XCTAssertFalse(
                send.exists && send.isEnabled,
                "A session with an active turn unexpectedly allowed another send."
            )
        }
        recordDiagnostics(named: "NoIdleSessionForSend")
        XCTFail("All inspected sessions still had active turns, so the verification message was not dispatched.")
        throw WorkflowError.requiredElementMissing
    }

    private func reopenSidebar() throws {
        let action = try requireFirst([
            app.buttons["打开侧栏"],
            app.buttons["Open sidebar"],
        ], timeout: 10, failure: "The sidebar could not be reopened while switching sessions.")
        action.tap()
        let sessions = app.buttons.matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "sidebar.session.")
        )
        _ = try requireUsableElement(in: sessions, timeout: 10, failure: "Expanded sessions were not restored in the sidebar.")
    }

    private func sendVerificationMessage() throws {
        let composer = app.textFields["composer.input"]
        XCTAssertTrue(waitUntil(timeout: 10) { composer.exists }, "The composer was unavailable before send.")
        XCTAssertTrue(composer.isHittable, "The composer was obstructed before send.")

        let message = "iOS 模拟器端到端性能验证 \(Int(Date().timeIntervalSince1970))"
        composer.tap()
        composer.typeText(message)
        XCTAssertEqual(composer.value as? String, message)

        let send = try requireFirst([
            app.buttons["发送"],
            app.buttons["Send"],
        ], timeout: 5, failure: "The Send button did not become available after typing.")
        XCTAssertTrue(send.isEnabled, "The selected session did not allow message sending.")
        let started = Date()
        send.tap()

        XCTAssertTrue(
            waitUntil(timeout: 10) { (composer.value as? String) != message },
            "The draft did not clear after dispatch."
        )
        recordStep("send_draft_cleared", since: started)

        let timelineMessage = app.staticTexts[message]
        XCTAssertTrue(
            waitUntil(timeout: 30) { timelineMessage.exists },
            "The submitted message did not appear in the active timeline."
        )
        recordStep("send_message_visible", since: started)
    }

    private func waitForDirectoryLoadingToFinish(timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let loading = app.staticTexts["正在加载"]
            let loadingWorkspaces = app.staticTexts["正在加载工作区"]
            if !loading.exists && !loadingWorkspaces.exists { return }
            RunLoop.current.run(until: Date().addingTimeInterval(0.1))
        }
    }

    private func waitForConversationLoadingToFinish(timeout: TimeInterval) {
        RunLoop.current.run(until: Date().addingTimeInterval(0.18))
        let loading = app.descendants(matching: .any).matching(
            NSPredicate(format: "label == %@", "正在加载")
        ).firstMatch
        guard loading.exists else { return }
        XCTAssertTrue(waitUntil(timeout: timeout) { !loading.exists }, "The conversation loading state did not finish.")
    }

    private func requireFirst(
        _ elements: [XCUIElement],
        timeout: TimeInterval,
        failure: String
    ) throws -> XCUIElement {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if let element = elements.first(where: \.exists) { return element }
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        } while Date() < deadline
        if let element = elements.first(where: \.exists) {
            return element
        }
        recordDiagnostics(named: "RequiredElementMissing")
        XCTFail(failure)
        throw WorkflowError.requiredElementMissing
    }

    private func requireUsableElement(
        in query: XCUIElementQuery,
        timeout: TimeInterval,
        failure: String
    ) throws -> XCUIElement {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            for index in 0..<query.count {
                let candidate = query.element(boundBy: index)
                if candidate.exists && candidate.isEnabled && candidate.isHittable { return candidate }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.1))
        }
        recordDiagnostics(named: "UsableElementMissing")
        XCTFail(failure)
        throw WorkflowError.requiredElementMissing
    }

    private func scrollToHittable(_ element: XCUIElement) -> Bool {
        guard waitUntil(timeout: 5, condition: { element.exists }) else { return false }
        for _ in 0..<8 {
            if element.isHittable { return true }
            app.swipeUp()
        }
        for _ in 0..<8 {
            if element.isHittable { return true }
            app.swipeDown()
        }
        return element.isHittable
    }

    private func waitUntil(timeout: TimeInterval, condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        } while Date() < deadline
        return condition()
    }

    private func uniqueIdentifiers(from query: XCUIElementQuery) -> [String] {
        var seen = Set<String>()
        var result: [String] = []
        for index in 0..<query.count {
            let identifier = query.element(boundBy: index).identifier
            if !identifier.isEmpty, seen.insert(identifier).inserted { result.append(identifier) }
        }
        return result
    }

    private func recordStep(_ name: String, since started: Date) {
        let elapsedMS = Date().timeIntervalSince(started) * 1_000
        let totalMS = Date().timeIntervalSince(workflowStartedAt) * 1_000
        let line = String(format: "[IOS_E2E_PERF] step=%@ elapsed_ms=%.0f total_ms=%.0f", name, elapsedMS, totalMS)
        print(line)
        let attachment = XCTAttachment(string: line)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func recordDiagnostics(named name: String) {
        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.name = name
        screenshot.lifetime = .keepAlways
        add(screenshot)

        let hierarchy = XCTAttachment(string: app.debugDescription)
        hierarchy.name = "\(name)-Hierarchy"
        hierarchy.lifetime = .keepAlways
        add(hierarchy)
    }

    private enum WorkflowError: Error {
        case requiredElementMissing
    }
}

/// Run on a fresh simulator installation so notification authorization is undecided.
final class NotificationOnboardingUITests: XCTestCase {
    func testLaterPersistsAcrossRelaunch() {
        let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")
        app.launch()
        let prompt = app.alerts.firstMatch
        XCTAssertTrue(prompt.waitForExistence(timeout: 15))
        let later = app.buttons.matching(NSPredicate(format: "label IN %@", ["Later", "稍后"])).firstMatch
        XCTAssertTrue(later.exists)
        later.tap()
        XCTAssertFalse(prompt.exists)
        app.terminate()
        app.launch()
        let sidebar = app.buttons.matching(NSPredicate(format: "label IN %@", ["Open sidebar", "打开侧栏"])).firstMatch
        XCTAssertTrue(sidebar.waitForExistence(timeout: 15))
        XCTAssertFalse(prompt.waitForExistence(timeout: 3), "Skipping onboarding must survive process restart.")
        // Use the existing settings inspection route without signing in a test account.
        app.terminate()
        app.launchArguments = ["--settings"]
        app.launch()
        let notifications = app.buttons["settings.notifications"]
        XCTAssertTrue(notifications.waitForExistence(timeout: 5))
        notifications.tap()
        let systemPrompt = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.firstMatch
        XCTAssertTrue(systemPrompt.waitForExistence(timeout: 10), "Settings must still allow authorization after Later.")
        let screenshot = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        screenshot.name = "NotificationAuthorizationAfterLater"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}

final class SidebarNavigationUITests: XCTestCase {
    func testNewChatOffersHarnessProfilesInCompactAndWideSidebar() {
        let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")
        app.launchArguments = ["--harness-preview", "--drawer", "--simplified-chinese"]
        app.launch()
        let later = app.alerts.buttons["稍后"]
        if later.waitForExistence(timeout: 3) { later.tap() }
        let newChat = app.buttons["sidebar.newChat"]
        XCTAssertTrue(newChat.waitForExistence(timeout: 10))
        XCTAssertTrue(newChat.isEnabled)
        XCTAssertTrue(newChat.label.contains("新聊天"))
        let workspace = app.buttons.matching(NSPredicate(format: "identifier BEGINSWITH %@", "sidebar.workspace.")).firstMatch
        XCTAssertTrue(workspace.exists)
        workspace.tap()
        let compact = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        compact.name = "SidebarCompact"
        compact.lifetime = .keepAlways
        add(compact)
        newChat.tap()
        XCTAssertTrue(app.buttons.containing(NSPredicate(format: "label CONTAINS %@", "标准")).firstMatch.waitForExistence(timeout: 5))
        app.terminate()
        XCUIDevice.shared.orientation = .landscapeLeft
        defer { XCUIDevice.shared.orientation = .portrait }
        app.launch()
        XCTAssertTrue(newChat.waitForExistence(timeout: 10))
        XCTAssertTrue(newChat.isHittable)
        app.scrollViews.firstMatch.swipeUp()
        XCTAssertLessThan(workspace.frame.maxY, newChat.frame.minY, "The final workspace must scroll clear of the floating actions.")
        let wide = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        wide.name = "SidebarWide"
        wide.lifetime = .keepAlways
        add(wide)
    }
}

final class GitHubLoginPresentationUITests: XCTestCase {
    func testLoginUsesCompactSheetAndOpensBrowserAutomatically() {
        let app = XCUIApplication(bundleIdentifier: "com.openbitfun.mobile.ios")
        app.launchArguments = ["--drawer", "--simplified-chinese"]
        app.launch()
        let later = app.alerts.buttons["稍后"]
        if later.waitForExistence(timeout: 3) { later.tap() }
        let account = app.buttons["使用 GitHub 登录"]
        XCTAssertTrue(account.waitForExistence(timeout: 10), "Run on a signed-out simulator.")
        account.tap()
        let login = app.buttons["account.login"]
        XCTAssertTrue(login.waitForExistence(timeout: 5))
        let title = app.staticTexts["使用 GitHub 登录"]
        XCTAssertGreaterThan(title.frame.minY, app.frame.height * 0.5)
        let sheet = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        sheet.name = "CompactGitHubLogin"
        sheet.lifetime = .keepAlways
        add(sheet)
        login.tap()
        let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")
        let opened = XCTNSPredicateExpectation(predicate: NSPredicate(format: "state == %d", XCUIApplication.State.runningForeground.rawValue), object: safari)
        XCTAssertEqual(XCTWaiter.wait(for: [opened], timeout: 30), .completed, "A single login tap must open the authorization browser.")
        let browser = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        browser.name = "AutomaticallyOpenedAuthorization"
        browser.lifetime = .keepAlways
        add(browser)
        app.activate()
        XCTAssertTrue(login.waitForExistence(timeout: 5))
        XCTAssertTrue(login.isEnabled, "Returning from the browser must allow reopening the same authorization.")
        XCTAssertTrue(login.label.contains("打开 GitHub 授权"))
        app.terminate()
    }
}
