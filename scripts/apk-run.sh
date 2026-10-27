#!/usr/bin/env bash
# pwd: should be executed at workspace root

set -e

cd ln_drawer/build/android/
./gradlew :app:installDebug
adb shell am start -W -a android.intent.action.MAIN -n org.linn.lndrawer.dev/org.linn.lndrawer.MainActivity
uid=$(adb shell pm list package -U org.linn.lndrawer.dev | sed -n 's/.*uid:\([0-9]*\).*/\1/p' | tr -d '\r')
adb logcat -v color --uid=$uid *:D