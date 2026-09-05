package com.openbitfun.mobile.core.persistence

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.alloc
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.COpaquePointerVar
import kotlinx.cinterop.readBytes
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.usePinned
import kotlinx.cinterop.value
import platform.CoreFoundation.CFDataCreate
import platform.CoreFoundation.CFDataGetBytePtr
import platform.CoreFoundation.CFDataGetLength
import platform.CoreFoundation.CFDataRef
import platform.CoreFoundation.CFDictionaryCreateMutable
import platform.CoreFoundation.CFDictionarySetValue
import platform.CoreFoundation.CFMutableDictionaryRef
import platform.CoreFoundation.CFRelease
import platform.CoreFoundation.CFStringCreateWithCString
import platform.CoreFoundation.kCFAllocatorDefault
import platform.CoreFoundation.kCFBooleanTrue
import platform.CoreFoundation.kCFStringEncodingUTF8
import platform.CoreFoundation.kCFTypeDictionaryKeyCallBacks
import platform.CoreFoundation.kCFTypeDictionaryValueCallBacks
import platform.Security.SecItemAdd
import platform.Security.SecItemCopyMatching
import platform.Security.SecItemDelete
import platform.Security.SecItemUpdate
import platform.Security.kSecAttrAccount
import platform.Security.kSecAttrService
import platform.Security.kSecClass
import platform.Security.kSecClassGenericPassword
import platform.Security.kSecMatchLimit
import platform.Security.kSecMatchLimitOne
import platform.Security.kSecReturnData
import platform.Security.kSecValueData
import platform.Security.errSecItemNotFound

internal enum class KeychainReadResult { FOUND, MISSING }

internal fun classifyKeychainReadStatus(status: Int): KeychainReadResult = when (status) {
    0 -> KeychainReadResult.FOUND
    errSecItemNotFound -> KeychainReadResult.MISSING
    else -> error("Keychain value could not be read (status=$status).")
}

/** Keychain-backed secret storage for iOS account credentials and keys. */
@OptIn(ExperimentalForeignApi::class)
private class IosSecureStore(private val service: String) : SecureStore {
    override fun read(key: String): ByteArray? = memScoped {
        val query = baseQuery(key)
        try {
            set(query, kSecReturnData, kCFBooleanTrue)
            set(query, kSecMatchLimit, kSecMatchLimitOne)
            val result = alloc<COpaquePointerVar>()
            when (classifyKeychainReadStatus(SecItemCopyMatching(query, result.ptr))) {
                KeychainReadResult.MISSING -> return@memScoped null
                KeychainReadResult.FOUND -> Unit
            }
            val data = result.value as? CFDataRef
                ?: error("Keychain returned success without data.")
            try {
                val length = CFDataGetLength(data).toInt()
                if (length == 0) return@memScoped ByteArray(0)
                CFDataGetBytePtr(data)?.readBytes(length)
            } finally {
                CFRelease(data)
            }
        } finally {
            CFRelease(query)
        }
    }

    override fun write(key: String, value: ByteArray) = memScoped {
        val data = value.usePinned { pinned ->
            if (value.isEmpty()) {
                CFDataCreate(kCFAllocatorDefault, null, 0)
            } else {
                CFDataCreate(kCFAllocatorDefault, pinned.addressOf(0).reinterpret(), value.size.toLong())
            }
        } ?: error("Keychain data allocation failed.")
        val query = baseQuery(key)
        val attributes = CFDictionaryCreateMutable(
            kCFAllocatorDefault,
            1,
            kCFTypeDictionaryKeyCallBacks.ptr,
            kCFTypeDictionaryValueCallBacks.ptr,
        )!!
        try {
            set(attributes, kSecValueData, data)
            val updateStatus = SecItemUpdate(query, attributes)
            if (updateStatus == 0) return@memScoped
            check(updateStatus == errSecItemNotFound) {
                "Keychain value could not be updated (status=$updateStatus)."
            }
            set(query, kSecValueData, data)
            check(SecItemAdd(query, null) == 0) {
                "Keychain value could not be persisted (status=$updateStatus)."
            }
        } finally {
            CFRelease(data)
            CFRelease(attributes)
            CFRelease(query)
        }
    }

    override fun delete(key: String) {
        val query = baseQuery(key)
        try {
            val status = SecItemDelete(query)
            check(status == 0 || status == errSecItemNotFound) {
                "Keychain value could not be removed (status=$status)."
            }
        } finally {
            CFRelease(query)
        }
    }

    private fun baseQuery(key: String): CFMutableDictionaryRef {
        val query = CFDictionaryCreateMutable(
            kCFAllocatorDefault,
            3,
            kCFTypeDictionaryKeyCallBacks.ptr,
            kCFTypeDictionaryValueCallBacks.ptr,
        )!!
        set(query, kSecClass, kSecClassGenericPassword)
        keyString(service).also { serviceRef ->
            set(query, kSecAttrService, serviceRef)
            CFRelease(serviceRef)
        }
        keyString(key).also { accountRef ->
            set(query, kSecAttrAccount, accountRef)
            CFRelease(accountRef)
        }
        return query
    }

    private fun keyString(value: String) =
        CFStringCreateWithCString(kCFAllocatorDefault, value, kCFStringEncodingUTF8)
            ?: error("Keychain string allocation failed.")

    private fun set(dict: CFMutableDictionaryRef, key: platform.CoreFoundation.CFTypeRef?, value: platform.CoreFoundation.CFTypeRef?) {
        CFDictionarySetValue(dict, key, value)
    }
}

/** Creates a Keychain namespace isolated from other OpenBitFun installations. */
public fun iosSecureStore(service: String): SecureStore = IosSecureStore(service)
