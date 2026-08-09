#include <cstring>
#include "napi/native_api.h"
#include "argon2.h"

static bool readBytes(napi_env env, napi_value value, const uint8_t **data, size_t *length) {
    bool isTypedArray = false;
    if (napi_is_typedarray(env, value, &isTypedArray) != napi_ok || !isTypedArray) return false;
    napi_typedarray_type type;
    size_t count = 0;
    void *raw = nullptr;
    napi_value arrayBuffer;
    size_t offset = 0;
    if (napi_get_typedarray_info(env, value, &type, &count, &raw, &arrayBuffer, &offset) != napi_ok ||
        type != napi_uint8_array || raw == nullptr) return false;
    *data = static_cast<const uint8_t *>(raw);
    *length = count;
    return true;
}

static napi_value Argon2idRaw(napi_env env, napi_callback_info info) {
    size_t argc = 5;
    napi_value argv[5] = {nullptr};
    if (napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr) != napi_ok || argc != 5) return nullptr;
    const uint8_t *password = nullptr;
    const uint8_t *salt = nullptr;
    size_t passwordLength = 0;
    size_t saltLength = 0;
    if (!readBytes(env, argv[0], &password, &passwordLength) || !readBytes(env, argv[1], &salt, &saltLength)) return nullptr;
    uint32_t memory = 0;
    uint32_t time = 0;
    uint32_t lanes = 0;
    if (napi_get_value_uint32(env, argv[2], &memory) != napi_ok ||
        napi_get_value_uint32(env, argv[3], &time) != napi_ok ||
        napi_get_value_uint32(env, argv[4], &lanes) != napi_ok) return nullptr;
    uint8_t output[32] = {0};
    const int result = argon2id_hash_raw(time, memory, lanes, password, passwordLength, salt, saltLength, output, sizeof(output));
    if (result != ARGON2_OK) return nullptr;
    napi_value resultBuffer;
    void *resultData = nullptr;
    if (napi_create_arraybuffer(env, sizeof(output), &resultData, &resultBuffer) != napi_ok) return nullptr;
    napi_value resultArray;
    if (napi_create_typedarray(env, napi_uint8_array, sizeof(output), resultBuffer, 0, &resultArray) != napi_ok) return nullptr;
    std::memcpy(resultData, output, sizeof(output));
    return resultArray;
}

static napi_value Init(napi_env env, napi_value exports) {
    napi_property_descriptor descriptor = {"argon2idRaw", nullptr, Argon2idRaw, nullptr, nullptr, nullptr, napi_default, nullptr};
    napi_define_properties(env, exports, 1, &descriptor);
    return exports;
}

EXTERN_C_START
static napi_module module = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = nullptr,
    .nm_register_func = Init,
    .nm_modname = "bitfun_crypto",
    .nm_priv = nullptr,
    .reserved = {0},
};
EXTERN_C_END

extern "C" __attribute__((constructor)) void RegisterBitfunCrypto(void) {
    napi_module_register(&module);
}
