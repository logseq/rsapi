# set the version to use the library
$NDK_APL_LEVEL = 21

# verify before executing this that you have the proper targets installed
cargo ndk -t aarch64-linux-android --platform $NDK_APL_LEVEL -- build --release -p rsapi-jni -Zbuild-std
cargo ndk -t armv7-linux-androideabi --platform $NDK_APL_LEVEL -- build --release -p rsapi-jni -Zbuild-std
cargo ndk -t i686-linux-android --platform $NDK_APL_LEVEL -- build --release -p rsapi-jni -Zbuild-std
cargo ndk -t x86_64-linux-android --platform $NDK_APL_LEVEL -- build --release -p rsapi-jni -Zbuild-std

$jniLibs = "${pwd}\jniLibs"

$libName = "librsapi.so"

Remove-Item -Recurse -Force ${jniLibs}

mkdir -Force ${jniLibs}
mkdir -Force ${jniLibs}/arm64-v8a
mkdir -Force ${jniLibs}/armeabi-v7a
mkdir -Force ${jniLibs}/x86
mkdir -Force ${jniLibs}/x86_64

Copy-Item target/aarch64-linux-android/release/${libName} ${jniLibs}/arm64-v8a/${libName}
Copy-Item target/armv7-linux-androideabi/release/${libName} ${jniLibs}/armeabi-v7a/${libName}
Copy-Item target/i686-linux-android/release/${libName} ${jniLibs}/x86/${libName}
Copy-Item target/x86_64-linux-android/release/${libName} ${jniLibs}/x86_64/${libName}

# $STRIP = "${env:ANDROID_NDK_HOME}\toolchains\llvm\prebuilt\windows-x86_64\bin\llvm-strip.exe"
# NOTE: strip is handled by cargo build profile
# foreach ($lib in $(Get-ChildItem .\jniLibs\*\*.so)) {
#    Write-Output "Stripping ${lib}"
#    & $STRIP $lib.FullName
# }

tree /F $jniLibs

Get-ChildItem .\jniLibs\*\*.so
