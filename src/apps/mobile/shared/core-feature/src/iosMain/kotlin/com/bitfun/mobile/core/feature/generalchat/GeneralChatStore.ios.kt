package com.bitfun.mobile.core.feature.generalchat

import com.bitfun.mobile.core.persistence.iosPersistenceStores
import com.bitfun.mobile.core.persistence.iosSecureStore
import kotlinx.coroutines.CoroutineScope

public fun GeneralChatStore.Companion.create(scope: CoroutineScope): GeneralChatStore {
    val persistence = iosPersistenceStores("bitfun-mobile.db")
    return GeneralChatStore.create(
        scope = scope,
        stream = GeneralChatStore.providerStream(),
        drafts = persistence.drafts,
        chats = persistence.chats,
        secure = iosSecureStore("com.bitfun.mobile.generalchat"),
    )
}
