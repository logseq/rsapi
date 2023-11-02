#!/usr/bin/env bash

set -ex

# set the version to use the library
NDK_APL_LEVEL=21

cargo ndk -t aarch64-linux-android --platform ${NDK_APL_LEVEL} -- build --profile=release-jni -p rsapi-jni -Zbuild-std
cargo ndk -t armv7-linux-androideabi --platform ${NDK_APL_LEVEL} -- build --profile=release-jni -p rsapi-jni -Zbuild-std
cargo ndk -t i686-linux-android --platform ${NDK_APL_LEVEL} -- build --profile=release-jni -p rsapi-jni -Zbuild-std
cargo ndk -t x86_64-linux-android --platform ${NDK_APL_LEVEL} -- build --profile=release-jni -p rsapi-jni -Zbuild-std

jniLibs=`pwd`/jniLibs

libName=librsapi.so

rm -rf ${jniLibs}

mkdir -p ${jniLibs}
mkdir -p ${jniLibs}/arm64-v8a
mkdir -p ${jniLibs}/armeabi-v7a
mkdir -p ${jniLibs}/x86
mkdir -p ${jniLibs}/x86_64

cp target/aarch64-linux-android/release-jni/${libName} ${jniLibs}/arm64-v8a/${libName}
cp target/armv7-linux-androideabi/release-jni/${libName} ${jniLibs}/armeabi-v7a/${libName}
cp target/i686-linux-android/release-jni/${libName} ${jniLibs}/x86/${libName}
cp target/x86_64-linux-android/release-jni/${libName} ${jniLibs}/x86_64/${libName}

# STRIP=$HOME/Library/Android/sdk/ndk/*/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-strip
