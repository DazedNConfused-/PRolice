#[cfg(test)]
mod macro_tests {

    /// awaits an async function, for easier usage in sync tests. Requires the `tokio_test` dependency.
    macro_rules! aw {
        ($e:expr) => {
            tokio_test::block_on($e)
        };
    }

    #[test]
    fn prolice_trace_time_does_not_modify_function_results_in_sync_functions() {
        use prpolice_lib::prolice_trace_time;

        #[prolice_trace_time]
        fn traced_function(string: &str) -> usize {
            string.len()
        }

        fn non_traced_function(string: &str) -> usize {
            string.len()
        }

        let dummy_input = "this is a dummy input";
        assert_eq!(traced_function(dummy_input), non_traced_function(dummy_input))
    }

    #[test]
    fn prolice_trace_time_does_not_modify_function_results_in_async_functions() {
        use prpolice_lib::prolice_trace_time;

        #[prolice_trace_time]
        async fn traced_function(string: &str) -> usize {
            string.len()
        }

        async fn non_traced_function(string: &str) -> usize {
            string.len()
        }

        let dummy_input = "this is a dummy input";

        assert_eq!(aw!(traced_function(dummy_input)), aw!(non_traced_function(dummy_input)));
    }
}
