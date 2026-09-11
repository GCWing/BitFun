package com.openbitfun.mobile.app.ui.chat

import org.junit.Assert.assertEquals
import org.junit.Test

class ConversationComposerDraftTest {
    @Test
    fun mergesDictationOntoExistingTextWithASingleSpace() {
        assertEquals("existing draft spoken text", mergeComposerDraft("existing draft", "spoken text"))
    }

    @Test
    fun preservesExistingWhitespaceWhileTrimmingRecognition() {
        assertEquals("  existing  spoken", mergeComposerDraft("  existing  ", "  spoken  "))
    }

    @Test
    fun keepsExistingWhitespaceDraft() {
        assertEquals("spoken", mergeComposerDraft("", "  spoken  "))
        assertEquals("   spoken", mergeComposerDraft("   ", "spoken"))
    }

    @Test
    fun dropsABlankSpokenFragment() {
        assertEquals("  existing  ", mergeComposerDraft("  existing  ", ""))
        assertEquals("existing", mergeComposerDraft("existing", "   "))
    }

    @Test
    fun blankRecognitionDoesNotEraseDraftWhitespace() {
        assertEquals("", mergeComposerDraft("", ""))
        assertEquals("   ", mergeComposerDraft("   ", "   "))
    }
}
