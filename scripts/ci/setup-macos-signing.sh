#!/usr/bin/env bash
set -euo pipefail

required=(
  APPLE_CERTIFICATE
  APPLE_CERTIFICATE_PASSWORD
  APPLE_SIGNING_IDENTITY
  APPLE_API_ISSUER
  APPLE_API_KEY
  APPLE_API_PRIVATE_KEY
  KEYCHAIN_PASSWORD
)

missing=()
for name in "${required[@]}"; do
  if [[ -z "${!name:-}" ]]; then
    missing+=("${name}")
  fi
done

if [[ "${#missing[@]}" -gt 0 ]]; then
  if [[ "${OPENBITFUN_REQUIRE_APPLE_SIGNING:-false}" == "true" ]]; then
    printf 'Apple signing is required, but these secrets are missing: %s\n' "${missing[*]}" >&2
    exit 1
  fi
  printf 'Apple signing is not fully configured; leaving this non-release macOS build unsigned. Missing: %s\n' "${missing[*]}"
  exit 0
fi

signing_dir="${RUNNER_TEMP}/openbitfun-apple-signing"
certificate_path="${signing_dir}/developer-id.p12"
api_key_path="${signing_dir}/AuthKey_${APPLE_API_KEY}.p8"
keychain_path="${RUNNER_TEMP}/openbitfun-signing.keychain-db"

mkdir -p "${signing_dir}"
chmod 700 "${signing_dir}"
printf '%s' "${APPLE_CERTIFICATE}" | base64 --decode >"${certificate_path}"
printf '%s' "${APPLE_API_PRIVATE_KEY}" >"${api_key_path}"
chmod 600 "${certificate_path}" "${api_key_path}"

security create-keychain -p "${KEYCHAIN_PASSWORD}" "${keychain_path}"
security set-keychain-settings -lut 21600 "${keychain_path}"
security unlock-keychain -p "${KEYCHAIN_PASSWORD}" "${keychain_path}"
security import "${certificate_path}" \
  -k "${keychain_path}" \
  -P "${APPLE_CERTIFICATE_PASSWORD}" \
  -T /usr/bin/codesign \
  -T /usr/bin/security
security set-key-partition-list \
  -S apple-tool:,apple:,codesign: \
  -s \
  -k "${KEYCHAIN_PASSWORD}" \
  "${keychain_path}"

curl --fail --location --silent --show-error \
  'https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer' \
  --output "${signing_dir}/DeveloperIDG2CA.cer"
security import "${signing_dir}/DeveloperIDG2CA.cer" -k "${keychain_path}"

security list-keychains -d user -s "${keychain_path}" "${HOME}/Library/Keychains/login.keychain-db"
security default-keychain -d user -s "${keychain_path}"
security find-identity -v -p codesigning "${keychain_path}"

if ! security find-identity -v -p codesigning "${keychain_path}" | grep -Fq "${APPLE_SIGNING_IDENTITY}"; then
  echo "The imported certificate does not provide ${APPLE_SIGNING_IDENTITY}." >&2
  exit 1
fi

{
  echo "APPLE_API_ISSUER=${APPLE_API_ISSUER}"
  echo "APPLE_API_KEY=${APPLE_API_KEY}"
  echo "APPLE_API_KEY_PATH=${api_key_path}"
  echo "APPLE_SIGNING_IDENTITY=${APPLE_SIGNING_IDENTITY}"
  echo "OPENBITFUN_APPLE_SIGNING_CONFIGURED=true"
} >>"${GITHUB_ENV}"

echo "Apple Developer ID signing and notarization credentials are ready."
