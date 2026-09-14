//! What the JNI boundary does when the JVM fails the calls it makes.
//!
//! Every case here calls one of the exported entry points with a JNI function
//! table this test owns, so what is under test is the packaged library's own
//! discipline: a pending exception is preserved and nothing further is asked of
//! JNI, an argument the JVM cannot measure or copy stops the call, and an answer
//! it cannot allocate or fill does not reach Java as a half-formed one.

use std::cell::RefCell;
use std::mem::size_of;
use std::process;
use std::ptr;

use jni_sys::{
    JNIEnv, JNINativeInterface_, JNINativeInterface__1_6, jboolean, jbyte, jbyteArray, jint,
    jobject, jsize,
};
use serde_json::{Value, json};
use slipbox_android::jni_seam;
use slipbox_rpc::android::{ADAPTER_LIMITS, ADAPTER_PROTOCOL_VERSION, AdapterCapability};
use tempfile::TempDir;

/// The `this` reference the JVM passes. The entry points are static in effect and
/// never touch it.
const THIS: jobject = ptr::null_mut();

const EXCEPTION_CHECK: &str = "ExceptionCheck";
const GET_ARRAY_LENGTH: &str = "GetArrayLength";
const GET_BYTE_ARRAY_REGION: &str = "GetByteArrayRegion";
const NEW_BYTE_ARRAY: &str = "NewByteArray";
const SET_BYTE_ARRAY_REGION: &str = "SetByteArrayRegion";

/// One exported entry point, called the way the JVM calls it.
type Entry = fn(*mut JNIEnv, jbyteArray) -> jbyteArray;

/// The entry points that take one `byte[]` argument.
fn argument_entries() -> Vec<(&'static str, Entry)> {
    vec![
        ("nativeOpenReadSession", |env, argument| unsafe {
            jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeOpenReadSession(env, THIS, argument)
        }),
        ("nativeOpenMaintenanceSession", |env, argument| unsafe {
            jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeOpenMaintenanceSession(env, THIS, argument)
        }),
        ("nativeReadSession", |env, argument| unsafe {
            jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeReadSession(env, THIS, argument)
        }),
        ("nativeMaintainSession", |env, argument| unsafe {
            jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeMaintainSession(env, THIS, argument)
        }),
        ("nativeCloseSession", |env, argument| unsafe {
            jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeCloseSession(env, THIS, argument)
        }),
        ("nativeRunFixtureProbe", |env, argument| unsafe {
            jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeRunFixtureProbe(env, THIS, argument)
        }),
    ]
}

/// The entry points that own no session, so a failed answer leaves nothing behind
/// to account for. The two that open a session are held to the same faults in
/// `every_slot_is_available_after_an_open_whose_answer_never_reached_java`.
fn stateless_entries() -> Vec<(&'static str, Entry)> {
    argument_entries()
        .into_iter()
        .filter(|(name, _)| !name.starts_with("nativeOpen"))
        .collect()
}

/// An argument each entry point reads to the end and answers without touching the
/// session table: an operation of its own vocabulary under a handle no session
/// holds, and for the probe a parent directory.
fn argument_of(entry: &str, probe: &TempDir) -> Vec<u8> {
    let binding = json!({"source": SOURCE, "generation": GENERATION});
    let request = match entry {
        "nativeRunFixtureProbe" => return text(probe.path()).into_bytes(),
        "nativeCloseSession" => json!({
            "version": ADAPTER_PROTOCOL_VERSION,
            "handle": 0,
            "binding": binding,
        }),
        "nativeMaintainSession" => json!({
            "version": ADAPTER_PROTOCOL_VERSION,
            "handle": 0,
            "binding": binding,
            "operation": {"kind": "index"},
        }),
        _ => json!({
            "version": ADAPTER_PROTOCOL_VERSION,
            "handle": 0,
            "binding": binding,
            "operation": {"kind": "status"},
        }),
    };
    encoded(&request)
}

const SOURCE: &str = "0102030405060708090a0b0c0d0e0f10";
const GENERATION: &str = "fixture-01";

#[test]
fn a_pending_exception_reaches_java_and_nothing_else_is_asked_of_jni() {
    let mut env = Env::new();
    let probe = tempfile::tempdir().expect("a temporary directory");

    for (name, entry) in argument_entries() {
        for argument in [array(), ptr::null_mut()] {
            arrange(Jvm {
                pending: true,
                argument: argument_of(name, &probe),
                ..Jvm::default()
            });
            let answered = entry(env.as_ptr(), argument);

            assert!(
                answered.is_null(),
                "{name} answered while an exception was pending"
            );
            assert_eq!(trace(), [EXCEPTION_CHECK], "{name}");
            assert_eq!(
                allocations(),
                0,
                "{name} allocated for an answer it cannot give"
            );
        }
    }

    // The contract takes no argument, and still asks nothing of a JVM that has
    // thrown.
    arrange(Jvm {
        pending: true,
        ..Jvm::default()
    });
    assert!(contract(&mut env).is_null());
    assert_eq!(trace(), [EXCEPTION_CHECK]);
}

#[test]
fn an_argument_the_jvm_cannot_measure_or_copy_stops_the_call() {
    let mut env = Env::new();
    let probe = tempfile::tempdir().expect("a temporary directory");

    for (name, entry) in argument_entries() {
        for (failing, expected) in [
            (
                GET_ARRAY_LENGTH,
                vec![EXCEPTION_CHECK, GET_ARRAY_LENGTH, EXCEPTION_CHECK],
            ),
            (
                GET_BYTE_ARRAY_REGION,
                vec![
                    EXCEPTION_CHECK,
                    GET_ARRAY_LENGTH,
                    EXCEPTION_CHECK,
                    GET_BYTE_ARRAY_REGION,
                    EXCEPTION_CHECK,
                ],
            ),
        ] {
            arrange(Jvm {
                argument: argument_of(name, &probe),
                failing: Some(failing),
                ..Jvm::default()
            });
            let answered = entry(env.as_ptr(), array());

            assert!(
                answered.is_null(),
                "{name} answered a request it never read"
            );
            assert_eq!(trace(), expected, "{name} after {failing} failed");
            assert_eq!(allocations(), 0, "{name} allocated after {failing} failed");
        }
    }
}

#[test]
fn an_answer_the_jvm_cannot_allocate_or_fill_does_not_reach_java() {
    let mut env = Env::new();
    let probe = tempfile::tempdir().expect("a temporary directory");

    for (name, entry) in stateless_entries() {
        arrange(Jvm {
            argument: argument_of(name, &probe),
            failing: Some(NEW_BYTE_ARRAY),
            ..Jvm::default()
        });
        assert!(entry(env.as_ptr(), array()).is_null(), "{name}");
        assert_eq!(
            trace().last().map(String::as_str),
            Some(NEW_BYTE_ARRAY),
            "{name}"
        );
        assert_eq!(
            answer(),
            None,
            "{name} answered out of an array it never got"
        );

        arrange(Jvm {
            argument: argument_of(name, &probe),
            failing: Some(SET_BYTE_ARRAY_REGION),
            ..Jvm::default()
        });
        assert!(entry(env.as_ptr(), array()).is_null(), "{name}");
        assert_eq!(
            trace().last().map(String::as_str),
            Some(EXCEPTION_CHECK),
            "{name} did not look for the exception filling the answer raised"
        );
        assert_eq!(allocations(), 1, "{name}");
        assert_eq!(
            answer(),
            None,
            "{name} filled the answer it was told it could not"
        );
    }

    arrange(Jvm {
        failing: Some(NEW_BYTE_ARRAY),
        ..Jvm::default()
    });
    assert!(contract(&mut env).is_null());
    assert_eq!(answer(), None);
}

#[test]
fn every_entry_point_answers_through_this_boundary() {
    let mut env = Env::new();
    let probe = tempfile::tempdir().expect("a temporary directory");

    arrange(Jvm::default());
    assert_eq!(contract(&mut env), array_of_answer());
    assert_eq!(document()["outcome"], "contract");
    assert_eq!(
        trace(),
        [
            EXCEPTION_CHECK,
            NEW_BYTE_ARRAY,
            EXCEPTION_CHECK,
            SET_BYTE_ARRAY_REGION,
            EXCEPTION_CHECK
        ]
    );

    for (name, entry) in stateless_entries() {
        arrange(Jvm {
            argument: argument_of(name, &probe),
            ..Jvm::default()
        });
        assert_eq!(entry(env.as_ptr(), array()), array_of_answer(), "{name}");

        let answered = document();
        if name == "nativeRunFixtureProbe" {
            assert_eq!(answered["passed"], true, "{answered}");
        } else {
            assert_eq!(answered["outcome"], "refused", "{answered}");
            assert_eq!(answered["reason"], "unknown-handle", "{answered}");
        }
        assert_eq!(
            trace(),
            [
                EXCEPTION_CHECK,
                GET_ARRAY_LENGTH,
                EXCEPTION_CHECK,
                GET_BYTE_ARRAY_REGION,
                EXCEPTION_CHECK,
                EXCEPTION_CHECK,
                NEW_BYTE_ARRAY,
                EXCEPTION_CHECK,
                SET_BYTE_ARRAY_REGION,
                EXCEPTION_CHECK
            ],
            "{name}"
        );
        assert_eq!(
            requested(),
            Some((0, argument_of(name, &probe).len() as jsize))
        );
    }
}

/// The session table is one per loaded library, and this is the only case here
/// that opens a session.
#[test]
fn every_slot_is_available_after_an_open_whose_answer_never_reached_java() {
    let mut env = Env::new();
    let corpus = Corpus::new();
    let request = encoded(&corpus.open_request(AdapterCapability::Read));

    // A session the caller cannot learn about is retired in Rust, so the slot it
    // held is not lost for the life of the process.
    for failing in [NEW_BYTE_ARRAY, SET_BYTE_ARRAY_REGION] {
        arrange(Jvm {
            argument: request.clone(),
            failing: Some(failing),
            ..Jvm::default()
        });
        assert!(
            open_read(&mut env).is_null(),
            "an open answered after {failing} failed"
        );

        let held: Vec<i64> = (0..ADAPTER_LIMITS.max_open_sessions)
            .map(|_| {
                arrange(Jvm {
                    argument: request.clone(),
                    ..Jvm::default()
                });
                assert_eq!(open_read(&mut env), array_of_answer());
                let opened = document();
                assert_eq!(opened["outcome"], "opened", "{opened}");
                opened["handle"]
                    .as_i64()
                    .expect("an opened session is named")
            })
            .collect();
        assert_eq!(
            held.len(),
            ADAPTER_LIMITS.max_open_sessions,
            "the open that failed with {failing} kept a slot"
        );

        arrange(Jvm {
            argument: request.clone(),
            ..Jvm::default()
        });
        assert_eq!(open_read(&mut env), array_of_answer());
        let exhausted = document();
        assert_eq!(exhausted["reason"], "sessions-exhausted", "{exhausted}");

        // Every session that was opened is still the caller's to retire, and the
        // table is empty again for the next arrangement.
        for handle in held {
            arrange(Jvm {
                argument: encoded(&corpus.close_request(handle)),
                ..Jvm::default()
            });
            assert_eq!(close(&mut env), array_of_answer());
            let closed = document();
            assert_eq!(closed["retired"], true, "{closed}");
        }
    }
}

/// The synthetic corpus one session is opened against.
struct Corpus {
    directory: TempDir,
}

impl Corpus {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(directory.path().join("notes")).expect("the corpus root is created");
        std::fs::write(
            directory.path().join("notes").join("alpha.org"),
            "#+title: Alpha\n",
        )
        .expect("the corpus is written");
        Self { directory }
    }

    fn open_request(&self, capability: AdapterCapability) -> Value {
        json!({
            "version": ADAPTER_PROTOCOL_VERSION,
            "capability": capability,
            "binding": {"source": SOURCE, "generation": GENERATION},
            "context": {
                "root": text(&self.directory.path().join("notes")),
                "database": text(&self.directory.path().join("session.sqlite")),
            },
        })
    }

    fn close_request(&self, handle: i64) -> Value {
        json!({
            "version": ADAPTER_PROTOCOL_VERSION,
            "handle": handle,
            "binding": {"source": SOURCE, "generation": GENERATION},
        })
    }
}

fn contract(env: &mut Env) -> jbyteArray {
    unsafe {
        jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeAdapterContract(
            env.as_ptr(),
            THIS,
        )
    }
}

fn open_read(env: &mut Env) -> jbyteArray {
    unsafe {
        jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeOpenReadSession(
            env.as_ptr(),
            THIS,
            array(),
        )
    }
}

fn close(env: &mut Env) -> jbyteArray {
    unsafe {
        jni_seam::Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeCloseSession(
            env.as_ptr(),
            THIS,
            array(),
        )
    }
}

/// What this JVM does to the calls the boundary makes, and what it recorded of
/// them.
#[derive(Default)]
struct Jvm {
    /// The JNI functions the entry point called, in order.
    trace: Vec<String>,
    /// True while an exception is on its way to Java.
    pending: bool,
    /// The bytes of the `byte[]` argument.
    argument: Vec<u8>,
    /// The call this JVM fails, raising an exception as the real one would.
    failing: Option<&'static str>,
    /// The region of the argument the boundary asked for.
    requested: Option<(jsize, jsize)>,
    allocations: usize,
    /// The length of the array last allocated for an answer.
    allocated: Option<usize>,
    /// The answer the boundary filled that array with.
    answer: Option<Vec<u8>>,
}

thread_local! {
    static JVM: RefCell<Jvm> = RefCell::new(Jvm::default());
}

fn jvm<T>(act: impl FnOnce(&mut Jvm) -> T) -> T {
    JVM.with(|cell| act(&mut cell.borrow_mut()))
}

fn arrange(arrangement: Jvm) {
    jvm(|jvm| *jvm = arrangement);
}

fn trace() -> Vec<String> {
    jvm(|jvm| jvm.trace.clone())
}

fn allocations() -> usize {
    jvm(|jvm| jvm.allocations)
}

fn answer() -> Option<Vec<u8>> {
    jvm(|jvm| jvm.answer.clone())
}

fn requested() -> Option<(jsize, jsize)> {
    jvm(|jvm| jvm.requested)
}

/// The answer the boundary handed back, as the document Kotlin decodes.
fn document() -> Value {
    let answer = answer().expect("the boundary filled an answer");
    serde_json::from_slice(&answer).expect("an answer is one JSON document")
}

/// Records one JNI call, and reports whether this JVM is arranged to fail it.
fn call(name: &'static str) -> bool {
    jvm(|jvm| {
        jvm.trace.push(name.to_owned());
        let failing = jvm.failing == Some(name);
        if failing {
            jvm.pending = true;
        }
        failing
    })
}

/// The argument reference this JVM hands out, and the one it allocates for an
/// answer. Neither is ever dereferenced.
fn array() -> jbyteArray {
    ptr::without_provenance_mut(0x10)
}

fn array_of_answer() -> jbyteArray {
    ptr::without_provenance_mut(0x20)
}

unsafe extern "system" fn exception_check(_: *mut JNIEnv) -> jboolean {
    jvm(|jvm| {
        jvm.trace.push(EXCEPTION_CHECK.to_owned());
        jvm.pending
    })
}

unsafe extern "system" fn get_array_length(_: *mut JNIEnv, _array: jbyteArray) -> jsize {
    if call(GET_ARRAY_LENGTH) {
        return 0;
    }
    jvm(|jvm| jvm.argument.len() as jsize)
}

unsafe extern "system" fn get_byte_array_region(
    _: *mut JNIEnv,
    _array: jbyteArray,
    start: jsize,
    len: jsize,
    buffer: *mut jbyte,
) {
    if call(GET_BYTE_ARRAY_REGION) {
        return;
    }
    jvm(|jvm| {
        jvm.requested = Some((start, len));
        let from = (start.max(0) as usize).min(jvm.argument.len());
        let count = (len.max(0) as usize).min(jvm.argument.len() - from);
        // SAFETY: the JVM copies into a buffer the caller sized for `len`, and
        // this copy is clamped to the argument it holds.
        unsafe {
            ptr::copy_nonoverlapping(jvm.argument[from..].as_ptr().cast::<jbyte>(), buffer, count);
        }
    });
}

unsafe extern "system" fn new_byte_array(_: *mut JNIEnv, len: jsize) -> jbyteArray {
    if call(NEW_BYTE_ARRAY) {
        return ptr::null_mut();
    }
    jvm(|jvm| {
        jvm.allocations += 1;
        jvm.allocated = Some(len.max(0) as usize);
    });
    array_of_answer()
}

unsafe extern "system" fn set_byte_array_region(
    _: *mut JNIEnv,
    _array: jbyteArray,
    _start: jsize,
    len: jsize,
    buffer: *const jbyte,
) {
    if call(SET_BYTE_ARRAY_REGION) {
        return;
    }
    // SAFETY: the boundary passes the buffer it just encoded, of `len` bytes.
    let filled =
        unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), len.max(0) as usize) }.to_vec();
    jvm(|jvm| jvm.answer = Some(filled));
}

/// Stands in for every JNI function this test does not provide. Reaching it means
/// the boundary asked for something no case here arranged, which is a failure of
/// the boundary and not of the JVM.
unsafe extern "system" fn unprovided(_: *mut JNIEnv) -> jint {
    eprintln!("the boundary called a JNI function this test does not provide");
    process::abort()
}

/// One JNI environment whose function table is this test's.
struct Env {
    interface: Box<JNINativeInterface_>,
    table: *const JNINativeInterface_,
}

impl Env {
    fn new() -> Self {
        let mut functions = unprovided_functions();
        functions.ExceptionCheck = exception_check;
        functions.GetArrayLength = get_array_length;
        functions.GetByteArrayRegion = get_byte_array_region;
        functions.NewByteArray = new_byte_array;
        functions.SetByteArrayRegion = set_byte_array_region;

        Self {
            interface: Box::new(JNINativeInterface_ { v1_6: functions }),
            table: ptr::null(),
        }
    }

    /// A `JNIEnv` is a pointer to a pointer to the function table, so the pointer
    /// the entry points are given has to live somewhere: it lives here.
    fn as_ptr(&mut self) -> *mut JNIEnv {
        self.table = &*self.interface;
        &mut self.table
    }
}

/// A function table whose every entry is a real function, so a call this test did
/// not provide reaches [`unprovided`] rather than an invalid pointer.
fn unprovided_functions() -> JNINativeInterface__1_6 {
    let width = size_of::<usize>();
    assert_eq!(
        size_of::<JNINativeInterface__1_6>() % width,
        0,
        "the JNI function table is not a table of pointers"
    );
    let entry = unprovided as *const () as usize;
    let entries = vec![entry; size_of::<JNINativeInterface__1_6>() / width];
    // SAFETY: the table is a C struct of pointer-sized fields, aligned as `usize`
    // is, and every word read here is the address of a real function.
    unsafe { ptr::read(entries.as_ptr().cast::<JNINativeInterface__1_6>()) }
}

fn encoded(document: &Value) -> Vec<u8> {
    serde_json::to_vec(document).expect("a request document encodes")
}

fn text(path: &std::path::Path) -> String {
    path.to_str()
        .unwrap_or_else(|| panic!("{} is not UTF-8", path.display()))
        .to_owned()
}
