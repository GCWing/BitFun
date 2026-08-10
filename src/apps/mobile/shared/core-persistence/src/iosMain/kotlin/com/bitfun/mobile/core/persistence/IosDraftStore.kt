package com.bitfun.mobile.core.persistence

import app.cash.sqldelight.driver.native.NativeSqliteDriver
import com.bitfun.mobile.core.persistence.db.MobileDatabase

public fun iosPersistenceStores(databaseName: String): MobilePersistenceStores =
    mobilePersistenceStores(NativeSqliteDriver(MobileDatabase.Schema, databaseName))
