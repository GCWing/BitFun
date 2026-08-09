cmake_minimum_required(VERSION 3.5.0)
project(bitfun_crypto)

add_library(bitfun_crypto SHARED
  napi_argon2.cpp
  argon2/argon2.c
  argon2/core.c
  argon2/encoding.c
  argon2/ref.c
  argon2/run.c
  argon2/thread.c
  argon2/blake2/blake2b.c
)

target_include_directories(bitfun_crypto PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/argon2)
target_compile_definitions(bitfun_crypto PRIVATE A2_VISCTL)
target_link_libraries(bitfun_crypto PUBLIC libace_napi.z.so)
