package com.openbitfun.mobile.core.feature.session

import com.openbitfun.mobile.core.domain.FilePreviewTargetContext
import com.openbitfun.mobile.core.domain.FileReferenceKind
import com.openbitfun.mobile.core.domain.FileTargetResolver
import com.openbitfun.mobile.core.feature.markdown.MarkdownBlock
import com.openbitfun.mobile.core.feature.markdown.MarkdownInline
import com.openbitfun.mobile.core.feature.markdown.MarkdownParser

/**
 * A file an assistant turn pointed at, lifted out of the prose so it can be
 * opened without hunting for the link in the paragraph.
 *
 * @param reference what the message actually wrote, punctuation and line marker
 * included — the preview needs it verbatim to know which lines to jump to.
 * @param remotePath the same file with its scheme stripped, which is what makes
 * two spellings of one file one card.
 */
public data class MessageFileReference public constructor(
    public val id: String,
    public val reference: String,
    public val remotePath: String,
    public val label: String,
)

/**
 * The files named in one message, ported from
 * `services/MessageFileReferenceProjector.ets`.
 *
 * A turn that touched a dozen files would otherwise put a dozen cards under two
 * sentences, so the projection stops at [DEFAULT_LIMIT]. The cards are a
 * shortcut to the ones being talked about, not an index of the turn.
 */
public object MessageFileReferenceProjector {
    /** As many cards as fit under a message without becoming the message. */
    public const val DEFAULT_LIMIT: Int = 4

    // The projection is about which files a turn named, not about which session
    // is open, so it resolves against a context that carries neither.
    private val CONTEXT = FilePreviewTargetContext(
        sessionId = "_message_projection_",
        workspacePath = "",
        controlTargetEpoch = 0,
    )

    private val BARE_REFERENCE = Regex("""computer://[^\s)\]}>"']+""")

    public fun project(source: String): List<MessageFileReference> = project(source, DEFAULT_LIMIT)

    public fun project(source: String, limit: Int): List<MessageFileReference> {
        val references = mutableListOf<MessageFileReference>()
        val seen = mutableSetOf<String>()
        MarkdownParser.parse(source).forEach { block: MarkdownBlock ->
            collect(block.inlines, references, seen, limit)
            block.items.forEach { collect(it.inlines, references, seen, limit) }
        }
        return references
    }

    private fun collect(
        inlines: List<MarkdownInline>,
        references: MutableList<MessageFileReference>,
        seen: MutableSet<String>,
        limit: Int,
    ) {
        inlines.forEach { inline ->
            if (references.size >= limit) return
            when (inline.type) {
                "link" -> add(inline.url, references, seen)
                // A path inside backticks is being shown, not offered — it is
                // often a fragment or a pattern rather than a file that exists.
                "code" -> Unit
                else -> BARE_REFERENCE.findAll(inline.text).forEach { match ->
                    if (references.size < limit) add(match.value, references, seen)
                }
            }
        }
    }

    private fun add(
        reference: String,
        references: MutableList<MessageFileReference>,
        seen: MutableSet<String>,
    ) {
        // Only the desktop's own scheme: an `http` link is a link, and a bare
        // relative path in prose is far more often a word than a file.
        if (!reference.trim().lowercase().startsWith("computer://")) return
        val resolution = FileTargetResolver.resolve(reference, "", CONTEXT)
        val target = resolution.target ?: return
        if (resolution.kind != FileReferenceKind.REMOTE_WORKSPACE_FILE) return
        if (!seen.add(target.remotePath)) return
        references += MessageFileReference(
            id = "file-${references.size}-${target.remotePath}",
            reference = target.path,
            remotePath = target.remotePath,
            label = target.displayName,
        )
    }
}

/**
 * Holds one message's projection so a recomposition does not re-parse it.
 *
 * Keyed on the source text rather than a message id: a streaming turn keeps its
 * id while its text grows, and the cards have to grow with it.
 */
public class MessageFileReferenceCache public constructor() {
    private var source: String? = null
    private var references: List<MessageFileReference> = emptyList()

    public fun referencesFor(text: String): List<MessageFileReference> {
        if (source == text) return references
        source = text
        references = MessageFileReferenceProjector.project(text)
        return references
    }
}
