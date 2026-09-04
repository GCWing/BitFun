package com.openbitfun.mobile.core.feature.generalchat

import com.openbitfun.mobile.core.persistence.iosPersistenceStores
import com.openbitfun.mobile.core.persistence.iosSecureStore
import kotlinx.coroutines.CoroutineScope

public fun GeneralChatStore.Companion.create(scope: CoroutineScope): GeneralChatStore {
    val persistence = iosPersistenceStores("openbitfun-mobile.db")
    return GeneralChatStore.create(
        scope = scope,
        stream = GeneralChatStore.providerStream(),
        drafts = persistence.drafts,
        chats = persistence.chats,
        secure = iosSecureStore("com.openbitfun.mobile.generalchat"),
    )
}
