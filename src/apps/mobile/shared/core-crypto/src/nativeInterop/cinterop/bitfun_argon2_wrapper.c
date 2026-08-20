#include <stddef.h>
#include <stdint.h>
#include "argon2.h"

int bitfun_argon2id_raw(
    const uint8_t *password,
    size_t password_length,
    const uint8_t *salt,
    size_t salt_length,
    uint32_t memory_kib,
    uint32_t iterations,
    uint32_t parallelism,
    uint8_t *output,
    size_t output_length) {
    return argon2id_hash_raw(
        iterations,
        memory_kib,
        parallelism,
        password,
        password_length,
        salt,
        salt_length,
        output,
        output_length);
}
