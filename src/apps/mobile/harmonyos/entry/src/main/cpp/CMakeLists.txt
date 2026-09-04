cmake_minimum_required(VERSION 3.5.0)
project(openbitfun_crypto)

# Argon2 is a shared, platform-neutral source set. HarmonyOS and the KMP iOS
# actual both build this same implementation; keep the algorithm out of either
# platform directory so the two copies cannot drift.
set(OPENBITFUN_ARGON2_DIR ${CMAKE_CURRENT_SOURCE_DIR}/../../../../../../../shared/argon2)

add_library(openbitfun_crypto SHARED
  napi_argon2.cpp
  ${OPENBITFUN_ARGON2_DIR}/argon2.c
  ${OPENBITFUN_ARGON2_DIR}/core.c
  ${OPENBITFUN_ARGON2_DIR}/encoding.c
  ${OPENBITFUN_ARGON2_DIR}/ref.c
  ${OPENBITFUN_ARGON2_DIR}/run.c
  ${OPENBITFUN_ARGON2_DIR}/thread.c
  ${OPENBITFUN_ARGON2_DIR}/blake2/blake2b.c
)

target_include_directories(openbitfun_crypto PRIVATE ${OPENBITFUN_ARGON2_DIR})
target_compile_definitions(openbitfun_crypto PRIVATE A2_VISCTL)
target_link_libraries(openbitfun_crypto PUBLIC libace_napi.z.so)
