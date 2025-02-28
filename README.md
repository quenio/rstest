[![Crate][crate-image]][crate-link]
[![Docs][docs-image]][docs-link]
[![Status][test-action-image]][test-action-link]
[![Apache 2.0 Licensed][license-apache-image]][license-apache-link]
[![MIT Licensed][license-mit-image]][license-mit-link]

# Fixture-based test framework for Rust

## Introduction

`rstest` uses procedural macros to help you on writing
fixtures and table-based tests. To use it, add the
following lines to your `Cargo.toml` file:

```
[dev-dependencies]
rstest = "0.9.0"
```

### Fixture

The core idea is that you can inject your test dependencies
by passing them as test arguments. In the following example,
a `fixture` is defined and then used in two tests,
simply providing it as an argument:

```rust
use rstest::*;

#[fixture]
pub fn fixture() -> u32 { 42 }

#[rstest]
fn should_success(fixture: u32) {
    assert_eq!(fixture, 42);
}

#[rstest]
fn should_fail(fixture: u32) {
    assert_ne!(fixture, 42);
}
```

### Parametrize

You can also inject values in some other ways. For instance, you can
create a set of tests by simply providing the injected values for each
case: `rstest` will generate an independent test for each case.

```rust
use rstest::rstest;

#[rstest]
#[case(0, 0)]
#[case(1, 1)]
#[case(2, 1)]
#[case(3, 2)]
#[case(4, 3)]
fn fibonacci_test(#[case] input: u32, #[case] expected: u32) {
    assert_eq!(expected, fibonacci(input))
}
```

Running `cargo test` in this case executes five tests:

```bash
running 5 tests
test fibonacci_test::case_1 ... ok
test fibonacci_test::case_2 ... ok
test fibonacci_test::case_3 ... ok
test fibonacci_test::case_4 ... ok
test fibonacci_test::case_5 ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

If you need to just providing a bunch of values for which you
need to run your test, you can use `#[values(list, of, values)]`
argument attribute:

```rust
use rstest::rstest;

#[rstest]
fn should_be_invalid(
    #[values(None, Some(""), Some("    "))]
    value: Option<&str>
) {
    assert!(!valid(value))
}
```

Or create a _matrix_ test by using _list of values_ for some
variables that will generate the cartesian product of all the
values.

#### Use Parametrize definition in more tests

If you need to use a test list for more than one test you can use [`rstest_reuse`][reuse-crate-link] 
crate. With this helper crate you can define a template and use it everywhere .

```rust
use rstest::rstest;
use rstest_reuse::{self, *};

#[template]
#[rstest]
#[case(2, 2)]
#[case(4/2, 2)]
fn two_simple_cases(#[case] a: u32, #[case] b: u32) {}

#[apply(two_simple_cases)]
fn it_works(a: u32, b: u32) {
    assert!(a == b);
}
```

See [`rstest_reuse`][reuse-crate-link] for more dettails.

### Magic Conversion

If you need a value where its type implement `FromStr()` trait you can use a literal 
string to build it:

```rust
# use rstest::rstest;
# use std::net::SocketAddr;
#[rstest]
#[case("1.2.3.4:8080", 8080)]
#[case("127.0.0.1:9000", 9000)]
fn check_port(#[case] addr: SocketAddr, #[case] expected: u16) {
    assert_eq!(expected, addr.port());
}
```
You can use this feature also in value list and in fixture default value.

### Async

`rstest` provides out of the box `async` support. Just mark your
test function as `async` and it'll use `#[async-std::test]` to
annotate it. This feature can be really useful to build async
parametric tests using a tidy syntax:

```rust
use rstest::*;

#[rstest]
#[case(5, 2, 3)]
#[should_panic]
#[case(42, 40, 1)]
async fn my_async_test(#[case] expected: u32, #[case] a: u32, #[case] b: u32) {
    assert_eq!(expected, async_sum(a, b).await);
}
```
Currently only `async-std` is supported out of the box. But if you need to use
another runtime that provide it's own test attribute (i.e. `tokio::test` or
`actix_rt::test`) you can use it in your `async` test like described in
[Inject Test Attribute](#inject-test-attribute).

To use this feature, you need to enable `attributes` in the `async-std`
features list in your `Cargo.toml`:

```toml
async-std = { version = "1.5", features = ["attributes"] }
```

### Inject Test Attribute

If you would like to use another `test` attribute for your test you can simply 
indicate it in your test function's attributes. For instance if you want
to test some async function with use `actix_rt::test` attribute you can just write:

```rust
use rstest::*;
use actix_rt;
use std::future::Future;

#[rstest]
#[case(2, async { 4 })]
#[case(21, async { 42 })]
#[actix_rt::test]
async fn my_async_test(#[case] a: u32, result: #[case] impl Future<Output=u32>) {
    assert_eq!(2 * a, result.await);
}
```
Just the attributes that ends with `test` (last path segment) can be injected.

## Complete Example

All these features can be used together with a mixture of fixture variables,
fixed cases and bunch of values. For instance, you might need two
test cases which test for panics, one for a logged in user and one for a guest user.

```rust
use rstest::*;

#[fixture]
fn repository() -> InMemoryRepository {
    let mut r = InMemoryRepository::default();
    // fill repository with some data
    r
}

#[fixture]
fn alice() -> User {
    User::logged("Alice", "2001-10-04", "London", "UK")
}

#[rstest]
#[case::authed_user(alice())] // We can use `fixture` also as standard function
#[case::guest(User::Guest)]   // We can give a name to every case : `guest` in this case
                              // and `authed_user`
#[should_panic(expected = "Invalid query error")] // We whould test a panic
fn should_be_invalid_query_error(
    repository: impl Repository, 
    #[case] user: User, 
    #[values("     ", "^%$#@!", "....")]
    query: &str
) {
    repository.find_items(&user, query).unwrap();
}
```

This example will generate exactly 6 tests grouped by 2 different cases:

```
running 6 tests
test should_be_invalid_query_error::case_1_authed_user::query_1 ... ok
test should_be_invalid_query_error::case_2_guest::query_2 ... ok
test should_be_invalid_query_error::case_2_guest::query_3 ... ok
test should_be_invalid_query_error::case_1_authed_user::query_2 ... ok
test should_be_invalid_query_error::case_1_authed_user::query_3 ... ok
test should_be_invalid_query_error::case_2_guest::query_1 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## More

Is that all? Not quite yet!

A fixture can be injected by another fixture and they can be called
using just some of its arguments.

```rust
#[fixture]
fn user(#[default("Alice")] name: &str, #[default(22)] age: u8) -> User {
    User::new(name, age)
}

#[rstest]
fn is_alice(user: User) {
    assert_eq!(user.name(), "Alice")
}

#[rstest]
fn is_22(user: User) {
    assert_eq!(user.age(), 22)
}

#[rstest]
fn is_bob(#[with("Bob")] user: User) {
    assert_eq!(user.name(), "Bob")
}

#[rstest]
fn is_42(#[with("", 42)] user: User) {
    assert_eq!(user.age(), 42)
}
```

As you noted you can provide default values without the need of a fixture
to define it.

Finally if you need tracing the input values you can just
add the `trace` attribute to your test to enable the dump of all input
variables.

```rust
#[rstest]
#[case(42, "FortyTwo", ("minus twelve", -12))]
#[case(24, "TwentyFour", ("minus twentyfour", -24))]
#[trace] //This attribute enable traceing
fn should_fail(#[case] number: u32, #[case] name: &str, #[case] tuple: (&str, i32)) {
    assert!(false); // <- stdout come out just for failed tests
}
```

```
running 2 tests
test should_fail::case_1 ... FAILED
test should_fail::case_2 ... FAILED

failures:

---- should_fail::case_1 stdout ----
------------ TEST ARGUMENTS ------------
number = 42
name = "FortyTwo"
tuple = ("minus twelve", -12)
-------------- TEST START --------------
thread 'should_fail::case_1' panicked at 'assertion failed: false', src/main.rs:64:5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace.

---- should_fail::case_2 stdout ----
------------ TEST ARGUMENTS ------------
number = 24
name = "TwentyFour"
tuple = ("minus twentyfour", -24)
-------------- TEST START --------------
thread 'should_fail::case_2' panicked at 'assertion failed: false', src/main.rs:64:5


failures:
    should_fail::case_1
    should_fail::case_2

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

In case one or more variables don't implement the `Debug` trait, an error
is raised, but it's also possible to exclude a variable using the
`#[notrace]` argument attribute.

You can learn more on [Docs][docs-link] and find more examples in 
[`tests/resources`](tests/resources) directory.

## Changelog

See [CHANGELOG.md](CHANGELOG.md)

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
[license-apache-link])

* MIT license [LICENSE-MIT](LICENSE-MIT) or [license-MIT-link]
at your option.

[//]: # (links)

[crate-image]: https://img.shields.io/crates/v/rstest.svg
[crate-link]: https://crates.io/crates/rstest
[docs-image]: https://docs.rs/rstest/badge.svg
[docs-link]: https://docs.rs/rstest/
[test-action-image]: https://github.com/la10736/rstest/workflows/Test/badge.svg
[test-action-link]: https://github.com/la10736/rstest/actions?query=workflow:Test
[license-apache-image]: https://img.shields.io/badge/license-Apache2.0-blue.svg
[license-mit-image]: https://img.shields.io/badge/license-MIT-blue.svg
[license-apache-link]: http://www.apache.org/licenses/LICENSE-2.0
[license-MIT-link]: http://opensource.org/licenses/MIT
[reuse-crate-link]: https://crates.io/crates/rstest_reuse
