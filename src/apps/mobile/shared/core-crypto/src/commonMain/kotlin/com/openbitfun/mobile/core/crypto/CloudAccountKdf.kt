package com.openbitfun.mobile.core.crypto

public data class CloudAccountKdfParams public constructor(
    public val memoryKiB: Int,
    public val iterations: Int,
    public val parallelism: Int,
)

public object CloudAccountKdfPolicy {
    public fun validate(params: CloudAccountKdfParams, salt: ByteArray) {
        require(salt.size in 8..64) { "Invalid Argon2id salt length." }
        require(params.memoryKiB in (8 * 1024)..(256 * 1024)) { "Invalid Argon2id memory cost." }
        require(params.iterations in 1..10) { "Invalid Argon2id iteration count." }
        require(params.parallelism in 1..16) { "Invalid Argon2id parallelism." }
    }
}

public expect object PlatformArgon2id {
    public suspend fun derive(
        password: String,
        salt: ByteArray,
        params: CloudAccountKdfParams,
    ): ByteArray
}
