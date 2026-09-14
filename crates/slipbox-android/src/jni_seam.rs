//! The JNI entry points of the packaged library.
//!
//! Paths, requests and answers cross the boundary as UTF-8 `byte[]`, not
//! `String`, so no value passes through Java's modified UTF-8 encoding. Every
//! entry point contains Rust panics and bounds its argument before allocating
//! for it. A null return means the answer did not reach Java: either an
//! exception was already pending, or allocating or filling the answer raised
//! one. Nothing but `ExceptionCheck` is asked of JNI while an exception is
//! pending.
//!
//! The exported set is closed: `jni-exports.map` lists it, the Kotlin seam
//! declares it, and `tests/native_packaging.rs` holds the two together.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;

use jni_sys::{JNIEnv, JNINativeInterface__1_6, jbyte, jbyteArray, jobject, jsize};
use slipbox_rpc::android::{
    ADAPTER_LIMITS, AdapterBound, AdapterCapability, AdapterRefusal, RefusalReason, encode_refusal,
};

use crate::adapter::{
    Refusable, Served, contained, serve_close, serve_contract, serve_maintenance, serve_open,
    serve_read, sessions,
};
use crate::probe::{ProbeReport, run_fixture_probe};

const ENCODE_FAILURE: &[u8] = br#"{"passed":false,"checks":[],"failure":{"stage":"encode","detail":"the probe report did not encode"}}"#;

/// Run the fixture probe under `parent_directory` and return its report as
/// UTF-8 JSON, or null when the JVM cannot allocate the result array.
///
/// The symbol name is the JNI mangling of
/// `io.github.b_vitamins.slipbox.engine.SlipboxNativeEngine.nativeRunFixtureProbe`,
/// where `_1` escapes the underscore in `b_vitamins`.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread, and
/// `parent_directory` a reference to a Java `byte[]` holding a UTF-8 absolute
/// path. The JVM supplies both when it resolves this symbol.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeRunFixtureProbe(
    env: *mut JNIEnv,
    _this: jobject,
    parent_directory: jbyteArray,
) -> jbyteArray {
    // Unwinding out of an `extern` function aborts the process, so the whole
    // probe reports its own panic instead.
    let report = panic::catch_unwind(AssertUnwindSafe(|| {
        match unsafe { read_path(env, parent_directory) } {
            Directory::Path(parent) => Some(run_fixture_probe(&parent)),
            Directory::Refused(detail) => Some(ProbeReport::aborted("argument", detail)),
            Directory::Pending => None,
        }
    }))
    .unwrap_or_else(|_| {
        Some(ProbeReport::aborted(
            "panic",
            "the probe panicked before reporting".to_owned(),
        ))
    });

    let Some(report) = report else {
        return ptr::null_mut();
    };
    let json = serde_json::to_vec(&report).unwrap_or_else(|_| ENCODE_FAILURE.to_vec());
    unsafe { new_byte_array(env, &json) }
}

/// Answer with the limits and operations this library admits.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeAdapterContract(
    env: *mut JNIEnv,
    _this: jobject,
) -> jbyteArray {
    unsafe { new_byte_array(env, &contained(serve_contract)) }
}

/// Open one reading session.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread and `request` a
/// reference to a Java `byte[]`, or null.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeOpenReadSession(
    env: *mut JNIEnv,
    _this: jobject,
    request: jbyteArray,
) -> jbyteArray {
    unsafe { opened(env, request, AdapterCapability::Read) }
}

/// Open one index maintenance session.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread and `request` a
/// reference to a Java `byte[]`, or null.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeOpenMaintenanceSession(
    env: *mut JNIEnv,
    _this: jobject,
    request: jbyteArray,
) -> jbyteArray {
    unsafe { opened(env, request, AdapterCapability::Maintenance) }
}

/// Answer one read operation on an open reading session.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread and `request` a
/// reference to a Java `byte[]`, or null.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeReadSession(
    env: *mut JNIEnv,
    _this: jobject,
    request: jbyteArray,
) -> jbyteArray {
    unsafe { answer(env, request, |request| serve_read(sessions(), request)) }
}

/// Carry out one maintenance operation on an open maintenance session.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread and `request` a
/// reference to a Java `byte[]`, or null.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeMaintainSession(
    env: *mut JNIEnv,
    _this: jobject,
    request: jbyteArray,
) -> jbyteArray {
    unsafe {
        answer(env, request, |request| {
            serve_maintenance(sessions(), request)
        })
    }
}

/// Retire one session.
///
/// # Safety
///
/// `env` must be the JNI environment of the calling thread and `request` a
/// reference to a Java `byte[]`, or null.
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeCloseSession(
    env: *mut JNIEnv,
    _this: jobject,
    request: jbyteArray,
) -> jbyteArray {
    unsafe { answer(env, request, |request| serve_close(sessions(), request)) }
}

/// The argument of one adapter call, or the reason there is none.
enum Argument {
    Bytes(Vec<u8>),
    Refused(AdapterRefusal),
    /// A JVM exception is pending. Nothing further may be asked of JNI until it
    /// reaches Java, so the call returns null and the JVM throws.
    Pending,
}

/// Reads the request, serves it, and returns the answer as a fresh `byte[]`.
unsafe fn answer(
    env: *mut JNIEnv,
    request: jbyteArray,
    serve: impl FnOnce(&[u8]) -> Vec<u8>,
) -> jbyteArray {
    let response = match unsafe { read_request(env, request) } {
        Argument::Bytes(request) => contained(|| serve(&request)),
        Argument::Refused(refusal) => encode_refusal(refusal),
        Argument::Pending => return ptr::null_mut(),
    };
    unsafe { new_byte_array(env, &response) }
}

/// Opens one session and hands the JVM its answer.
///
/// A session whose answer does not reach Java is retired here, while the failure
/// that stopped it is still pending: Java never learns that handle, so nothing
/// else could retire it, and the slot it holds would otherwise stay held for the
/// life of the process. Only that session is retired.
unsafe fn opened(
    env: *mut JNIEnv,
    request: jbyteArray,
    capability: AdapterCapability,
) -> jbyteArray {
    let served = match unsafe { read_request(env, request) } {
        Argument::Bytes(request) => contained(|| serve_open(sessions(), capability, &request)),
        Argument::Refused(refusal) => Served::refused(refusal),
        Argument::Pending => return ptr::null_mut(),
    };
    let array = unsafe { new_byte_array(env, &served.response) };
    if !array.is_null() {
        return array;
    }
    if let Some(handle) = served.opened {
        sessions().abandon(handle);
    }
    array
}

/// Copies one request out of the JVM, refusing an oversized one before
/// allocating for it.
unsafe fn read_request(env: *mut JNIEnv, array: jbyteArray) -> Argument {
    // Before the argument is judged: a refusal is a side effect, and a null
    // argument is what a JVM that has already thrown passes.
    if unsafe { pending(env) } {
        return Argument::Pending;
    }
    if array.is_null() {
        return Argument::Refused(AdapterRefusal::of(RefusalReason::MalformedRequest));
    }
    let jni = unsafe { interface(env) };
    let length = unsafe { (jni.GetArrayLength)(env, array) };
    if unsafe { pending(env) } {
        return Argument::Pending;
    }
    let Ok(length) = usize::try_from(length) else {
        return Argument::Refused(AdapterRefusal::of(RefusalReason::MalformedRequest));
    };
    if length > ADAPTER_LIMITS.max_request_bytes {
        return Argument::Refused(AdapterRefusal::bounded(AdapterBound::RequestBytes));
    }

    let mut bytes = vec![0u8; length];
    unsafe {
        (jni.GetByteArrayRegion)(
            env,
            array,
            0,
            length as jsize,
            bytes.as_mut_ptr().cast::<jbyte>(),
        );
    }
    if unsafe { pending(env) } {
        return Argument::Pending;
    }
    Argument::Bytes(bytes)
}

/// Android implements JNI 1.6, so that namespace resolves every call here.
unsafe fn interface(env: *mut JNIEnv) -> JNINativeInterface__1_6 {
    unsafe { (**env).v1_6 }
}

/// True while a JVM exception is on its way to Java, when nothing else may be
/// asked of JNI.
unsafe fn pending(env: *mut JNIEnv) -> bool {
    unsafe { (interface(env).ExceptionCheck)(env) }
}

/// The parent directory of one probe run, or the reason there is none.
enum Directory {
    Path(PathBuf),
    Refused(String),
    /// A JVM exception is pending, so the probe does not run.
    Pending,
}

unsafe fn read_path(env: *mut JNIEnv, array: jbyteArray) -> Directory {
    if unsafe { pending(env) } {
        return Directory::Pending;
    }
    if array.is_null() {
        return Directory::Refused(
            "the probe needs an application-private parent directory".to_owned(),
        );
    }
    let jni = unsafe { interface(env) };
    let length = unsafe { (jni.GetArrayLength)(env, array) };
    if unsafe { pending(env) } {
        return Directory::Pending;
    }
    if length <= 0 {
        return Directory::Refused("the parent directory path is empty".to_owned());
    }
    if length as usize > ADAPTER_LIMITS.max_path_bytes {
        return Directory::Refused(
            "the parent directory path exceeds the declared bound".to_owned(),
        );
    }
    let mut bytes = vec![0u8; length as usize];
    unsafe {
        (jni.GetByteArrayRegion)(env, array, 0, length, bytes.as_mut_ptr().cast::<jbyte>());
    }
    if unsafe { pending(env) } {
        return Directory::Pending;
    }
    match String::from_utf8(bytes) {
        Ok(path) => Directory::Path(PathBuf::from(path)),
        Err(_) => Directory::Refused("the parent directory path is not UTF-8".to_owned()),
    }
}

/// Allocates the answer and fills it, leaving any exception the JVM raised
/// pending: the null return is what Kotlin sees, and the JVM throws.
///
/// A null return therefore means the answer did not reach Java, whether because
/// an exception was already pending, the allocation failed, or filling it did.
unsafe fn new_byte_array(env: *mut JNIEnv, bytes: &[u8]) -> jbyteArray {
    if unsafe { pending(env) } {
        return ptr::null_mut();
    }
    let jni = unsafe { interface(env) };
    let Ok(length) = jsize::try_from(bytes.len()) else {
        return ptr::null_mut();
    };
    let array = unsafe { (jni.NewByteArray)(env, length) };
    if array.is_null() || unsafe { pending(env) } {
        return ptr::null_mut();
    }
    unsafe {
        (jni.SetByteArrayRegion)(env, array, 0, length, bytes.as_ptr().cast::<jbyte>());
    }
    if unsafe { pending(env) } {
        return ptr::null_mut();
    }
    array
}
