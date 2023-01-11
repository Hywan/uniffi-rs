/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

/// Non-blocking timer future.
pub struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();

        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerFuture {
    pub fn new(duration: Duration) -> Self {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shared_state = shared_state.clone();

        // Let's mimic an event coming from somewhere else, like the system.
        thread::spawn(move || {
            thread::sleep(duration);

            let mut shared_state: MutexGuard<_> = thread_shared_state.lock().unwrap();
            shared_state.completed = true;

            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });

        TimerFuture { shared_state }
    }
}

/// Sync function.
#[uniffi::export]
pub fn greet(who: String) -> String {
    format!("Hello, {who}")
}

/// Async function that is immediatly ready.
#[uniffi::export]
pub async fn always_ready() -> bool {
    true
}

#[uniffi::export]
pub async fn void() {}

/// Async function that says something after 2s.
#[uniffi::export]
pub async fn say() -> String {
    TimerFuture::new(Duration::from_secs(2)).await;

    "Hello, Future!".to_string()
}

/// Async function that says something after a certain time.
#[uniffi::export]
pub async fn say_after(secs: u8, who: String) -> String {
    TimerFuture::new(Duration::from_secs(secs.into())).await;

    format!("Hello, {who}!")
}

/// Async function that sleeps!
#[uniffi::export]
pub async fn sleep(secs: u8) -> bool {
    TimerFuture::new(Duration::from_secs(secs.into())).await;

    true
}

/// Sync function that generates a new `Megaphone`.
///
/// It builds a `Megaphone` which has async methods on it.
#[uniffi::export]
pub fn new_megaphone() -> Arc<Megaphone> {
    Arc::new(Megaphone)
}

/// A megaphone. Be careful with the neighbours.
#[derive(uniffi::Object)]
pub struct Megaphone;

#[uniffi::export]
impl Megaphone {
    /// An async function that yells something after a certain time.
    async fn say_after(self: Arc<Self>, secs: u8, who: String) -> String {
        say_after(secs, who).await.to_uppercase()
    }
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn say_after_with_tokio(secs: u8, who: String) -> String {
    tokio::time::sleep(Duration::from_secs(secs.into())).await;

    format!("Hello, {who} (with Tokio)!")
}

#[derive(uniffi::Error, Debug)]
pub enum MyError {
    Foo,
}

//#[uniffi::export]
pub async fn fallible_me(do_fail: bool) -> Result<u8, MyError> {
    if do_fail {
        dbg!("Err(MyError::Foo)");
        Err(MyError::Foo)
    } else {
        dbg!("Ok(42)");
        Ok(42)
    }
}

#[doc(hidden)]
#[no_mangle]
pub extern "C" fn _uniffi_uniffi_futures_fallible_me_d39d(
    arg0: <bool as ::uniffi::FfiConverter>::FfiType,
    call_status: &mut ::uniffi::RustCallStatus,
) -> Option<Box<::uniffi::RustFuture<Result<u8, MyError>>>> {
    ::uniffi::call_with_output(call_status, || {
        Some(Box::new(::uniffi::RustFuture::new(async move {
            fallible_me(
                <bool as ::uniffi::FfiConverter>::try_lift(arg0)
                    .unwrap_or_else(|err| panic!("foo bar baz hack")),
            )
            .await
        })))
    })
}

#[doc(hidden)]
#[no_mangle]
pub extern "C" fn _uniffi_uniffi_futures_fallible_me_d39d_poll(
    future: ::std::option::Option<&mut ::uniffi::RustFuture<Result<u8, MyError>>>,
    waker: ::std::option::Option<::uniffi::RustFutureForeignWakerFunction>,
    waker_environment: *const ::uniffi::RustFutureForeignWakerEnvironment,
    polled_result: &mut <u8 as ::uniffi::FfiReturn>::FfiType,
    call_status: &mut ::uniffi::RustCallStatus,
) -> bool {
    ::uniffi::ffi::uniffi_rustfuture_poll(
        future,
        waker,
        waker_environment,
        polled_result,
        call_status,
    )
}

#[doc(hidden)]
#[no_mangle]
pub extern "C" fn _uniffi_uniffi_futures_fallible_me_d39d_drop(
    future: ::std::option::Option<::std::boxed::Box<::uniffi::RustFuture<Result<u8, MyError>>>>,
    call_status: &mut ::uniffi::RustCallStatus,
) {
    ::uniffi::ffi::uniffi_rustfuture_drop(future, call_status)
}

#[no_mangle]
#[doc(hidden)]
pub static UNIFFI_META_fallible_me: [u8; 102usize] = [
    0u8, 0u8, 0u8, 0u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 14u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
    0u8, 117u8, 110u8, 105u8, 102u8, 102u8, 105u8, 95u8, 102u8, 117u8, 116u8, 117u8, 114u8, 101u8,
    115u8, 11u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 102u8, 97u8, 108u8, 108u8, 105u8, 98u8, 108u8,
    101u8, 95u8, 109u8, 101u8, 1u8, 1u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 7u8, 0u8, 0u8, 0u8,
    0u8, 0u8, 0u8, 0u8, 100u8, 111u8, 95u8, 102u8, 97u8, 105u8, 108u8, 10u8, 0u8, 0u8, 0u8, 1u8,
    0u8, 0u8, 0u8, 0u8, 1u8, 7u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 77u8, 121u8, 69u8, 114u8,
    114u8, 111u8, 114u8,
];

include!(concat!(env!("OUT_DIR"), "/uniffi_futures.uniffi.rs"));

mod uniffi_types {
    pub(crate) use super::Megaphone;
    pub(crate) use super::MyError;
}
