package com.openbitfun.mobile.core.feature.generalchat

import android.content.Context
import com.openbitfun.mobile.core.persistence.androidPersistenceStores
import com.openbitfun.mobile.core.persistence.androidSecureStore
import kotlinx.coroutines.CoroutineScope

public fun GeneralChatStore.Companion.create(
    scope: CoroutineScope,
    context: Context,
): GeneralChatStore {
    val application = context.applicationContext
    val persistence = androidPersistenceStores(application, "openbitfun-mobile.db")
    return GeneralChatStore.create(
        scope = scope,
        stream = GeneralChatStore.providerStream(),
        drafts = persistence.drafts,
        chats = persistence.chats,
        secure = androidSecureStore(application, "general-chat"),
    )
}
