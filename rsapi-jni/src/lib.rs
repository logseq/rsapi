#![feature(read_buf)]
#![feature(result_flattening)]
#![allow(non_snake_case)]

use std::path::PathBuf;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jbyteArray, jint, jlong, jobjectArray, jstring, JNI_VERSION_1_6};
use jni::{JNIEnv, JavaVM};

use rsapi_impl as implementation;
pub use rsapi_impl::{FileMeta, Progress};

use crate::error::Error;

pub mod error;

pub type Result<T> = ::std::result::Result<T, Error>;

/// Used for error handling
static mut LAST_ERROR: Option<Error> = None;

static mut VM: Option<JavaVM> = Option::None;

static mut RUNNER: Option<tokio::runtime::Runtime> = Option::None;

fn runtime() -> &'static tokio::runtime::Runtime {
    unsafe {
        RUNNER
            .as_ref()
            .expect("tokio runner is inited in JNI_OnLoad")
    }
}

pub struct AndroidLogger;

impl log::Log for AndroidLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        (metadata.target().starts_with("rsapi") || metadata.target().starts_with("sync"))
            && metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        #[cfg(target_os = "android")]
        if self.enabled(record.metadata()) {
            use android_log_sys::{LogPriority, __android_log_print};

            let priority = match record.level() {
                log::Level::Error => LogPriority::ERROR,
                log::Level::Warn => LogPriority::WARN,
                log::Level::Info => LogPriority::INFO,
                log::Level::Debug => LogPriority::DEBUG,
                log::Level::Trace => LogPriority::VERBOSE,
            };
            // NOTE: Android logcat doesn't support %, so we replace it with _
            let message = format!("{}\0", record.args()).replace("%", "_");
            let tag = b"rsapi-jni\0";
            unsafe {
                __android_log_print(
                    priority as _,
                    tag.as_ptr() as *const _,
                    message.as_ptr() as *const _,
                );
            }
        }
    }

    fn flush(&self) {}
}

#[no_mangle]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut ()) -> jint {
    unsafe {
        VM = Some(vm);
    }

    log::set_logger(&AndroidLogger).expect("JNI_OnLoad is called at most once");
    log::set_max_level(log::LevelFilter::Debug);

    log::debug!("rsapi JNI_OnLoad called");

    std::panic::set_hook(Box::new(|panic_info| {
        let msg = panic_info.payload().downcast_ref::<&str>().unwrap();
        debug_log(format!("PANIC: {}", msg));
        debug_log(panic_info.to_string());
    }));

    // tokio runner
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("tokio runtime");
    unsafe {
        RUNNER = Some(rt);
    }

    // Ref: https://developer.android.com/training/articles/perf-jni#faq_FindClass
    let env: JNIEnv = unsafe { VM.as_ref().unwrap().get_env().unwrap() };
    let inst = {
        env.call_static_method(
            "com/logseq/app/filesync/FileSyncPlugin",
            "getInstance",
            "()Lcom/logseq/app/filesync/FileSyncPlugin;",
            &[],
        )
        .unwrap()
        .l()
        .unwrap()
    };
    let inst = env.new_global_ref(inst).unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<Progress>();
    std::thread::spawn(move || {
        let env = unsafe { VM.as_ref().unwrap() }
            .attach_current_thread()
            .expect("VM cannot attach to current thread");

        while let Ok(info) = rx.recv() {
            let _ret = env
                .call_method(
                    &inst,
                    "progressNotify",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;JJ)V",
                    &[
                        JValue::Object(env.new_string(&info.graph_uuid).unwrap().into()),
                        JValue::Object(env.new_string(&info.file).unwrap().into()),
                        JValue::Object(env.new_string(&info.r#type).unwrap().into()),
                        JValue::Long(info.progress),
                        JValue::Long(info.total),
                    ],
                )
                .expect("progressNotify");
        }
    });

    let progress_callback = move |info: Progress| {
        tx.send(info).unwrap();
    };

    implementation::set_progress_callback(progress_callback);

    JNI_VERSION_1_6
}

// MARK: Export JNI functions
#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_getLastError(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    unsafe {
        if let Some(err) = &LAST_ERROR {
            let ret = env.new_string(err.to_string()).unwrap().into_raw();
            LAST_ERROR = None;
            ret
        } else {
            JObject::null().into_raw()
        }
    }
}

// Public rsapi API Part
#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_cancelAllRequests(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    debug_log("cancel all requests");
    match implementation::cancel_all_requests() {
        Ok(()) => 0,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            -1
        }
    }
}

/// String[secret, public], won't fail
#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_keygen(
    env: JNIEnv,
    _class: JClass,
) -> jobjectArray {
    let array = env
        .new_object_array(2, "java/lang/String", JObject::null())
        .unwrap();

    let (secret, public) = implementation::keygen();

    let _ = env.set_object_array_element(array, 0, env.new_string(secret).unwrap());
    let _ = env.set_object_array_element(array, 1, env.new_string(public).unwrap());

    return array;
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_setEnvironment(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    variant: JString,
    secret_key: JString,
    public_key: JString,
) -> jlong {
    let variant = env.get_string(variant).map(String::from).expect("env");
    let graph_uuid = env
        .get_string(graph_uuid)
        .map(String::from)
        .expect("graph_uuid");
    debug_log(format!("setting env: {} {:?}", graph_uuid, variant));
    let secret_key = env
        .get_string(secret_key)
        .map(String::from)
        .expect("secret key must set");
    let public_key = env
        .get_string(public_key)
        .map(String::from)
        .expect("public key must set");

    match implementation::set_env(&graph_uuid, &variant, &secret_key, &public_key) {
        Ok(()) => 0,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            -1
        }
    }
}

/// Return null when error
#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_encryptFilenames(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    fnames: JObject, // List<String>
) -> jobjectArray {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        fnames: JObject, // List<String>
    ) -> Result<jobjectArray> {
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let fnames = jlist_to_string_vec(env, fnames)?;

        let graph = implementation::get_graph(&graph_uuid)?;
        let array =
            env.new_object_array(fnames.len() as i32, "java/lang/String", JObject::null())?;
        for (i, fname) in fnames.iter().enumerate() {
            let encrypted = graph.encrypt_filename(fname)?;
            env.set_object_array_element(array, i as i32, env.new_string(encrypted)?)?;
        }

        Ok(array)
    }

    match inner(env, graph_uuid, fnames) {
        Ok(array) => array,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            JObject::null().into_raw()
        }
    }
}

/// Return null when error
#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_decryptFilenames(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    encrypted_fnames: JObject, // List<String>
) -> jobjectArray {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        fnames: JObject, // List<String>
    ) -> Result<jobjectArray> {
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let fnames = jlist_to_string_vec(env, fnames)?;

        let graph = implementation::get_graph(&graph_uuid)?;
        let array =
            env.new_object_array(fnames.len() as i32, "java/lang/String", JObject::null())?;
        for (i, fname) in fnames.iter().enumerate() {
            let encrypted = graph.decrypt_filename(fname)?;
            env.set_object_array_element(array, i as i32, env.new_string(encrypted)?)?;
        }

        Ok(array)
    }

    match inner(env, graph_uuid, encrypted_fnames) {
        Ok(array) => array,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            JObject::null().into_raw()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_getLocalFilesMeta(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
) -> jobjectArray {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        base_path: JString,
        file_paths: JObject,
    ) -> Result<jobjectArray> {
        let base_path = uri_to_full_path(env, base_path)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let mut file_paths = jlist_to_string_vec(env, file_paths)?;

        file_paths.sort();
        file_paths.dedup();
        // NOTE: Assume Android is using a case-sensitive fs.

        let graph = implementation::get_graph(&graph_uuid)?;
        let files_meta = runtime().block_on(graph.get_files_meta(base_path, file_paths))?;
        let nfiles = files_meta.len();
        let array =
            env.new_object_array(nfiles as i32, "com/logseq/sync/FileMeta", JObject::null())?;
        // TODO: use real filename key
        for (i, (_file_name, file_meta)) in files_meta.iter().enumerate() {
            env.set_object_array_element(array, i as i32, to_java_file_meta(env, file_meta)?)?;
        }

        Ok(array)
    }

    match inner(env, graph_uuid, base_path, file_paths) {
        Ok(array) => array,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            JObject::null().into_raw()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_getLocalAllFilesMeta(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
) -> jobjectArray {
    fn inner(env: JNIEnv, graph_uuid: JString, base_path: JString) -> Result<jobjectArray> {
        let base_path = uri_to_full_path(env, base_path)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();

        let graph = implementation::get_graph(&graph_uuid)?;
        let files_meta = runtime()
            .block_on(graph.get_all_files_meta(base_path))?
            .collect::<Vec<_>>();
        let nfiles = files_meta.len();
        let array =
            env.new_object_array(nfiles as i32, "com/logseq/sync/FileMeta", JObject::null())?;
        for (i, file_meta) in files_meta.iter().enumerate() {
            env.set_object_array_element(array, i as i32, to_java_file_meta(env, file_meta)?)?;
        }

        Ok(array)
    }

    match inner(env, graph_uuid, base_path) {
        Ok(array) => array,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            JObject::null().into_raw()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_renameLocalFile(
    env: JNIEnv,
    _class: JClass,
    _graph_uuid: JString,
    base_path: JString,
    old_file_path: JString,
    new_file_path: JString,
) -> jlong {
    use std::fs;

    let base_path = uri_to_full_path(env, base_path).unwrap();
    let from: String = env.get_string(old_file_path).unwrap().into();
    let to: String = env.get_string(new_file_path).unwrap().into();

    match fs::rename(base_path.join(&from), base_path.join(&to)) {
        Ok(()) => 0,
        Err(err) => {
            debug_log(format!("cannot rename: {:?}", err));
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_deleteLocalFiles(
    env: JNIEnv,
    _class: JClass,
    _graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
) {
    use std::fs;

    let base_path = uri_to_full_path(env, base_path).unwrap();
    let file_paths = jlist_to_string_vec(env, file_paths).unwrap();
    for file_path in file_paths {
        let full_path = base_path.join(file_path);
        debug_log(format!("delete file {:?}", full_path));
        let _ = fs::remove_file(full_path); // ignore any errors
    }
}

// returns String[]
#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_fetchRemoteFiles(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
    token: JString,
) -> jobjectArray {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        base_path: JString,
        file_paths: JObject, // List<String>
        token: JString,
    ) -> Result<jobjectArray> {
        let base_path = uri_to_full_path(env, base_path)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let token: String = env.get_string(token)?.into();
        let file_paths = jlist_to_string_vec(env, file_paths)?;

        let graph = implementation::get_graph(&graph_uuid)?;

        let files_to_be_merged =
            runtime().block_on(graph.fetch_remote_files(base_path, file_paths, &token))?;
        let array = env.new_object_array(
            files_to_be_merged.len() as i32,
            "java/lang/String",
            JObject::null(),
        )?;
        for (i, fname) in files_to_be_merged.iter().enumerate() {
            env.set_object_array_element(array, i as i32, env.new_string(fname)?)?;
        }
        Ok(array)
    }

    match inner(env, graph_uuid, base_path, file_paths, token) {
        Ok(array) => array,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            JObject::null().into_raw()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_updateLocalFiles(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
    token: JString,
) -> jlong {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        base_path: JString,
        file_paths: JObject, // List<String>
        token: JString,
    ) -> Result<()> {
        let base_path = uri_to_full_path(env, base_path)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let token: String = env.get_string(token)?.into();
        let file_paths = jlist_to_string_vec(env, file_paths)?;

        let graph = implementation::get_graph(&graph_uuid)?;

        runtime().block_on(graph.update_local_files(base_path, file_paths, &token))?;

        Ok(())
    }

    match inner(env, graph_uuid, base_path, file_paths, token) {
        Ok(_) => 0,
        Err(err) => {
            unsafe {
                LAST_ERROR = Some(err.into());
            }
            -1
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_updateLocalVersionFiles(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
    token: JString,
) -> jlong {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        base_path: JString,
        file_paths: JObject,
        token: JString,
    ) -> Result<()> {
        let base_path = uri_to_full_path(env, base_path)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let token: String = env.get_string(token)?.into();
        let file_paths = jlist_to_string_vec(env, file_paths)?;

        let graph = implementation::get_graph(&graph_uuid)?;

        runtime().block_on(graph.update_local_version_files(base_path, file_paths, &token))?;

        Ok(())
    }

    match inner(env, graph_uuid, base_path, file_paths, token) {
        Ok(_) => 0,
        Err(err) => {
            unsafe { LAST_ERROR = Some(err) };
            return -1;
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_deleteRemoteFiles(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
    token: JString,
    txid: jlong,
) -> jlong {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        base_path: JString,
        file_paths: JObject, // List<String>
        token: JString,
        txid: jlong,
    ) -> Result<i64> {
        let base_path = uri_to_full_path(env, base_path)?;
        let file_paths = jlist_to_string_vec(env, file_paths)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let token: String = env.get_string(token)?.into();

        let graph = implementation::get_graph(&graph_uuid)?;

        let txid =
            runtime().block_on(graph.delete_remote_files(base_path, file_paths, txid, &token))?;

        Ok(txid)
    }

    match inner(env, graph_uuid, base_path, file_paths, token, txid) {
        Ok(txid) => txid,
        Err(err) => {
            unsafe { LAST_ERROR = Some(err) };
            return -1;
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_updateRemoteFiles(
    env: JNIEnv,
    _class: JClass,
    graph_uuid: JString,
    base_path: JString,
    file_paths: JObject, // List<String>
    token: JString,
    txid: jlong,
) -> jlong {
    fn inner(
        env: JNIEnv,
        graph_uuid: JString,
        base_path: JString,
        file_paths: JObject, // List<String>
        token: JString,
        txid: jlong,
    ) -> Result<i64> {
        let base_path = uri_to_full_path(env, base_path)?;
        let graph_uuid: String = env.get_string(graph_uuid)?.into();
        let token: String = env.get_string(token)?.into();
        let file_paths = jlist_to_string_vec(env, file_paths)?;

        let graph = implementation::get_graph(&graph_uuid)?;

        let txid = runtime()
            .block_on(graph.update_remote_files(base_path, file_paths, txid, &token, None))?;
        Ok(txid)
    }

    match inner(env, graph_uuid, base_path, file_paths, token, txid) {
        Ok(txid) => txid,
        Err(err) => {
            unsafe { LAST_ERROR = Some(err) };
            return -1;
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_ageEncryptWithPassphrase(
    env: JNIEnv,
    _class: JClass,
    passphrase: JString,
    buf: jbyteArray,
) -> jbyteArray {
    let pass: String = env.get_string(passphrase).unwrap().into();
    let buf = env.convert_byte_array(buf).unwrap();

    match lsq_encryption::encrypt_with_user_passphrase(&pass, &buf, true) {
        Ok(ret) => env.byte_array_from_slice(&ret).unwrap(),
        Err(e) => {
            unsafe { LAST_ERROR = Some(e.into()) };
            JObject::null().into_raw()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_logseq_sync_RSFileSync_ageDecryptWithPassphrase(
    env: JNIEnv,
    _class: JClass,
    passphrase: JString,
    buf: jbyteArray,
) -> jbyteArray {
    let pass: String = env.get_string(passphrase).unwrap().into();
    let buf = env.convert_byte_array(buf).unwrap();

    match lsq_encryption::decrypt_with_user_passphrase(&pass, &buf) {
        Ok(ret) => env.byte_array_from_slice(&ret).unwrap(),
        Err(e) => {
            unsafe { LAST_ERROR = Some(e.into()) };
            JObject::null().into_raw()
        }
    }
}

// MARK: Debug logging
#[cfg(not(target_os = "android"))]
pub fn debug_log<T: AsRef<str>>(message: T) {
    println!("{}", message.as_ref());
}

#[cfg(target_os = "android")]
pub fn debug_log<T: AsRef<str>>(message: T) {
    use android_log_sys::{LogPriority, __android_log_print};
    use std::ffi::CString;

    match CString::new(message.as_ref()) {
        Ok(message) => {
            let priority = LogPriority::DEBUG;
            let tag = "rsapi-jni\0";

            unsafe {
                __android_log_print(priority as _, tag.as_ptr() as *const _, message.as_ptr());
            }
        }
        Err(_) => {
            panic!("cannot convert log message");
        }
    }
}

// MARK: misc helpers
/// convert java:List<String> to rust:Vec<String>
fn jlist_to_string_vec(env: JNIEnv, list: JObject) -> jni::errors::Result<Vec<String>> {
    let list = env.get_list(list)?;

    list.iter()?
        .map(|pat| env.get_string(pat.into()).map(String::from))
        .collect()
}

/// convert Uri to path
fn uri_to_full_path<'a>(env: JNIEnv<'a>, path: JString) -> jni::errors::Result<PathBuf> {
    let uri_class = env.find_class("android/net/Uri").unwrap();
    let uri = env
        .call_static_method(
            uri_class,
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[JValue::Object(path.into())],
        )?
        .l()?;
    let path: JString = env
        .call_method(uri, "getPath", "()Ljava/lang/String;", &[])?
        .l()?
        .into();
    let path: String = env.get_string(path)?.into();
    Ok(PathBuf::from(path))
}

fn to_java_file_meta<'a>(env: JNIEnv<'a>, metadata: &FileMeta) -> Result<JObject<'a>> {
    // construct com.logseq.sync.FileMeta
    let class = env.find_class("com/logseq/sync/FileMeta").unwrap();
    let obj = env.new_object(
        class,
        "(Ljava/lang/String;JJJLjava/lang/String;)V",
        &[
            JValue::Object(env.new_string(&metadata.fname)?.into()),
            JValue::Long(metadata.size),
            JValue::Long(metadata.mtime),
            JValue::Long(metadata.ctime),
            JValue::Object(env.new_string(&metadata.md5)?.into()),
        ],
    )?;
    env.set_field(
        obj,
        "encryptedFilename",
        "Ljava/lang/String;",
        JValue::Object(env.new_string(&metadata.encrypted_fname).unwrap().into()),
    )?;
    env.set_field(
        obj,
        "incomingFilename",
        "Ljava/lang/String;",
        JValue::Object(env.new_string(&metadata.incoming_fname).unwrap().into()),
    )?;

    Ok(obj)
}
