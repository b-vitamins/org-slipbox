//! The single JNI entry point of the packaged library.
//!
//! Paths and reports cross the boundary as UTF-8 `byte[]`, not `String`, so no
//! value passes through Java's modified UTF-8 encoding.

use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::ptr;

use jni_sys::{JNIEnv, JNINativeInterface__1_6, jbyte, jbyteArray, jobject, jsize};

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
            Ok(parent) => run_fixture_probe(&parent),
            Err(detail) => ProbeReport::aborted("argument", detail),
        }
    }))
    .unwrap_or_else(|_| {
        ProbeReport::aborted("panic", "the probe panicked before reporting".to_owned())
    });

    let json = serde_json::to_vec(&report).unwrap_or_else(|_| ENCODE_FAILURE.to_vec());
    unsafe { new_byte_array(env, &json) }
}

/// Android implements JNI 1.6, so that namespace resolves every call here.
unsafe fn interface(env: *mut JNIEnv) -> JNINativeInterface__1_6 {
    unsafe { (**env).v1_6 }
}

unsafe fn read_path(env: *mut JNIEnv, array: jbyteArray) -> Result<PathBuf, String> {
    if array.is_null() {
        return Err("the probe needs an application-private parent directory".to_owned());
    }
    let jni = unsafe { interface(env) };
    let length = unsafe { (jni.GetArrayLength)(env, array) };
    if length <= 0 {
        return Err("the parent directory path is empty".to_owned());
    }
    let mut bytes = vec![0u8; length as usize];
    unsafe {
        (jni.GetByteArrayRegion)(env, array, 0, length, bytes.as_mut_ptr().cast::<jbyte>());
    }
    String::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|_| "the parent directory path is not UTF-8".to_owned())
}

unsafe fn new_byte_array(env: *mut JNIEnv, bytes: &[u8]) -> jbyteArray {
    let jni = unsafe { interface(env) };
    let Ok(length) = jsize::try_from(bytes.len()) else {
        return ptr::null_mut();
    };
    let array = unsafe { (jni.NewByteArray)(env, length) };
    if array.is_null() {
        return array;
    }
    unsafe {
        (jni.SetByteArrayRegion)(env, array, 0, length, bytes.as_ptr().cast::<jbyte>());
    }
    array
}
