const { existsSync, readFileSync } = require('fs')
const { join } = require('path')

const { platform, arch } = process

let nativeBinding = null
let localFileExisted = false
let loadError = null

function isMusl() {
  // For Node 10
  if (!process.report || typeof process.report.getReport !== 'function') {
    try {
      return readFileSync('/usr/bin/ldd', 'utf8').includes('musl')
    } catch (e) {
      return false
    }
  } else {
    const { glibcVersionRuntime } = process.report.getReport().header
    return !Boolean(glibcVersionRuntime)
  }
}

switch (platform) {
  case 'android':
    if (arch !== 'arm64') {
      throw new Error(`Unsupported architecture on Android ${arch}`)
    }
    localFileExisted = existsSync(join(__dirname, 'rsapi.android-arm64.node'))
    try {
      if (localFileExisted) {
        nativeBinding = require('./rsapi.android-arm64.node')
      } else {
        nativeBinding = require('@logseq/rsapi-android-arm64')
      }
    } catch (e) {
      loadError = e
    }
    break
  case 'win32':
    switch (arch) {
      case 'x64':
        localFileExisted = existsSync(
          join(__dirname, 'rsapi.win32-x64-msvc.node')
        )
        try {
          if (localFileExisted) {
            nativeBinding = require('./rsapi.win32-x64-msvc.node')
          } else {
            nativeBinding = require('@logseq/rsapi-win32-x64-msvc')
          }
        } catch (e) {
          loadError = e
        }
        break
      case 'ia32':
        localFileExisted = existsSync(
          join(__dirname, 'rsapi.win32-ia32-msvc.node')
        )
        try {
          if (localFileExisted) {
            nativeBinding = require('./rsapi.win32-ia32-msvc.node')
          } else {
            nativeBinding = require('@logseq/rsapi-win32-ia32-msvc')
          }
        } catch (e) {
          loadError = e
        }
        break
      case 'arm64':
        localFileExisted = existsSync(
          join(__dirname, 'rsapi.win32-arm64-msvc.node')
        )
        try {
          if (localFileExisted) {
            nativeBinding = require('./rsapi.win32-arm64-msvc.node')
          } else {
            nativeBinding = require('@logseq/rsapi-win32-arm64-msvc')
          }
        } catch (e) {
          loadError = e
        }
        break
      default:
        throw new Error(`Unsupported architecture on Windows: ${arch}`)
    }
    break
  case 'darwin':
    switch (arch) {
      case 'x64':
        localFileExisted = existsSync(join(__dirname, 'rsapi.darwin-x64.node'))
        try {
          if (localFileExisted) {
            nativeBinding = require('./rsapi.darwin-x64.node')
          } else {
            nativeBinding = require('@logseq/rsapi-darwin-x64')
          }
        } catch (e) {
          loadError = e
        }
        break
      case 'arm64':
        localFileExisted = existsSync(
          join(__dirname, 'rsapi.darwin-arm64.node')
        )
        try {
          if (localFileExisted) {
            nativeBinding = require('./rsapi.darwin-arm64.node')
          } else {
            nativeBinding = require('@logseq/rsapi-darwin-arm64')
          }
        } catch (e) {
          loadError = e
        }
        break
      default:
        throw new Error(`Unsupported architecture on macOS: ${arch}`)
    }
    break
  case 'freebsd':
    if (arch !== 'x64') {
      throw new Error(`Unsupported architecture on FreeBSD: ${arch}`)
    }
    localFileExisted = existsSync(join(__dirname, 'rsapi.freebsd-x64.node'))
    try {
      if (localFileExisted) {
        nativeBinding = require('./rsapi.freebsd-x64.node')
      } else {
        nativeBinding = require('@logseq/rsapi-freebsd-x64')
      }
    } catch (e) {
      loadError = e
    }
    break
  case 'linux':
    switch (arch) {
      case 'x64':
        if (isMusl()) {
          localFileExisted = existsSync(
            join(__dirname, 'rsapi.linux-x64-musl.node')
          )
          try {
            if (localFileExisted) {
              nativeBinding = require('./rsapi.linux-x64-musl.node')
            } else {
              nativeBinding = require('@logseq/rsapi-linux-x64-musl')
            }
          } catch (e) {
            loadError = e
          }
        } else {
          localFileExisted = existsSync(
            join(__dirname, 'rsapi.linux-x64-gnu.node')
          )
          try {
            if (localFileExisted) {
              nativeBinding = require('./rsapi.linux-x64-gnu.node')
            } else {
              nativeBinding = require('@logseq/rsapi-linux-x64-gnu')
            }
          } catch (e) {
            loadError = e
          }
        }
        break
      case 'arm64':
        if (isMusl()) {
          localFileExisted = existsSync(
            join(__dirname, 'rsapi.linux-arm64-musl.node')
          )
          try {
            if (localFileExisted) {
              nativeBinding = require('./rsapi.linux-arm64-musl.node')
            } else {
              nativeBinding = require('@logseq/rsapi-linux-arm64-musl')
            }
          } catch (e) {
            loadError = e
          }
        } else {
          localFileExisted = existsSync(
            join(__dirname, 'rsapi.linux-arm64-gnu.node')
          )
          try {
            if (localFileExisted) {
              nativeBinding = require('./rsapi.linux-arm64-gnu.node')
            } else {
              nativeBinding = require('@logseq/rsapi-linux-arm64-gnu')
            }
          } catch (e) {
            loadError = e
          }
        }
        break
      case 'arm':
        localFileExisted = existsSync(
          join(__dirname, 'rsapi.linux-arm-gnueabihf.node')
        )
        try {
          if (localFileExisted) {
            nativeBinding = require('./rsapi.linux-arm-gnueabihf.node')
          } else {
            nativeBinding = require('@logseq/rsapi-linux-arm-gnueabihf')
          }
        } catch (e) {
          loadError = e
        }
        break
      default:
        throw new Error(`Unsupported architecture on Linux: ${arch}`)
    }
    break
  default:
    throw new Error(`Unsupported OS: ${platform}, architecture: ${arch}`)
}

if (!nativeBinding) {
  if (loadError) {
    throw loadError
  }
  throw new Error(`Failed to load native binding`)
}

const { initLogger, keygen, setEnv, setProxy, setProgressCallback, cancelAllRequests, getLocalFilesMeta, getLocalAllFilesMeta, renameLocalFile, deleteLocalFiles, fetchRemoteFiles, updateLocalFiles, updateLocalVersionFiles, updateRemoteFiles, deleteRemoteFiles, ageEncryptWithPassphrase, ageDecryptWithPassphrase, encryptFnames, decryptFnames, canonicalizePath } = nativeBinding

module.exports.initLogger = initLogger
module.exports.keygen = keygen
module.exports.setEnv = setEnv
module.exports.setProxy = setProxy
module.exports.setProgressCallback = setProgressCallback
module.exports.cancelAllRequests = cancelAllRequests
module.exports.getLocalFilesMeta = getLocalFilesMeta
module.exports.getLocalAllFilesMeta = getLocalAllFilesMeta
module.exports.renameLocalFile = renameLocalFile
module.exports.deleteLocalFiles = deleteLocalFiles
module.exports.fetchRemoteFiles = fetchRemoteFiles
module.exports.updateLocalFiles = updateLocalFiles
module.exports.updateLocalVersionFiles = updateLocalVersionFiles
module.exports.updateRemoteFiles = updateRemoteFiles
module.exports.deleteRemoteFiles = deleteRemoteFiles
module.exports.ageEncryptWithPassphrase = ageEncryptWithPassphrase
module.exports.ageDecryptWithPassphrase = ageDecryptWithPassphrase
module.exports.encryptFnames = encryptFnames
module.exports.decryptFnames = decryptFnames
module.exports.canonicalizePath = canonicalizePath
