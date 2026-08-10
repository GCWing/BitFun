cmake_minimum_required(VERSION 3.5.0)
project(bitfun_crypto)

# Argon2 is a shared, platform-neutral source set. HarmonyOS and the KMP iOS
# actual both build this same implementation; keep the algorithm out of either
# platform directory so the two copies cannot drift.
set(BITFUN_ARGON2_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../../../../../../../shared/argon2)

add_library(bitfun_crypto SHARED
  napi_argon2.cpp
  ${BITFUN_ARGON2_DIR}/argon2.c
  ${BITFUN_ARGON2_DIR}/core.c
  ${BITFUN_ARGON2_DIR}/encoding.c
  ${BITFUN_ARGON2_DIR}/ref.c
  ${BITFUN_ARGON2_DIR}/run.c
  ${BITFUN_ARGON2_DIR}/thread.c
  ${BITFUN_ARGON2_DIR}/blake2/blake2b.c
)

target_include_directories(bitfun_crypto PRIVATE ${BITFUN_ARGON2_DIR})
target_compile_definitions(bitfun_crypto PRIVATE A2_VISCTL)
target_link_libraries(bitfun_crypto PUBLIC libace_napi.z.so)
