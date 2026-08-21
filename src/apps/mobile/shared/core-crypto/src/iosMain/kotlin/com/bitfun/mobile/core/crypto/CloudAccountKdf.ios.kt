package com.bitfun.mobile.core.crypto

import com.bitfun.mobile.core.crypto.argon2.bitfun_argon2id_raw
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.CPointer
import kotlinx.cinterop.UByteVar
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.usePinned
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

@OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)
public actual object PlatformArgon2id {
    public actual suspend fun derive(
        password: String,
        salt: ByteArray,
        params: CloudAccountKdfParams,
    ): ByteArray = withContext(Dispatchers.Default) {
        CloudAccountKdfPolicy.validate(params, salt)
        val passwordBytes = password.encodeToByteArray()
        ByteArray(32).also { output ->
            passwordBytes.usePinned { passwordPinned ->
                salt.usePinned { saltPinned ->
                    output.usePinned { outputPinned ->
                        val passwordPointer: CPointer<UByteVar>? =
                            if (passwordBytes.isEmpty()) null else passwordPinned.addressOf(0).reinterpret()
                        val result = bitfun_argon2id_raw(
                            passwordPointer, passwordBytes.size.toULong(),
                            saltPinned.addressOf(0).reinterpret(), salt.size.toULong(),
                            params.memoryKiB.toUInt(), params.iterations.toUInt(), params.parallelism.toUInt(),
                            outputPinned.addressOf(0).reinterpret(), output.size.toULong(),
                        )
                        check(result == 0) { "Argon2id failed with native status $result." }
                    }
                }
            }
        }
    }
}
