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
