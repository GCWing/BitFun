package com.openbitfun.mobile.core.feature.session

import kotlin.test.Test
import kotlin.test.assertEquals

class SessionActionPolicyTest {
    @Test
    fun aRemoteSessionNeverOffersTheLocalStorageActions() {
        val capabilities = SessionActionPolicy.resolve(SessionActionScope.REMOTE, "code", busy = false)

        assertEquals(SessionActionCapabilities(true, false, false, true), capabilities)
    }

    @Test
    fun archiveAndExportBelongToGeneralChatAlone() {
        assertEquals(
            SessionActionCapabilities(true, true, true, true),
            SessionActionPolicy.resolve(SessionActionScope.GENERAL, "chat", busy = false),
        )
        assertEquals(
            SessionActionCapabilities(true, false, false, true),
            SessionActionPolicy.resolve(SessionActionScope.GENERAL, "code", busy = false),
        )
    }

    @Test
    fun aCommandInFlightWithdrawsEverything() {
        // The row the menu was opened over may not survive the answer.
        listOf(SessionActionScope.GENERAL, SessionActionScope.REMOTE).forEach { scope ->
            assertEquals(
                SessionActionCapabilities(false, false, false, false),
                SessionActionPolicy.resolve(scope, "chat", busy = true),
                scope.name,
            )
        }
    }
}
