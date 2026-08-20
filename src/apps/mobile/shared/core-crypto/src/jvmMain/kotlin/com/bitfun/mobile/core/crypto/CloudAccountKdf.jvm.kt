package com.bitfun.mobile.core.crypto

import org.bouncycastle.crypto.generators.Argon2BytesGenerator
import org.bouncycastle.crypto.params.Argon2Parameters
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

public actual object PlatformArgon2id {
    public actual suspend fun derive(
        password: String,
        salt: ByteArray,
        params: CloudAccountKdfParams,
    ): ByteArray = withContext(Dispatchers.Default) {
        CloudAccountKdfPolicy.validate(params, salt)
        val parameters = Argon2Parameters.Builder(Argon2Parameters.ARGON2_id)
            .withVersion(Argon2Parameters.ARGON2_VERSION_13)
            .withSalt(salt)
            .withMemoryAsKB(params.memoryKiB)
            .withIterations(params.iterations)
            .withParallelism(params.parallelism)
            .build()
        ByteArray(32).also { output ->
            Argon2BytesGenerator().also { it.init(parameters) }
                .generateBytes(password.encodeToByteArray(), output)
        }
    }
}
