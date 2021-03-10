use proc_macro::TokenStream;
use quote::quote;

/// Attribute macro that wraps a function with the sole purpose of benchmarking the duration of its execution,
/// and printing the gathered metrics through Rust's logging API.
/// <br/><br/>
/// This macro will introduce minimal overhead in **debug** builds; wrapping the selected functions in closures
/// that track their execution time, and printing the tracked result in a `trace!()` call upon their return. This
/// is *disabled* in **release** builds, which makes the macro a zero-cost abstraction in productive environments
/// (not that said cost was much at all to begin with).
/// <br/><br/>
/// **Heavily** inspired on: https://stackoverflow.com/a/60732300.
///
/// See more: https://blog.rust-lang.org/2018/12/21/Procedural-Macros-in-Rust-2018.html.
///
/// ---
///
/// # Requirements
///
/// - Rust's logging facade crate - https://crates.io/crates/log
/// - Time crate - https://crates.io/crates/time
///
/// # Usage
/// ## `fn()` example:
/// ```rust
/// use prpolice_lib::*;
///
/// use octocrab::models::pulls::PullRequest;
///
/// #[prolice_trace_time]
/// fn get_pr_message(pr: &PullRequest) -> String {
///     pr.body.as_ref().unwrap().clone()
/// }
/// ```
/// This will output:
/// ```text
///  TRACE prolice > Tracing time for `fn get_pr_message()`...
///  TRACE prolice > Time elapsed for `fn get_pr_message()` was: Duration { seconds: 0, nanoseconds: 2407 }
/// ```
///
/// ## `async fn()` example
///
/// ```rust
/// use prpolice_lib::*;
///
/// use octocrab::{Page, Octocrab};
/// use octocrab::models::pulls::Review;
///
/// #[prolice_trace_time]
/// async fn get_pr_reviews(repo_name: String, pr_number: u64) -> octocrab::Result<Page<Review>> {
///     Octocrab::default()
///         .pulls("OWNER", repo_name)
///         .list_reviews(pr_number)
///         .await
/// }
/// ```
/// This will output:
/// ```text
///  TRACE prolice > Tracing time for `fn get_pr_reviews()`...
///  TRACE prolice > Time elapsed for `fn get_pr_reviews()` was: Duration { seconds: 1, nanoseconds: 268012488 }
/// ```
#[proc_macro_attribute]
pub fn prolice_trace_time(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // parse the passed item as a function
    let func = syn::parse_macro_input!(item as syn::ItemFn);

    // break the function down into its parts
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = func;

    // determine async-ness of function
    let is_async_fn = sig.asyncness.is_some();

    // extract function name for prettier output
    let name = format!("{}", sig.ident);

    // determine type of build (debug/release)
    let release_build = !cfg!(debug_assertions);

    // wrap body only if function is async, otherwise just put it in the middle of the time-tracking
    let block = if release_build {
        quote! { #block } // disable time tracker on release builds
    } else if is_async_fn {
        quote! {
            use log::trace;
            use time::Instant;

            let start = Instant::now();
            let result = async move { #block }.await;
            trace!("Time elapsed for `fn {}()` was: {:?}", #name, start.elapsed());
            result
        }
    } else {
        quote! {
            use log::trace;
            use time::Instant;

            let start = Instant::now();
            let result = { #block };
            trace!("Time elapsed for `fn {}()` was: {:?}", #name, start.elapsed());
            result
        }
    };

    // generate the output, rewriting function with our tracked wrapper
    let output = quote! {
        #[track_caller]
        #(#attrs)*
        #vis #sig {
            #block
        }
    };

    // convert the output from a `proc_macro2::TokenStream` to a `proc_macro::TokenStream`
    TokenStream::from(output)
}
