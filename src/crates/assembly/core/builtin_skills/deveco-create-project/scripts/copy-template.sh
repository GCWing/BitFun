#!/bin/sh

# POSIX sh implementation for OHOS agent workers. Do not depend on Node/V8 here:
# Node can abort in the Deveco worker shell context on OpenHarmony.

SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" 2>/dev/null && pwd)

PROJECT_PATH=
APP_NAME=
BUNDLE_NAME=
API_LEVEL=
TEMPLATE_DIR="$SCRIPT_DIR/../application"

die() {
  echo "$1" >&2
  exit 1
}

json_escape() {
  printf "%s" "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

sed_escape_replacement() {
  printf "%s" "$1" | sed 's/[&|\\]/\\&/g'
}

missing_value() {
  die "Missing value for $1"
}

invalid_app_name() {
  RAW_APP_NAME=$(json_escape "$APP_NAME")
  cat >&2 <<EOF
{
  "code": "APP_NAME_INVALID",
  "message": "appName \"$RAW_APP_NAME\" is invalid. It must start with an English letter and contain only [A-Za-z0-9_], length 1-128.",
  "rawAppName": "$RAW_APP_NAME",
  "hint": "Ask the user to choose from 2-3 valid UpperCamelCase ASCII app names, then rerun this script with the selected --app-name."
}
EOF
  exit 4
}

project_exists() {
  TARGET_ESCAPED=$(json_escape "$TARGET_ROOT")
  cat >&2 <<EOF
{
  "code": "PROJECT_EXISTS",
  "message": "Target \"$TARGET_ESCAPED\" already exists and is not empty.",
  "targetRoot": "$TARGET_ESCAPED",
  "hint": "Ask the user whether to overwrite, rename, or cancel. Never overwrite without explicit user confirmation."
}
EOF
  exit 2
}

replace_in_file() {
  FILE=$1
  PATTERN=$2
  VALUE=$3
  TMP_FILE="$FILE.tmp.$$"

  [ -f "$FILE" ] || die "File not found: $FILE"

  ESCAPED_VALUE=$(sed_escape_replacement "$VALUE")
  if sed "s|$PATTERN|$ESCAPED_VALUE|g" "$FILE" > "$TMP_FILE"; then
    mv "$TMP_FILE" "$FILE" || {
      rm -f "$TMP_FILE"
      die "Failed to update file: $FILE"
    }
  else
    rm -f "$TMP_FILE"
    die "Failed to update file: $FILE"
  fi
}

while [ "$#" -gt 0 ]; do
  KEY=$1
  case "$KEY" in
    --project-path|--app-name|--bundle-name|--api-level|--template-dir)
      [ "$#" -ge 2 ] || missing_value "$KEY"
      case "$2" in
        --*) missing_value "$KEY" ;;
      esac
      VALUE=$2
      case "$KEY" in
        --project-path) PROJECT_PATH=$VALUE ;;
        --app-name) APP_NAME=$VALUE ;;
        --bundle-name) BUNDLE_NAME=$VALUE ;;
        --api-level) API_LEVEL=$VALUE ;;
        --template-dir) TEMPLATE_DIR=$VALUE ;;
      esac
      shift 2
      ;;
    --*)
      [ "$#" -ge 2 ] || missing_value "$KEY"
      case "$2" in
        --*) missing_value "$KEY" ;;
      esac
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

[ -n "$PROJECT_PATH" ] || die "Missing required argument --project-path"
[ -n "$APP_NAME" ] || die "Missing required argument --app-name"

APP_NAME_LEN=$(printf "%s" "$APP_NAME" | wc -c | tr -d " ")
case "$APP_NAME" in
  [A-Za-z]*)
    case "$APP_NAME" in
      *[!A-Za-z0-9_]*) invalid_app_name ;;
    esac
    ;;
  *)
    invalid_app_name
    ;;
esac
[ "$APP_NAME_LEN" -le 128 ] || invalid_app_name

if [ -z "$BUNDLE_NAME" ]; then
  APP_NAME_LOWER=$(printf "%s" "$APP_NAME" | tr "ABCDEFGHIJKLMNOPQRSTUVWXYZ" "abcdefghijklmnopqrstuvwxyz")
  BUNDLE_NAME="com.example.$APP_NAME_LOWER"
fi

SOURCE="fallback"
if [ -n "$API_LEVEL" ]; then
  SOURCE="user_input"
else
  API_LEVEL=22
fi

case "$API_LEVEL" in
  17) SDK_VERSION="5.0.5(17)"; MODEL_VERSION="5.0.5" ;;
  18) SDK_VERSION="5.0.6(18)"; MODEL_VERSION="5.0.6" ;;
  19) SDK_VERSION="5.0.7(19)"; MODEL_VERSION="5.0.7" ;;
  20) SDK_VERSION="6.0.0(20)"; MODEL_VERSION="6.0.0" ;;
  21) SDK_VERSION="6.0.1(21)"; MODEL_VERSION="6.0.1" ;;
  22) SDK_VERSION="6.0.2(22)"; MODEL_VERSION="6.0.2" ;;
  23) SDK_VERSION="6.1.0(23)"; MODEL_VERSION="6.1.0" ;;
  24) SDK_VERSION="6.1.1(24)"; MODEL_VERSION="6.1.1" ;;
  *) die "Unsupported apiLevel: $API_LEVEL" ;;
esac

[ -d "$TEMPLATE_DIR" ] || die "Template directory not found: $TEMPLATE_DIR"
TEMPLATE_DIR=$(CDPATH= cd "$TEMPLATE_DIR" 2>/dev/null && pwd) || die "Template directory not found: $TEMPLATE_DIR"

mkdir -p "$PROJECT_PATH" || die "Failed to create project path: $PROJECT_PATH"
PROJECT_PATH=$(CDPATH= cd "$PROJECT_PATH" 2>/dev/null && pwd) || die "Failed to resolve project path: $PROJECT_PATH"
TARGET_ROOT="$PROJECT_PATH/$APP_NAME"

if [ -d "$TARGET_ROOT" ] && [ -n "$(ls -A "$TARGET_ROOT" 2>/dev/null)" ]; then
  project_exists
fi

mkdir -p "$TARGET_ROOT" || die "Failed to create target directory: $TARGET_ROOT"
cp -R "$TEMPLATE_DIR/." "$TARGET_ROOT/" || die "Failed to copy template files"

replace_in_file "$TARGET_ROOT/AppScope/resources/base/element/string.json" "MyApplication" "$APP_NAME"
replace_in_file "$TARGET_ROOT/AppScope/app.json5" "com\.example\.myapplication" "$BUNDLE_NAME"

if [ "$API_LEVEL" != "22" ]; then
  replace_in_file "$TARGET_ROOT/build-profile.json5" "6\.0\.2(22)" "$SDK_VERSION"
  replace_in_file "$TARGET_ROOT/hvigor/hvigor-config.json5" "6\.0\.2" "$MODEL_VERSION"
  replace_in_file "$TARGET_ROOT/oh-package.json5" "6\.0\.2" "$MODEL_VERSION"
fi

MISSING_FILES=
MISSING_SEPARATOR=
for REQUIRED_FILE in \
  "build-profile.json5" \
  "AppScope/resources/base/media/layered_image.json" \
  "AppScope/resources/base/media/background.png" \
  "AppScope/resources/base/media/foreground.png" \
  "entry/src/main/resources/base/media/layered_image.json" \
  "entry/src/main/resources/base/media/background.png" \
  "entry/src/main/resources/base/media/foreground.png"
do
  if [ ! -e "$TARGET_ROOT/$REQUIRED_FILE" ]; then
    MISSING_FILES="$MISSING_FILES$MISSING_SEPARATOR$REQUIRED_FILE"
    MISSING_SEPARATOR=", "
  fi
done

if [ -n "$MISSING_FILES" ]; then
  die "Template copy incomplete. Missing files: $MISSING_FILES"
fi

PROJECT_ROOT_ESCAPED=$(json_escape "$TARGET_ROOT")
APP_NAME_ESCAPED=$(json_escape "$APP_NAME")
BUNDLE_NAME_ESCAPED=$(json_escape "$BUNDLE_NAME")

cat <<EOF
{
  "projectRoot": "$PROJECT_ROOT_ESCAPED",
  "appName": "$APP_NAME_ESCAPED",
  "bundleName": "$BUNDLE_NAME_ESCAPED",
  "apiLevel": $API_LEVEL,
  "source": "$SOURCE",
  "verified": true
}
EOF
