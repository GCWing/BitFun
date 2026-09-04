package com.openbitfun.mobile.core.persistence

import app.cash.sqldelight.db.SqlDriver
import com.openbitfun.mobile.core.persistence.db.MobileDatabase
import kotlin.time.Clock

public interface DraftStore {
    public fun load(draftId: String): String?

    public fun save(draftId: String, text: String)

    public fun delete(draftId: String)
}

/** SQLDelight-backed drafts; credentials and keys are intentionally absent. */
public class SqlDelightDraftStore public constructor(
    driver: SqlDriver,
) : DraftStore {
    private val queries = MobileDatabase(driver).mobileQueries

    override fun load(draftId: String): String? =
        queries.selectDraft(draftId).executeAsOneOrNull()?.text

    override fun save(draftId: String, text: String) {
        queries.upsertDraft(draftId, text, Clock.System.now().toEpochMilliseconds())
    }

    override fun delete(draftId: String) {
        queries.deleteDraft(draftId)
    }
}
