package com.bitfun.mobile.app.ui.chat

import org.junit.Assert.assertEquals
import org.junit.Test

class ConversationComposerDraftTest {
    @Test
    fun mergesDictationOntoExistingTextWithASingleSpace() {
        assertEquals("existing draft spoken text", mergeComposerDraft("existing draft", "spoken text"))
    }

    @Test
    fun trimsBothSidesBeforeJoining() {
        assertEquals("existing spoken", mergeComposerDraft("  existing  ", "  spoken  "))
    }

    @Test
    fun dropsABlankExistingDraft() {
        assertEquals("spoken", mergeComposerDraft("", "  spoken  "))
        assertEquals("spoken", mergeComposerDraft("   ", "spoken"))
    }

    @Test
    fun dropsABlankSpokenFragment() {
        assertEquals("existing", mergeComposerDraft("  existing  ", ""))
        assertEquals("existing", mergeComposerDraft("existing", "   "))
    }

    @Test
    fun bothBlankProducesEmptyDraft() {
        assertEquals("", mergeComposerDraft("", ""))
        assertEquals("", mergeComposerDraft("   ", "   "))
    }
}
