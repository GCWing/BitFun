package com.openbitfun.mobile.core.persistence

import app.cash.sqldelight.driver.native.NativeSqliteDriver
import com.openbitfun.mobile.core.persistence.db.MobileDatabase

public fun iosPersistenceStores(databaseName: String): MobilePersistenceStores =
    mobilePersistenceStores(NativeSqliteDriver(MobileDatabase.Schema, databaseName))
